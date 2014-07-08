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

extern crate gl;
extern crate libc;

use log;
use std;
use a = super::attrib;
use std::fmt;
use std::str;
use std::collections::HashSet;

mod rast;
mod shade;

pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;
pub type Sampler        = gl::types::GLuint;

fn get_uint(name: gl::types::GLenum) -> uint {
    let mut value = 0 as gl::types::GLint;
    unsafe { gl::GetIntegerv(name, &mut value) };
    value as uint
}

unsafe fn get_static_string(name: gl::types::GLenum) -> &'static str {
    let ptr = gl::GetString(name) as *const i8;
    debug_assert!(!ptr.is_null());
    str::raw::c_str_to_static_slice(ptr)
}

#[deriving(Eq, PartialEq)]
pub enum Version {
    Version(uint, uint, Option<uint>, &'static str),
    VersionUnknown(&'static str),
}

impl Version {
    fn parse(src: &'static str) -> Version {
        let (version, vendor_info) = src.find(' ').map_or((src, ""), |i| {
            (src.slice_to(i), src.slice_from(i + 1))
        });

        let mut it = version.split('.');
        let major = it.next().and_then(|x| from_str(x));
        let minor = it.next().and_then(|x| from_str(x));
        let revision = it.next().and_then(|x| from_str(x));
        let tail = it.next();

        match (major, minor, revision, tail) {
            (Some(major), Some(minor), revision, None) =>
                Version(major, minor, revision, vendor_info),
            _ => VersionUnknown(src),
        }
    }

    pub fn is_unknown(&self) -> bool {
        match *self { VersionUnknown(_) => true, _ => false }
    }
}

impl fmt::Show for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Version(major, minor, Some(revision), "") =>
                write!(f, "Version({}.{}.{})", major, minor, revision),
            Version(major, minor, None, "") =>
                write!(f, "Version({}.{})", major, minor),
            Version(major, minor, Some(revision), vendor_info) =>
                write!(f, "Version({}.{}.{}, {})", major, minor, revision, vendor_info),
            Version(major, minor, None, vendor_info) =>
                write!(f, "Version({}.{}, {})", major, minor, vendor_info),
            VersionUnknown(data) =>
                write!(f, "VersionUnknown({})", data),
        }
    }
}

#[deriving(Show)]
pub struct Info {
    pub vendor: &'static str,
    pub renderer: &'static str,
    pub version: Version,
    pub shading_language: Version,
    pub extensions: HashSet<&'static str>,
}

impl Info {
    fn new() -> Info {
        let num_exts = get_uint(gl::NUM_EXTENSIONS) as gl::types::GLuint;
        let info = unsafe {
            Info {
                vendor: get_static_string(gl::VENDOR),
                renderer: get_static_string(gl::RENDERER),
                version: Version::parse(get_static_string(gl::VERSION)),
                shading_language: Version::parse(get_static_string(gl::SHADING_LANGUAGE_VERSION)),
                extensions: range(0, num_exts).map(|i| {
                    str::raw::c_str_to_static_slice(
                        gl::GetStringi(gl::EXTENSIONS, i) as *const i8,
                    )
                }).collect(),
            }
        };
        info!("Vendor: {}", info.vendor);
        info!("Renderer: {}", info.renderer);
        info!("Version: {}", info.version);
        info!("Shading Language: {}", info.shading_language);
        info!("Loaded Extensions:")
        for extension in info.extensions.iter() {
            info!("- {}", *extension);
        }
        info
    }

    pub fn is_extension_supported(&self, s: &str) -> bool {
        self.extensions.contains_equiv(&s)
    }
}

pub struct GlBackEnd {
    caps: super::Capabilities,
    info: Info,
}

impl GlBackEnd {
    pub fn new(provider: &super::GlProvider) -> GlBackEnd {
        gl::load_with(|s| provider.get_proc_address(s));
        let info = Info::new();
        let caps = super::Capabilities {
            shader_model: shade::get_model(),
            max_draw_buffers: get_uint(gl::MAX_DRAW_BUFFERS),
            max_texture_size: get_uint(gl::MAX_TEXTURE_SIZE),
            max_vertex_attributes: get_uint(gl::MAX_VERTEX_ATTRIBS),
            uniform_block_supported: info.is_extension_supported("GL_ARB_uniform_buffer_object"),
            array_buffer_supported: info.is_extension_supported("GL_ARB_vertex_array_object"),
        };
        GlBackEnd {
            caps: caps,
            info: info,
        }
    }

