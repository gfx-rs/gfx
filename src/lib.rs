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
#![feature(slice_patterns)]

#[macro_use]
extern crate log;
extern crate libc;
extern crate gfx_gl as gl;
extern crate gfx;

use gfx::{Device, Factory, Resources};
use gfx::device as d;
use gfx::device::attrib::*;
use gfx::device::draw::{Access, Target};
use gfx::device::handle;
use gfx::device::state::{CullFace, RasterMethod, FrontFace};

pub use self::draw::{Command, CommandBuffer};
pub use self::info::{Info, PlatformName, Version};

mod draw;
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

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum GlResources {}

impl Resources for GlResources {
    type Buffer         = Buffer;
    type ArrayBuffer    = ArrayBuffer;
    type Shader         = Shader;
    type Program        = Program;
    type FrameBuffer    = FrameBuffer;
    type Surface        = Surface;
    type Texture        = Texture;
    type Sampler        = Sampler;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GlError {
    NoError,
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    InvalidFramebufferOperation,
    OutOfMemory,
    UnknownError,
}

impl GlError {
    pub fn from_error_code(error_code: gl::types::GLenum) -> GlError {
        match error_code {
            gl::NO_ERROR                      => GlError::NoError,
            gl::INVALID_ENUM                  => GlError::InvalidEnum,
            gl::INVALID_VALUE                 => GlError::InvalidValue,
            gl::INVALID_OPERATION             => GlError::InvalidOperation,
            gl::INVALID_FRAMEBUFFER_OPERATION => GlError::InvalidFramebufferOperation,
            gl::OUT_OF_MEMORY                 => GlError::OutOfMemory,
            _                                 => GlError::UnknownError,
        }
    }
}

const RESET_CB: [Command; 11] = [
    Command::BindProgram(0),
    Command::BindArrayBuffer(0),
    // BindAttribute
    Command::BindIndex(0),
    Command::BindFrameBuffer(Access::Draw, 0),
    Command::BindFrameBuffer(Access::Read, 0),
    // UnbindTarget
    // BindUniformBlock
    // BindUniform
    // BindTexture
    Command::SetPrimitiveState(d::state::Primitive {
        front_face: FrontFace::CounterClockwise,
        method: RasterMethod::Fill(CullFace::Back),
        offset: None,
    }),
    Command::SetViewport(d::target::Rect{x: 0, y: 0, w: 0, h: 0}),
    Command::SetScissor(None),
    Command::SetDepthStencilState(None, None, CullFace::Nothing),
    Command::SetBlendState(None),
    Command::SetColorMask(d::state::MASK_ALL),
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

/// An OpenGL device with GLSL shaders
pub struct GlDevice {
    info: Info,
    caps: d::Capabilities,
    gl: gl::Gl,
    main_fbo: handle::FrameBuffer<GlResources>,
    frame_handles: handle::Manager<GlResources>,
    handles: handle::Manager<GlResources>,
    max_resource_count: Option<usize>,
}

impl GlDevice {
    /// Load OpenGL symbols and detect driver information
    pub fn new<F>(fn_proc: F) -> GlDevice where F: FnMut(&str) -> *const ::libc::c_void {
        let gl = gl::Gl::load_with(fn_proc);

        let (info, caps) = info::get(&gl);

        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }

        let mut handles = handle::Manager::new();

        GlDevice {
            info: info,
            caps: caps,
            gl: gl,
            main_fbo: factory::make_default_frame_buffer(&mut handles),
            frame_handles: handle::Manager::new(),
            handles: handles,
            max_resource_count: Some(999999),
        }
    }

    /// Access the OpenGL directly via a closure. OpenGL types and enumerations
    /// can be found in the `gl` crate.
    pub unsafe fn with_gl<F>(&mut self, mut fun: F) where F: FnMut(&gl::Gl) {
        self.reset_state();
        fun(&self.gl);
    }

    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&mut self, cmd: &Command) {
        if cfg!(not(ndebug)) {
            let err = GlError::from_error_code(unsafe { self.gl.GetError() });
            if err != GlError::NoError {
                panic!("Error after executing command {:?}: {:?}", cmd, err);
            }
        }
    }

