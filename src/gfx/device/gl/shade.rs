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
use std::cell::Cell;


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

enum ParseResult {
    Var(common::BaseType, common::ContainerType),
    Sampler(common::BaseType, common::SamplerType),
    Unknown,
}

fn parse_storage(storage: gl::types::GLenum) -> ParseResult {
    match storage {
        // float vecs
        gl::FLOAT =>
            Var(common::BaseFloat, common::Single),
        gl::FLOAT_VEC2 | gl::FLOAT_VEC3 | gl::FLOAT_VEC4 =>
            Var(common::BaseFloat, common::Vector((storage+2-gl::FLOAT_VEC2) as u8)),
        // int vecs
        gl::INT =>
            Var(common::BaseInt, common::Single),
        gl::INT_VEC2 | gl::INT_VEC3 | gl::INT_VEC4 =>
            Var(common::BaseInt, common::Vector((storage+2-gl::INT_VEC2) as u8)),
        // unsigned vecs
        gl::UNSIGNED_INT =>
            Var(common::BaseUnsigned, common::Single),
        gl::UNSIGNED_INT_VEC2 | gl::UNSIGNED_INT_VEC3 | gl::UNSIGNED_INT_VEC4 =>
            Var(common::BaseUnsigned, common::Vector((storage+2-gl::UNSIGNED_INT_VEC2) as u8)),
        // bool vecs
        gl::BOOL =>
            Var(common::BaseBool, common::Single),
        gl::BOOL_VEC2 | gl::BOOL_VEC3 | gl::BOOL_VEC4 =>
            Var(common::BaseBool, common::Vector((storage+2-gl::BOOL_VEC2) as u8)),
        // float matrices
        gl::FLOAT_MAT2 | gl::FLOAT_MAT3 | gl::FLOAT_MAT4 => {
            let dim = (storage+2-gl::FLOAT_MAT2) as u8;
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, dim, dim))
        },
        gl::FLOAT_MAT2x3 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 2, 3)),
        gl::FLOAT_MAT2x4 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 2, 4)),
        gl::FLOAT_MAT3x2 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 3, 2)),
        gl::FLOAT_MAT3x4 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 3, 4)),
        gl::FLOAT_MAT4x2 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 4, 2)),
        gl::FLOAT_MAT4x3 =>
            Var(common::BaseFloat, common::Matrix(common::ColumnMajor, 4, 3)),
        // double matrices //TODO
        _ => Unknown
    }
}

fn query_attributes(prog: super::Program) -> Vec<common::Attribute> {
    let num     = query_program_int(prog, gl::ACTIVE_ATTRIBUTES);
    let max_len = query_program_int(prog, gl::ACTIVE_ATTRIBUTE_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, 0u8 as char);
    range(0, num as gl::types::GLuint).map(|i| {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl::GetActiveAttrib(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl::GetAttribLocation(prog, raw as *gl::types::GLchar)
        };
        let real_name = name.as_slice().slice_to(length as uint).to_string();
        let (base, container) = match parse_storage(storage) {
            Var(b, c) => (b, c),
            _ => fail!("Unrecognized attribute storage: {}", storage)
        };
        info!("\t\tAttrib[{}] = '{}',\tbase = {}, container = {}",
            loc, real_name, base, container);
        common::Attribute {
            name: real_name,
            location: loc as uint,
            count: size as uint,
            base_type: base,
            container: container,
        }
    }).collect()
}

fn query_blocks(prog: super::Program) -> Vec<common::BlockVar> {
    let num     = query_program_int(prog, gl::ACTIVE_UNIFORM_BLOCKS);
    range(0, num as gl::types::GLuint).map(|i| {
        let mut length  = 0 as gl::types::GLint;
        let mut size    = 0 as gl::types::GLint;
        let mut tmp     = 0 as gl::types::GLint;
        let mut usage = 0u8;
        unsafe {
            gl::GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_NAME_LENGTH, &mut size);
            for (stage, &eval) in [gl::UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER,
                    gl::UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER].iter().enumerate() {
                gl::GetActiveUniformBlockiv(prog, i, eval, &mut tmp);
                if tmp != 0 {usage |= 1<<stage;}
            }
        }
        let mut name = String::with_capacity(size as uint); //includes terminating null
        name.grow(size as uint, 0u8 as char);
        let mut actual_name_size = 0 as gl::types::GLint;
        unsafe {
            gl::GetActiveUniformBlockName(prog, i, size, &mut actual_name_size,
                name.as_slice().as_ptr() as *mut gl::types::GLchar);
            gl::GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_DATA_SIZE, &mut size);
        }
        name.truncate(actual_name_size as uint);
        info!("\t\tBlock '{}' of size {}", name, size);
        common::BlockVar {
            name: name,
            size: size as uint,
            usage: usage,
            active_slot: Cell::new(0),
        }
    }).collect()
}

fn query_parameters(prog: super::Program) -> (Vec<common::UniformVar>, Vec<common::SamplerVar>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let mut num = 0 as gl::types::GLint;
    // obtain the indices of uniforms in the default block
    unsafe {    //experimental
        gl::GetActiveUniformBlockiv(prog, -1, gl::UNIFORM_BLOCK_ACTIVE_UNIFORMS, &mut num);
    }
    let mut indices = Vec::from_elem(num as uint, 0 as gl::types::GLint);
    unsafe {
        gl::GetActiveUniformBlockiv(prog, -1, gl::UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES,
            indices.as_mut_slice().as_mut_ptr());
    }
    // prepare the name string
    let max_len = query_program_int(prog, gl::ACTIVE_UNIFORM_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, 0u8 as char);
    // walk the indices
    for &id in indices.iter() {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl::GetActiveUniform(prog, id as gl::types::GLuint,
                max_len, &mut length, &mut size, &mut storage, raw);
            gl::GetUniformLocation(prog, raw as *gl::types::GLchar)
        };
        let real_name = name.as_slice().slice_to(length as uint).to_string();
        match parse_storage(storage) {
            Var(base, container) => {
                info!("\t\tUniform[{}] = '{}',\tbase = {}, type = {}",
                    loc, real_name, base, container);
            },
            Sampler(base, sam_type) => {
                info!("\t\tSampler[{}] = '{}',\tbase = {}, type = {}",
                    loc, real_name, base, sam_type);
            },
            Unknown => fail!("Unrecognized uniform storage: {}", storage)
        }
    }
    (uniforms, textures)
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
        let (uniforms, textures) = query_parameters(name);
        let meta = common::ProgramMeta {
            name: name, 
            attributes: query_attributes(name),
            uniforms: uniforms,   //TODO
            blocks: query_blocks(name),     //TODO
            textures: textures,   //TODO
        };
        Some(meta)
    }else {
        None
    }, info)
}
