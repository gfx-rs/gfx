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
use gfx_core as core;
use gfx_core::shade as s;


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
    use gfx_core::shade::IsArray::*;
    use gfx_core::shade::IsMultiSample::*;
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
    if stage == s::Stage::Vertex {
        // record vertex attributes
        for i in 0 .. shader_desc.InputParameters {
            let (hr, desc) = unsafe {
                let mut desc = mem::zeroed();
                let hr = (*reflection).GetInputParameterDesc(i as UINT, &mut desc);
                (hr, desc)
            };
            assert!(SUCCEEDED(hr));
            debug!("Attribute {}, system type {:?}, mask {}, read-write mask {}",
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
                container: s::ContainerType::Vector(4), // how to get it?
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
            debug!("Output {}, system type {:?}, mask {}, read-write mask {}",
                name, desc.SystemValueType, desc.Mask, desc.ReadWriteMask);
            if desc.SystemValueType == winapi::D3D_NAME_TARGET {
                info.outputs.push(s::OutputVar {
                    name: format!("Target{}", desc.SemanticIndex), //care!
                    slot: desc.Register as core::ColorSlot,
                    base_type: map_base_type_from_component(desc.ComponentType),
                    container: s::ContainerType::Vector(4), // how to get it?
                });
            }else
            if desc.SystemValueType == winapi::D3D_NAME_UNDEFINED {
                warn!("Custom PS output semantic is ignored: {}", name)
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
        debug!("Resource {}, type {:?}", name, res_desc.Type);
        if res_desc.Type == winapi::D3D_SIT_CBUFFER {
            if let Some(cb) = info.constant_buffers.iter_mut().find(|cb| cb.name == name) {
                cb.usage = cb.usage | usage;
                continue;
            }
            let desc = unsafe {
                let cbuf = (*reflection).GetConstantBufferByName(res_desc.Name);
                let mut desc = mem::zeroed();
                let hr = (*cbuf).GetDesc(&mut desc);
                assert!(SUCCEEDED(hr));
                desc
            };
            info.constant_buffers.push(s::ConstantBufferVar {
                name: name,
                slot: res_desc.BindPoint as core::ConstantBufferSlot,
                size: desc.Size as usize,
                usage: usage,
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
                name: name,
                slot: res_desc.BindPoint as core::SamplerSlot,
                ty: s::SamplerType(cmp, s::IsRect::NoRect),
                usage: usage,
            });
        }else {
            error!("Unsupported resource type {:?} for {}", res_desc.Type, name);
        }
    }
}
