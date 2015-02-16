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

extern crate libc;
extern crate "gfx_gl" as gl;

use log::LogLevel;

use attrib::{SignFlag, IntSubType, IntSize, FloatSubType, FloatSize, Type};
use state::{CullMode, RasterMethod, WindingOrder};
use target::{Access, Target};

use BufferUsage;
use {Device, Resources};
use {MapAccess, ReadableMapping, WritableMapping, RWMapping, BufferHandle, PrimitiveType};
use self::draw::{Command, CommandBuffer};
pub use self::info::{Info, PlatformName, Version};

mod draw;
mod shade;
mod state;
mod tex;
mod info;

#[allow(raw_pointer_derive)]
#[derive(Copy)]
pub struct RawMapping {
    pub pointer: *mut libc::c_void,
    target: gl::types::GLenum,
}

pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Sampler        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;

#[derive(Copy)]
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

#[derive(Copy, Eq, PartialEq, Debug)]
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

fn primitive_to_gl(prim_type: ::PrimitiveType) -> gl::types::GLenum {
    match prim_type {
        PrimitiveType::Point => gl::POINTS,
        PrimitiveType::Line => gl::LINES,
        PrimitiveType::LineStrip => gl::LINE_STRIP,
        PrimitiveType::TriangleList => gl::TRIANGLES,
        PrimitiveType::TriangleStrip => gl::TRIANGLE_STRIP,
        PrimitiveType::TriangleFan => gl::TRIANGLE_FAN,
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
    caps: ::Capabilities,
    gl: gl::Gl,
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
        info!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            info!("- {:?}", *extension);
        }

        GlDevice {
            info: info,
            caps: caps,
            gl: gl,
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
        info!("\tCreated buffer {:?}", name);
        name
    }

    fn init_buffer(&mut self, buffer: Buffer, info: &::BufferInfo) {
        unsafe { self.gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        let usage = match info.usage {
            BufferUsage::Static  => gl::STATIC_DRAW,
            BufferUsage::Dynamic => gl::DYNAMIC_DRAW,
            BufferUsage::Stream  => gl::STREAM_DRAW,
        };
        unsafe {
            self.gl.BufferData(gl::ARRAY_BUFFER,
                info.size as gl::types::GLsizeiptr,
                0 as *const gl::types::GLvoid,
                usage
            );
        }
    }

    fn update_sub_buffer(&mut self, buffer: Buffer, address: *const u8,
                         size: usize, offset: usize) {
        unsafe { self.gl.BindBuffer(gl::ARRAY_BUFFER, buffer) };
        unsafe {
            self.gl.BufferSubData(gl::ARRAY_BUFFER,
                offset as gl::types::GLintptr,
                size as gl::types::GLsizeiptr,
                address as *const gl::types::GLvoid
            );
        }
    }

    fn process(&mut self, cmd: &Command, data_buf: &::draw::DataBuffer) {
        match *cmd {
            Command::Clear(ref data, mask) => {
                let mut flags = 0;
                if mask.intersects(::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                    state::bind_color_mask(&self.gl, ::state::MASK_ALL);
                    let [r, g, b, a] = data.color;
                    unsafe { self.gl.ClearColor(r, g, b, a) };
                }
                if mask.intersects(::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                    unsafe {
                        self.gl.DepthMask(gl::TRUE);
                        self.gl.ClearDepth(data.depth as gl::types::GLclampd);
                    }
                }
                if mask.intersects(::target::STENCIL) {
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
                    error!("Ignored VAO bind command: {:?}", array_buffer)
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
                    (anchor, None, Some(::Handle(sam, ref info))) => {
                        if self.caps.sampler_objects_supported {
                            unsafe { self.gl.BindSampler(slot as gl::types::GLenum, sam) };
                        } else {
                            debug_assert_eq!(sam, 0);
                            tex::bind_sampler(&self.gl, anchor, info);
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
                self.update_sub_buffer(buffer, data.as_ptr(), data.len(), offset);
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
            Command::DrawIndexed(prim_type, index_type, start, count, basevertex, instances) => {
                let (offset, gl_index) = match index_type {
                    IntSize::U8  => (start * 1u32, gl::UNSIGNED_BYTE),
                    IntSize::U16 => (start * 2u32, gl::UNSIGNED_SHORT),
                    IntSize::U32 => (start * 4u32, gl::UNSIGNED_INT),
                };
                match instances {
                    Some((num, baseinstance)) if self.caps.instance_call_supported => unsafe {
                        if !self.caps.vertex_base_supported {
                            if baseinstance != 0 && !self.caps.instance_base_supported {
                                error!("Instance bases with indexed drawing is not supported")
                                // else, baseinstance == 0 OR instance_base_supported
                            } else if !self.caps.instance_base_supported {
                                // feature's not supported, but the base is 0
                                self.gl.DrawElementsInstanced(
                                    primitive_to_gl(prim_type),
                                    count as gl::types::GLsizei,
                                    gl_index,
                                    offset as *const gl::types::GLvoid,
                                    num as gl::types::GLsizei,
                                );
                            } else {
                                self.gl.DrawElementsInstancedBaseInstance(
                                    primitive_to_gl(prim_type),
                                    count as gl::types::GLsizei,
                                    gl_index,
                                    offset as *const gl::types::GLvoid,
                                    num as gl::types::GLsizei,
                                    baseinstance as gl::types::GLuint,
                                );
                            }
                        } else {
                            if baseinstance != 0 && !self.caps.instance_base_supported {
                                error!("Instance bases with indexed drawing not supported");
                            } else if !self.caps.instance_base_supported {
                                self.gl.DrawElementsInstancedBaseVertex(
                                    primitive_to_gl(prim_type),
                                    count as gl::types::GLsizei,
                                    gl_index,
                                    offset as *const gl::types::GLvoid,
                                    num as gl::types::GLsizei,
                                    basevertex as gl::types::GLint,
                                );
                            } else {
                                self.gl.DrawElementsInstancedBaseVertexBaseInstance(
                                    primitive_to_gl(prim_type),
                                    count as gl::types::GLsizei,
                                    gl_index,
                                    offset as *const gl::types::GLvoid,
                                    num as gl::types::GLsizei,
                                    basevertex as gl::types::GLint,
                                    baseinstance as gl::types::GLuint,
                                );
                            }
                        }
                    },
                    Some(_) => {
                        error!("Instanced draw calls are not supported");
                    },
                    None => unsafe {
                        if basevertex != 0 && !self.caps.vertex_base_supported {
                            error!("Base vertex with indexed drawing not supported");
                        } else if !self.caps.vertex_base_supported {
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
                                basevertex as gl::types::GLint,
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
                if mirror.intersects(::target::MIRROR_X) {
                    s_end_x = s_rect.x;
                    s_rect.x += s_rect.w;
                }
                if mirror.intersects(::target::MIRROR_Y) {
                    s_end_y = s_rect.y;
                    s_rect.y += s_rect.h;
                }
                // build mask
                let mut flags = 0;
                if mask.intersects(::target::COLOR) {
                    flags |= gl::COLOR_BUFFER_BIT;
                }
                if mask.intersects(::target::DEPTH) {
                    flags |= gl::DEPTH_BUFFER_BIT;
                }
                if mask.intersects(::target::STENCIL) {
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

    fn get_capabilities<'a>(&'a self) -> &'a ::Capabilities {
        &self.caps
    }

    fn reset_state(&mut self) {
        let data = ::draw::DataBuffer::new();
        self.process(&Command::BindProgram(0), &data);
        self.process(&Command::BindArrayBuffer(0), &data);
        // self.process(&command::BindAttribute, &data);
        self.process(&Command::BindIndex(0), &data);
        self.process(&Command::BindFrameBuffer(Access::Draw, 0), &data);
        self.process(&Command::BindFrameBuffer(Access::Read, 0), &data);
        // self.process(&command::UnbindTarget, &data);
        // self.process(&command::BindUniformBlock, &data);
        // self.process(&command::BindUniform, &data);
        // self.process(&command::BindTexture, &data);
        self.process(&Command::SetPrimitiveState(::state::Primitive {
            front_face: WindingOrder::CounterClockwise,
            method: RasterMethod::Fill(CullMode::Back),
            offset: None,
        }), &data);
        self.process(&Command::SetViewport(::target::Rect{x: 0, y: 0, w: 0, h: 0}), &data);
        self.process(&Command::SetScissor(None), &data);
        self.process(&Command::SetDepthStencilState(None, None, CullMode::Nothing), &data);
        self.process(&Command::SetBlendState(None), &data);
        self.process(&Command::SetColorMask(::state::MASK_ALL), &data);
    }

    fn submit(&mut self, (cb, db): (&CommandBuffer, &::draw::DataBuffer)) {
        self.reset_state();
        for com in cb.iter() {
            self.process(com, db);
        }
    }

    fn create_buffer_raw(&mut self, size: usize, usage: BufferUsage) -> ::BufferHandle<GlResources, ()> {
        let name = self.create_buffer_internal();
        let info = ::BufferInfo {
            usage: usage,
            size: size,
        };
        self.init_buffer(name, &info);
        ::BufferHandle::from_raw(::Handle(name, info))
    }

    fn create_buffer_static_raw(&mut self, data: &[u8]) -> ::BufferHandle<GlResources, ()> {
        let name = self.create_buffer_internal();

        let info = ::BufferInfo {
            usage: BufferUsage::Static,
            size: data.len(),
        };
        self.init_buffer(name, &info);
        self.update_sub_buffer(name, data.as_ptr(), data.len(), 0);
        ::BufferHandle::from_raw(::Handle(name, info))
    }

    fn create_array_buffer(&mut self) -> Result<::ArrayBufferHandle<GlResources>, ()> {
        if self.caps.array_buffer_supported {
            let mut name = 0 as ArrayBuffer;
            unsafe {
                self.gl.GenVertexArrays(1, &mut name);
            }
            info!("\tCreated array buffer {}", name);
            Ok(::Handle(name, ()))
        } else {
            error!("\tarray buffer creation unsupported, ignored");
            Err(())
        }
    }

    fn create_shader(&mut self, stage: ::shade::Stage, code: &[u8])
                     -> Result<::ShaderHandle<GlResources>, ::shade::CreateShaderError> {
        let (name, info) = shade::create_shader(&self.gl, stage, code);
        info.map(|info| {
            let level = if name.is_err() { LogLevel::Error } else { LogLevel::Warn };
            log!(level, "\tShader compile log: {}", info);
        });
        name.map(|sh| ::Handle(sh, stage))
    }

    fn create_program(&mut self, shaders: &[::ShaderHandle<GlResources>], targets: Option<&[&str]>) -> Result<::ProgramHandle<GlResources>, ()> {
        let (prog, log) = shade::create_program(&self.gl, &self.caps, shaders, targets);
        log.map(|log| {
            let level = if prog.is_err() { LogLevel::Error } else { LogLevel::Warn };
            log!(level, "\tProgram link log: {}", log);
        });
        prog
    }

    fn create_frame_buffer(&mut self) -> ::FrameBufferHandle<GlResources> {
        if !self.caps.render_targets_supported {
            panic!("No framebuffer objects, can't make a new one!");
        }

        let mut name = 0 as FrameBuffer;
        unsafe {
            self.gl.GenFramebuffers(1, &mut name);
        }
        info!("\tCreated frame buffer {}", name);
        ::Handle(name, ())
    }

    fn create_surface(&mut self, info: ::tex::SurfaceInfo) ->
                      Result<::SurfaceHandle<GlResources>, ::tex::SurfaceError> {
        tex::make_surface(&self.gl, &info).map(|suf| ::Handle(suf, info))
    }

    fn create_texture(&mut self, info: ::tex::TextureInfo) ->
                      Result<::TextureHandle<GlResources>, ::tex::TextureError> {
        if info.width == 0 || info.height == 0 || info.levels == 0 {
            return Err(::tex::TextureError::InvalidTextureInfo(info))
        }

        let name = if self.caps.immutable_storage_supported {
            tex::make_with_storage(&self.gl, &info)
        } else {
            tex::make_without_storage(&self.gl, &info)
        };
        name.map(|tex| ::Handle(tex, info))
    }

    fn create_sampler(&mut self, info: ::tex::SamplerInfo) -> ::SamplerHandle<GlResources> {
        let sam = if self.caps.sampler_objects_supported {
            tex::make_sampler(&self.gl, &info)
        } else {
            0
        };
        ::Handle(sam, info)
    }

    fn delete_buffer_raw(&mut self, handle: ::BufferHandle<GlResources, ()>) {
        let name = handle.get_name();
        unsafe {
            self.gl.DeleteBuffers(1, &name);
        }
    }

    fn delete_shader(&mut self, handle: ::ShaderHandle<GlResources>) {
        unsafe { self.gl.DeleteShader(handle.get_name()) };
    }

    fn delete_program(&mut self, handle: ::ProgramHandle<GlResources>) {
        unsafe { self.gl.DeleteProgram(handle.get_name()) };
    }

    fn delete_surface(&mut self, handle: ::SurfaceHandle<GlResources>) {
        let name = handle.get_name();
        unsafe {
            self.gl.DeleteRenderbuffers(1, &name);
        }
    }

    fn delete_texture(&mut self, handle: ::TextureHandle<GlResources>) {
        let name = handle.get_name();
        unsafe {
            self.gl.DeleteTextures(1, &name);
        }
    }

    fn delete_sampler(&mut self, handle: ::SamplerHandle<GlResources>) {
        let name = handle.get_name();
        unsafe {
            self.gl.DeleteSamplers(1, &name);
        }
    }

    fn update_buffer_raw(&mut self, buffer: ::BufferHandle<GlResources, ()>, data: &[u8],
                         offset_bytes: usize) {
        debug_assert!(offset_bytes + data.len() <= buffer.get_info().size);
        self.update_sub_buffer(buffer.get_name(), data.as_ptr(), data.len(),
                               offset_bytes)
    }

    fn update_texture_raw(&mut self, texture: &::TextureHandle<GlResources>,
                          img: &::tex::ImageInfo, data: &[u8])
                          -> Result<(), ::tex::TextureError> {
        tex::update_texture(&self.gl, texture.get_info().kind,
                            texture.get_name(), img, data.as_ptr(), data.len())
    }

    fn generate_mipmap(&mut self, texture: &::TextureHandle<GlResources>) {
        tex::generate_mipmap(&self.gl, texture.get_info().kind, texture.get_name());
    }

    fn map_buffer_raw(&mut self, buf: BufferHandle<GlResources, ()>, access: MapAccess) -> RawMapping {
        let ptr;
        unsafe { self.gl.BindBuffer(gl::ARRAY_BUFFER, buf.get_name()) };
        ptr = unsafe { self.gl.MapBuffer(gl::ARRAY_BUFFER, match access {
            MapAccess::Readable => gl::READ_ONLY,
            MapAccess::Writable => gl::WRITE_ONLY,
            MapAccess::RW => gl::READ_WRITE
        }) } as *mut libc::c_void;
        RawMapping {
            pointer: ptr,
            target: gl::ARRAY_BUFFER
        }
    }

    fn unmap_buffer_raw(&mut self, map: RawMapping) {
        unsafe { self.gl.UnmapBuffer(map.target) };
    }

    fn map_buffer_readable<T: Copy>(&mut self, buf: BufferHandle<GlResources, T>) -> ReadableMapping<T, GlDevice> {
        let map = self.map_buffer_raw(buf.cast(), MapAccess::Readable);
        ReadableMapping {
            raw: map,
            len: buf.len(),
            device: self
        }
    }

    fn map_buffer_writable<T: Copy>(&mut self, buf: BufferHandle<GlResources, T>) -> WritableMapping<T, GlDevice> {
        let map = self.map_buffer_raw(buf.cast(), MapAccess::Writable);
        WritableMapping {
            raw: map,
            len: buf.len(),
            device: self
        }
    }

    fn map_buffer_rw<T: Copy>(&mut self, buf: BufferHandle<GlResources, T>) -> RWMapping<T, GlDevice> {
        let map = self.map_buffer_raw(buf.cast(), MapAccess::RW);
        RWMapping {
            raw: map,
            len: buf.len(),
            device: self
        }
    }
}
