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

use super::super::shade as s;
use super::super::shade::{BaseType, ContainerType, CreateShaderError, IsArray, IsShadow, IsRect,
                          IsMultiSample, MatrixFormat, SamplerType, Stage, UniformValue};
use super::gl;
use super::info::Version;

use self::StorageType::{
    Var,
    Sampler,
    Unknown,
};

pub fn create_shader(gl: &gl::Gl, stage: s::Stage, data: s::ShaderSource, lang: Version)
        -> (Result<super::Shader, s::CreateShaderError>, Option<String>) {
    let target = match stage {
        Stage::Vertex => gl::VERTEX_SHADER,
        Stage::Geometry => gl::GEOMETRY_SHADER,
        Stage::Fragment => gl::FRAGMENT_SHADER,
    };
    let name = unsafe { gl.CreateShader(target) };
    let data = match data {
        s::ShaderSource { glsl_150: Some(s), .. } if lang >= Version::new(1, 50, None, "") => s,
        s::ShaderSource { glsl_140: Some(s), .. } if lang >= Version::new(1, 40, None, "") => s,
        s::ShaderSource { glsl_130: Some(s), .. } if lang >= Version::new(1, 30, None, "") => s,
        s::ShaderSource { glsl_120: Some(s), .. } if lang >= Version::new(1, 20, None, "") => s,
        _ => return (Err(CreateShaderError::NoSupportedShaderProvided),
                     Some("[gfx-rs] No supported GLSL shader provided!".to_string())),
    };
    unsafe {
        gl.ShaderSource(name, 1,
            &(data.as_ptr() as *const gl::types::GLchar),
            &(data.len() as gl::types::GLint));
        gl.CompileShader(name);
    }
    info!("\tCompiled shader {}", name);

    let status = get_shader_iv(gl, name, gl::COMPILE_STATUS);
    let mut length = get_shader_iv(gl, name, gl::INFO_LOG_LENGTH);

    let log = if length > 0 {
        let mut log = String::with_capacity(length as uint);
        log.grow(length as uint, '\0');
        unsafe {
            gl.GetShaderInfoLog(name, length, &mut length,
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
        Err(CreateShaderError::ShaderCompilationFailed)
    };

    (name, log)
}

pub fn shader_outputs<'a>(code: &'a ::shade::ShaderSource, lang: Version) -> Vec<&'a str> {
    match *code {
        s::ShaderSource { targets: Some(a), .. }
            if lang >= Version::new(1, 30, None, "") => a.iter().map(|&x| x).filter(|&x| x != "").collect(),
        _ => vec![]
    }
}

fn get_shader_iv(gl: &gl::Gl, shader: super::Shader, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetShaderiv(shader, query, &mut iv) };
    iv
}

fn get_program_iv(gl: &gl::Gl, program: super::Program, query: gl::types::GLenum) -> gl::types::GLint {
    let mut iv = 0;
    unsafe { gl.GetProgramiv(program, query, &mut iv) };
    iv
}

enum StorageType {
    Var(BaseType, s::ContainerType),
    Sampler(BaseType, s::SamplerType),
    Unknown,
}

