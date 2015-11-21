// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! OpenGL implementation of a device, striving to support OpenGL 2.0 with at
//! least VAOs, but using newer extensions when available.

#![allow(missing_docs)]
#![deny(missing_copy_implementations)]

#[macro_use]
extern crate log;
extern crate gfx_gl as gl;
extern crate gfx;

use std::cell::RefCell;
use std::rc::Rc;
use gfx::device as d;
use gfx::device::attrib::*;
use gfx::device::draw::{Access, Gamma, Target};
use gfx::device::handle;
use gfx::state as s;

pub use gfx::device::command::{Command, CommandBuffer};
pub use self::factory::{Factory, Output};
pub use self::info::{Info, PlatformName, Version};

mod factory;
mod shade;
mod state;
mod tex;
mod info;


pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Sampler        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PipelineDrawState {
    /// How to rasterize geometric primitives.
    pub primitive: s::Primitive,
    /// Multi-sampling mode
    pub multi_sample: Option<s::MultiSample>,
    /// Stencil test to use. If None, no stencil testing is done.
    pub stencil: Option<(s::StencilSide, s::StencilSide)>,
    /// Depth test to use. If None, no depth testing is done.
    pub depth: Option<s::Depth>,
    /// Blend function to use. If None, no blending is done.
    pub blend: [Option<s::Blend>; d::MAX_COLOR_TARGETS],
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PipelineState {
    topology: d::PrimitiveType,
    program: Program,
    vertex_import: d::pso::VertexImportLayout,
    draw_target_mask: usize,
    state: PipelineDrawState,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TargetView {
    Surface(Surface),
    Texture(Texture, gfx::Level),
    TextureLayer(Texture, gfx::Level, gfx::Layer),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Fence(gl::types::GLsync);

unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl gfx::Resources for Resources {
    type Buffer              = Buffer;
    type ArrayBuffer         = ArrayBuffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = PipelineState;
    type FrameBuffer         = FrameBuffer;
    type Surface             = Surface;
    type RenderTargetView    = TargetView;
    type DepthStencilView    = TargetView;
    type ShaderResourceView  = Texture; //TODO
    type UnorderedAccessView = Texture; //TODO
    type Texture             = Texture;
    type Sampler             = Sampler;
    type Fence               = Fence;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NoError,
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    InvalidFramebufferOperation,
    OutOfMemory,
    UnknownError,
}

impl Error {
    pub fn from_error_code(error_code: gl::types::GLenum) -> Error {
        match error_code {
            gl::NO_ERROR                      => Error::NoError,
            gl::INVALID_ENUM                  => Error::InvalidEnum,
            gl::INVALID_VALUE                 => Error::InvalidValue,
            gl::INVALID_OPERATION             => Error::InvalidOperation,
            gl::INVALID_FRAMEBUFFER_OPERATION => Error::InvalidFramebufferOperation,
            gl::OUT_OF_MEMORY                 => Error::OutOfMemory,
            _                                 => Error::UnknownError,
        }
    }
}

const RESET_CB: [Command<Resources>; 14] = [
    Command::BindProgram(0),
    Command::BindArrayBuffer(0),
    // BindAttribute
    Command::BindIndex(0),
    Command::BindFrameBuffer(Access::Draw, 0, Gamma::Original),
    Command::BindFrameBuffer(Access::Read, 0, Gamma::Original),
    // UnbindTarget
    // BindUniformBlock
    // BindUniform
    // BindTexture
    Command::SetPrimitiveState(d::state::Primitive {
        front_face: s::FrontFace::CounterClockwise,
        method: s::RasterMethod::Fill(s::CullFace::Back),
        offset: None,
    }),
    Command::SetViewport(d::target::Rect{x: 0, y: 0, w: 0, h: 0}),
    Command::SetScissor(None),
    Command::SetDepthStencilState(None, None, s::CullFace::Nothing),
    Command::SetBlendState(0, None),
    Command::SetBlendState(1, None),
    Command::SetBlendState(2, None),
    Command::SetBlendState(3, None),
    Command::SetRefValues([0f32; 4], 0, 0),
];

fn primitive_to_gl(prim_type: d::PrimitiveType) -> gl::types::GLenum {
    match prim_type {
        d::PrimitiveType::Point => gl::POINTS,
        d::PrimitiveType::Line => gl::LINES,
        d::PrimitiveType::LineStrip => gl::LINE_STRIP,
        d::PrimitiveType::TriangleList => gl::TRIANGLES,
        d::PrimitiveType::TriangleStrip => gl::TRIANGLE_STRIP,
        d::PrimitiveType::TriangleFan => gl::TRIANGLE_FAN,
    }
}

fn access_to_gl(access: Access) -> gl::types::GLenum {
    match access {
        Access::Draw => gl::DRAW_FRAMEBUFFER,
        Access::Read => gl::READ_FRAMEBUFFER,
    }
}

fn target_to_gl(target: Target) -> gl::types::GLenum {
    match target {
        Target::Color(index) => gl::COLOR_ATTACHMENT0 + (index as gl::types::GLenum),
        Target::Depth => gl::DEPTH_ATTACHMENT,
        Target::Stencil => gl::STENCIL_ATTACHMENT,
        Target::DepthStencil => gl::DEPTH_STENCIL_ATTACHMENT,
    }
}

/// Create a new device with a factory.
pub fn create<F>(fn_proc: F) -> (Device, Factory) where
    F: FnMut(&str) -> *const std::os::raw::c_void
{
    let device = Device::new(fn_proc);
    let factory = Factory::new(device.share.clone());
    (device, factory)
}


/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    context: gl::Gl,
    capabilities: d::Capabilities,
    handles: RefCell<handle::Manager<Resources>>,
    main_fbo: handle::FrameBuffer<Resources>,
}

/// Temporary data stored between different gfx calls that
/// can not be separated on the GL backend.
struct Temp {
    primitive_type: gl::types::GLenum,
    vertex_import: d::pso::VertexImportLayout,
    stencil: Option<s::Stencil>,
    cull_face: s::CullFace,
}

impl Temp {
    fn new() -> Temp {
        Temp {
            primitive_type: 0,
            vertex_import: d::pso::VertexImportLayout::new(),
            stencil: None,
            cull_face: s::CullFace::Nothing,
        }
    }
}

/// An OpenGL device with GLSL shaders.
pub struct Device {
    info: Info,
    share: Rc<Share>,
    temp: Temp,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

impl Device {
    /// Create a new device. There can be only one!
    /// Also, load OpenGL symbols and detect driver information.
    fn new<F>(fn_proc: F) -> Device where
        F: FnMut(&str) -> *const std::os::raw::c_void
    {
        use gfx::device::handle::Producer;
        let gl = gl::Gl::load_with(fn_proc);
        // query information
        let (info, caps) = info::get(&gl);
        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }
        // initialize permanent states
        unsafe {
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        }
        // create the device
        let mut handles = handle::Manager::new();
        let main_fbo = handles.make_frame_buffer(0);
        Device {
            info: info,
            share: Rc::new(Share {
                context: gl,
                capabilities: caps,
                handles: RefCell::new(handles),
                main_fbo: main_fbo,
            }),
            temp: Temp::new(),
            frame_handles: handle::Manager::new(),
            max_resource_count: Some(999999),
        }
    }

    /// Access the OpenGL directly via a closure. OpenGL types and enumerations
    /// can be found in the `gl` crate.
    pub unsafe fn with_gl<F: FnMut(&gl::Gl)>(&mut self, mut fun: F) {
        use gfx::Device;
        self.reset_state();
        fun(&self.share.context);
    }

    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&mut self, cmd: &Command<Resources>) {
        if cfg!(not(ndebug)) {
            let gl = &self.share.context;
            let err = Error::from_error_code(unsafe { gl.GetError() });
            if err != Error::NoError {
                panic!("Error after executing command {:?}: {:?}", cmd, err);
            }
        }
    }

    /// Get the OpenGL-specific driver information
    pub fn get_info<'a>(&'a self) -> &'a Info {
        &self.info
    }

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buffer: Buffer,
                      format: d::attrib::Format) {
        let gl_type = match format.elem_type {
            Type::Int(_, IntSize::U8, SignFlag::Unsigned)  => gl::UNSIGNED_BYTE,
            Type::Int(_, IntSize::U8, SignFlag::Signed)    => gl::BYTE,
            Type::Int(_, IntSize::U16, SignFlag::Unsigned) => gl::UNSIGNED_SHORT,
            Type::Int(_, IntSize::U16, SignFlag::Signed)   => gl::SHORT,
            Type::Int(_, IntSize::U32, SignFlag::Unsigned) => gl::UNSIGNED_INT,
            Type::Int(_, IntSize::U32, SignFlag::Signed)   => gl::INT,
            Type::Float(_, FloatSize::F16) => gl::HALF_FLOAT,
            Type::Float(_, FloatSize::F32) => gl::FLOAT,
            Type::Float(_, FloatSize::F64) => gl::DOUBLE,
            _ => {
                error!("Unsupported element type: {:?}", format.elem_type);
                return
            }
        };
        let gl = &self.share.context;
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        let offset = format.offset as *const gl::types::GLvoid;
        match format.elem_type {
            Type::Int(IntSubType::Raw, _, _) => unsafe {
                gl.VertexAttribIPointer(slot as gl::types::GLuint,
                    format.elem_count as gl::types::GLint, gl_type,
                    format.stride as gl::types::GLint, offset);
            },
            Type::Int(IntSubType::Normalized, _, _) => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    format.elem_count as gl::types::GLint, gl_type, gl::TRUE,
                    format.stride as gl::types::GLint, offset);
            },
            Type::Int(IntSubType::AsFloat, _, _) => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    format.elem_count as gl::types::GLint, gl_type, gl::FALSE,
                    format.stride as gl::types::GLint, offset);
            },
            Type::Float(FloatSubType::Default, _) => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    format.elem_count as gl::types::GLint, gl_type, gl::FALSE,
                    format.stride as gl::types::GLint, offset);
            },
            Type::Float(FloatSubType::Precision, _) => unsafe {
                gl.VertexAttribLPointer(slot as gl::types::GLuint,
                    format.elem_count as gl::types::GLint, gl_type,
                    format.stride as gl::types::GLint, offset);
            },
            _ => ()
        }
        unsafe { gl.EnableVertexAttribArray(slot as gl::types::GLuint) };
        if self.share.capabilities.instance_rate_supported {
            unsafe { gl.VertexAttribDivisor(slot as gl::types::GLuint,
                format.instance_rate as gl::types::GLuint) };
        }else if format.instance_rate != 0 {
            error!("Instanced arrays are not supported");
        }
    }

    fn process(&mut self, cmd: &Command<Resources>,
               data_buf: &d::draw::DataBuffer) {
        match *cmd {
            Command::Clear(ref data, mask) => {
                let mut flags = 0;
                let gl = &self.share.context;
                if mask.intersects(d::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                    state::unlock_color_mask(gl);
                    let c = data.color;
                    unsafe { gl.ClearColor(c[0], c[1], c[2], c[3]) };
                }
                if mask.intersects(d::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                    unsafe {
                        gl.DepthMask(gl::TRUE);
                        gl.ClearDepth(data.depth as gl::types::GLclampd);
                    }
                }
                if mask.intersects(d::target::STENCIL) {
                    flags |= gl::STENCIL_BUFFER_BIT;
                    unsafe {
                        gl.StencilMask(gl::types::GLuint::max_value());
                        gl.ClearStencil(data.stencil as gl::types::GLint);
                    }
                }
                unsafe { gl.Clear(flags) };
            },
            Command::BindProgram(program) => {
                let gl = &self.share.context;
                unsafe { gl.UseProgram(program) };
            },
            Command::BindPipelineState(pso) => {
                let gl = &self.share.context;
                unsafe { gl.UseProgram(pso.program) };
                self.temp.primitive_type = primitive_to_gl(pso.topology);
                self.temp.vertex_import = pso.vertex_import;
                state::bind_draw_color_buffers(gl, pso.draw_target_mask);
                state::bind_primitive(gl, pso.state.primitive);
                state::bind_multi_sample(gl, pso.state.multi_sample);
                self.temp.stencil = pso.state.stencil.map(|s| s::Stencil {
                    front: s.0, back: s.1, front_ref: 0, back_ref: 0,
                    });
                self.temp.cull_face = pso.state.primitive.get_cull_face();
                state::bind_stencil(gl, &self.temp.stencil, self.temp.cull_face);
                state::bind_depth(gl, &pso.state.depth);
                for i in 0 .. d::MAX_COLOR_TARGETS {
                    state::bind_blend_slot(gl, i as d::ColorSlot, pso.state.blend[i]);
                }
            },
            Command::BindVertexBuffers(vbs) => {
                for i in 0 .. d::pso::MAX_VERTEX_ATTRIBUTES {
                    match (vbs.0[i], self.temp.vertex_import.formats[i]) {
                        (None, Some(fm)) => {
                            error!("No vertex input provided for slot {} of format {:?}", i, fm)
                        },
                        (Some((buffer, offset)), Some(mut format)) => {
                            format.offset += offset as gl::types::GLuint;
                            self.bind_attribute(i as d::AttributeSlot, buffer, format);
                        },
                        (_, None) => {},
                    }
                }
            },
            Command::BindArrayBuffer(array_buffer) => {
                let gl = &self.share.context;
                if self.share.capabilities.array_buffer_supported {
                    unsafe { gl.BindVertexArray(array_buffer) };
                } else {
                    error!("Ignored VAO bind command: {}", array_buffer)
                }
            },
            Command::BindAttribute(slot, buffer, format) => {
                self.bind_attribute(slot, buffer, format);
            },
            Command::BindIndex(buffer) => {
                let gl = &self.share.context;
                unsafe { gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer) };
            },
            Command::BindFrameBuffer(access, frame_buffer, gamma) => {
                let caps = &self.share.capabilities;
                let gl = &self.share.context;
                if !caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                unsafe { gl.BindFramebuffer(point, frame_buffer) };
                match (caps.srgb_color_supported, gamma) {
                    (true, Gamma::Original) => unsafe { gl.Disable(gl::FRAMEBUFFER_SRGB) },
                    (true, Gamma::Convert)  => unsafe { gl.Enable( gl::FRAMEBUFFER_SRGB) },
                    (false, _) => (),
                }
            },
            Command::UnbindTarget(access, target) => {
                if !self.share.capabilities.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let gl = &self.share.context;
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                unsafe { gl.FramebufferRenderbuffer(point, att, gl::RENDERBUFFER, 0) };
            },
            Command::BindTargetSurface(access, target, name) => {
                if !self.share.capabilities.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let gl = &self.share.context;
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                unsafe { gl.FramebufferRenderbuffer(point, att, gl::RENDERBUFFER, name) };
            },
            Command::BindTargetTexture(access, target, name, level, layer) => {
                if !self.share.capabilities.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let gl = &self.share.context;
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                match layer {
                    Some(layer) => unsafe { gl.FramebufferTextureLayer(
                        point, att, name, level as gl::types::GLint,
                        layer as gl::types::GLint) },
                    None => unsafe { gl.FramebufferTexture(
                        point, att, name, level as gl::types::GLint) },
                }
            },
            Command::BindUniformBlock(program, slot, loc, buffer) => {
                let gl = &self.share.context;
                unsafe {
                    gl.UniformBlockBinding(program, slot as gl::types::GLuint, loc as gl::types::GLuint);
                    gl.BindBufferBase(gl::UNIFORM_BUFFER, loc as gl::types::GLuint, buffer);
                }
            },
            Command::BindUniform(loc, uniform) => {
                let gl = &self.share.context;
                shade::bind_uniform(gl, loc as gl::types::GLint, uniform);
            },
            Command::BindTexture(slot, kind, texture, sampler) => {
                let gl = &self.share.context;
                let anchor = tex::bind_texture(gl,
                    gl::TEXTURE0 + slot as gl::types::GLenum,
                    kind, texture);
                match (anchor, kind.get_aa_mode(), sampler) {
                    (anchor, None, Some((name, info))) => {
                        if self.share.capabilities.sampler_objects_supported {
                            unsafe { gl.BindSampler(slot as gl::types::GLenum, name) };
                        } else {
                            debug_assert_eq!(name, 0);
                            tex::bind_sampler(gl, anchor, &info);
                        }
                    },
                    (_, Some(_), Some(_)) =>
                        error!("Unable to bind a multi-sampled texture with a sampler"),
                    (_, _, _) => (),
                }
            },
            Command::SetDrawColorBuffers(num) => {
                let mask = (1 << (num as usize)) - 1;
                state::bind_draw_color_buffers(&self.share.context, mask);
            },
            Command::SetPrimitiveState(prim) => {
                state::bind_primitive(&self.share.context, prim);
            },
            Command::SetViewport(rect) => {
                state::bind_viewport(&self.share.context, rect);
            },
            Command::SetMultiSampleState(ms) => {
                state::bind_multi_sample(&self.share.context, ms);
            },
            Command::SetScissor(rect) => {
                state::bind_scissor(&self.share.context, rect);
            },
            Command::SetDepthStencilState(depth, stencil, cull) => {
                let gl = &self.share.context;
                state::bind_stencil(gl, &stencil, cull);
                state::bind_depth(gl, &depth);
            },
            Command::SetBlendState(slot, blend) => {
                if self.share.capabilities.separate_blending_slots_supported {
                    state::bind_blend_slot(&self.share.context, slot, blend);
                }else if slot == 0 {
                    state::bind_blend(&self.share.context, blend);
                }else {
                    error!("Separate blending slots are not supported");
                }
            },
            Command::SetRefValues(blend, stencil_front, stencil_back) => {
                state::set_ref_values(&self.share.context, blend, stencil_front,
                                      stencil_back, &self.temp.stencil,
                                      self.temp.cull_face);
            },
            Command::UpdateBuffer(buffer, pointer, offset) => {
                let data = data_buf.get_ref(pointer);
                factory::update_sub_buffer(&self.share.context, buffer,
                    data.as_ptr(), data.len(), offset, gfx::BufferRole::Vertex);
            },
            Command::UpdateTexture(kind, texture, image_info, pointer) => {
                let data = data_buf.get_ref(pointer);
                match tex::update_texture(&self.share.context, kind, texture,
                        &image_info, data.as_ptr(), data.len()) {
                    Ok(_) => (),
                    Err(e) => {
                        error!("Error updating a texture: {:?}", e);
                    },
                }
            },
            Command::Draw(prim_type, start, count, instances) => {
                let gl = &self.share.context;
                match instances {
                    Some((num, base)) if self.share.capabilities.instance_call_supported => unsafe {
                        gl.DrawArraysInstancedBaseInstance(
                            primitive_to_gl(prim_type),
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei,
                            num as gl::types::GLsizei,
                            base as gl::types::GLuint,
                        );
                    },
                    Some(_) => {
                        error!("Instanced draw calls are not supported");
                    },
                    None => unsafe {
                        gl.DrawArrays(
                            primitive_to_gl(prim_type),
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei
                        );
                    },
                }
            },
            Command::DrawIndexed(prim_type, index_type, start, count, base_vertex, instances) => {
                let gl = &self.share.context;
                let caps = &self.share.capabilities;
                let (offset, gl_index) = match index_type {
                    IntSize::U8  => (start * 1u32, gl::UNSIGNED_BYTE),
                    IntSize::U16 => (start * 2u32, gl::UNSIGNED_SHORT),
                    IntSize::U32 => (start * 4u32, gl::UNSIGNED_INT),
                };
                match instances {
                    Some((num, base_instance)) if caps.instance_call_supported => unsafe {
                        if (base_vertex == 0 && base_instance == 0) || !caps.vertex_base_supported {
                            if base_vertex != 0 || base_instance != 0 {
                                error!("Instance bases with indexed drawing is not supported")
                            }
                            gl.DrawElementsInstanced(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                            );
                        } else if base_vertex != 0 && base_instance == 0 {
                            gl.DrawElementsInstancedBaseVertex(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_vertex as gl::types::GLint,
                            );
                        } else if base_vertex == 0 && base_instance != 0 {
                            gl.DrawElementsInstancedBaseInstance(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_instance as gl::types::GLuint,
                            );
                        } else {
                            gl.DrawElementsInstancedBaseVertexBaseInstance(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_vertex as gl::types::GLint,
                                base_instance as gl::types::GLuint,
                            );
                        }
                    },
                    Some(_) => {
                        error!("Instanced draw calls are not supported");
                    },
                    None => unsafe {
                        if base_vertex == 0 || !caps.vertex_base_supported {
                            if base_vertex != 0 {
                                error!("Base vertex with indexed drawing not supported");
                            }
                            gl.DrawElements(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                            );
                        } else {
                            gl.DrawElementsBaseVertex(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                base_vertex as gl::types::GLint,
                            );
                        }
                    },
                }
            },
            Command::Blit(mut s_rect, d_rect, mirror, mask) => {
                type GLint = gl::types::GLint;
                // mirror
                let mut s_end_x = s_rect.x + s_rect.w;
                let mut s_end_y = s_rect.y + s_rect.h;
                if mirror.intersects(d::target::MIRROR_X) {
                    s_end_x = s_rect.x;
                    s_rect.x += s_rect.w;
                }
                if mirror.intersects(d::target::MIRROR_Y) {
                    s_end_y = s_rect.y;
                    s_rect.y += s_rect.h;
                }
                // build mask
                let mut flags = 0;
                if mask.intersects(d::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                }
                if mask.intersects(d::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                }
                if mask.intersects(d::target::STENCIL) {
                    flags |= gl::STENCIL_BUFFER_BIT;
                }
                // build filter
                let filter = if s_rect.w == d_rect.w && s_rect.h == d_rect.h {
                    gl::NEAREST
                }else {
                    gl::LINEAR
                };
                // blit
                let gl = &self.share.context;
                unsafe { gl.BlitFramebuffer(
                    s_rect.x as GLint,
                    s_rect.y as GLint,
                    s_end_x as GLint,
                    s_end_y as GLint,
                    d_rect.x as GLint,
                    d_rect.y as GLint,
                    (d_rect.x + d_rect.w) as GLint,
                    (d_rect.y + d_rect.h) as GLint,
                    flags,
                    filter
                ) };
            },
        }
        self.check(cmd);
    }
}

