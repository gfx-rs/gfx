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

use s = super::super::shade;
use super::gl;
use std::cell::Cell;
use std::str::raw;

pub fn get_model() -> s::ShaderModel {
    let bytes = gl::GetString(gl::SHADING_LANGUAGE_VERSION);
    let full = unsafe {
        raw::c_str_to_static_slice(bytes as *const i8)
    };
    info!("GLSL version: {}", full);
    let s = full.slice_to(4);
    if s < "1.20" {
        s::ModelUnsupported
    }else if s < "1.50" {
        s::Model30
    }else if s < "3.00" {
        s::Model40
    }else if s < "4.30" {
        s::Model41
    }else {
        s::Model50
    }
}

pub fn create_shader(stage: s::Stage, data: s::ShaderSource, model: s::ShaderModel)
        -> (Result<super::Shader, s::CreateShaderError>, Option<String>) {
    let target = match stage {
        s::Vertex => gl::VERTEX_SHADER,
        s::Geometry => gl::GEOMETRY_SHADER,
        s::Fragment => gl::FRAGMENT_SHADER,
    };
    let name = gl::CreateShader(target);
    let data = match data {
        s::ShaderSource { glsl_150: Some(ref s), .. } if model >= s::Model40 => s.as_slice(),
        s::ShaderSource { glsl_120: Some(ref s), .. } if model >= s::Model30 => s.as_slice(),
        _ => return (Err(s::NoSupportedShaderProvided),
                     Some("[gfx-rs] No supported GLSL shader provided!".to_string())),
    };
    unsafe {
        gl::ShaderSource(name, 1,
            &(data.as_ptr() as *const gl::types::GLchar),
            &(data.len() as gl::types::GLint));
    }
    gl::CompileShader(name);
    info!("\tCompiled shader {}", name);

    let status = get_shader_iv(name, gl::COMPILE_STATUS);
    let mut length = get_shader_iv(name, gl::INFO_LOG_LENGTH);

    let log = if length > 0 {
        let mut log = String::with_capacity(length as uint);
        log.grow(length as uint, '\0');
        unsafe {
            gl::GetShaderInfoLog(name, length, &mut length,
                log.as_slice().as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as uint);
        Some(log)
    } else {
        None
    };

    let name = if status != 0 {
        Ok(name)
    }else {
        Err(s::ShaderCompilationFailed)
    };

    (name, log)
}

fn get_shader_iv(shader: super::Shader, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl::GetShaderiv(shader, query, &mut iv) };
    iv
}

fn get_program_iv(program: super::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl::GetProgramiv(program, query, &mut iv) };
    iv
}

enum StorageType {
    Var(s::BaseType, s::ContainerType),
    Sampler(s::BaseType, s::SamplerType),
    Unknown,
}