impl StorageType {
    fn new(storage: gl::types::GLenum) -> StorageType {
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
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, '\0');
    range(0, num as gl::types::GLuint).map(|i| {
        let mut length = 0 as gl::types::GLint;
        let mut size = 0 as gl::types::GLint;
        let mut storage = 0 as gl::types::GLenum;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl.GetActiveAttrib(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl.GetAttribLocation(prog, raw as *const gl::types::GLchar)
        };
        let real_name = name.as_slice().slice_to(length as uint).to_string();
        let (base, container) = match StorageType::new(storage) {
            Var(b, c) => (b, c),
            _ => {
                error!("Unrecognized attribute storage: {}", storage);
                (BaseType::F32, ContainerType::Single)
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

fn query_blocks(gl: &gl::Gl, caps: &::Capabilities, prog: super::Program) -> Vec<s::BlockVar> {
    let num = if caps.uniform_block_supported {
        get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_BLOCKS)
    } else {
        0
    };
    range(0, num as gl::types::GLuint).map(|i| {
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
        let mut name = String::with_capacity(size as uint); //includes terminating null
        name.grow(size as uint, '\0');
        let mut actual_name_size = 0;
        unsafe {
            gl.GetActiveUniformBlockName(prog, i, size, &mut actual_name_size,
                name.as_slice().as_ptr() as *mut gl::types::GLchar);
            gl.GetActiveUniformBlockiv(prog, i, gl::UNIFORM_BLOCK_DATA_SIZE, &mut size);
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

fn query_parameters(gl: &gl::Gl, caps: &::Capabilities, prog: super::Program) -> (Vec<s::UniformVar>, Vec<s::SamplerVar>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let total_num = get_program_iv(gl, prog, gl::ACTIVE_UNIFORMS);
    let indices: Vec<_> = range(0, total_num as gl::types::GLuint).collect();
    let mut block_indices = Vec::from_elem(total_num as uint, -1 as gl::types::GLint);
    if caps.uniform_block_supported {
        unsafe {
            gl.GetActiveUniformsiv(prog, total_num as gl::types::GLsizei,
                indices.as_slice().as_ptr(), gl::UNIFORM_BLOCK_INDEX,
                block_indices.as_mut_slice().as_mut_ptr());
        }
        //TODO: UNIFORM_IS_ROW_MAJOR
    }
    // prepare the name string
    let max_len = get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_MAX_LENGTH);
    let mut name = String::with_capacity(max_len as uint);
    name.grow(max_len as uint, '\0');
    // walk the indices
    for (&i, _) in indices.iter().zip(block_indices.iter()).filter(|&(_, &b)| b<0) {
        let mut length = 0;
        let mut size = 0;
        let mut storage = 0;
        let loc = unsafe {
            let raw = name.as_slice().as_ptr() as *mut gl::types::GLchar;
            gl.GetActiveUniform(prog, i, max_len, &mut length, &mut size, &mut storage, raw);
            gl.GetUniformLocation(prog, raw as *const gl::types::GLchar)
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

pub fn create_program(gl: &gl::Gl, caps: &::Capabilities, shaders: &[::ShaderHandle], targets: &[&str])
        -> (Result<::ProgramHandle, ()>, Option<String>) {
    let name = unsafe { gl.CreateProgram() };
    for sh in shaders.iter() {
        unsafe { gl.AttachShader(name, sh.get_name()) };
    }

    for (i, target) in range(0, targets.len()).zip(targets.iter()) {
        unsafe { target.with_c_str(|ptr| gl.BindFragDataLocation(name, i as u32, ptr)); };
    }

    unsafe { gl.LinkProgram(name) };
    info!("\tLinked program {}", name);

    for target in targets.iter() {
        info!("\t\tOutput[{}] = '{}'", unsafe { gl.GetFragDataLocation(name, target.to_c_str().as_ptr()) }, target);
    }

    // get info message
    let status = get_program_iv(gl, name, gl::LINK_STATUS);
    let mut length  = get_program_iv(gl, name, gl::INFO_LOG_LENGTH);
    let log = if length > 0 {
        let mut log = String::with_capacity(length as uint);
        log.grow(length as uint, '\0');
        unsafe {
            gl.GetProgramInfoLog(name, length, &mut length,
                log.as_slice().as_ptr() as *mut gl::types::GLchar);
        }
        log.truncate(length as uint);
        Some(log)
    } else {
        None
    };

    let prog = if status != 0 {
        let (uniforms, textures) = query_parameters(gl, caps, name);
        let info = s::ProgramInfo {
            attributes: query_attributes(gl, name),
            uniforms: uniforms,
            blocks: query_blocks(gl, caps, name),
            textures: textures,
        };
        Ok(::Handle(name, info))
    } else {
        Err(())
    };

    (prog, log)
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
