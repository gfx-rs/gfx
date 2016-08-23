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
extern crate gfx_core;

use std::cell::RefCell;
use std::rc::Rc;
use std::{cmp, hash, fmt};
use gfx_core as d;
use gfx_core::{mapping, handle};
use gfx_core::state as s;
use gfx_core::target::{Layer, Level};
use command::{Command, DataBuffer};
use factory::MappingKind;

pub use self::command::CommandBuffer;
pub use self::factory::Factory;
pub use self::info::{Info, PlatformName, Version};

mod command;
mod factory;
mod info;
mod shade;
mod state;
mod tex;


pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;
pub type Sampler        = gl::types::GLuint;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct RawFence(gl::types::GLsync);

impl RawFence {
    fn wait(&self, gl: &gl::Gl) {
        unsafe {
            let timeout = 1_000_000_000_000;
            gl.ClientWaitSync(self.0, gl::SYNC_FLUSH_COMMANDS_BIT, timeout);
        }
    }
}

#[derive(Clone)]
pub struct Fence {
    raw: RawFence,
    share: Rc<Share>,
}

impl cmp::PartialEq for Fence {
    fn eq(&self, other: &Fence) -> bool { self.raw.eq(&other.raw) }
}

impl cmp::Eq for Fence {}

impl hash::Hash for Fence {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.raw.hash(state);
    }
}

impl fmt::Debug for Fence {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.raw.fmt(f)
    }
}

