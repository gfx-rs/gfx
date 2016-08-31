// Copyright 2016 The Gfx-rs Developers.
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

use std::{mem, ptr};
use d3dcompiler;
use dxguid;
use winapi;
use core::{self, shade as s};

pub fn reflect_shader(code: &[u8]) -> *mut winapi::ID3D11ShaderReflection {
    let mut reflection = ptr::null_mut();
    let hr = unsafe {
        d3dcompiler::D3DReflect(code.as_ptr() as *const winapi::VOID,
            code.len() as winapi::SIZE_T, &dxguid::IID_ID3D11ShaderReflection, &mut reflection)
    };
    if winapi::SUCCEEDED(hr) {
        reflection as *mut winapi::ID3D11ShaderReflection
    }else {
        panic!("Shader reflection failed with code {:x}", hr);
    }
}

fn convert_str(pchar: *const i8) -> String {
    use std::ffi::CStr;
    unsafe {
        CStr::from_ptr(pchar).to_string_lossy().into_owned()
    }
}

fn map_base_type_from_component(ct: winapi::D3D_REGISTER_COMPONENT_TYPE) -> s::BaseType {
    match ct {
        winapi::D3D_REGISTER_COMPONENT_UINT32 => s::BaseType::U32,
        winapi::D3D_REGISTER_COMPONENT_SINT32 => s::BaseType::I32,
        winapi::D3D_REGISTER_COMPONENT_FLOAT32 => s::BaseType::F32,
        winapi::D3D_REGISTER_COMPONENT_TYPE(t) => {
            error!("Unknown register component type {} detected!", t);
            s::BaseType::F32
        }
    }
}

fn map_base_type_from_return(rt: winapi::D3D_RESOURCE_RETURN_TYPE) -> s::BaseType {
    match rt {
        winapi::D3D_RETURN_TYPE_UINT => s::BaseType::U32,
        winapi::D3D_RETURN_TYPE_SINT => s::BaseType::I32,
        winapi::D3D_RETURN_TYPE_FLOAT => s::BaseType::F32,
        winapi::D3D_RESOURCE_RETURN_TYPE(t) => {
            error!("Unknown return type {} detected!", t);
            s::BaseType::F32
        }
    }
}

fn map_texture_type(tt: winapi::D3D_SRV_DIMENSION) -> s::TextureType {
    use winapi::*;
    use core::shade::IsArray::*;
    use core::shade::IsMultiSample::*;
    match tt {
        D3D_SRV_DIMENSION_BUFFER            => s::TextureType::Buffer,
        D3D_SRV_DIMENSION_TEXTURE1D         => s::TextureType::D1(NoArray),
        D3D_SRV_DIMENSION_TEXTURE1DARRAY    => s::TextureType::D1(Array),
        D3D_SRV_DIMENSION_TEXTURE2D         => s::TextureType::D2(NoArray, NoMultiSample),
        D3D_SRV_DIMENSION_TEXTURE2DARRAY    => s::TextureType::D2(Array, NoMultiSample),
        D3D_SRV_DIMENSION_TEXTURE2DMS       => s::TextureType::D2(NoArray, MultiSample),
        D3D_SRV_DIMENSION_TEXTURE2DMSARRAY  => s::TextureType::D2(Array, MultiSample),
        D3D_SRV_DIMENSION_TEXTURE3D         => s::TextureType::D3,
        D3D_SRV_DIMENSION_TEXTURECUBE       => s::TextureType::Cube(NoArray),
        D3D_SRV_DIMENSION_TEXTURECUBEARRAY  => s::TextureType::Cube(Array),
        D3D_SRV_DIMENSION(t) => {
            error!("Unknow texture dimension {}", t);
            s::TextureType::Buffer
        }
    }
}

fn map_container(stype: &winapi::D3D11_SHADER_TYPE_DESC) -> s::ContainerType {
    use core::shade::Dimension as Dim;
    //TODO: use `match` when winapi allows
    if stype.Class == winapi::D3D_SVC_SCALAR {
        s::ContainerType::Single
    } else if stype.Class == winapi::D3D_SVC_VECTOR {
        s::ContainerType::Vector(stype.Columns as Dim)
    } else if stype.Class == winapi::D3D_SVC_MATRIX_ROWS {
        s::ContainerType::Matrix(s::MatrixFormat::RowMajor, stype.Rows as Dim, stype.Columns as Dim)
    } else if stype.Class == winapi::D3D_SVC_MATRIX_COLUMNS {
        s::ContainerType::Matrix(s::MatrixFormat::ColumnMajor, stype.Rows as Dim, stype.Columns as Dim)
    } else  {
        error!("Unexpected type to classify as container: {:?}", stype);
        s::ContainerType::Single
    }
}

