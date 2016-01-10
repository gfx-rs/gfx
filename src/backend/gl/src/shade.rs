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

use std::iter::repeat;
use std::ffi::CString;

use gfx::device as d;
use gfx::device::shade as s;
use gfx::device::shade::{BaseType, ContainerType, CreateShaderError,
                         IsArray, IsShadow, IsRect, IsMultiSample, MatrixFormat,
                         SamplerType, Stage, UniformValue};
use super::gl;


fn get_shader_iv(gl: &gl::Gl, name: super::Shader, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetShaderiv(name, query, &mut iv) };
    iv
}

fn get_program_iv(gl: &gl::Gl, name: super::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetProgramiv(name, query, &mut iv) };
    iv
}

pub fn get_shader_log(gl: &gl::Gl, name: super::Shader) -> String {
    let mut length = get_shader_iv(gl, name, gl::INFO_LOG_LENGTH);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(repeat('\0').take(length as usize));
        unsafe {
            gl.GetShaderInfoLog(name, length, &mut length,
                (&log[..]).as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as usize);
        log
    } else {
        String::new()
    }
}

pub fn create_shader(gl: &gl::Gl, stage: s::Stage, data: &[u8])
                     -> Result<super::Shader, s::CreateShaderError> {
    let target = match stage {
        Stage::Vertex => gl::VERTEX_SHADER,
        Stage::Geometry => gl::GEOMETRY_SHADER,
        Stage::Pixel => gl::FRAGMENT_SHADER,
    };
    let name = unsafe { gl.CreateShader(target) };
    unsafe {
        gl.ShaderSource(name, 1,
            &(data.as_ptr() as *const gl::types::GLchar),
            &(data.len() as gl::types::GLint));
        gl.CompileShader(name);
    }
    info!("\tCompiled shader {}", name);

    let status = get_shader_iv(gl, name, gl::COMPILE_STATUS);
    let log = get_shader_log(gl, name);
    if status != 0 {
        if !log.is_empty() {
            warn!("\tLog: {}", log);
        }
        Ok(name)
    }else {
        Err(CreateShaderError::ShaderCompilationFailed(log))
    }
}

#[derive(Copy, Clone, Debug)]
enum StorageType {
    Var(BaseType, s::ContainerType),
    Sampler(BaseType, s::SamplerType),
    Unknown,
}

