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

use common = super::super::shade;
use super::gl;


pub fn create_object(stage: common::Stage, data: &[u8]) -> (Option<super::Shader>, String) {
    let target = match stage {
        common::Vertex => gl::VERTEX_SHADER,
        common::Geometry => gl::GEOMETRY_SHADER,
        common::Fragment => gl::FRAGMENT_SHADER,
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
    (if status == 0 {None} else {Some(name)}, info)
}


fn query_program_int(prog: super::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut ret = 0 as gl::types::GLint;
    unsafe {
        gl::GetProgramiv(prog, query, &mut ret);
    }
    ret
}

pub fn create_program(shaders: &[super::Shader]) -> (Option<common::ProgramMeta>, String) {
    let name = gl::CreateProgram();
    for &sh in shaders.iter() {
        gl::AttachShader(name, sh);
    }
    gl::LinkProgram(name);
    info!("\tLinked program {}", name);
    // get info message
    let status      = query_program_int(name, gl::LINK_STATUS);
    let mut length  = query_program_int(name, gl::INFO_LOG_LENGTH);
    let mut info = String::with_capacity(length as uint);
    info.grow(length as uint, 0u8 as char);
    unsafe {
        gl::GetProgramInfoLog(name, length, &mut length,
            info.as_slice().as_ptr() as *mut gl::types::GLchar);
    }
    info.truncate(length as uint);
    (if status != 0 {
        let meta = common::ProgramMeta {
            name: name, 
            attributes: Vec::new(),
            uniforms: Vec::new(),
            blocks: Vec::new(),
            textures: Vec::new(),
        };
        Some(meta)
    }else {
        None
    }, info)
}