impl StorageType {
    fn new(storage: gl::types::GLenum) -> StorageType {
        match storage {
            gl::FLOAT                        => Var(s::BaseF32, s::Single),
            gl::FLOAT_VEC2                   => Var(s::BaseF32, s::Vector(2)),
            gl::FLOAT_VEC3                   => Var(s::BaseF32, s::Vector(3)),
            gl::FLOAT_VEC4                   => Var(s::BaseF32, s::Vector(4)),

            gl::INT                          => Var(s::BaseI32, s::Single),
            gl::INT_VEC2                     => Var(s::BaseI32, s::Vector(2)),
            gl::INT_VEC3                     => Var(s::BaseI32, s::Vector(3)),
            gl::INT_VEC4                     => Var(s::BaseI32, s::Vector(4)),

            gl::UNSIGNED_INT                 => Var(s::BaseU32, s::Single),
            gl::UNSIGNED_INT_VEC2            => Var(s::BaseU32, s::Vector(2)),
            gl::UNSIGNED_INT_VEC3            => Var(s::BaseU32, s::Vector(3)),
            gl::UNSIGNED_INT_VEC4            => Var(s::BaseU32, s::Vector(4)),

            gl::BOOL                         => Var(s::BaseBool, s::Single),
            gl::BOOL_VEC2                    => Var(s::BaseBool, s::Vector(2)),
            gl::BOOL_VEC3                    => Var(s::BaseBool, s::Vector(3)),
            gl::BOOL_VEC4                    => Var(s::BaseBool, s::Vector(4)),

            gl::FLOAT_MAT2                   => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 2, 2)),
            gl::FLOAT_MAT3                   => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 3, 3)),
            gl::FLOAT_MAT4                   => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 4, 4)),
            gl::FLOAT_MAT2x3                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 2, 3)),
            gl::FLOAT_MAT2x4                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 2, 4)),
            gl::FLOAT_MAT3x2                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 3, 2)),
            gl::FLOAT_MAT3x4                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 3, 4)),
            gl::FLOAT_MAT4x2                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 4, 2)),
            gl::FLOAT_MAT4x3                 => Var(s::BaseF32, s::Matrix(s::ColumnMajor, 4, 3)),

            // TODO: double matrices

            gl::SAMPLER_1D                   => Sampler(s::BaseF32, s::Sampler1D(s::NoArray, s::NoShadow)),
            gl::SAMPLER_1D_ARRAY             => Sampler(s::BaseF32, s::Sampler1D(s::Array,   s::NoShadow)),
            gl::SAMPLER_1D_SHADOW            => Sampler(s::BaseF32, s::Sampler1D(s::NoArray, s::Shadow)),
            gl::SAMPLER_1D_ARRAY_SHADOW      => Sampler(s::BaseF32, s::Sampler1D(s::Array,   s::Shadow)),

            gl::SAMPLER_2D                   => Sampler(s::BaseF32, s::Sampler2D(s::NoArray, s::NoShadow, s::NoMultiSample, s::NoRect)),
            gl::SAMPLER_2D_ARRAY             => Sampler(s::BaseF32, s::Sampler2D(s::Array,   s::NoShadow, s::NoMultiSample, s::NoRect)),
            gl::SAMPLER_2D_SHADOW            => Sampler(s::BaseF32, s::Sampler2D(s::NoArray, s::Shadow,   s::NoMultiSample, s::NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE       => Sampler(s::BaseF32, s::Sampler2D(s::NoArray, s::NoShadow, s::MultiSample,   s::NoRect)),
            gl::SAMPLER_2D_RECT              => Sampler(s::BaseF32, s::Sampler2D(s::NoArray, s::NoShadow, s::NoMultiSample, s::Rect)),
            gl::SAMPLER_2D_ARRAY_SHADOW      => Sampler(s::BaseF32, s::Sampler2D(s::Array,   s::Shadow,   s::NoMultiSample, s::NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE_ARRAY => Sampler(s::BaseF32, s::Sampler2D(s::Array,   s::NoShadow, s::MultiSample,   s::NoRect)),
            gl::SAMPLER_2D_RECT_SHADOW       => Sampler(s::BaseF32, s::Sampler2D(s::NoArray, s::Shadow,   s::NoMultiSample, s::Rect)),

            gl::SAMPLER_3D                   => Sampler(s::BaseF32, s::Sampler3D),
            gl::SAMPLER_CUBE                 => Sampler(s::BaseF32, s::SamplerCube(s::NoShadow)),
            gl::SAMPLER_CUBE_SHADOW          => Sampler(s::BaseF32, s::SamplerCube(s::Shadow)),

            // TODO: int samplers

            // TODO: unsigned samplers

            _ => Unknown,
        }
    }
}

fn query_attributes(prog: super::Program) -> Vec<s::Attribute> {
    let num = get_program_iv(prog, gl::ACTIVE_ATTRIBUTES);
    let max_len = get_program_iv(prog, gl::ACTIVE_ATTRIBUTE_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, '\0');
    range(0, num as gl::types::GLuint).map(|i| {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl::GetActiveAttrib(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl::GetAttribLocation(prog, raw as *const gl::types::GLchar)
        };
        let real_name = name.as_slice().slice_to(length as uint).to_string();
        let (base, container) = match StorageType::new(storage) {
            Var(b, c) => (b, c),
            _ => {
                error!("Unrecognized attribute storage: {}", storage);
                (s::BaseF32, s::Single)
            }
        };
        info!("\t\tAttrib[{}] = '{}'\t{}\t{}", loc, real_name, base, container);
        s::Attribute {
            name: real_name,
            location: loc as uint,
            count: size as uint,
            base_type: base,
            container: container,
        }
    }).collect()
}

fn query_blocks(caps: &super::super::Capabilities, prog: super::Program) -> Vec<s::BlockVar> {
    let num = if caps.uniform_block_supported {
        get_program_iv(prog, gl::ACTIVE_UNIFORM_BLOCKS)
    } else {
        error!("Uniform blocks are not supported, ignored");
        0
    };
    range(0, num as gl::types::GLuint).map(|i| {
        let mut size = 0;
        let mut tmp = 0;
        let mut usage = 0;
        unsafe {
            gl::GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_NAME_LENGTH, &mut size);
            for (stage, &eval) in [gl::UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER,
                    gl::UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER].iter().enumerate() {
                gl::GetActiveUniformBlockiv(prog, i, eval, &mut tmp);
                if tmp != 0 {usage |= 1<<stage;}
            }
        }
        let mut name = String::with_capacity(size as uint); //includes terminating null
        name.grow(size as uint, '\0');
        let mut actual_name_size = 0;
        unsafe {
            gl::GetActiveUniformBlockName(prog, i, size, &mut actual_name_size,
                name.as_slice().as_ptr() as *mut gl::types::GLchar);
            gl::GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_DATA_SIZE, &mut size);
        }
        name.truncate(actual_name_size as uint);
        info!("\t\tBlock '{}' of size {}", name, size);
        s::BlockVar {
            name: name,
            size: size as uint,
            usage: usage,
        }
    }).collect()
}

