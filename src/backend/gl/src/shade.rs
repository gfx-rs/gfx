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
use gfx_core as d;
use gfx_core::shade as s;
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

fn get_block_iv(gl: &gl::Gl, name: super::Program, index: gl::types::GLuint,
                query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetActiveUniformBlockiv(name, index, query, &mut iv) };
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
        s::Stage::Vertex => gl::VERTEX_SHADER,
        s::Stage::Geometry => gl::GEOMETRY_SHADER,
        s::Stage::Pixel => gl::FRAGMENT_SHADER,
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
        Err(s::CreateShaderError::ShaderCompilationFailed(log))
    }
}

#[derive(Copy, Clone, Debug)]
enum StorageType {
    Var(s::BaseType, s::ContainerType),
    Sampler(s::BaseType, s::TextureType, s::SamplerType),
    Unknown,
}

impl StorageType {
    fn new(storage: gl::types::GLenum) -> StorageType {
        use gfx_core::shade::{BaseType, ContainerType, TextureType, SamplerType, MatrixFormat};
        use gfx_core::shade::IsArray::*;
        use gfx_core::shade::IsRect::*;
        use gfx_core::shade::IsComparison::*;
        use gfx_core::shade::IsMultiSample::*;
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

            gl::SAMPLER_1D                   => Sampler(BaseType::F32, TextureType::D1(NoArray), SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_1D_ARRAY             => Sampler(BaseType::F32, TextureType::D1(Array),   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_1D_SHADOW            => Sampler(BaseType::F32, TextureType::D1(NoArray), SamplerType(Compare,   NoRect)),
            gl::SAMPLER_1D_ARRAY_SHADOW      => Sampler(BaseType::F32, TextureType::D1(Array),   SamplerType(Compare,   NoRect)),

            gl::SAMPLER_2D                   => Sampler(BaseType::F32, TextureType::D2(NoArray, NoMultiSample), SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_2D_ARRAY             => Sampler(BaseType::F32, TextureType::D2(Array,   NoMultiSample), SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_2D_SHADOW            => Sampler(BaseType::F32, TextureType::D2(NoArray, NoMultiSample), SamplerType(Compare,   NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE       => Sampler(BaseType::F32, TextureType::D2(NoArray, MultiSample),   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_2D_RECT              => Sampler(BaseType::F32, TextureType::D2(NoArray, NoMultiSample), SamplerType(NoCompare, Rect)),
            gl::SAMPLER_2D_ARRAY_SHADOW      => Sampler(BaseType::F32, TextureType::D2(Array,   NoMultiSample), SamplerType(Compare,   NoRect)),
            gl::SAMPLER_2D_MULTISAMPLE_ARRAY => Sampler(BaseType::F32, TextureType::D2(Array,   MultiSample),   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_2D_RECT_SHADOW       => Sampler(BaseType::F32, TextureType::D2(NoArray, NoMultiSample), SamplerType(Compare,   Rect)),

            gl::SAMPLER_3D                   => Sampler(BaseType::F32, TextureType::D3,   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE                 => Sampler(BaseType::F32, TextureType::Cube(NoArray), SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE_MAP_ARRAY       => Sampler(BaseType::F32, TextureType::Cube(Array),   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE_SHADOW          => Sampler(BaseType::F32, TextureType::Cube(NoArray), SamplerType(Compare,   NoRect)),
            gl::SAMPLER_CUBE_MAP_ARRAY_SHADOW=> Sampler(BaseType::F32, TextureType::Cube(Array),   SamplerType(Compare,   NoRect)),

            gl::INT_SAMPLER_BUFFER           => Sampler(BaseType::I32, TextureType::Buffer,        SamplerType(NoCompare, NoRect)),

            // TODO: int samplers

            // TODO: unsigned samplers

            _ => Unknown,
        }
    }
}

fn query_attributes(gl: &gl::Gl, prog: super::Program) -> Vec<s::AttributeVar> {
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
                (s::BaseType::F32, s::ContainerType::Single)
            }
        };
        // we expect only built-ins to have location -1
        if loc == -1 && !real_name.starts_with("gl_") {
            error!("Invalid location {} for attribute {}", loc, real_name);
        }
        info!("\t\tAttrib[{}] = {:?}\t{:?}\t{:?}", loc, real_name, base, container);
        s::AttributeVar {
            name: real_name,
            slot: loc as d::AttributeSlot,
            count: size as usize,
            base_type: base,
            container: container,
        }
    }).filter(|a| !a.name.starts_with("gl_")) // remove built-ins
    .collect()
}

fn query_blocks(gl: &gl::Gl, caps: &d::Capabilities, prog: super::Program) -> Vec<s::ConstantBufferVar> {
    let num = if caps.uniform_block_supported {
        get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_BLOCKS)
    } else {
        0
    };

    let bindings: Vec<gl::types::GLuint> = (0..num).map(
        |idx| get_block_iv(gl, prog, idx as gl::types::GLuint, gl::UNIFORM_BLOCK_BINDING) as gl::types::GLuint
    ).collect();
    
    // check if the shader specifies binding points manually via
    // `layout(binding = n)`
    let explicit_binding = bindings.iter().any(|&i| i > 0);

    (0..num as gl::types::GLuint).zip(bindings.iter()).map(|(idx, &bind)| {
        // the string identifier for the block
        let name = unsafe {
            let size = get_block_iv(gl, prog, idx, gl::UNIFORM_BLOCK_NAME_LENGTH);
            let mut name = String::with_capacity(size as usize);
            name.extend(repeat('\0').take(size as usize));

            let mut real_size = 0;
            gl.GetActiveUniformBlockName(prog, idx, size, &mut real_size,
                (&name[..]).as_ptr() as *mut gl::types::GLchar);
            name.truncate(real_size as usize);
            name
        };

        let usage = {
            let mut usage = 0;

            for (stage, &eval) in [gl::UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER,
                    gl::UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER].iter().enumerate() {
                if get_block_iv(gl, prog, idx, eval) != 0 {
                    usage |= 1 << stage;
                }
            }

            usage
        };

        let size = get_block_iv(gl, prog, idx, gl::UNIFORM_BLOCK_DATA_SIZE);

        // if we don't detect any explicit layout bindings in the program, we
        // automatically assign them a binding to their respective block indices
        let slot = if explicit_binding {
            bind
        } else {
            unsafe { gl.UniformBlockBinding(prog, idx, idx); }
            idx
        };

        info!("\t\tBlock[{}] = '{}' of size {}", slot, name, size);
        s::ConstantBufferVar {
            name: name,
            slot: slot as d::ConstantBufferSlot,
            size: size as usize,
            usage: usage,
        }
    }).collect()
}

fn query_parameters(gl: &gl::Gl, caps: &d::Capabilities, prog: super::Program)
                    -> (Vec<s::ConstVar>, Vec<s::TextureVar>, Vec<s::SamplerVar>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let mut samplers = Vec::new();
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
    let mut texture_slot = 0;
    unsafe { gl.UseProgram(prog); } //TODO: passive mode
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
                info!("\t\tUniform[{}] = '{}'\t{:?}\t{:?}", loc, real_name, base, container);
                uniforms.push(s::ConstVar {
                    name: real_name,
                    location: loc as s::Location,
                    count: size as usize,
                    base_type: base,
                    container: container,
                });
            },
            StorageType::Sampler(base, tex_type, samp_type) => {
                let slot = texture_slot;
                texture_slot += 1;
                unsafe {
                    gl.Uniform1i(loc, slot as gl::types::GLint);
                }
                //TODO: detect the texture slot instead of trying to set it up
                info!("\t\tSampler[{}] = '{}'\t{:?}\t{:?}", slot, real_name, base, tex_type);
                textures.push(s::TextureVar {
                    name: real_name.clone(),
                    slot: slot as d::ResourceViewSlot,
                    base_type: base,
                    ty: tex_type,
                });
                if tex_type.can_sample() {
                    samplers.push(s::SamplerVar {
                        name: real_name,
                        slot: slot as d::SamplerSlot,
                        ty: samp_type,
                    });
                }
            },
            StorageType::Unknown => {
                error!("Unrecognized uniform storage: {}", storage);
            },
        }
    }
    (uniforms, textures, samplers)
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

pub fn create_program(gl: &gl::Gl, caps: &d::Capabilities, shaders: &[super::Shader])
                      -> Result<(::Program, s::ProgramInfo), s::CreateProgramError> {
    let name = unsafe { gl.CreateProgram() };
    for &sh in shaders {
        unsafe { gl.AttachShader(name, sh) };
    }

    unsafe { gl.LinkProgram(name) };
    info!("\tLinked program {}", name);

    let status = get_program_iv(gl, name, gl::LINK_STATUS);
    let log = get_program_log(gl, name);
    if status != 0 {
        if !log.is_empty() {
            warn!("\tLog: {}", log);
        }
        let (uniforms, textures, samplers) = query_parameters(gl, caps, name);
        let info = s::ProgramInfo {
            vertex_attributes: query_attributes(gl, name),
            globals: uniforms,
            constant_buffers: query_blocks(gl, caps, name),
            textures: textures,
            unordereds: Vec::new(), //TODO
            samplers: samplers,
            outputs: Vec::new(),
            knows_outputs: false,
        };
        Ok((name, info))
    } else {
        Err(log)
    }
}

pub fn bind_uniform(gl: &gl::Gl, loc: gl::types::GLint, uniform: s::UniformValue) {
    use gfx_core::shade::UniformValue;
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