impl StorageType {
    fn new(storage: gl::types::GLenum) -> StorageType {
        use self::StorageType::*;
        match storage {
            gl::FLOAT                        => Var(BaseType::F32,  ContainerType::Single),
            gl::FLOAT_VEC2                   => Var(BaseType::F32,  ContainerType::Vector(2)),
            gl::FLOAT_VEC3                   => Var(BaseType::F32,  ContainerType::Vector(3)),
            gl::FLOAT_VEC4                   => Var(BaseType::F32,  ContainerType::Vector(4)),

            gl::INT                          => Var(BaseType::I32,  ContainerType::Single),
            gl::INT_VEC2                     => Var(BaseType::I32,  ContainerType::Vector(2)),
            gl::INT_VEC3                     => Var(BaseType::I32,  ContainerType::Vector(3)),
            gl::INT_VEC4                     => Var(BaseType::I32,  ContainerType::Vector(4)),

            gl::UNSIGNED_INT                 => Var(BaseType::U32,  ContainerType::Single),
            gl::UNSIGNED_INT_VEC2            => Var(BaseType::U32,  ContainerType::Vector(2)),
            gl::UNSIGNED_INT_VEC3            => Var(BaseType::U32,  ContainerType::Vector(3)),
            gl::UNSIGNED_INT_VEC4            => Var(BaseType::U32,  ContainerType::Vector(4)),

            gl::BOOL                         => Var(BaseType::Bool, ContainerType::Single),
            gl::BOOL_VEC2                    => Var(BaseType::Bool, ContainerType::Vector(2)),
            gl::BOOL_VEC3                    => Var(BaseType::Bool, ContainerType::Vector(3)),
            gl::BOOL_VEC4                    => Var(BaseType::Bool, ContainerType::Vector(4)),

            gl::FLOAT_MAT2                   => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 2, 2)),
            gl::FLOAT_MAT3                   => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 3, 3)),
            gl::FLOAT_MAT4                   => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 4, 4)),
            gl::FLOAT_MAT2x3                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 2, 3)),
            gl::FLOAT_MAT2x4                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 2, 4)),
            gl::FLOAT_MAT3x2                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 3, 2)),
            gl::FLOAT_MAT3x4                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 3, 4)),
            gl::FLOAT_MAT4x2                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 4, 2)),
            gl::FLOAT_MAT4x3                 => Var(BaseType::F32,  ContainerType::Matrix(MatrixFormat::ColumnMajor, 4, 3)),

            // TODO: double matrices

            gl::SAMPLER_1D                   => Sampler(BaseType::F32, SamplerType::Sampler1D(IsArray::NoArray, IsShadow::NoShadow)),
            gl::SAMPLER_1D_ARRAY             => Sampler(BaseType::F32, SamplerType::Sampler1D(IsArray::Array,   IsShadow::NoShadow)),
            gl::SAMPLER_1D_SHADOW            => Sampler(BaseType::F32, SamplerType::Sampler1D(IsArray::NoArray, IsShadow::Shadow)),
            gl::SAMPLER_1D_ARRAY_SHADOW      => Sampler(BaseType::F32, SamplerType::Sampler1D(IsArray::Array,   IsShadow::Shadow)),

            gl::SAMPLER_2D                   => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::NoArray, IsShadow::NoShadow, IsMultiSample::NoMultiSample, IsRect::NoRect)),
            gl::SAMPLER_2D_ARRAY             => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::Array,   IsShadow::NoShadow, IsMultiSample::NoMultiSample, IsRect::NoRect)),
            gl::SAMPLER_2D_SHADOW            => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::NoArray, IsShadow::Shadow,   IsMultiSample::NoMultiSample, IsRect::NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE       => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::NoArray, IsShadow::NoShadow, IsMultiSample::MultiSample,   IsRect::NoRect)),
            gl::SAMPLER_2D_RECT              => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::NoArray, IsShadow::NoShadow, IsMultiSample::NoMultiSample, IsRect::Rect)),
            gl::SAMPLER_2D_ARRAY_SHADOW      => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::Array,   IsShadow::Shadow,   IsMultiSample::NoMultiSample, IsRect::NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE_ARRAY => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::Array,   IsShadow::NoShadow, IsMultiSample::MultiSample,   IsRect::NoRect)),
            gl::SAMPLER_2D_RECT_SHADOW       => Sampler(BaseType::F32, SamplerType::Sampler2D(IsArray::NoArray, IsShadow::Shadow,   IsMultiSample::NoMultiSample, IsRect::Rect)),

            gl::SAMPLER_3D                   => Sampler(BaseType::F32, SamplerType::Sampler3D),
            gl::SAMPLER_CUBE                 => Sampler(BaseType::F32, SamplerType::SamplerCube(IsShadow::NoShadow)),
            gl::SAMPLER_CUBE_SHADOW          => Sampler(BaseType::F32, SamplerType::SamplerCube(IsShadow::Shadow)),

            // TODO: int samplers

            // TODO: unsigned samplers

            _ => Unknown,
        }
    }
}

fn query_attributes(gl: &gl::Gl, prog: super::Program) -> Vec<s::Attribute> {
    let num = get_program_iv(gl, prog, gl::ACTIVE_ATTRIBUTES);
    let max_len = get_program_iv(gl, prog, gl::ACTIVE_ATTRIBUTE_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as usize);
    name.extend(repeat('\0').take(max_len as usize));
    (0..num as gl::types::GLuint).map(|i| {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = (&name[..]).as_ptr() as *mut gl::types::GLchar;
            gl.GetActiveAttrib(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl.GetAttribLocation(prog, raw as *const gl::types::GLchar)
        };
        let real_name = name[..length as usize].to_string();
        let (base, container) = match StorageType::new(storage) {
            StorageType::Var(b, c) => (b, c),
            _ => {
                error!("Unrecognized attribute storage: {}", storage);
                (BaseType::F32, ContainerType::Single)
            }
        };
        // we expect only built-ins to have location -1
        if loc == -1 && !real_name.starts_with("gl_") {
            error!("Invalid location {} for attribute {}", loc, real_name);
        }
        info!("\t\tAttrib[{}] = {:?}\t{:?}\t{:?}", loc, real_name, base, container);
        s::Attribute {
            name: real_name,
            location: loc as usize,
            count: size as usize,
            base_type: base,
            container: container,
        }
    }).filter(|a| !a.name.starts_with("gl_")) // remove built-ins
    .collect()
}

