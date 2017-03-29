// Copyright 2017 The Gfx-rs Developers.
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

use comptr::ComPtr;
use d3d12;
use d3dcompiler;
use dxguid;
use winapi;

use std::ffi;
use std::{mem, ptr};
use std::os::raw::c_void;
use std::collections::BTreeMap;

use core::{self, shade, factory as f};
use core::SubPass;
use core::pso::{self, EntryPoint};
use {data, state, mirror, native};
use {Factory, Resources as R};

impl Factory {
    pub fn create_shader_library(&mut self, shaders: &[(EntryPoint, &[u8])]) -> Result<native::ShaderLib, shade::CreateShaderError> {
        let mut shader_map = BTreeMap::new();
        // TODO: handle entry points with the same name
        for &(entry_point, byte_code) in shaders {
            let mut blob = ComPtr::<winapi::ID3DBlob>::new(ptr::null_mut());
            let hr = unsafe {
                d3dcompiler::D3DCreateBlob(
                    byte_code.len() as u64,
                    blob.as_mut() as *mut *mut _)
            };
            // TODO: error handling

            unsafe {
                ptr::copy(
                    byte_code.as_ptr(),
                    blob.GetBufferPointer() as *mut u8,
                    byte_code.len());
            }
            shader_map.insert(entry_point, blob);
        }
        Ok(native::ShaderLib { shaders: shader_map })
    }

    pub fn create_shader_library_from_hlsl(&mut self, shaders: &[(EntryPoint, shade::Stage, &[u8])]) -> Result<native::ShaderLib, shade::CreateShaderError> {
        let stage_to_str = |stage| {
            match stage {
                shade::Stage::Vertex => "vs_5_0",
                shade::Stage::Pixel => "ps_5_0",
                _ => unimplemented!(),
            }
        };

        let mut shader_map = BTreeMap::new();
        // TODO: handle entry points with the same name
        for &(entry_point, stage, byte_code) in shaders {
            let mut blob = ComPtr::<winapi::ID3DBlob>::new(ptr::null_mut());
            let mut error = ComPtr::<winapi::ID3DBlob>::new(ptr::null_mut());
			let entry = ffi::CString::new(entry_point).unwrap();
            let hr = unsafe {
                d3dcompiler::D3DCompile(
                    byte_code.as_ptr() as *const _,
                    byte_code.len() as u64,
                    ptr::null(),
                    ptr::null(),
                    ptr::null_mut(),
                    entry.as_ptr() as *const _,
                    stage_to_str(stage).as_ptr() as *const i8,
                    1,
                    0,
                    blob.as_mut() as *mut *mut _,
                    error.as_mut() as *mut *mut _) // TODO: error handling
            };

            shader_map.insert(entry_point, blob);
        }
        Ok(native::ShaderLib { shaders: shader_map })
    }
}

impl core::Factory<R> for Factory {
    fn create_renderpass(&mut self) -> native::RenderPass {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self) -> native::PipelineLayout {
        let desc = winapi::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: 0,
            pParameters: ptr::null(),
            NumStaticSamplers: 0,
            pStaticSamplers: ptr::null(),
            Flags: winapi::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };

        let mut signature = ComPtr::<winapi::ID3D12RootSignature>::new(ptr::null_mut());
        let mut signature_raw = ComPtr::<winapi::ID3DBlob>::new(ptr::null_mut());
        let mut error = ComPtr::<winapi::ID3DBlob>::new(ptr::null_mut());

        // TODO: error handling
        unsafe {
            d3d12::D3D12SerializeRootSignature(
                &desc,
                winapi::D3D_ROOT_SIGNATURE_VERSION_1,
                signature_raw.as_mut() as *mut *mut _ ,
                error.as_mut() as *mut *mut _);

            self.inner.CreateRootSignature(
                0,
                signature_raw.GetBufferPointer(),
                signature_raw.GetBufferSize(),
                &dxguid::IID_ID3D12RootSignature,
                signature.as_mut() as *mut *mut _ as *mut *mut c_void);
        }

