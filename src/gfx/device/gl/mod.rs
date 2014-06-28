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

use std;
use platform::GlProvider;

mod shade;

pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;
pub type FrameBuffer    = gl::types::GLuint;
pub type Surface        = gl::types::GLuint;
pub type Texture        = gl::types::GLuint;

pub struct Device;


impl Device {
    pub fn new(provider: &GlProvider) -> Device {
        gl::load_with(|s| provider.get_proc_address(s));
        Device
    }

    fn check(&self) {
        assert_eq!(gl::GetError(), gl::NO_ERROR);
    }

    pub fn clear(&self, color: &[f32]) {
        gl::ClearColor(color[0], color[1], color[2], color[3]);
        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
    }

    /// Buffer

    pub fn create_buffer<T>(&self, data: &[T]) -> Buffer {
        let mut name = 0 as Buffer;
        unsafe{
            gl::GenBuffers(1, &mut name);
        }
        gl::BindBuffer(gl::ARRAY_BUFFER, name);
        info!("\tCreated buffer {}", name);
        let size = (data.len() * std::mem::size_of::<T>()) as gl::types::GLsizeiptr;
        let raw = data.as_ptr() as *gl::types::GLvoid;
        unsafe{
            gl::BufferData(gl::ARRAY_BUFFER, size, raw, gl::STATIC_DRAW);
        }
        name
    }

    pub fn bind_vertex_buffer(&self, buffer: Buffer) {
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
    }

    /// Vertex Array Buffer

    pub fn create_array_buffer(&self) -> ArrayBuffer {
        let mut name = 0 as ArrayBuffer;
        unsafe{
            gl::GenVertexArrays(1, &mut name);
        }
        info!("\tCreated array buffer {}", name);
        name
    }

    pub fn bind_array_buffer(&self, vao: ArrayBuffer) {
        gl::BindVertexArray(vao);
    }

    pub fn bind_attribute(&self, slot: u8, count: u32, offset: u32, stride: u32) {
        unsafe{
            gl::VertexAttribPointer(slot as gl::types::GLuint,
                count as gl::types::GLint, gl::FLOAT, gl::FALSE,
                stride as gl::types::GLint, offset as *gl::types::GLvoid);
        }
        gl::EnableVertexAttribArray(slot as gl::types::GLuint);
    }

    /// Shader Object

    pub fn create_shader(&self, stage: super::shade::Stage, data: &[u8]) -> Shader {
        let (name_opt, info) = shade::create_object(stage, data);
        if info.len() != 0 {
            warn!("\tObject compile log: {}", info);
        }
        name_opt.unwrap_or(0)
    }

    /// Shader Program

    pub fn create_program(&self, shaders: &[Shader]) -> Program {
        let (meta_opt, info) = shade::create_program(shaders);
        if info.len() != 0 {
            warn!("\tProgram link log: {}", info);
        }
        match meta_opt {
            Some(meta) => meta.name,
            None => 0,
        }
    }

    pub fn bind_program(&self, program: Program) {
        gl::UseProgram(program);
    }

    /// Frame Buffer

    pub fn bind_frame_buffer(&self, fbo: FrameBuffer) {
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
    }

    /// Draw

    pub fn draw(&self, start: u32, count: u32) {
        gl::DrawArrays(gl::TRIANGLES,
            start as gl::types::GLsizei,
            count as gl::types::GLsizei);
    }
}