fn query_blocks(gl: &gl::Gl, caps: &d::Capabilities, prog: super::Program) -> Vec<s::BlockVar> {
    let num = if caps.uniform_block_supported {
        get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_BLOCKS)
    } else {
        0
    };
    (0..num as gl::types::GLuint).map(|i| {
        let mut size = 0;
        let mut tmp = 0;
        let mut usage = 0;
        unsafe {
            gl.GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_NAME_LENGTH, &mut size);
            for (stage, &eval) in [gl::UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER,
                    gl::UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER].iter().enumerate() {
                gl.GetActiveUniformBlockiv(prog, i, eval, &mut tmp);
                if tmp != 0 {usage |= 1<<stage;}
            }
        }
        let mut name = String::with_capacity(size as usize); //includes terminating null
        name.extend(repeat('\0').take(size as usize));
        let mut actual_name_size = 0;
        unsafe {
            gl.GetActiveUniformBlockName(prog, i, size, &mut actual_name_size,
                (&name[..]).as_ptr() as *mut gl::types::GLchar);
            gl.GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_DATA_SIZE, &mut size);
        }
        name.truncate(actual_name_size as usize);
        info!("\t\tBlock '{}' of size {}", name, size);
        s::BlockVar {
            name: name,
            size: size as usize,
            usage: usage,
        }
    }).collect()
}

fn query_parameters(gl: &gl::Gl, caps: &d::Capabilities, prog: super::Program)
                    -> (Vec<s::UniformVar>, Vec<s::SamplerVar>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let total_num = get_program_iv(gl, prog, gl::ACTIVE_UNIFORMS);
    let indices: Vec<_> = (0..total_num as gl::types::GLuint).collect();
    let mut block_indices: Vec<gl::types::GLint> = repeat(-1 as gl::types::GLint).take(total_num as usize).collect();
    if caps.uniform_block_supported {
        unsafe {
            gl.GetActiveUniformsiv(prog, total_num as gl::types::GLsizei,
                (&indices[..]).as_ptr(), gl::UNIFORM_BLOCK_INDEX,
                block_indices.as_mut_ptr());
        }
        //TODO: UNIFORM_IS_ROW_MAJOR
    }
    // prepare the name string
    let max_len = get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as usize);
    name.extend(repeat('\0').take(max_len as usize));
    // walk the indices
    for (&i, _) in indices.iter().zip(block_indices.iter()).filter(|&(_, &b)| b<0) {
        let mut length = 0;
        let mut size = 0;
        let mut storage = 0;
        let loc = unsafe {
            let raw = (&name[..]).as_ptr() as *mut gl::types::GLchar;
            gl.GetActiveUniform(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl.GetUniformLocation(prog, raw as *const gl::types::GLchar)
        };
        let real_name = name[..length as usize].to_string();
        if real_name.starts_with("gl_") {
            continue;
        }
        match StorageType::new(storage) {
            StorageType::Var(base, container) => {
                info!("\t\tUniform[{}] = {:?}\t{:?}\t{:?}", loc, real_name, base, container);
                uniforms.push(s::UniformVar {
                    name: real_name,
                    location: loc as usize,
                    count: size as usize,
                    base_type: base,
                    container: container,
                });
            },
            StorageType::Sampler(base, sam_type) => {
                info!("\t\tSampler[{}] = {:?}\t{:?}\t{:?}", loc, real_name, base, sam_type);
                textures.push(s::SamplerVar {
                    name: real_name,
                    location: loc as usize,
                    base_type: base,
                    sampler_type: sam_type,
                });
            },
            StorageType::Unknown => {
                error!("Unrecognized uniform storage: {}", storage);
            },
        }
    }
    (uniforms, textures)
}