        native::PipelineLayout { inner: signature }
    }

    fn create_graphics_pipelines<'a>(&mut self, descs: &[(&native::ShaderLib, &native::PipelineLayout, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
        -> Vec<Result<native::GraphicsPipeline, pso::CreationError>>
    {
        descs.iter().map(|&(shader_lib, ref signature, _, ref desc)| {
            let build_shader = |lib: &native::ShaderLib, entry: Option<EntryPoint>| {
                // TODO: better handle case where looking up shader fails
                let shader = entry.and_then(|entry| lib.shaders.get(entry));
                match shader {
                    Some(shader) => {
                        winapi::D3D12_SHADER_BYTECODE {
                            pShaderBytecode: unsafe { (&mut *shader.as_mut_ptr()).GetBufferPointer() as *const _ },
                            BytecodeLength: unsafe { (&mut *shader.as_mut_ptr()).GetBufferSize() as u64 },
                        }
                    }
                    None => {
                        winapi::D3D12_SHADER_BYTECODE {
                            pShaderBytecode: ptr::null(),
                            BytecodeLength: 0,
                        }
                    }
                }
            };

            let vs = build_shader(shader_lib, Some(desc.shader_entries.vertex_shader));
            let ps = build_shader(shader_lib, desc.shader_entries.pixel_shader);
            let gs = build_shader(shader_lib, desc.shader_entries.geometry_shader);
            let ds = build_shader(shader_lib, desc.shader_entries.domain_shader);
            let hs = build_shader(shader_lib, desc.shader_entries.hull_shader);

            // Define input element descriptions
            let mut vs_reflect = mirror::reflect_shader(&vs);
            let input_element_descs = {
                let input_descs = mirror::reflect_input_elements(&mut vs_reflect);

                let mut input_element_descs = Vec::new();
                for (input_desc, attrib) in input_descs.iter().zip(desc.attributes.iter()) {
                    let vertex_buffer_desc = if let Some(buffer_desc) = desc.vertex_buffers.get(attrib.0 as usize) {
                        buffer_desc
                    } else {
                        error!("Couldn't find associated vertex buffer description {:?}", attrib.0);
                        return Err(pso::CreationError);
                    };

                    let slot_class = match vertex_buffer_desc.rate {
                        0 => winapi::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                        _ => winapi::D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                    };

                    input_element_descs.push(winapi::D3D12_INPUT_ELEMENT_DESC {
                        SemanticName: input_desc.semantic_name,
                        SemanticIndex: input_desc.semantic_index,
                        Format: match data::map_format(attrib.1.format, false) {
                            Some(fm) => fm,
                            None => {
                                error!("Unable to find DXGI format for {:?}", attrib.1.format);
                                return Err(core::pso::CreationError);
                            }
                        },
                        InputSlot: input_desc.input_slot,
                        AlignedByteOffset: attrib.1.offset,
                        InputSlotClass: slot_class,
                        InstanceDataStepRate: vertex_buffer_desc.rate as u32,
                    });
                }

                input_element_descs
            };

            //
            let (rtvs, num_rtvs) = {
                let mut rtvs = [winapi::DXGI_FORMAT_UNKNOWN; 8];
                let mut num_rtvs = 0;
                for (mut rtv, target) in rtvs.iter_mut().zip(desc.color_targets.iter()) {
                    match *target {
                        Some((format, _)) => {
                            *rtv = data::map_format(format, true)
                                    .unwrap_or(winapi::DXGI_FORMAT_UNKNOWN);
                            num_rtvs += 1;   
                        }
                        None => break,
                    }
                }

                (rtvs, num_rtvs)
            };
            
            // Setup pipeline description
            let pso_desc = winapi::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: signature.inner.as_mut_ptr(), // TODO
                VS: vs, PS: ps, GS: gs, DS: ds, HS: hs,
                StreamOutput: winapi::D3D12_STREAM_OUTPUT_DESC {
                    pSODeclaration: ptr::null(),
                    NumEntries: 0,
                    pBufferStrides: ptr::null(),
                    NumStrides: 0,
                    RasterizedStream: 0,
                },
                BlendState: winapi::D3D12_BLEND_DESC {
                    AlphaToCoverageEnable: winapi::FALSE, // TODO
                    IndependentBlendEnable: winapi::FALSE, // TODO
                    RenderTarget: state::map_render_targets(&desc.color_targets), // TODO
                },
                SampleMask: winapi::UINT::max_value(),
                RasterizerState: state::map_rasterizer(&desc.rasterizer),
                DepthStencilState: state::map_depth_stencil(
                    &match desc.depth_stencil {
                        Some((_, info)) => info,
                        None => pso::DepthStencilInfo {
                            depth: None,
                            front: None,
                            back: None,
                        }
                    }),
                InputLayout: winapi::D3D12_INPUT_LAYOUT_DESC {
                    pInputElementDescs: input_element_descs.as_ptr(),
                    NumElements: input_element_descs.len() as u32,
                },
                IBStripCutValue: winapi::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
                PrimitiveTopologyType: state::map_primitive_topology(desc.primitive),
                NumRenderTargets: num_rtvs,
                RTVFormats: rtvs,
                DSVFormat: desc.depth_stencil.and_then(|(format, _)| data::map_format(format, true))
                                             .unwrap_or(winapi::DXGI_FORMAT_UNKNOWN),
                SampleDesc: winapi::DXGI_SAMPLE_DESC {
                    Count: 1, // TODO
                    Quality: 0, // TODO
                },
                NodeMask: 0,
                CachedPSO: winapi::D3D12_CACHED_PIPELINE_STATE {
                    pCachedBlob: ptr::null(),
                    CachedBlobSizeInBytes: 0,
                },
                Flags: winapi::D3D12_PIPELINE_STATE_FLAG_NONE,
            };

            // Create PSO
            let mut pipeline = ComPtr::<winapi::ID3D12PipelineState>::new(ptr::null_mut());
            let hr = unsafe {
                self.inner.CreateGraphicsPipelineState(
                    &pso_desc,
                    &dxguid::IID_ID3D12PipelineState,
                    pipeline.as_mut() as *mut *mut _ as *mut *mut c_void)
            };

            if winapi::SUCCEEDED(hr) {
                Ok(native::GraphicsPipeline { inner: pipeline })
            } else {
                Err(pso::CreationError)
            }
        }).collect()
    }

    fn create_compute_pipelines(&mut self) -> Vec<Result<native::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &native::RenderPass,
        color_attachments: &[native::RenderTargetView], depth_stencil_attachments: &[native::DepthStencilView],
        width: u32, height: u32, layers: u32) -> native::FrameBuffer
    {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &native::Image) -> Result<native::RenderTargetView, f::TargetViewError> {
        // TODO: basic implementation only, needs checks and multiple heaps
        let mut handle = winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 };
        unsafe { self.rtv_heap.GetCPUDescriptorHandleForHeapStart(&mut handle) };
        handle.ptr += self.next_rtv as u64 * self.rtv_handle_size;

        // create descriptor
        unsafe {
            self.inner.CreateRenderTargetView(
                image.resource.as_mut_ptr(),
                ptr::null_mut(),
                handle
            );
        }

        let rtv = native::RenderTargetView { handle: handle };
        self.next_rtv += 1;
        Ok(rtv)
    }
}