    /// Get the OpenGL-specific driver information
    pub fn get_info<'a>(&'a self) -> &'a Info {
        &self.info
    }

    fn create_buffer_internal(&mut self) -> Buffer {
        let mut name = 0 as Buffer;
        unsafe {
            self.gl.GenBuffers(1, &mut name);
        }
        info!("\tCreated buffer {}", name);
        name
    }

    fn init_buffer(&mut self, buffer: Buffer, info: &d::BufferInfo) {
        let target = match info.role {
            gfx::BufferRole::Vertex => gl::ARRAY_BUFFER,
            gfx::BufferRole::Index  => gl::ELEMENT_ARRAY_BUFFER,
        };
        unsafe { self.gl.BindBuffer(target, buffer) };
        let usage = match info.usage {
            gfx::BufferUsage::Static  => gl::STATIC_DRAW,
            gfx::BufferUsage::Dynamic => gl::DYNAMIC_DRAW,
            gfx::BufferUsage::Stream  => gl::STREAM_DRAW,
        };
        unsafe {
            self.gl.BufferData(target,
                info.size as gl::types::GLsizeiptr,
                0 as *const gl::types::GLvoid,
                usage
            );
        }
    }

    fn update_sub_buffer(&mut self, buffer: Buffer, address: *const u8,
                         size: usize, offset: usize, role: gfx::BufferRole) {
        let target = match role {
            gfx::BufferRole::Vertex => gl::ARRAY_BUFFER,
            gfx::BufferRole::Index  => gl::ELEMENT_ARRAY_BUFFER,
        };
        unsafe { self.gl.BindBuffer(target, buffer) };
        unsafe {
            self.gl.BufferSubData(target,
                offset as gl::types::GLintptr,
                size as gl::types::GLsizeiptr,
                address as *const gl::types::GLvoid
            );
        }
    }

    fn process(&mut self, cmd: &Command, data_buf: &d::draw::DataBuffer) {
        match *cmd {
            Command::Clear(ref data, mask) => {
                let mut flags = 0;
                if mask.intersects(d::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                    state::bind_color_mask(&self.gl, d::state::MASK_ALL);
                    let [r, g, b, a] = data.color;
                    unsafe { self.gl.ClearColor(r, g, b, a) };
                }
                if mask.intersects(d::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                    unsafe {
                        self.gl.DepthMask(gl::TRUE);
                        self.gl.ClearDepth(data.depth as gl::types::GLclampd);
                    }
                }
                if mask.intersects(d::target::STENCIL) {
                    flags |= gl::STENCIL_BUFFER_BIT;
                    unsafe {
                        self.gl.StencilMask(-1);
                        self.gl.ClearStencil(data.stencil as gl::types::GLint);
                    }
                }
                unsafe { self.gl.Clear(flags) };
            },
            Command::BindProgram(program) => {
                unsafe { self.gl.UseProgram(program) };
            },
            Command::BindArrayBuffer(array_buffer) => {
                if self.caps.array_buffer_supported {
                    unsafe { self.gl.BindVertexArray(array_buffer) };
                } else {
                    error!("Ignored VAO bind command: {}", array_buffer)
                }
            },
            Command::BindAttribute(slot, buffer, format) => {
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
                unsafe { self.gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
                let offset = format.offset as *const gl::types::GLvoid;
                match format.elem_type {
                    Type::Int(IntSubType::Raw, _, _) => unsafe {
                        self.gl.VertexAttribIPointer(slot as gl::types::GLuint,
                            format.elem_count as gl::types::GLint, gl_type,
                            format.stride as gl::types::GLint, offset);
                    },
                    Type::Int(IntSubType::Normalized, _, _) => unsafe {
                        self.gl.VertexAttribPointer(slot as gl::types::GLuint,
                            format.elem_count as gl::types::GLint, gl_type, gl::TRUE,
                            format.stride as gl::types::GLint, offset);
                    },
                    Type::Int(IntSubType::AsFloat, _, _) => unsafe {
                        self.gl.VertexAttribPointer(slot as gl::types::GLuint,
                            format.elem_count as gl::types::GLint, gl_type, gl::FALSE,
                            format.stride as gl::types::GLint, offset);
                    },
                    Type::Float(FloatSubType::Default, _) => unsafe {
                        self.gl.VertexAttribPointer(slot as gl::types::GLuint,
                            format.elem_count as gl::types::GLint, gl_type, gl::FALSE,
                            format.stride as gl::types::GLint, offset);
                    },
                    Type::Float(FloatSubType::Precision, _) => unsafe {
                        self.gl.VertexAttribLPointer(slot as gl::types::GLuint,
                            format.elem_count as gl::types::GLint, gl_type,
                            format.stride as gl::types::GLint, offset);
                    },
                    _ => ()
                }
                unsafe { self.gl.EnableVertexAttribArray(slot as gl::types::GLuint) };
                if self.caps.instance_rate_supported {
                    unsafe { self.gl.VertexAttribDivisor(slot as gl::types::GLuint,
                        format.instance_rate as gl::types::GLuint) };
                }else if format.instance_rate != 0 {
                    error!("Instanced arrays are not supported");
                }
            },
            Command::BindIndex(buffer) => {
                unsafe { self.gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer) };
            },
            Command::BindFrameBuffer(access, frame_buffer) => {
                if !self.caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                unsafe { self.gl.BindFramebuffer(point, frame_buffer) };
            },
            Command::UnbindTarget(access, target) => {
                if !self.caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                unsafe { self.gl.FramebufferRenderbuffer(point, att, gl::RENDERBUFFER, 0) };
            },
            Command::BindTargetSurface(access, target, name) => {
                if !self.caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                unsafe { self.gl.FramebufferRenderbuffer(point, att, gl::RENDERBUFFER, name) };
            },
            Command::BindTargetTexture(access, target, name, level, layer) => {
                if !self.caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                let att = target_to_gl(target);
                match layer {
                    Some(layer) => unsafe { self.gl.FramebufferTextureLayer(
                        point, att, name, level as gl::types::GLint,
                        layer as gl::types::GLint) },
                    None => unsafe { self.gl.FramebufferTexture(
                        point, att, name, level as gl::types::GLint) },
                }
            },
            Command::BindUniformBlock(program, slot, loc, buffer) => { unsafe {
                self.gl.UniformBlockBinding(program, slot as gl::types::GLuint, loc as gl::types::GLuint);
                self.gl.BindBufferBase(gl::UNIFORM_BUFFER, loc as gl::types::GLuint, buffer);
            }},
            Command::BindUniform(loc, uniform) => {
                shade::bind_uniform(&self.gl, loc as gl::types::GLint, uniform);
            },
            Command::BindTexture(slot, kind, texture, sampler) => {
                let anchor = tex::bind_texture(&self.gl,
                    gl::TEXTURE0 + slot as gl::types::GLenum,
                    kind, texture);
                match (anchor, kind.get_aa_mode(), sampler) {
                    (anchor, None, Some((name, info))) => {
                        if self.caps.sampler_objects_supported {
                            unsafe { self.gl.BindSampler(slot as gl::types::GLenum, name) };
                        } else {
                            debug_assert_eq!(name, 0);
                            tex::bind_sampler(&self.gl, anchor, &info);
                        }
                    },
                    (_, Some(_), Some(_)) =>
                        error!("Unable to bind a multi-sampled texture with a sampler"),
                    (_, _, _) => (),
                }
            },
            Command::SetDrawColorBuffers(num) => {
                state::bind_draw_color_buffers(&self.gl, num);
            },
            Command::SetPrimitiveState(prim) => {
                state::bind_primitive(&self.gl, prim);
            },
            Command::SetViewport(rect) => {
                state::bind_viewport(&self.gl, rect);
            },
            Command::SetMultiSampleState(ms) => {
                state::bind_multi_sample(&self.gl, ms);
            },
            Command::SetScissor(rect) => {
                state::bind_scissor(&self.gl, rect);
            },
            Command::SetDepthStencilState(depth, stencil, cull) => {
                state::bind_stencil(&self.gl, stencil, cull);
                state::bind_depth(&self.gl, depth);
            },
            Command::SetBlendState(blend) => {
                state::bind_blend(&self.gl, blend);
            },
            Command::SetColorMask(mask) => {
                state::bind_color_mask(&self.gl, mask);
            },
            Command::UpdateBuffer(buffer, pointer, offset) => {
                let data = data_buf.get_ref(pointer);
                self.update_sub_buffer(buffer, data.as_ptr(), data.len(), offset,
                    gfx::BufferRole::Vertex);
            },
            Command::UpdateTexture(kind, texture, image_info, pointer) => {
                let data = data_buf.get_ref(pointer);
                match tex::update_texture(&self.gl, kind, texture, &image_info,
                                          data.as_ptr(), data.len()) {
                    Ok(_) => (),
                    Err(_) => unimplemented!(),
                }
            },
            Command::Draw(prim_type, start, count, instances) => {
                match instances {
                    Some((num, base)) if self.caps.instance_call_supported => { unsafe {
                        self.gl.DrawArraysInstancedBaseInstance(
                            primitive_to_gl(prim_type),
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei,
                            num as gl::types::GLsizei,
                            base as gl::types::GLuint,
                        );
                    }},
                    Some(_) => {
                        error!("Instanced draw calls are not supported");
                    },
                    None => { unsafe {
                        self.gl.DrawArrays(
                            primitive_to_gl(prim_type),
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei
                        );
                    }},
                }
            },
            Command::DrawIndexed(prim_type, index_type, start, count, base_vertex, instances) => {
                let (offset, gl_index) = match index_type {
                    IntSize::U8  => (start * 1u32, gl::UNSIGNED_BYTE),
                    IntSize::U16 => (start * 2u32, gl::UNSIGNED_SHORT),
                    IntSize::U32 => (start * 4u32, gl::UNSIGNED_INT),
                };
                match instances {
                    Some((num, base_instance)) if self.caps.instance_call_supported => unsafe {
                        if (base_vertex == 0 && base_instance == 0) || !self.caps.vertex_base_supported {
                            if base_vertex != 0 || base_instance != 0 {
                                error!("Instance bases with indexed drawing is not supported")
                            }
                            self.gl.DrawElementsInstanced(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                            );
                        } else if base_vertex != 0 && base_instance == 0 {
                            self.gl.DrawElementsInstancedBaseVertex(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_vertex as gl::types::GLint,
                            );
                        } else if base_vertex == 0 && base_instance != 0 {
                            self.gl.DrawElementsInstancedBaseInstance(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_instance as gl::types::GLuint,
                            );
                        } else {
                            self.gl.DrawElementsInstancedBaseVertexBaseInstance(
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
                        if base_vertex == 0 || !self.caps.vertex_base_supported {
                            if base_vertex != 0 {
                                error!("Base vertex with indexed drawing not supported");
                            }
                            self.gl.DrawElements(
                                primitive_to_gl(prim_type),
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                            );
                        } else {
                            self.gl.DrawElementsBaseVertex(
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
                unsafe { self.gl.BlitFramebuffer(
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

impl Device for GlDevice {
    type Resources = GlResources;
    type CommandBuffer  = CommandBuffer;

    fn get_capabilities<'a>(&'a self) -> &'a d::Capabilities {
        &self.caps
    }

    fn reset_state(&mut self) {
        let data = d::draw::DataBuffer::new();
        for com in RESET_CB.iter() {
            self.process(com, &data);
        }
    }

    fn submit(&mut self, (cb, db, handles): d::SubmitInfo<GlDevice>) {
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

    fn after_frame(&mut self) {
        self.handles.extend(&self.frame_handles);
        self.frame_handles.clear();
        self.cleanup();
    }
}
