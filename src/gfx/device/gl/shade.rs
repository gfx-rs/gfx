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
    (if status != 0 {Some(name)} else {None}, info)
}


fn query_program_int(prog: super::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut ret = 0 as gl::types::GLint;
    unsafe {
        gl::GetProgramiv(prog, query, &mut ret);
    }
    ret
}

fn derive_attribute(storage: gl::types::GLenum) -> common::VarType {
    match storage {
        // float vecs
        gl::FLOAT =>
            common::Vector(common::BaseFloat, 1),
        gl::FLOAT_VEC2 | gl::FLOAT_VEC3 | gl::FLOAT_VEC4 =>
            common::Vector(common::BaseFloat, (storage+2-gl::FLOAT_VEC2) as u8),
        // int vecs
        gl::INT =>
            common::Vector(common::BaseInt, 1),
        gl::INT_VEC2 | gl::INT_VEC3 | gl::INT_VEC4 =>
            common::Vector(common::BaseInt, (storage+2-gl::INT_VEC2) as u8),
        // unsigned vecs
        gl::UNSIGNED_INT =>
            common::Vector(common::BaseUnsigned, 1),
        gl::UNSIGNED_INT_VEC2 | gl::UNSIGNED_INT_VEC3 | gl::UNSIGNED_INT_VEC4 =>
            common::Vector(common::BaseUnsigned, (storage+2-gl::UNSIGNED_INT_VEC2) as u8),
        // bool vecs
        gl::BOOL =>
            common::Vector(common::BaseBool, 1),
        gl::BOOL_VEC2 | gl::BOOL_VEC3 | gl::BOOL_VEC4 =>
            common::Vector(common::BaseBool, (storage+2-gl::BOOL_VEC2) as u8),
        // float matrices
        gl::FLOAT_MAT2 | gl::FLOAT_MAT3 | gl::FLOAT_MAT4 => {
            let dim = (storage+2-gl::FLOAT_MAT2) as u8;
            common::Matrix(common::ColumnMajor, false, dim, dim)
        },
        gl::FLOAT_MAT2x3 =>
            common::Matrix(common::ColumnMajor, false, 2, 3),
        gl::FLOAT_MAT2x4 =>
            common::Matrix(common::ColumnMajor, false, 2, 4),
        gl::FLOAT_MAT3x2 =>
            common::Matrix(common::ColumnMajor, false, 3, 2),
        gl::FLOAT_MAT3x4 =>
            common::Matrix(common::ColumnMajor, false, 3, 4),
        gl::FLOAT_MAT4x2 =>
            common::Matrix(common::ColumnMajor, false, 4, 2),
        gl::FLOAT_MAT4x3 =>
            common::Matrix(common::ColumnMajor, false, 4, 3),
        // double matrices //TODO
        // unknown
        _ => {
            error!("Unrecognized attribute storage: {}", storage);
            common::Vector(common::BaseFloat, 0)
        }
    }
}

fn query_attributes(prog: super::Program) -> Vec<common::Attribute> {
    let num     = query_program_int(prog, gl::ACTIVE_ATTRIBUTES);
    let max_len = query_program_int(prog, gl::ACTIVE_ATTRIBUTE_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, 0u8 as char);
    range(0, num).map(|i| {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl::GetActiveAttrib(prog, i as gl::types::GLuint,
                max_len, &mut length, &mut size, &mut storage, raw);
            gl::GetAttribLocation(prog, raw as *gl::types::GLchar)
        };
        info!("\t\tAttrib[{}] = '{}',\tformat = 0x{:x}", loc, name, storage);
        common::Attribute {
            name: name.clone(),
            location: loc as uint,
            count: size as uint,
            var_type: derive_attribute(storage),
        }
    }).collect()
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
            attributes: query_attributes(name),
            uniforms: Vec::new(),   //TODO
            blocks: Vec::new(),     //TODO
            textures: Vec::new(),   //TODO
        };
        Some(meta)
    }else {
        None
    }, info)
}