impl gfx::Device for Device {
    type Resources = Resources;
    type CommandBuffer = CommandBuffer<Self::Resources>;

    fn get_capabilities<'a>(&'a self) -> &'a d::Capabilities {
        &self.share.capabilities
    }

    fn reset_state(&mut self) {
        let data = d::draw::DataBuffer::new();
        for com in RESET_CB.iter() {
            self.process(com, &data);
        }
    }

    fn submit(&mut self, (cb, db, handles): d::SubmitInfo<Device>) {
        self.frame_handles.extend(handles);
        self.reset_state();
        for com in cb.iter() {
            self.process(com, db);
        }
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::after_frame()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn cleanup(&mut self) {
        use gfx::device::handle::Producer;
        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut &self.share.context,
            |gl, v| unsafe { gl.DeleteBuffers(1, v) },
            |gl, v| unsafe { gl.DeleteVertexArrays(1, v) },
            |gl, v| unsafe { gl.DeleteShader(*v) },
            |gl, v| unsafe { gl.DeleteProgram(*v) },
            |gl, v| unsafe { gl.DeleteProgram(v.program) },
            |gl, v| unsafe { gl.DeleteFramebuffers(1, v) },
            |gl, v| unsafe { gl.DeleteRenderbuffers(1, v) },
            |gl, v| unsafe { gl.DeleteTextures(1, v) },
            |gl, v| unsafe { gl.DeleteSamplers(1, v) },
            |gl, v| unsafe { gl.DeleteSync(v.0) },
        );
    }
}

impl gfx::traits::DeviceFence<Resources> for Device {
    fn fenced_submit(&mut self, info: d::SubmitInfo<Device>, after: Option<handle::Fence<Resources>>) -> handle::Fence<Resources> {
        use gfx::Device;
        use gfx::handle::Producer;

        unsafe {
            if let Some(fence) = after {
                let f = self.frame_handles.ref_fence(&fence);
                self.share.context.WaitSync(f.0, 0, 1_000_000_000_000);
            }
        }

        self.submit(info);

        let fence = unsafe {
            self.share.context.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0)
        };
        self.frame_handles.make_fence(Fence(fence))
    }

    fn fence_wait(&mut self, fence: &handle::Fence<Resources>) {
        let f = self.frame_handles.ref_fence(fence);
        unsafe {
            self.share.context.ClientWaitSync(f.0, gl::SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000_000);
        }
    }
}
