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
use gfx_core as d;
use gfx_core::draw::{Access, Target};
use gfx_core::handle;
use gfx_core::state as s;
use gfx_core::target::{Layer, Level};
use command::Command;

pub use self::factory::{Factory, Output};
pub use self::info::{Info, PlatformName, Version};

pub mod command;
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
pub struct Fence(gl::types::GLsync);

unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl d::Resources for Resources {
    type Buffer              = Buffer;
    type ArrayBuffer         = ArrayBuffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = PipelineState;
    type NewTexture          = NewTexture;
    type FrameBuffer         = FrameBuffer;
    type Surface             = Surface;
    type RenderTargetView    = TargetView;
    type DepthStencilView    = TargetView;
    type ShaderResourceView  = ResourceView;
    type UnorderedAccessView = ();
    type Texture             = Texture;
    type Sampler             = FatSampler;
    type Fence               = Fence;
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct OutputMerger {
    /// Color attachment draw mask.
    pub draw_mask: u32,
    /// Stencil test to use. If None, no stencil testing is done.
    pub stencil: Option<s::Stencil>,
    /// Depth test to use. If None, no depth testing is done.
    pub depth: Option<s::Depth>,
    /// Blend function to use. If None, no blending is done.
    pub blend: [Option<s::Blend>; d::MAX_COLOR_TARGETS],
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PipelineState {
    program: Program,
    primitive: d::Primitive,
    input: [Option<d::pso::AttributeDesc>; d::MAX_VERTEX_ATTRIBUTES],
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

fn primitive_to_gl(primitive: d::Primitive) -> gl::types::GLenum {
    match primitive {
        d::Primitive::Point => gl::POINTS,
        d::Primitive::Line => gl::LINES,
        d::Primitive::LineStrip => gl::LINE_STRIP,
        d::Primitive::TriangleList => gl::TRIANGLES,
        d::Primitive::TriangleStrip => gl::TRIANGLE_STRIP,
        d::Primitive::TriangleFan => gl::TRIANGLE_FAN,
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

/// Create the proxy target views (RTV and DSV) for the attachments of the
/// main framebuffer. These have GL names equal to 0.
pub fn create_main_targets<Tc, Td>(dim: d::tex::Dimensions)
                           -> (handle::RenderTargetView<Resources, Tc>,
                               handle::DepthStencilView<Resources, Td>)
where
    Tc: d::format::RenderFormat,
    Td: d::format::DepthFormat,
{
    use gfx_core::handle::Producer;
    let mut temp = handle::Manager::new();
    let texture = temp.make_new_texture(
        NewTexture::Surface(0),
        d::tex::Descriptor {
            levels: 0,
            kind: d::tex::Kind::D2(dim.0, dim.1, dim.3),
            format: d::format::SurfaceType::R8_G8_B8_A8,
            bind: d::factory::RENDER_TARGET,
        },
    );
    let m_color = temp.make_rtv(TargetView::Surface(0), &texture, dim);
    let m_ds = temp.make_dsv(TargetView::Surface(0), &texture, dim);
    (d::factory::Phantom::new(m_color), d::factory::Phantom::new(m_ds))
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
    primitive: gl::types::GLenum,
    attributes: [Option<d::pso::AttributeDesc>; d::MAX_VERTEX_ATTRIBUTES],
    resource_views: [Option<(Texture, tex::BindAnchor)>; d::MAX_RESOURCE_VIEWS],
    stencil: Option<s::Stencil>,
    cull_face: s::CullFace,
    blend: Option<s::Blend>,
}

impl Temp {
    fn new() -> Temp {
        Temp {
            primitive: 0,
            attributes: [None; d::MAX_VERTEX_ATTRIBUTES],
            resource_views: [None; d::MAX_RESOURCE_VIEWS],
            stencil: None,
            cull_face: s::CullFace::Nothing,
            blend: None,
        }
    }
}

/// An OpenGL device with GLSL shaders.
pub struct Device {
    info: Info,
    share: Rc<Share>,
    temp: Temp,
    _vao: ArrayBuffer,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

impl Device {
    /// Create a new device. There can be only one!
    /// Also, load OpenGL symbols and detect driver information.
    fn new<F>(fn_proc: F) -> Device where
        F: FnMut(&str) -> *const std::os::raw::c_void
    {
        use gfx_core::handle::Producer;
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
            gl.Enable(gl::FRAMEBUFFER_SRGB);
        }
        // create main VAO and bind it
        let mut vao = 0;
        if caps.array_buffer_supported {
            unsafe {
                gl.GenVertexArrays(1, &mut vao);
                gl.BindVertexArray(vao);
            }
        }
        // create the main FBO surface proxies
        let mut handles = handle::Manager::new();
        let main_fbo = handles.make_frame_buffer(0);
        // create the device
        Device {
            info: info,
            share: Rc::new(Share {
                context: gl,
                capabilities: caps,
                handles: RefCell::new(handles),
                main_fbo: main_fbo,
            }),
            temp: Temp::new(),
            _vao: vao,
            frame_handles: handle::Manager::new(),
            max_resource_count: Some(999999),
        }
    }

    /// Access the OpenGL directly via a closure. OpenGL types and enumerations
    /// can be found in the `gl` crate.
    pub unsafe fn with_gl<F: FnMut(&gl::Gl)>(&mut self, mut fun: F) {
        use gfx_core::Device;
        self.reset_state();
        fun(&self.share.context);
    }

    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&mut self, cmd: &Command) {
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
                      (elem, instance_rate): d::pso::AttributeDesc) {
        use gfx_core::format::SurfaceType as S;
        use gfx_core::format::ChannelType as C;
        let (fm8, fm16, fm32) = match elem.format.1.ty {
            C::Int | C::IntNormalized | C::IntScaled =>
                (gl::BYTE, gl::SHORT, gl::INT),
            C::Uint | C::UintNormalized | C::UintScaled =>
                (gl::UNSIGNED_BYTE, gl::UNSIGNED_SHORT, gl::UNSIGNED_INT),
            C::Float => (gl::ZERO, gl::HALF_FLOAT, gl::FLOAT),
            C::Srgb => {
                error!("Unsupported Srgb channel type");
                return
            }
        };
        let (count, gl_type) = match elem.format.0 {
            S::R8              => (1, fm8),
            S::R8_G8           => (2, fm8),
            S::R8_G8_B8        => (3, fm8),
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
                error!("Unsupported element type: {:?}", elem.format.0);
                return
            }
        };
        let gl = &self.share.context;
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        let offset = elem.offset as *const gl::types::GLvoid;
        let stride = elem.stride as gl::types::GLint;
        match elem.format.1.ty {
            C::Int | C::Uint => unsafe {
                gl.VertexAttribIPointer(slot as gl::types::GLuint,
                    count, gl_type, stride, offset);
            },
            C::IntNormalized | C::UintNormalized => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::TRUE, stride, offset);
            },
            C::IntScaled | C:: UintScaled => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::FALSE, stride, offset);
            },
            C::Float => unsafe {
                gl.VertexAttribPointer(slot as gl::types::GLuint,
                    count, gl_type, gl::FALSE, stride, offset);
            },
            C::Srgb => (),
        }
        unsafe { gl.EnableVertexAttribArray(slot as gl::types::GLuint) };
        if self.share.capabilities.instance_rate_supported {
            unsafe { gl.VertexAttribDivisor(slot as gl::types::GLuint,
                instance_rate as gl::types::GLuint) };
        }else if instance_rate != 0 {
            error!("Instanced arrays are not supported");
        }
    }

    fn bind_attribute_old(&mut self, slot: d::AttributeSlot, buffer: Buffer,
                          format: d::attrib::Format) {
        use gfx_core::attrib::{Type, IntSize, IntSubType, FloatSize, FloatSubType, SignFlag};
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

    fn process(&mut self, cmd: &Command, data_buf: &d::draw::DataBuffer) {
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
                self.temp.primitive = primitive_to_gl(pso.primitive);
                self.temp.attributes = pso.input;
                state::bind_rasterizer(gl, &pso.rasterizer);
                self.temp.stencil = pso.output.stencil;
                self.temp.cull_face = pso.rasterizer.method.get_cull_face();
                state::bind_stencil(gl, &self.temp.stencil, (0, 0), self.temp.cull_face);
                state::bind_depth(gl, &pso.output.depth);
                for i in 0 .. d::MAX_COLOR_TARGETS {
                    state::bind_blend_slot(gl, i as d::ColorSlot, pso.output.blend[i]);
                }
            },
            Command::BindVertexBuffers(vbs) => {
                for i in 0 .. d::MAX_VERTEX_ATTRIBUTES {
                    match (vbs.0[i], self.temp.attributes[i]) {
                        (None, Some(fm)) => {
                            error!("No vertex input provided for slot {} of format {:?}", i, fm)
                        },
                        (Some((buffer, offset)), Some(mut format)) => {
                            format.0.offset += offset as gl::types::GLuint;
                            self.bind_attribute(i as d::AttributeSlot, buffer, format);
                        },
                        (_, None) => {},
                    }
                }
            },
            Command::BindConstantBuffers(cbs) => {
                let gl = &self.share.context;
                for i in 0 .. d::MAX_CONSTANT_BUFFERS {
                    if let Some(buffer) = cbs.0[i] {
                        unsafe {
                            gl.BindBufferBase(gl::UNIFORM_BUFFER, i as gl::types::GLuint, buffer);
                        }
                    }
                }
            },
            Command::BindResourceViews(srvs) => {
                let gl = &self.share.context;
                for i in 0 .. d::MAX_RESOURCE_VIEWS {
                    if let Some(view) = srvs.0[i] {
                        unsafe {
                            gl.ActiveTexture(gl::TEXTURE0 + i as gl::types::GLenum);
                            gl.BindTexture(view.bind, view.object);
                        }
                    }
                }
            },
            Command::BindUnorderedViews(uavs) => {
                for i in 0 .. d::MAX_UNORDERED_VIEWS {
                    if let Some(_view) = uavs.0[i] {
                        //TODO
                    }
                }
            },
            Command::BindSamplers(ss) => {
                let gl = &self.share.context;
                for i in 0 .. d::MAX_SAMPLERS {
                    if let Some(s) = ss.0[i] {
                        if self.share.capabilities.sampler_objects_supported {
                            unsafe { gl.BindSampler(i as gl::types::GLenum, s.object) };
                        } else {
                            assert!(d::MAX_SAMPLERS <= d::MAX_RESOURCE_VIEWS);
                            debug_assert_eq!(s.object, 0);
                            if let Some((_, anchor)) = self.temp.resource_views[i] {
                                tex::bind_sampler(gl, anchor, &s.info);
                            }else {
                                error!("Trying to bind a sampler to slot {}, when sampler objects are not supported, and no texture is bound there", i);
                            }
                        }
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
                if let Some(ref ds) = pts.depth_stencil {
                    let att = gl::DEPTH_STENCIL_ATTACHMENT;
                    self.bind_target(point, att, ds);
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
                self.bind_attribute_old(slot, buffer, format);
            },
            Command::BindIndex(buffer) => {
                let gl = &self.share.context;
                unsafe { gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer) };
            },
            Command::BindFrameBuffer(access, frame_buffer) => {
                let caps = &self.share.capabilities;
                let gl = &self.share.context;
                if !caps.render_targets_supported {
                    panic!("Tried to do something with an FBO without FBO support!")
                }
                let point = access_to_gl(access);
                unsafe { gl.BindFramebuffer(point, frame_buffer) };
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
            Command::BindUniformBlock(slot, buffer) => {
                let gl = &self.share.context;
                unsafe {
                    gl.BindBufferBase(gl::UNIFORM_BUFFER, slot as gl::types::GLuint, buffer);
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
                    kind, None, texture);
                let (_, _, _, aa) = kind.get_dimensions();
                match (anchor, aa, sampler) {
                    (anchor, d::tex::AaMode::Single, Some(s)) => {
                        if self.share.capabilities.sampler_objects_supported {
                            unsafe { gl.BindSampler(slot as gl::types::GLenum, s.object) };
                        } else {
                            debug_assert_eq!(s.object, 0);
                            tex::bind_sampler(gl, anchor, &s.info);
                        }
                    },
                    (_, _, Some(_)) =>
                        error!("Unable to bind a multi-sampled texture with a sampler"),
                    (_, _, _) => (),
                }
            },
            Command::SetDrawColorBuffers(num) => {
                let mask = (1 << (num as usize)) - 1;
                state::bind_draw_color_buffers(&self.share.context, mask);
            },
            Command::SetRasterizer(rast) => {
                state::bind_rasterizer(&self.share.context, &rast);
            },
            Command::SetViewport(rect) => {
                state::bind_viewport(&self.share.context, rect);
            },
            Command::SetScissor(rect) => {
                state::bind_scissor(&self.share.context, rect);
            },
            Command::SetDepthStencilState(depth, stencil, cull) => {
                let gl = &self.share.context;
                state::bind_stencil(gl, &stencil, (0, 0), cull);
                state::bind_depth(gl, &depth);
            },
            Command::SetBlendState(slot, blend) => {
                if self.share.capabilities.separate_blending_slots_supported {
                    state::bind_blend_slot(&self.share.context, slot, blend);
                }else if slot == 0 {
                    self.temp.blend = blend;
                    state::bind_blend(&self.share.context, blend);
                }else if blend != self.temp.blend {
                    error!("Separate blending slots are not supported");
                }
            },
            Command::SetRefValues(rv) => {
                let gl = &self.share.context;
                state::set_blend_color(gl, rv.blend);
                state::bind_stencil(gl, &self.temp.stencil, rv.stencil, self.temp.cull_face);
            },
            Command::UpdateBuffer(buffer, pointer, offset) => {
                let data = data_buf.get_ref(pointer);
                factory::update_sub_buffer(&self.share.context, buffer,
                    data.as_ptr(), data.len(), offset, d::factory::BufferRole::Vertex);
            },
            Command::UpdateTexture(kind, texture, image_info, pointer) => {
                let face = None; //TODO
                let data = data_buf.get_ref(pointer);
                match tex::update_texture(&self.share.context, kind, face, texture, &image_info, data) {
                    Ok(_) => (),
                    Err(e) => {
                        error!("Error updating a texture: {:?}", e);
                    },
                }
            },
            Command::SetPrimitive(primitive) => {
                self.temp.primitive = primitive_to_gl(primitive);
            },
            Command::Draw(start, count, instances) => {
                let gl = &self.share.context;
                match instances {
                    Some((num, base)) if self.share.capabilities.instance_call_supported => unsafe {
                        gl.DrawArraysInstancedBaseInstance(
                            self.temp.primitive,
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
                            self.temp.primitive,
                            start as gl::types::GLsizei,
                            count as gl::types::GLsizei
                        );
                    },
                }
            },
            Command::DrawIndexed(index_type, start, count, base_vertex, instances) => {
                use gfx_core::attrib::IntSize;
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
                                self.temp.primitive,
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                            );
                        } else if base_vertex != 0 && base_instance == 0 {
                            gl.DrawElementsInstancedBaseVertex(
                                self.temp.primitive,
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_vertex as gl::types::GLint,
                            );
                        } else if base_vertex == 0 && base_instance != 0 {
                            gl.DrawElementsInstancedBaseInstance(
                                self.temp.primitive,
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                                num as gl::types::GLsizei,
                                base_instance as gl::types::GLuint,
                            );
                        } else {
                            gl.DrawElementsInstancedBaseVertexBaseInstance(
                                self.temp.primitive,
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
                                self.temp.primitive,
                                count as gl::types::GLsizei,
                                gl_index,
                                offset as *const gl::types::GLvoid,
                            );
                        } else {
                            gl.DrawElementsBaseVertex(
                                self.temp.primitive,
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

impl d::Device for Device {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer;

    fn get_capabilities<'a>(&'a self) -> &'a d::Capabilities {
        &self.share.capabilities
    }

    fn reset_state(&mut self) {
        let data = d::draw::DataBuffer::new();
        for com in command::RESET.iter() {
            self.process(com, &data);
        }
    }

    fn submit(&mut self, submit_info: d::SubmitInfo<Self>) {
        let d::SubmitInfo(cb, db, handles) = submit_info;
        self.frame_handles.extend(handles);
        self.reset_state();
        for com in &cb.buf {
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
        use gfx_core::handle::Producer;
        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut &self.share.context,
            |gl, v| unsafe { gl.DeleteBuffers(1, v) },
            |gl, v| unsafe { gl.DeleteVertexArrays(1, v) },
            |gl, v| unsafe { gl.DeleteShader(*v) },
            |gl, v| unsafe { gl.DeleteProgram(*v) },
            |_, _| {}, //PSO
            |gl, v| match v {
                &NewTexture::Surface(ref suf) => unsafe { gl.DeleteRenderbuffers(1, suf) },
                &NewTexture::Texture(ref tex) => unsafe { gl.DeleteTextures(1, tex) },
            }, // new texture
            |gl, v| if v.owned {
                unsafe { gl.DeleteTextures(1, &v.object) }
            }, //SRV
            |_, _| {}, //UAV
            |gl, v| unsafe { gl.DeleteFramebuffers(1, v) },
            |gl, v| unsafe { gl.DeleteRenderbuffers(1, v) },
            |_, _| {}, //RTV
            |_, _| {}, //DSV
            |gl, v| unsafe { gl.DeleteTextures(1, v) },
            |gl, v| unsafe { if v.object != 0 { gl.DeleteSamplers(1, &v.object) }},
            |gl, v| unsafe { gl.DeleteSync(v.0) },
        );
    }
}

impl gfx_core::DeviceFence<Resources> for Device {
    fn fenced_submit(&mut self, info: d::SubmitInfo<Self>, after: Option<handle::Fence<Resources>>)
                     -> handle::Fence<Resources> {
        use gfx_core::Device;
        use gfx_core::handle::Producer;

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