fn map_base_type(_svt: winapi::D3D_SHADER_VARIABLE_TYPE) -> s::BaseType {
    s::BaseType::F32 //TODO
}

pub fn populate_info(info: &mut s::ProgramInfo, stage: s::Stage,
                     reflection: *mut winapi::ID3D11ShaderReflection) {
    use winapi::{UINT, SUCCEEDED};
    let usage = stage.into();
    let (shader_desc, _feature_level) = unsafe {
        let mut desc = mem::zeroed();
        let mut level = winapi::D3D_FEATURE_LEVEL_9_1;
        (*reflection).GetDesc(&mut desc);
        (*reflection).GetMinFeatureLevel(&mut level);
        (desc, level)
    };
    fn mask_to_vector(mask: u8) -> s::ContainerType {
        s::ContainerType::Vector(match mask {
            0...1 => 1,
            2...3 => 2,
            4...7 => 3,
            _ => 4,
        })
    }
    if stage == s::Stage::Vertex {
        // record vertex attributes
        for i in 0 .. shader_desc.InputParameters {
            let (hr, desc) = unsafe {
                let mut desc = mem::zeroed();
                let hr = (*reflection).GetInputParameterDesc(i as UINT, &mut desc);
                (hr, desc)
            };
            assert!(SUCCEEDED(hr));
            info!("\tAttribute {}, system type {:?}, mask {}, read-write mask {}",
                convert_str(desc.SemanticName), desc.SystemValueType, desc.Mask, desc.ReadWriteMask);
            if desc.SystemValueType != winapi::D3D_NAME_UNDEFINED {
                // system value semantic detected, skipping
                continue
            }
            if desc.Mask == 0 {
                // not used, skipping
                continue
            }
            let name = convert_str(desc.SemanticName);
            if desc.SemanticIndex != 0 {
                error!("Semantic {} has non-zero index {} - not supported by the backend", name, desc.SemanticIndex);
            }
            info.vertex_attributes.push(s::AttributeVar {
                name: name,
                slot: desc.Register as core::AttributeSlot,
                base_type: map_base_type_from_component(desc.ComponentType),
                container: mask_to_vector(desc.Mask),
            });
        }
    }
    if stage == s::Stage::Pixel {
        // record pixel outputs
        for i in 0 .. shader_desc.OutputParameters {
            let (hr, desc) = unsafe {
                let mut desc = mem::zeroed();
                let hr = (*reflection).GetOutputParameterDesc(i as UINT, &mut desc);
                (hr, desc)
            };
            assert!(SUCCEEDED(hr));
            let name = convert_str(desc.SemanticName);
            info!("\tOutput {}, system type {:?}, mask {}, read-write mask {}",
                name, desc.SystemValueType, desc.Mask, desc.ReadWriteMask);
            match desc.SystemValueType {
                winapi::D3D_NAME_TARGET =>
                    info.outputs.push(s::OutputVar {
                        name: format!("Target{}", desc.SemanticIndex), //care!
                        slot: desc.Register as core::ColorSlot,
                        base_type: map_base_type_from_component(desc.ComponentType),
                        container: mask_to_vector(desc.Mask),
                    }),
                winapi::D3D_NAME_DEPTH => info.output_depth = true,
                winapi::D3D_NAME_UNDEFINED =>
                    warn!("Custom PS output semantic is ignored: {}", name),
                _ =>
                    warn!("Unhandled PS output semantic {} of type {:?}", name, desc.SystemValueType),
            }
        }
    }
    // record resources
    for i in 0 .. shader_desc.BoundResources {
        let (hr, res_desc) = unsafe {
            let mut desc = mem::zeroed();
            let hr = (*reflection).GetResourceBindingDesc(i as UINT, &mut desc);
            (hr, desc)
        };
        assert!(SUCCEEDED(hr));
        let name = convert_str(res_desc.Name);
        info!("\tResource {}, type {:?}", name, res_desc.Type);
        if res_desc.Type == winapi::D3D_SIT_CBUFFER {
            if let Some(cb) = info.constant_buffers.iter_mut().find(|cb| cb.name == name) {
                cb.usage = cb.usage | usage;
                continue;
            }
            let cbuf = unsafe {
                (*reflection).GetConstantBufferByName(res_desc.Name)
            };
            let desc = unsafe {
                let mut desc = mem::zeroed();
                let hr = (*cbuf).GetDesc(&mut desc);
                assert!(SUCCEEDED(hr));
                desc
            };
            let mut elements = Vec::new();
            for i in 0 .. desc.Variables {
                let var = unsafe {
                    (*cbuf).GetVariableByIndex(i)
                };
                let var_desc = unsafe {
                    let mut vd = mem::zeroed();
                    let hr1 = (*var).GetDesc(&mut vd);
                    assert!(SUCCEEDED(hr1));
                    vd
                };
                let vtype = unsafe {
                    (*var).GetType()
                };
                let vtype_desc = unsafe {
                    let mut vtd = mem::zeroed();
                    let hr2 = (*vtype).GetDesc(&mut vtd);
                    assert!(SUCCEEDED(hr2));
                    vtd
                };
                let el_name = convert_str(var_desc.Name);
                debug!("\t\tElement at {}\t= '{}'", var_desc.StartOffset, el_name);
                if vtype_desc.Class == winapi::D3D_SVC_STRUCT {
                    let stride = var_desc.Size / vtype_desc.Elements;
                    for j in 0 .. vtype_desc.Members {
                        let member = unsafe {
                            (*vtype).GetMemberTypeByIndex(j)
                        };
                        let mem_name_ptr = unsafe {
                            (*vtype).GetMemberTypeName(j)
                        };
                        let mem_desc = unsafe {
                            let mut mtd = mem::zeroed();
                            let hr3 = (*member).GetDesc(&mut mtd);
                            assert!(SUCCEEDED(hr3));
                            mtd
                        };
                        let mem_name = convert_str(mem_name_ptr); //mem_desc.Name
                        debug!("\t\t\tMember at {}\t= '{}'", mem_desc.Offset, mem_name);
                        let btype = map_base_type(mem_desc.Type);
                        let container = map_container(&mem_desc);
                        for k in 0 .. vtype_desc.Elements {
                            let offset = var_desc.StartOffset + k * stride + mem_desc.Offset;
                            elements.push(s::ConstVar {
                                name: format!("{}[{}].{}", el_name, k, mem_name),
                                location: offset as s::Location,
                                count: mem_desc.Elements as usize,
                                base_type: btype,
                                container: container,
                            })
                        }
                    }
                } else {
                    elements.push(s::ConstVar {
                        name: el_name,
                        location: var_desc.StartOffset as s::Location,
                        count: vtype_desc.Elements as usize,
                        base_type: map_base_type(vtype_desc.Type),
                        container: map_container(&vtype_desc),
                    })
                }
            }
            info.constant_buffers.push(s::ConstantBufferVar {
                name: name,
                slot: res_desc.BindPoint as core::ConstantBufferSlot,
                size: desc.Size as usize,
                usage: usage,
                elements: elements,
            });
        }else if res_desc.Type == winapi::D3D_SIT_TEXTURE {
            if let Some(t) = info.textures.iter_mut().find(|t| t.name == name) {
                t.usage = t.usage | usage;
                continue;
            }
            info.textures.push(s::TextureVar {
                name: name,
                slot: res_desc.BindPoint as core::ResourceViewSlot,
                base_type: map_base_type_from_return(res_desc.ReturnType),
                ty: map_texture_type(res_desc.Dimension),
                usage: usage,
            });
        }else if res_desc.Type == winapi::D3D_SIT_SAMPLER {
            let name = name.trim_right_matches('_');
            if let Some(s) = info.samplers.iter_mut().find(|s| s.name == name) {
                s.usage = s.usage | usage;
                continue;
            }
            let cmp = if res_desc.uFlags & winapi::D3D_SIF_COMPARISON_SAMPLER.0 != 0 {
                s::IsComparison::Compare
            }else {
                s::IsComparison::NoCompare
            };
            info.samplers.push(s::SamplerVar {
                name: name.to_owned(),
                slot: res_desc.BindPoint as core::SamplerSlot,
                ty: s::SamplerType(cmp, s::IsRect::NoRect),
                usage: usage,
            });
        }else {
            error!("Unsupported resource type {:?} for {}", res_desc.Type, name);
        }
    }
}