impl d::Fence for Fence {
    fn wait(&self) {
        self.raw.wait(&self.share.context);
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl d::Resources for Resources {
    type Buffer              = Buffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = PipelineState;
    type Texture             = NewTexture;
    type RenderTargetView    = TargetView;
    type DepthStencilView    = TargetView;
    type ShaderResourceView  = ResourceView;
    type UnorderedAccessView = ();
    type Sampler             = FatSampler;
    type Fence               = Fence;
    type Mapping             = factory::BackendMapping;
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BufferElement {
    pub desc: d::pso::VertexBufferDesc,
    pub elem: d::pso::Element<d::format::Format>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct OutputMerger {
    pub draw_mask: u32,
    pub stencil: Option<s::Stencil>,
    pub depth: Option<s::Depth>,
    pub colors: [s::Color; d::MAX_COLOR_TARGETS],
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PipelineState {
    program: Program,
    primitive: d::Primitive,
    input: [Option<BufferElement>; d::MAX_VERTEX_ATTRIBUTES],
    scissor: bool,
    rasterizer: s::Rasterizer,
    output: OutputMerger,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum NewTexture {
    Surface(Surface),
    Texture(Texture),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceView {
    object: Texture,
    bind: gl::types::GLenum,
    owned: bool,
}

impl ResourceView {
    pub fn new_texture(t: Texture, kind: d::tex::Kind) -> ResourceView {
        ResourceView {
            object: t,
            bind: tex::kind_to_gl(kind),
            owned: false,
        }
    }
    pub fn new_buffer(b: Texture) -> ResourceView {
        ResourceView {
            object: b,
            bind: gl::TEXTURE_BUFFER,
            owned: true,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FatSampler {
    object: Sampler,
    info: d::tex::SamplerInfo,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TargetView {
    Surface(Surface),
    Texture(Texture, Level),
    TextureLayer(Texture, Level, Layer),
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

/// Create a new device with a factory.
pub fn create<F>(fn_proc: F) -> (Device, Factory) where
    F: FnMut(&str) -> *const std::os::raw::c_void
{
    let device = Device::new(fn_proc);
    let factory = Factory::new(device.share.clone());
    (device, factory)
}

/// Create the proxy target views (RTV and DSV) for the attachments of the
/// main framebuffer. These have GL names equal to 0.
/// Not supposed to be used by the users directly.
pub fn create_main_targets_raw(dim: d::tex::Dimensions, color_format: d::format::SurfaceType, depth_format: d::format::SurfaceType)
                               -> (handle::RawRenderTargetView<Resources>, handle::RawDepthStencilView<Resources>) {
    use gfx_core::handle::Producer;
    let mut temp = handle::Manager::new();
    let color_tex = temp.make_texture(
        NewTexture::Surface(0),
        d::tex::Descriptor {
            levels: 1,
            kind: d::tex::Kind::D2(dim.0, dim.1, dim.3),
            format: color_format,
            bind: d::factory::RENDER_TARGET,
            usage: d::factory::Usage::GpuOnly,
        },
    );
    let depth_tex = temp.make_texture(
        NewTexture::Surface(0),
        d::tex::Descriptor {
            levels: 1,
            kind: d::tex::Kind::D2(dim.0, dim.1, dim.3),
            format: depth_format,
            bind: d::factory::DEPTH_STENCIL,
            usage: d::factory::Usage::GpuOnly,
        },
    );
    let m_color = temp.make_rtv(TargetView::Surface(0), &color_tex, dim);
    let m_ds = temp.make_dsv(TargetView::Surface(0), &depth_tex, dim);
    (m_color, m_ds)
}

/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    context: gl::Gl,
    capabilities: d::Capabilities,
    private_caps: info::PrivateCaps,
    handles: RefCell<handle::Manager<Resources>>,
}

/// An OpenGL device with GLSL shaders.
pub struct Device {
    info: Info,
    share: Rc<Share>,
    _vao: ArrayBuffer,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

impl Device {
    /// Create a new device. Each GL context can only have a single
    /// Device on GFX side to represent it. //TODO: enforce somehow
    /// Also, load OpenGL symbols and detect driver information.
    fn new<F>(fn_proc: F) -> Device where
        F: FnMut(&str) -> *const std::os::raw::c_void
    {
        let gl = gl::Gl::load_with(fn_proc);
        // query information
        let (info, caps, private) = info::get(&gl);
        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }
        // initialize permanent states
        if caps.srgb_color_supported {
            unsafe {
                gl.Enable(gl::FRAMEBUFFER_SRGB);
            }
        }
        unsafe {
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        }
        // create main VAO and bind it
        let mut vao = 0;
        if private.array_buffer_supported {
            unsafe {
                gl.GenVertexArrays(1, &mut vao);
                gl.BindVertexArray(vao);
            }
        }
        let handles = handle::Manager::new();
        // create the device
        Device {
            info: info,
            share: Rc::new(Share {
                context: gl,
                capabilities: caps,
                private_caps: private,
                handles: RefCell::new(handles),
            }),
            _vao: vao,
            frame_handles: handle::Manager::new(),
            max_resource_count: Some(999999),
        }
    }

    /// Access the OpenGL directly via a closure. OpenGL types and enumerations
    /// can be found in the `gl` crate.
    pub unsafe fn with_gl<F: FnMut(&gl::Gl)>(&mut self, mut fun: F) {
        self.reset_state();
        fun(&self.share.context);
    }

    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&mut self, cmd: &Command) {
        if cfg!(debug_assertions) {
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

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buffer: Buffer, bel: BufferElement) {
        use gfx_core::format::SurfaceType as S;
        use gfx_core::format::ChannelType as C;
        let (fm8, fm16, fm32) = match bel.elem.format.1 {
            C::Int | C::Inorm =>
                (gl::BYTE, gl::SHORT, gl::INT),
            C::Uint | C::Unorm =>
                (gl::UNSIGNED_BYTE, gl::UNSIGNED_SHORT, gl::UNSIGNED_INT),
            C::Float => (gl::ZERO, gl::HALF_FLOAT, gl::FLOAT),
            C::Srgb => {
                error!("Unsupported Srgb channel type");
                return
            }
        };
        let (count, gl_type) = match bel.elem.format.0 {
            S::R8              => (1, fm8),
            S::R8_G8           => (2, fm8),
            S::R8_G8_B8_A8     => (4, fm8),
            S::R16             => (1, fm16),
            S::R16_G16         => (1, fm16),
            S::R16_G16_B16     => (1, fm16),
            S::R16_G16_B16_A16 => (1, fm16),
            S::R32             => (1, fm32),
            S::R32_G32         => (2, fm32),
            S::R32_G32_B32     => (3, fm32),
            S::R32_G32_B32_A32 => (4, fm32),
            _ => {
                error!("Unsupported element type: {:?}", bel.elem.format.0);
                return
            }
        };
        let gl = &self.share.context;
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        let offset = bel.elem.offset as *const gl::types::GLvoid;
        let stride = bel.desc.stride as gl::types::GLint;
        match bel.elem.format.1 {
            C::Int | C::Uint => unsafe {
                gl.VertexAttribIPointer(slot as gl::types::GLuint,
                    count, gl_type, stride, offset);
            },
            C::Inorm | C::Unorm => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::TRUE, stride, offset);
            },
            //C::Iscaled | C::Uscaled => unsafe {
            //    gl.VertexAttribPointer(slot as gl::types::GLuint,
            //        count, gl_type, gl::FALSE, stride, offset);
            //},
            C::Float => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::FALSE, stride, offset);
            },
            C::Srgb => (),
        }
        unsafe { gl.EnableVertexAttribArray(slot as gl::types::GLuint) };
        if self.share.capabilities.instance_rate_supported {
            unsafe { gl.VertexAttribDivisor(slot as gl::types::GLuint,
                bel.desc.rate as gl::types::GLuint) };
        }else if bel.desc.rate != 0 {
            error!("Instanced arrays are not supported");
        }
    }

    fn bind_target(&mut self, point: gl::types::GLenum, attachment: gl::types::GLenum, view: &TargetView) {
        let gl = &self.share.context;
        match view {
            &TargetView::Surface(surface) => unsafe {
                gl.FramebufferRenderbuffer(point, attachment, gl::RENDERBUFFER, surface);
            },
            &TargetView::Texture(texture, level) => unsafe {
                gl.FramebufferTexture(point, attachment, texture,
                                      level as gl::types::GLint);
            },
            &TargetView::TextureLayer(texture, level, layer) => unsafe {
                gl.FramebufferTextureLayer(point, attachment, texture,
                                           level as gl::types::GLint,
                                           layer as gl::types::GLint);
            },
        }
    }

    fn reset_state(&mut self) {
        let data = DataBuffer::new();
        for com in command::RESET.iter() {
            self.process(com, &data);
        }
    }

    fn place_fence(&mut self) -> handle::Fence<Resources> {
        use gfx_core::handle::Producer;

        let gl = &self.share.context;
        let fence = unsafe {
            gl.FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0)
        };
        self.frame_handles.make_fence(Fence {
            raw: RawFence(fence),
            share: self.share.clone(),
        })
    }

    fn place_memory_barrier(&mut self) {
        let gl = &self.share.context;
        // TODO: other flags ?
        unsafe { gl.MemoryBarrier(gl::CLIENT_MAPPED_BUFFER_BARRIER_BIT); }
    }

    fn process(&mut self, cmd: &Command, data_buf: &DataBuffer) {
        match *cmd {
            Command::Clear(color, depth, stencil) => {
                let gl = &self.share.context;
                if self.share.private_caps.clear_buffer_supported {
                    if let Some(c) = color {
                        let slot = 0; //TODO?
                        state::unlock_color_mask(gl);
                        match c {
                            d::draw::ClearColor::Float(v) => unsafe {
                                gl.ClearBufferfv(gl::COLOR, slot, &v[0]);
                            },
                            d::draw::ClearColor::Int(v) => unsafe {
                                gl.ClearBufferiv(gl::COLOR, slot, &v[0]);
                            },
                            d::draw::ClearColor::Uint(v) => unsafe {
                                gl.ClearBufferuiv(gl::COLOR, slot, &v[0]);
                            },
                        }
                    }
                    if let Some(ref d) = depth {
                        unsafe {
                            gl.DepthMask(gl::TRUE);
                            gl.ClearBufferfv(gl::DEPTH, 0, d);
                        }
                    }
                    if let Some(s) = stencil {
                        let v = s as gl::types::GLint;
                        unsafe {
                            gl.StencilMask(gl::types::GLuint::max_value());
                            gl.ClearBufferiv(gl::STENCIL, 0, &v);
                        }
                    }
                } else {
                    let mut flags = 0;
                    if let Some(col) = color {
                        flags |= gl::COLOR_BUFFER_BIT;
                        let v = if let d::draw::ClearColor::Float(v) = col {
                            v
                        } else {
                            warn!("Integer clears are not supported on GL2");
                            [0.0, 0.0, 0.0, 0.0]
                        };
                        state::unlock_color_mask(gl);
                        unsafe {
                            gl.ClearColor(v[0], v[1], v[2], v[3]);
                        }
                    }
                    if let Some(d) = depth {
                        flags |= gl::DEPTH_BUFFER_BIT;
                        unsafe  {
                            gl.DepthMask(gl::TRUE);
                            gl.ClearDepth(d as gl::types::GLdouble);
                        }
                    }
                    if let Some(s) = stencil {
                        flags |= gl::STENCIL_BUFFER_BIT;
                        unsafe  {
                            gl.StencilMask(gl::types::GLuint::max_value());
                            gl.ClearStencil(s as gl::types::GLint);
                        }
                    }
                    unsafe {
                        gl.Clear(flags);
                    }
                }
            },
            Command::BindProgram(program) => unsafe {
                self.share.context.UseProgram(program);
            },
            Command::BindConstantBuffer(d::pso::ConstantBufferParam(buffer, _, slot)) => unsafe {
                self.share.context.BindBufferBase(gl::UNIFORM_BUFFER, slot as gl::types::GLuint, buffer);
            },
            Command::BindResourceView(d::pso::ResourceViewParam(view, _, slot)) => unsafe {
                self.share.context.ActiveTexture(gl::TEXTURE0 + slot as gl::types::GLenum);
                self.share.context.BindTexture(view.bind, view.object);
            },
            Command::BindUnorderedView(_uav) => unimplemented!(),
            Command::BindSampler(d::pso::SamplerParam(sampler, _, slot), bind_opt) => {
                let gl = &self.share.context;
                if self.share.private_caps.sampler_objects_supported {
                    unsafe { gl.BindSampler(slot as gl::types::GLuint, sampler.object) };
                } else {
                    assert!(d::MAX_SAMPLERS <= d::MAX_RESOURCE_VIEWS);
                    debug_assert_eq!(sampler.object, 0);
                    if let Some(bind) = bind_opt {
                        tex::bind_sampler(gl, bind, &sampler.info, self.info.version.is_embedded);
                    }else {
                        error!("Trying to bind a sampler to slot {}, when sampler objects are not supported, and no texture is bound there", slot);
                    }
                }
            },
            Command::BindPixelTargets(pts) => {
                let point = gl::DRAW_FRAMEBUFFER;
                for i in 0 .. d::MAX_COLOR_TARGETS {
                    if let Some(ref target) = pts.colors[i] {
                        let att = gl::COLOR_ATTACHMENT0 + i as gl::types::GLuint;
                        self.bind_target(point, att, target);
                    }
                }
                if let Some(ref depth) = pts.depth {
                    self.bind_target(point, gl::DEPTH_ATTACHMENT, depth);
                }
                if let Some(ref stencil) = pts.stencil {
                    self.bind_target(point, gl::STENCIL_ATTACHMENT, stencil);
                }
            },
            Command::BindAttribute(slot, buffer,  bel) => {
                self.bind_attribute(slot, buffer, bel);
            },
            Command::BindIndex(buffer) => {
                let gl = &self.share.context;
                unsafe { gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer) };
            },
            Command::BindFrameBuffer(point, frame_buffer) => {
                if self.share.private_caps.frame_buffer_supported {
                    let gl = &self.share.context;
                    unsafe { gl.BindFramebuffer(point, frame_buffer) };
                } else if frame_buffer != 0 {
                    error!("Tried to bind FBO {} without FBO support!", frame_buffer);
                }
            },
            Command::BindUniform(loc, uniform) => {
                let gl = &self.share.context;
                shade::bind_uniform(gl, loc as gl::types::GLint, uniform);
            },
            Command::SetDrawColorBuffers(num) => {
                let mask = (1 << (num as usize)) - 1;
                state::bind_draw_color_buffers(&self.share.context, mask);
            },
            Command::SetRasterizer(rast) => {
                state::bind_rasterizer(&self.share.context, &rast, self.info.version.is_embedded);
            },
            Command::SetViewport(rect) => {
                state::bind_viewport(&self.share.context, rect);
            },
            Command::SetScissor(rect) => {
                state::bind_scissor(&self.share.context, rect);
            },
            Command::SetDepthState(depth) => {
                state::bind_depth(&self.share.context, &depth);
            },
            Command::SetStencilState(stencil, refs, cull) => {
                state::bind_stencil(&self.share.context, &stencil, refs, cull);
            },
            Command::SetBlendState(slot, color) => {
                if self.share.capabilities.separate_blending_slots_supported {
                    state::bind_blend_slot(&self.share.context, slot, color);
                }else if slot == 0 {
                    //self.temp.color = color; //TODO
                    state::bind_blend(&self.share.context, color);
                }else if false {
                    error!("Separate blending slots are not supported");
                }
            },
            Command::SetBlendColor(color) => {
                state::set_blend_color(&self.share.context, color);
            },
            Command::UpdateBuffer(buffer, pointer, offset) => {
                let data = data_buf.get(pointer);
                factory::update_sub_buffer(&self.share.context, buffer,
                    data.as_ptr(), data.len(), offset, d::factory::BufferRole::Vertex);
            },
            Command::UpdateTexture(texture, kind, face, pointer, ref image) => {
                let data = data_buf.get(pointer);
                match tex::update_texture(&self.share.context, texture, kind, face, image, data) {
                    Ok(_) => (),
                    Err(e) => error!("GL: Texture({}) update failed: {:?}", texture, e),
                }
            },
            Command::GenerateMipmap(view) => {
                tex::generate_mipmap(&self.share.context, view.object, view.bind);
            },
            Command::Draw(primitive, start, count, instances) => {
                let gl = &self.share.context;
                match instances {
                    Some((num, base)) if self.share.capabilities.instance_call_supported => unsafe {
                        gl.DrawArraysInstancedBaseInstance(
                            primitive,
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
                            primitive,
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei
                        );
                    },
                }
            },
            Command::DrawIndexed(primitive, index_type, offset, count, base_vertex, instances) => {
                let gl = &self.share.context;
                let caps = &self.share.capabilities;
                match instances {
                    Some((num, base_instance)) if caps.instance_call_supported => unsafe {
                        if (base_vertex == 0 && base_instance == 0) || !caps.vertex_base_supported {
                            if base_vertex != 0 || base_instance != 0 {
                                error!("Instance bases with indexed drawing is not supported")
                            }
                            gl.DrawElementsInstanced(
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
                                num as gl::types::GLsizei,
                            );
                        } else if base_vertex != 0 && base_instance == 0 {
                            gl.DrawElementsInstancedBaseVertex(
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
                                num as gl::types::GLsizei,
                                base_vertex as gl::types::GLint,
                            );
                        } else if base_vertex == 0 && base_instance != 0 {
                            gl.DrawElementsInstancedBaseInstance(
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
                                num as gl::types::GLsizei,
                                base_instance as gl::types::GLuint,
                            );
                        } else {
                            gl.DrawElementsInstancedBaseVertexBaseInstance(
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
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
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
                            );
                        } else {
                            gl.DrawElementsBaseVertex(
                                primitive,
                                count as gl::types::GLsizei,
                                index_type,
                                offset.0,
                                base_vertex as gl::types::GLint,
                            );
                        }
                    },
                }
            },
            Command::_Blit(mut s_rect, d_rect, mirror, _) => {
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
                let flags = 0;
                error!("Blit mask setup is not implemented");
                /*TODO
                if mask.intersects(d::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                }
                if mask.intersects(d::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                }
                if mask.intersects(d::target::STENCIL) {
                    flags |= gl::STENCIL_BUFFER_BIT;
                }*/
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

    fn no_fence_submit(&mut self, cb: &mut command::CommandBuffer) {
        self.reset_state();
        for com in &cb.buf {
            self.process(com, &cb.data);
        }
    }

    fn handle_write_syncs(&mut self, mappings: &[handle::RawMapping<Resources>]) {
        let gl = &self.share.context;
        for mapping in mappings {
            let mut inner = mapping.access()
                .expect("user error: mapping still in use on submit");

            match inner.resource.kind {
                MappingKind::Persistent => {
                    if inner.status.cpu { unsafe {
                        gl.BindBuffer(inner.resource.target, *inner.buffer.resource());
                        let size = inner.buffer.get_info().size as isize;
                        gl.FlushMappedBufferRange(inner.resource.target, 0, size);
                    } }
                }
                MappingKind::Temporary => {
                    if inner.resource.is_mapped {
                        unsafe {
                            gl.BindBuffer(inner.resource.target, *inner.buffer.resource());
                            gl.UnmapBuffer(inner.resource.target);
                        }
                        inner.resource.is_mapped = false;
                    }
                }
            }

            inner.status.cpu = false;
        }
    }

    fn handle_read_syncs(&mut self, mappings: &[handle::RawMapping<Resources>],
                                    fence: &handle::Fence<Resources>) {
        let gl = &self.share.context;
        for mapping in mappings {
            let mut inner = mapping.access()
                .expect("user error: mapping still in use on submit");

            match inner.resource.kind {
                MappingKind::Persistent => {
                    if inner.access.contains(mapping::READABLE) {
                        inner.status.gpu = Some(fence.clone());
                    }
                }
                MappingKind::Temporary => {
                    // TODO: this could be done on later user access for performance
                    if !inner.resource.is_mapped {
                        let access = factory::access_to_gl(inner.access);
                        unsafe {
                            gl.BindBuffer(inner.resource.target, *inner.buffer.resource());
                            inner.resource.pointer = gl.MapBuffer(inner.resource.target, access)
                                as *mut ::std::os::raw::c_void;
                        }
                        inner.resource.is_mapped = true;
                    }
                }
            }
        }
    }
}

impl d::Device for Device {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer;

    fn get_capabilities(&self) -> &d::Capabilities {
        &self.share.capabilities
    }

    fn pin_submitted_resources(&mut self, man: &handle::Manager<Resources>) {
        self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn submit(&mut self, cb: &mut command::CommandBuffer,
                         mapped_reads: &[handle::RawMapping<Resources>],
                         mapped_writes: &[handle::RawMapping<Resources>]) {
        self.handle_write_syncs(mapped_reads);
        self.no_fence_submit(cb);
        if mapped_writes.len() > 0 {
            self.place_memory_barrier();
            let fence = self.place_fence();
            self.handle_read_syncs(mapped_writes, &fence);
        }
    }

    fn fenced_submit(&mut self,
                     cb: &mut command::CommandBuffer,
                     mapped_reads: &[handle::RawMapping<Resources>],
                     mapped_writes: &[handle::RawMapping<Resources>],
                     after: Option<handle::Fence<Resources>>) -> handle::Fence<Resources> {

        if let Some(fence) = after {
            let f = self.frame_handles.ref_fence(&fence);
            let timeout = 1_000_000_000_000;
            // FIXME: should we use 'glFlush' here ?
            // see https://www.opengl.org/wiki/Sync_Object
            unsafe { self.share.context.WaitSync(f.raw.0, 0, timeout); }
        }

        self.handle_write_syncs(mapped_reads);
        self.no_fence_submit(cb);
        if mapped_writes.len() > 0 { self.place_memory_barrier(); }
        let fence = self.place_fence();
        self.handle_read_syncs(mapped_writes, &fence);
        fence
    }

    fn cleanup(&mut self) {
        use gfx_core::handle::Producer;
        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut &self.share.context,
            |gl, raw_buffer| unsafe { gl.DeleteBuffers(1, &raw_buffer.resource) },
            |gl, v| unsafe { gl.DeleteShader(*v) },
            |gl, program| unsafe { gl.DeleteProgram(program.resource) },
            |_, _| {}, //PSO
            |gl, raw_texture| match raw_texture.resource {
                NewTexture::Surface(ref suf) => unsafe { gl.DeleteRenderbuffers(1, suf) },
                NewTexture::Texture(ref tex) => unsafe { gl.DeleteTextures(1, tex) },
            }, // new texture
            |gl, v| if v.owned {
                unsafe { gl.DeleteTextures(1, &v.object) }
            }, //SRV
            |_, _| {}, //UAV
            |_, _| {}, //RTV
            |_, _| {}, //DSV
            |gl, v| unsafe { if v.object != 0 { gl.DeleteSamplers(1, &v.object) }},
            |gl, v| unsafe { gl.DeleteSync(v.raw.0) },
            |gl, raw_mapping| {
                let inner = raw_mapping.access().unwrap();
                match inner.resource.kind {
                    MappingKind::Persistent => (),
                    MappingKind::Temporary => {
                        if inner.resource.is_mapped { unsafe {
                            gl.BindBuffer(inner.resource.target, *inner.buffer.resource());
                            gl.UnmapBuffer(inner.resource.target);
                        } }
                    }
                }
            },
        );
    }
}