fn query_parameters(prog: super::Program) -> (Vec<s::UniformVar>, Vec<s::SamplerVar>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let total_num = get_program_iv(prog, gl::ACTIVE_UNIFORMS);
    let indices: Vec<_> = range(0, total_num as gl::types::GLuint).collect();
    let mut block_indices = Vec::from_elem(total_num as uint, 0 as gl::types::GLint);
    unsafe {
        gl::GetActiveUniformsiv(prog, total_num as gl::types::GLsizei,
            indices.as_slice().as_ptr(), gl::UNIFORM_BLOCK_INDEX,
            block_indices.as_mut_slice().as_mut_ptr());
        //TODO: UNIFORM_IS_ROW_MAJOR
    }
    // prepare the name string
    let max_len = get_program_iv(prog, gl::ACTIVE_UNIFORM_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, '\0');
    // walk the indices
    for (&i, _) in indices.iter().zip(block_indices.iter()).filter(|&(_, &b)| b<0) {
        let mut length = 0;
        let mut size = 0;
        let mut storage = 0;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl::GetActiveUniform(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl::GetUniformLocation(prog, raw as *const gl::types::GLchar)
        };
        let real_name = name.as_slice().slice_to(length as uint).to_string();
        match StorageType::new(storage) {
            Var(base, container) => {
                info!("\t\tUniform[{}] = '{}'\t{}\t{}", loc, real_name, base, container);
                uniforms.push(s::UniformVar {
                    name: real_name,
                    location: loc as uint,
                    count: size as uint,
                    base_type: base,
                    container: container,
                });
            },
            Sampler(base, sam_type) => {
                info!("\t\tSampler[{}] = '{}'\t{}\t{}", loc, real_name, base, sam_type);
                textures.push(s::SamplerVar {
                    name: real_name,
                    location: loc as uint,
                    base_type: base,
                    sampler_type: sam_type,
                });
            },
            Unknown => {
                error!("Unrecognized uniform storage: {}", storage);
            },
        }
    }
    (uniforms, textures)
}

pub fn create_program(caps: &super::super::Capabilities, shaders: &[super::Shader])
        -> (Result<::ProgramHandle, ()>, Option<String>) {
    let name = gl::CreateProgram();
    for &sh in shaders.iter() {
        gl::AttachShader(name, sh);
    }
    gl::LinkProgram(name);
    info!("\tLinked program {}", name);

    // get info message
    let status = get_program_iv(name, gl::LINK_STATUS);
    let mut length  = get_program_iv(name, gl::INFO_LOG_LENGTH);
    let log = if length > 0 {
        let mut log = String::with_capacity(length as uint);
        log.grow(length as uint, '\0');
        unsafe {
            gl::GetProgramInfoLog(name, length, &mut length,
                log.as_slice().as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as uint);
        Some(log)
    } else {
        None
    };

    let prog = if status != 0 {
        let (uniforms, textures) = query_parameters(name);
        let info = s::ProgramInfo {
            attributes: query_attributes(name),
            uniforms: uniforms,
            blocks: query_blocks(caps, name),
            textures: textures,
        };
        Ok(::Handle(name, info))
    } else {
        Err(())
    };

    (prog, log)
}

pub fn bind_uniform(loc: gl::types::GLint, uniform: s::UniformValue) {
    match uniform {
        s::ValueI32(val) => gl::Uniform1i(loc, val),
        s::ValueF32(val) => gl::Uniform1f(loc, val),
        s::ValueI32Vec(val) => unsafe { gl::Uniform4iv(loc, 1, val.as_ptr()) },
        s::ValueF32Vec(val) => unsafe { gl::Uniform4fv(loc, 1, val.as_ptr()) },
        s::ValueF32Matrix(val) => unsafe{ gl::UniformMatrix4fv(loc, 1, gl::FALSE, val[0].as_ptr()) },
    }
}