    #[allow(dead_code)]
    fn check(&mut self) {
        debug_assert_eq!(gl::GetError(), gl::NO_ERROR);
    }

    pub fn get_info<'a>(&'a self) -> &'a Info {
        &self.info
    }
}

impl super::ApiBackEnd for GlBackEnd {
    fn get_capabilities<'a>(&'a self) -> &'a super::Capabilities {
        &self.caps
    }

    fn create_buffer(&mut self) -> Buffer {
        let mut name = 0 as Buffer;
        unsafe{
            gl::GenBuffers(1, &mut name);
        }
        info!("\tCreated buffer {}", name);
        name
    }

    fn create_array_buffer(&mut self) -> Result<ArrayBuffer, ()> {
        if self.caps.array_buffer_supported {
            let mut name = 0 as ArrayBuffer;
            unsafe{
                gl::GenVertexArrays(1, &mut name);
            }
            info!("\tCreated array buffer {}", name);
            Ok(name)
        } else {
            error!("\tarray buffer creation unsupported, ignored")
            Err(())
        }
    }

    fn create_shader(&mut self, stage: super::shade::Stage, code: super::shade::ShaderSource) -> Result<Shader, super::shade::CreateShaderError> {
        let (name, info) = shade::create_shader(stage, code, self.get_capabilities().shader_model);
        info.map(|info| {
            let level = if name.is_err() { log::ERROR } else { log::WARN };
            log!(level, "\tShader compile log: {}", info);
        });
        name
    }

    fn create_program(&mut self, shaders: &[Shader]) -> Result<super::shade::ProgramMeta, ()> {
        let (meta, info) = shade::create_program(&self.caps, shaders);
        info.map(|info| {
            let level = if meta.is_err() { log::ERROR } else { log::WARN };
            log!(level, "\tProgram link log: {}", info);
        });
        meta
    }

    fn create_frame_buffer(&mut self) -> FrameBuffer {
        let mut name = 0 as FrameBuffer;
        unsafe{
            gl::GenFramebuffers(1, &mut name);
        }
        info!("\tCreated frame buffer {}", name);
        name
    }


    fn update_buffer<T>(&mut self, buffer: Buffer, data: &[T], usage: super::BufferUsage) {
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
        let size = (data.len() * std::mem::size_of::<T>()) as gl::types::GLsizeiptr;
        let raw = data.as_ptr() as *const gl::types::GLvoid;
        let usage = match usage {
            super::UsageStatic  => gl::STATIC_DRAW,
            super::UsageDynamic => gl::DYNAMIC_DRAW,
            super::UsageStream  => gl::STREAM_DRAW,
        };
        unsafe{
            gl::BufferData(gl::ARRAY_BUFFER, size, raw, usage);
        }
    }

    fn process(&mut self, request: super::Request) {
        match request {
            super::CastClear(data) => {
                let mut flags = match data.color {
                    //gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);
                    Some(super::target::Color([r,g,b,a])) => {
                        gl::ClearColor(r, g, b, a);
                        gl::COLOR_BUFFER_BIT
                    },
                    None => 0 as gl::types::GLenum
                };
                data.depth.map(|value| {
                    gl::DepthMask(gl::TRUE);
                    gl::ClearDepth(value as gl::types::GLclampd);
                    flags |= gl::DEPTH_BUFFER_BIT;
                });
                data.stencil.map(|value| {
                    gl::StencilMask(-1);
                    gl::ClearStencil(value as gl::types::GLint);
                    flags |= gl::STENCIL_BUFFER_BIT;
                });
                gl::Clear(flags);
            },
            super::CastBindProgram(program) => {
                gl::UseProgram(program);
            },
            super::CastBindArrayBuffer(array_buffer) => {
                if self.caps.array_buffer_supported {
                    gl::BindVertexArray(array_buffer);
                } else {
                    error!("Ignored unsupported GL Request: {}", request)
                }
            },
            super::CastBindAttribute(slot, buffer, count, el_type, stride, offset) => {
                let gl_type = match el_type {
                    a::Int(_, a::U8, a::Unsigned)  => gl::UNSIGNED_BYTE,
                    a::Int(_, a::U8, a::Signed)    => gl::BYTE,
                    a::Int(_, a::U16, a::Unsigned) => gl::UNSIGNED_SHORT,
                    a::Int(_, a::U16, a::Signed)   => gl::SHORT,
                    a::Int(_, a::U32, a::Unsigned) => gl::UNSIGNED_INT,
                    a::Int(_, a::U32, a::Signed)   => gl::INT,
                    a::Float(_, a::F16) => gl::HALF_FLOAT,
                    a::Float(_, a::F32) => gl::FLOAT,
                    a::Float(_, a::F64) => gl::DOUBLE,
                    _ => {
                        error!("Unsupported element type: {}", el_type);
                        return
                    }
                };
                gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
                let offset = offset as *const gl::types::GLvoid;
                match el_type {
                    a::Int(a::IntRaw, _, _) => unsafe {
                        gl::VertexAttribIPointer(slot as gl::types::GLuint,
                            count as gl::types::GLint, gl_type,
                            stride as gl::types::GLint, offset);
                    },
                    a::Int(sub, _, _) => unsafe {
                        gl::VertexAttribPointer(slot as gl::types::GLuint,
                            count as gl::types::GLint, gl_type,
                            if sub == a::IntNormalized {gl::TRUE} else {gl::FALSE},
                            stride as gl::types::GLint, offset);
                    },
                    a::Float(a::FloatDefault, _) => unsafe {
                        gl::VertexAttribPointer(slot as gl::types::GLuint,
                            count as gl::types::GLint, gl_type, gl::FALSE,
                            stride as gl::types::GLint, offset);
                    },
                    a::Float(a::FloatPrecision, _) => unsafe {
                        gl::VertexAttribLPointer(slot as gl::types::GLuint,
                            count as gl::types::GLint, gl_type,
                            stride as gl::types::GLint, offset);
                    },
                    _ => ()
                }
                gl::EnableVertexAttribArray(slot as gl::types::GLuint);
            },
            super::CastBindIndex(buffer) => {
                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buffer);
            },
            super::CastBindFrameBuffer(frame_buffer) => {
                gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, frame_buffer);
            },
            super::CastBindTarget(target, plane) => {
                let attachment = match target {
                    super::target::TargetColor(index) =>
                        gl::COLOR_ATTACHMENT0 + (index as gl::types::GLenum),
                    super::target::TargetDepth => gl::DEPTH_ATTACHMENT,
                    super::target::TargetStencil => gl::STENCIL_ATTACHMENT,
                    super::target::TargetDepthStencil => gl::DEPTH_STENCIL_ATTACHMENT,
                };
                match plane {
                    super::target::PlaneEmpty => gl::FramebufferRenderbuffer
                        (gl::DRAW_FRAMEBUFFER, attachment, gl::RENDERBUFFER, 0),
                    super::target::PlaneSurface(name) => gl::FramebufferRenderbuffer
                        (gl::DRAW_FRAMEBUFFER, attachment, gl::RENDERBUFFER, name),
                    super::target::PlaneTexture(name, level) => gl::FramebufferTexture
                        (gl::DRAW_FRAMEBUFFER, attachment, name, level as gl::types::GLint),
                    super::target::PlaneTextureLayer(name, level, layer) => gl::FramebufferTextureLayer
                        (gl::DRAW_FRAMEBUFFER, attachment, name, level as gl::types::GLint, layer as gl::types::GLint),
                }
            },
            super::CastBindUniformBlock(program, index, loc, buffer) => {
                gl::UniformBlockBinding(program, index as gl::types::GLuint, loc as gl::types::GLuint);
                gl::BindBufferBase(gl::UNIFORM_BUFFER, loc as gl::types::GLuint, buffer);
            },
            super::CastBindUniform(loc, uniform) => {
                shade::bind_uniform(loc as gl::types::GLint, uniform);
            },
            super::CastPrimitiveState(prim) => {
                rast::bind_primitive(prim);
            },
            super::CastDepthState(depth) => {
                rast::bind_depth(depth);
            },
            super::CastBlendState(blend) => {
                rast::bind_blend(blend);
            },
            super::CastUpdateBuffer(buffer, data) => {
                self.update_buffer(buffer, data.as_slice(), super::UsageDynamic);
            },
            super::CastDraw(start, count) => {
                gl::DrawArrays(gl::TRIANGLES,
                    start as gl::types::GLsizei,
                    count as gl::types::GLsizei);
                self.check();
            },
            super::CastDrawIndexed(start, count) => {
                let offset = start * (std::mem::size_of::<u16>() as u16);
                unsafe {
                    gl::DrawElements(gl::TRIANGLES,
                        count as gl::types::GLsizei,
                        gl::UNSIGNED_SHORT,
                        offset as *const gl::types::GLvoid);
                }
                self.check();
            },
            _ => fail!("Unknown GL request: {}", request)
        }
    }
}
