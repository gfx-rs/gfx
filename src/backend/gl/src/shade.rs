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
use core::{self as c, shade as s};
use info::PrivateCaps;
use gl;


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
        s::Stage::Hull => gl::TESS_CONTROL_SHADER,
        s::Stage::Domain => gl::TESS_EVALUATION_SHADER,
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
        Err(s::CreateShaderError::CompilationFailed(log))
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
        use core::shade::{BaseType, ContainerType, TextureType, SamplerType, MatrixFormat};
        use core::shade::IsArray::*;
        use core::shade::IsRect::*;
        use core::shade::IsComparison::*;
        use core::shade::IsMultiSample::*;
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
            gl::SAMPLER_3D                   => Sampler(BaseType::F32, TextureType::D3,            SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE                 => Sampler(BaseType::F32, TextureType::Cube(NoArray), SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE_MAP_ARRAY       => Sampler(BaseType::F32, TextureType::Cube(Array),   SamplerType(NoCompare, NoRect)),
            gl::SAMPLER_CUBE_SHADOW          => Sampler(BaseType::F32, TextureType::Cube(NoArray), SamplerType(Compare,   NoRect)),
            gl::SAMPLER_CUBE_MAP_ARRAY_SHADOW=> Sampler(BaseType::F32, TextureType::Cube(Array),   SamplerType(Compare,   NoRect)),
            gl::SAMPLER_BUFFER               => Sampler(BaseType::F32, TextureType::Buffer,        SamplerType(NoCompare, NoRect)),

            // TODO: int samplers
            gl::INT_SAMPLER_BUFFER           => Sampler(BaseType::I32, TextureType::Buffer,        SamplerType(NoCompare, NoRect)),

            gl::UNSIGNED_INT_SAMPLER_1D                   => Sampler(BaseType::U32, TextureType::D1(NoArray), SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_1D_ARRAY             => Sampler(BaseType::U32, TextureType::D1(Array),   SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_2D                   => Sampler(BaseType::U32, TextureType::D2(NoArray, NoMultiSample), SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_2D_ARRAY             => Sampler(BaseType::U32, TextureType::D2(Array,   NoMultiSample), SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE       => Sampler(BaseType::U32, TextureType::D2(NoArray, MultiSample),   SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_2D_RECT              => Sampler(BaseType::U32, TextureType::D2(NoArray, NoMultiSample), SamplerType(NoCompare, Rect)),
            gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE_ARRAY => Sampler(BaseType::U32, TextureType::D2(Array,   MultiSample),   SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_3D                   => Sampler(BaseType::U32, TextureType::D3,            SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_CUBE                 => Sampler(BaseType::U32, TextureType::Cube(NoArray), SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_CUBE_MAP_ARRAY       => Sampler(BaseType::U32, TextureType::Cube(Array),   SamplerType(NoCompare, NoRect)),
            gl::UNSIGNED_INT_SAMPLER_BUFFER               => Sampler(BaseType::U32, TextureType::Buffer,        SamplerType(NoCompare, NoRect)),

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
        if size != 1 {
            error!("Array [{}] attributes are not supported", size);
        }
        s::AttributeVar {
            name: real_name,
            slot: loc as c::AttributeSlot,
            base_type: base,
            container: container,
        }
    }).filter(|a| !a.name.starts_with("gl_")) // remove built-ins
    .collect()
}

fn query_blocks(gl: &gl::Gl, caps: &c::Capabilities, prog: super::Program,
                block_indices: &[gl::types::GLint], block_offsets: &[gl::types::GLint])
                -> Vec<s::ConstantBufferVar> {
    let num = if caps.constant_buffer_supported {
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

    // Some implementations seem to return the length of the uniform name without
    // null termination. Therefore we allocate an extra byte here.
    let max_len = get_program_iv(gl, prog, gl::ACTIVE_UNIFORM_MAX_LENGTH) + 1;
    let mut el_name = String::with_capacity(max_len as usize);
    el_name.extend(repeat('\0').take(max_len as usize));

    (0 .. num as gl::types::GLuint).zip(bindings.iter()).map(|(idx, &bind)| {
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
            let usage_list = [
                (s::VERTEX,   gl::UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER),
                (s::GEOMETRY, gl::UNIFORM_BLOCK_REFERENCED_BY_GEOMETRY_SHADER),
                (s::PIXEL,    gl::UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER),
            ];
            let mut usage = s::Usage::empty();
            for &(stage, eval) in usage_list.iter() {
                if get_block_iv(gl, prog, idx, eval) != 0 {
                    usage = usage | stage;
                }
            }
            usage
        };

        let total_size = get_block_iv(gl, prog, idx, gl::UNIFORM_BLOCK_DATA_SIZE);

        // if we don't detect any explicit layout bindings in the program, we
        // automatically assign them a binding to their respective block indices
        let slot = if explicit_binding {
            bind
        } else {
            unsafe { gl.UniformBlockBinding(prog, idx, idx); }
            idx
        };

        info!("\t\tBlock[{}] = '{}' of size {}", slot, name, total_size);
        s::ConstantBufferVar {
            name: name,
            slot: slot as c::ConstantBufferSlot,
            size: total_size as usize,
            usage: usage,
            elements: block_indices.iter().zip(block_offsets.iter()).enumerate().filter_map(|(i, (parent, offset))| {
                if *parent == idx as gl::types::GLint {
                    let mut length = 0;
                    let mut size = 0;
                    let mut storage = 0;
                    unsafe {
                        let raw = (&el_name[..]).as_ptr() as *mut gl::types::GLchar;
                        gl.GetActiveUniform(prog, i as gl::types::GLuint, max_len, &mut length, &mut size, &mut storage, raw);
                    };
                    let real_name = el_name[..length as usize].to_string();
                    let (base, container) = match StorageType::new(storage) {
                        StorageType::Var(base, cont) => {
                            info!("\t\t\tElement at {}\t= '{}'\t{:?}\t{:?}", *offset, real_name, base, cont);
                            (base, cont)
                        },
                        _ => {
                            error!("Unrecognized element storage: {}", storage);
                            (s::BaseType::F32, s::ContainerType::Single)
                        },
                    };
                    Some(s::ConstVar {
                        name: real_name,
                        location: *offset as s::Location,
                        count: size as usize,
                        base_type: base,
                        container: container,
                    })
                } else { None }
            }).collect()
        }
    }).collect()
}

fn query_parameters(gl: &gl::Gl, caps: &c::Capabilities, prog: super::Program, usage: s::Usage)
                    -> (Vec<s::ConstVar>, Vec<s::TextureVar>, Vec<s::SamplerVar>, Vec<gl::types::GLint>, Vec<gl::types::GLint>) {
    let mut uniforms = Vec::new();
    let mut textures = Vec::new();
    let mut samplers = Vec::new();
    let total_num = get_program_iv(gl, prog, gl::ACTIVE_UNIFORMS);
    let indices: Vec<_> = (0..total_num as gl::types::GLuint).collect();
    let mut block_indices = vec![-1 as gl::types::GLint; total_num as usize];
    let mut block_offsets = vec![-1 as gl::types::GLint; total_num as usize];
    if caps.constant_buffer_supported {
        unsafe {
            gl.GetActiveUniformsiv(prog, total_num as gl::types::GLsizei,
                (&indices[..]).as_ptr(), gl::UNIFORM_BLOCK_INDEX,
                block_indices.as_mut_ptr());
            gl.GetActiveUniformsiv(prog, total_num as gl::types::GLsizei,
                (&indices[..]).as_ptr(), gl::UNIFORM_OFFSET,
                block_offsets.as_mut_ptr());
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
                    slot: slot as c::ResourceViewSlot,
                    base_type: base,
                    ty: tex_type,
                    usage: usage,
                });
                if tex_type.can_sample() {
                    samplers.push(s::SamplerVar {
                        name: real_name,
                        slot: slot as c::SamplerSlot,
                        ty: samp_type,
                        usage: usage,
                    });
                }
            },
            StorageType::Unknown => {
                error!("Unrecognized uniform storage: {}", storage);
            },
        }
    }
    (uniforms, textures, samplers, block_indices, block_offsets)
}

fn query_outputs(gl: &gl::Gl, prog: super::Program) -> (Vec<s::OutputVar>, bool) {
    use std::ptr;

    let mut out_depth = false;
    let mut num_slots = 0;
    unsafe {
        gl.GetProgramInterfaceiv(prog, gl::PROGRAM_OUTPUT, gl::ACTIVE_RESOURCES, &mut num_slots);
    }
    let mut out = Vec::with_capacity(num_slots as usize);
    for i in 0..num_slots as u32 {
        let mut length = 0;
        unsafe {
            gl.GetProgramResourceiv(prog, gl::PROGRAM_OUTPUT, i, 1, &gl::NAME_LENGTH, 1, ptr::null_mut(), &mut length);
        }

        let mut name = String::with_capacity(length as usize);
        name.extend(repeat('\0').take(length as usize));
        unsafe {
            gl.GetProgramResourceName(prog, gl::PROGRAM_OUTPUT, i, length, ptr::null_mut(),
                                     (&name[..]).as_ptr() as *mut gl::types::GLchar);
        }

        // remove the \0
        name.pop();

        let mut index = 0;
        let mut type_ = 0;
        unsafe {
            gl.GetProgramResourceiv(prog, gl::PROGRAM_OUTPUT, i, 1, &gl::LOCATION, 1, ptr::null_mut(), &mut index);
            gl.GetProgramResourceiv(prog, gl::PROGRAM_OUTPUT, i, 1, &gl::TYPE,     1, ptr::null_mut(), &mut type_);
        }

        // special index reported for GLSL 120 to 140 shaders
        if index == !0 {
            if name.starts_with("gl_FragData") {
                index = (name.chars().nth(12).unwrap() as i32) - ('0' as i32);
                name = format!("Target{}", index);
            }else
            if &name == "gl_FragColor" {
                index = 0;
                name = "Target0".to_owned();
            }else
            if &name == "gl_FragDepth" {
                out_depth = true;
                continue;
            }else {
                warn!("Unhandled GLSL built-in: {}", name);
                continue;
            }
        }

        if let StorageType::Var(base, container) = StorageType::new(type_ as u32) {
            out.push(s::OutputVar{
                name: name,
                slot: index as u8,
                base_type: base,
                container: container,
            });
        }
    }
    (out, out_depth)
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

pub fn create_program(gl: &gl::Gl, caps: &c::Capabilities, private: &PrivateCaps,
                      shaders: &[super::Shader], usage: s::Usage)
                      -> Result<(::Program, s::ProgramInfo), s::CreateProgramError> {
    let name = unsafe { gl.CreateProgram() };
    for &sh in shaders {
        unsafe { gl.AttachShader(name, sh) };
    }

    if !private.program_interface_supported && private.frag_data_location_supported {
        for i in 0..c::MAX_COLOR_TARGETS {
            let color_name = format!("Target{}\0", i);
            unsafe {
                gl.BindFragDataLocation(name, i as u32, (&color_name[..]).as_ptr() as *mut gl::types::GLchar);
            }
         }
    }

    unsafe { gl.LinkProgram(name) };
    info!("\tLinked program {}", name);

    let status = get_program_iv(gl, name, gl::LINK_STATUS);
    let log = get_program_log(gl, name);
    if status != 0 {
        if !log.is_empty() {
            warn!("\tLog: {}", log);
        }

        let (uniforms, textures, samplers, block_indices, block_offsets) =
            query_parameters(gl, caps, name, usage);
        let mut info = s::ProgramInfo {
            vertex_attributes: query_attributes(gl, name),
            globals: uniforms,
            constant_buffers: query_blocks(gl, caps, name, &block_indices, &block_offsets),
            textures: textures,
            unordereds: Vec::new(), //TODO
            samplers: samplers,
            outputs: Vec::new(),
            output_depth: false,
            knows_outputs: false,
        };
        if private.program_interface_supported {
            let (outs, od) = query_outputs(gl, name);
            info.outputs = outs;
            info.output_depth = od;
            info.knows_outputs = true;
        }
        debug!("Program {} reflection: {:?}", name, info);

        Ok((name, info))
    } else {
        Err(log.into())
    }
}

pub fn bind_uniform(gl: &gl::Gl, loc: gl::types::GLint, uniform: s::UniformValue) {
    use core::shade::UniformValue;
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
