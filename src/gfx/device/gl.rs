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

pub type Buffer         = gl::types::GLuint;
pub type ArrayBuffer    = gl::types::GLuint;
pub type Shader         = gl::types::GLuint;
pub type Program        = gl::types::GLuint;

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

    pub fn create_shader(&self, kind: char, data: &[u8]) -> Shader {
        let target = match kind {
            'v' => gl::VERTEX_SHADER,
            'g' => gl::GEOMETRY_SHADER,
            'f' => gl::FRAGMENT_SHADER,
            _   => fail!("Unknown shader kind: {}", kind)
        };
        let name = gl::CreateShader(target);
        let mut length = data.len() as gl::types::GLint;
        unsafe {
            gl::ShaderSource(name, 1, &(data.as_ptr() as *gl::types::GLchar), &length);
        }
        gl::CompileShader(name);
        info!("\tCompiled shader {}", name);
        // get info message
        let mut status = 0 as gl::types::GLint;
        length = 0;
        unsafe {
            gl::GetShaderiv(name, gl::COMPILE_STATUS,  &mut status);
            gl::GetShaderiv(name, gl::INFO_LOG_LENGTH, &mut length);
        }
        let mut info = String::with_capacity(length as uint);
        info.grow(length as uint, 0u8 as char);
        unsafe {
            gl::GetShaderInfoLog(name, length, &mut length,
                info.as_slice().as_ptr() as *mut gl::types::GLchar);
        }
        info.truncate(length as uint);
        if status == 0  {
            error!("Failed shader code:\n{}\n", std::str::from_utf8(data).unwrap());
            fail!("GLSL: {}", info);
        }
        name
    }

    /// Shader Program

    fn query_program_int(&self, prog: Program, query: gl::types::GLenum) -> gl::types::GLint {
        let mut ret = 0 as gl::types::GLint;
        unsafe {
            gl::GetProgramiv(prog, query, &mut ret);
        }
        ret
    }

    pub fn create_program(&self, shaders: &[Shader]) -> Program {
        let name = gl::CreateProgram();
        for &sh in shaders.iter() {
            gl::AttachShader(name, sh);
        }
        gl::LinkProgram(name);
        info!("\tLinked program {}", name);
        //info!("\tLinked program {} from objects {}", h, shaders);
        // get info message
        let status      = self.query_program_int(name, gl::LINK_STATUS);
        let mut length  = self.query_program_int(name, gl::INFO_LOG_LENGTH);
        let mut info = String::with_capacity(length as uint);
        info.grow(length as uint, 0u8 as char);
        unsafe {
            gl::GetProgramInfoLog(name, length, &mut length,
                info.as_slice().as_ptr() as *mut gl::types::GLchar);
        }
        info.truncate(length as uint);
        if status == 0  {
            error!("GL error {}", gl::GetError());
            fail!("GLSL program error: {}", info)
        }
        name
    }

    pub fn bind_program(&self, program: Program) {
        gl::UseProgram(program);
    }

    /// Draw

    pub fn draw(&self, start: u32, count: u32) {
        gl::DrawArrays(gl::TRIANGLES,
            start as gl::types::GLsizei,
            count as gl::types::GLsizei);
    }
}