pub fn get_program_log(gl: &gl::Gl, name: super::Program) -> String {
    let mut length  = get_program_iv(gl, name, gl::INFO_LOG_LENGTH);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(repeat('\0').take(length as usize));
        unsafe {
            gl.GetProgramInfoLog(name, length, &mut length,
                (&log[..]).as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as usize);
        log
    } else {
        String::new()
    }
}

pub fn create_program(gl: &gl::Gl, caps: &d::Capabilities, targets: Option<&[&str]>,
                      shaders: &[super::Shader])
                      -> Result<(::Program, s::ProgramInfo), s::CreateProgramError> {
    let name = unsafe { gl.CreateProgram() };
    for &sh in shaders {
        unsafe { gl.AttachShader(name, sh) };
    }

    let c_targets = targets.map(|targets| {
        let targets: Vec<CString> = targets.iter().map(|&s| CString::new(s).unwrap()).collect();

        for (i, target) in targets.iter().enumerate() {
            unsafe {
                gl.BindFragDataLocation(name, i as u32,
                    target.as_bytes_with_nul().as_ptr() as *const _);
            }
        }

        targets
    });

    unsafe { gl.LinkProgram(name) };
    info!("\tLinked program {}", name);

    if let (Some(targets), Some(c_targets)) = (targets, c_targets) {
        let unbound = targets.iter()
            .zip(c_targets)
            .map(|(s, target)| (unsafe {
                gl.GetFragDataLocation(name, target.as_bytes_with_nul().as_ptr() as *const _)
                }, s))
            .inspect(|&(loc, s)| info!("\t\tOutput[{}] = {}", loc, s))
            .filter(|&(loc, _)| loc == -1)
            .map(|(_, s)| s.to_string())
            .collect::<Vec<_>>();
        if !unbound.is_empty() {
            return Err(s::CreateProgramError::TargetMismatch(unbound));
        }
    }

    let status = get_program_iv(gl, name, gl::LINK_STATUS);
    let log = get_program_log(gl, name);
    if status != 0 {
        if !log.is_empty() {
            warn!("\tLog: {}", log);
        }
        let (uniforms, textures) = query_parameters(gl, caps, name);
        let info = s::ProgramInfo {
            attributes: query_attributes(gl, name),
            uniforms: uniforms,
            blocks: query_blocks(gl, caps, name),
            textures: textures,
        };
        Ok((name, info))
    } else {
        Err(s::CreateProgramError::LinkFail(log))
    }
}

pub fn bind_uniform(gl: &gl::Gl, loc: gl::types::GLint, uniform: UniformValue) {
    match uniform {
        UniformValue::I32(val) => unsafe { gl.Uniform1i(loc, val) },
        UniformValue::F32(val) => unsafe { gl.Uniform1f(loc, val) },

        UniformValue::I32Vector2(val) => unsafe { gl.Uniform2iv(loc, 1, val.as_ptr()) },
        UniformValue::I32Vector3(val) => unsafe { gl.Uniform3iv(loc, 1, val.as_ptr()) },
        UniformValue::I32Vector4(val) => unsafe { gl.Uniform4iv(loc, 1, val.as_ptr()) },

        UniformValue::F32Vector2(val) => unsafe { gl.Uniform2fv(loc, 1, val.as_ptr()) },
        UniformValue::F32Vector3(val) => unsafe { gl.Uniform3fv(loc, 1, val.as_ptr()) },
        UniformValue::F32Vector4(val) => unsafe { gl.Uniform4fv(loc, 1, val.as_ptr()) },

        UniformValue::F32Matrix2(val) => unsafe{ gl.UniformMatrix2fv(loc, 1, gl::FALSE, val[0].as_ptr()) },
        UniformValue::F32Matrix3(val) => unsafe{ gl.UniformMatrix3fv(loc, 1, gl::FALSE, val[0].as_ptr()) },
        UniformValue::F32Matrix4(val) => unsafe{ gl.UniformMatrix4fv(loc, 1, gl::FALSE, val[0].as_ptr()) },
    }
}
