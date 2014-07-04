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

mod shade;

pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;
pub type Sampler        = gl::types::GLuint;

pub struct Device;

impl Device {
    pub fn new(provider: &super::GlProvider) -> Device {
        gl::load_with(|s| provider.get_proc_address(s));
        Device
    }

    #[allow(dead_code)]
    fn check(&mut self) {
        debug_assert_eq!(gl::GetError(), gl::NO_ERROR);
    }
}

impl super::DeviceTask for Device {
    fn create_shader(&mut self, stage: super::shade::Stage, code: &[u8]) -> Result<Shader, ()> {
        let (name, info) = shade::create_shader(stage, code);
        info.map(|info| {
            let level = if name.is_err() { log::ERROR } else { log::WARN };
            log!(level, "\tShader compile log: {}", info);
        });
        name
    }

    fn create_program(&mut self, shaders: &[Shader]) -> Result<super::shade::ProgramMeta, ()> {
        let (meta, info) = shade::create_program(shaders);
        info.map(|info| {
            let level = if meta.is_err() { log::ERROR } else { log::WARN };
            log!(level, "\tProgram link log: {}", info);
        });
        meta
    }

    fn create_array_buffer(&mut self) -> ArrayBuffer {
        let mut name = 0 as ArrayBuffer;
        unsafe{
            gl::GenVertexArrays(1, &mut name);
        }
        info!("\tCreated array buffer {}", name);
        name
    }

    fn create_buffer(&mut self) -> Buffer {
        let mut name = 0 as Buffer;
        unsafe{
            gl::GenBuffers(1, &mut name);
        }
        info!("\tCreated buffer {}", name);
        name
    }

    fn update_buffer<T>(&mut self, buffer: Buffer, data: &[T], usage: super::BufferUsage) {
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
        let size = (data.len() * std::mem::size_of::<T>()) as gl::types::GLsizeiptr;
        let raw = data.as_ptr() as *const gl::types::GLvoid;
        let usage = match usage {
            super::UsageStatic => gl::STATIC_DRAW,
            super::UsageDynamic => gl::DYNAMIC_DRAW,
        };
        unsafe{
            gl::BufferData(gl::ARRAY_BUFFER, size, raw, usage);
        }
    }

    fn process(&mut self, request: super::Request) {
        match request {
            super::CastClear(color) => {
                let super::target::Color([r,g,b,a]) = color;
                gl::ClearColor(r, g, b, a);
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
            },
            super::CastBindProgram(program) => {
                gl::UseProgram(program);
            },
            super::CastBindArrayBuffer(array_buffer) => {
                gl::BindVertexArray(array_buffer);
            },
            super::CastBindAttribute(slot, buffer, count, offset, stride) => {
                gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
                unsafe{
                    gl::VertexAttribPointer(slot as gl::types::GLuint,
                        count as gl::types::GLint, gl::FLOAT, gl::FALSE,
                        stride as gl::types::GLint, offset as *const gl::types::GLvoid);
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
