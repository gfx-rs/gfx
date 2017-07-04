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

use std::{cmp, ffi, mem, ptr, slice};
use std::os::raw::c_void;
use std::collections::BTreeMap;

use core::{self, buffer, format, image, mapping, memory, pass, shade, factory as f};
use core::pso::{self, EntryPoint};
use {data, state, mirror, native};
use {Factory, Resources as R};


const IMAGE_LEVEL_ALIGNMENT: u32 = 16;
const IMAGE_SLICE_ALIGNMENT: u32 = 64;

#[derive(Debug)]
pub struct UnboundBuffer {
    requirements: memory::MemoryRequirements,
    usage: buffer::Usage,
}

#[derive(Debug)]
pub struct UnboundImage {
    desc: winapi::D3D12_RESOURCE_DESC,
    requirements: memory::MemoryRequirements,
    kind: image::Kind,
    usage: image::Usage,
}

pub struct Mapping {
    //TODO
}

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

    pub fn create_descriptor_heap_impl(device: &mut ComPtr<winapi::ID3D12Device>,
                                       heap_type: winapi::D3D12_DESCRIPTOR_HEAP_TYPE,
                                       shader_visible: bool, capacity: usize)
                                       -> native::DescriptorHeap
    {
        let desc = winapi::D3D12_DESCRIPTOR_HEAP_DESC {
            Type: heap_type,
            NumDescriptors: capacity as u32,
            Flags: if shader_visible {
                winapi::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
            } else {
                winapi::D3D12_DESCRIPTOR_HEAP_FLAG_NONE
            },
            NodeMask: 0,
        };

        let mut heap: *mut winapi::ID3D12DescriptorHeap = ptr::null_mut();
        let mut handle = winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 };
        let descriptor_size = unsafe {
            device.CreateDescriptorHeap(
                &desc,
                &dxguid::IID_ID3D12DescriptorHeap,
                &mut heap as *mut *mut _ as *mut *mut c_void,
            );
            (*heap).GetCPUDescriptorHandleForHeapStart(&mut handle);
            device.GetDescriptorHandleIncrementSize(heap_type) as u64
        };

        native::DescriptorHeap {
            inner: ComPtr::new(heap),
            handle_size: descriptor_size,
            total_handles: capacity as u64,
            start_handle: handle,
        }
    }

    fn update_descriptor_sets_impl<F>(&mut self, writes: &[f::DescriptorSetWrite<R>],
                                      heap_type: winapi::D3D12_DESCRIPTOR_HEAP_TYPE, fun: F)
    where F: Fn(&f::DescriptorWrite<R>, &mut Vec<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>)
    {
        let mut dst_starts = Vec::new();
        let mut dst_sizes = Vec::new();
        let mut src_starts = Vec::new();
        let mut src_sizes = Vec::new();

        for sw in writes.iter() {
            let old_count = src_starts.len();
            fun(&sw.write, &mut src_starts);
            if old_count != src_starts.len() {
                for _ in old_count .. src_starts.len() {
                    src_sizes.push(1);
                }
                dst_starts.push(sw.set.ranges[sw.binding].at(sw.array_offset));
                dst_sizes.push((src_starts.len() - old_count) as u32);
            }
        }

        if !dst_starts.is_empty() {
            unsafe {
                self.inner.CopyDescriptors(
                    dst_starts.len() as u32,
                    dst_starts.as_ptr(),
                    dst_sizes.as_ptr(),
                    src_starts.len() as u32,
                    src_starts.as_ptr(),
                    src_sizes.as_ptr(),
                    heap_type,
                );
            }
        }
    }
}

impl core::Factory<R> for Factory {
    fn create_heap(&mut self, heap_type: &core::HeapType, size: u64) -> native::Heap {
        let mut heap = ptr::null_mut();
        let desc = winapi::D3D12_HEAP_DESC {
            SizeInBytes: size,
            Properties: data::map_heap_properties(heap_type.properties),
            Alignment: 0,
            Flags: winapi::D3D12_HEAP_FLAGS(0),
        };

        assert_eq!(winapi::S_OK, unsafe {
            self.inner.CreateHeap(&desc, &dxguid::IID_ID3D12Heap, &mut heap)
        });

        native::Heap {
            inner: ComPtr::new(heap as *mut _),
            ty: heap_type.clone(),
            size: size,
            //TODO: merge with `map_heap_properties`
            default_state: if !heap_type.properties.contains(memory::CPU_VISIBLE) {
                winapi::D3D12_RESOURCE_STATE_COMMON
            } else if heap_type.properties.contains(memory::WRITE_COMBINED) {
                winapi::D3D12_RESOURCE_STATE_GENERIC_READ
            } else {
                winapi::D3D12_RESOURCE_STATE_COPY_DEST
            },
        }
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> native::RenderPass
    {
        native::RenderPass {}
    }

    fn create_pipeline_layout(&mut self, sets: &[&native::DescriptorSetLayout]) -> native::PipelineLayout {
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

    fn create_graphics_pipelines<'a>(&mut self, descs: &[(&native::ShaderLib, &native::PipelineLayout, core::SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
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

    fn create_compute_pipelines(&mut self, descs: &[(&native::ShaderLib, EntryPoint, &native::PipelineLayout)]) -> Vec<Result<native::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &native::RenderPass,
        color_attachments: &[&native::RenderTargetView], depth_stencil_attachments: &[&native::DepthStencilView],
        width: u32, height: u32, layers: u32) -> native::FrameBuffer
    {
        native::FrameBuffer {
            color: color_attachments.iter().cloned().cloned().collect(),
            depth_stencil: depth_stencil_attachments.iter().cloned().cloned().collect(),
        }
    }

    fn create_sampler(&mut self, info: image::SamplerInfo) -> native::Sampler {
        let handle = self.sampler_pool.alloc_handles(1);

        let op = match info.comparison {
            Some(_) => data::FilterOp::Comparison,
            None => data::FilterOp::Product,
        };
        let desc = winapi::D3D12_SAMPLER_DESC {
            Filter: data::map_filter(info.filter, op),
            AddressU: data::map_wrap(info.wrap_mode.0),
            AddressV: data::map_wrap(info.wrap_mode.1),
            AddressW: data::map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.filter {
                image::FilterMethod::Anisotropic(max) => max as winapi::UINT,
                _ => 0,
            },
            ComparisonFunc: data::map_function(info.comparison.unwrap_or(core::state::Comparison::Always)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.0.into(),
            MaxLOD: info.lod_range.1.into(),
        };

        unsafe {
            self.inner.CreateSampler(&desc, handle);
        }

        native::Sampler{ handle }
    }

    fn create_buffer(&mut self, size: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        let requirements = memory::MemoryRequirements {
            size: size,
            alignment: winapi::D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as u64,
        };
        Ok(UnboundBuffer {
            requirements, usage
        })
    }

    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> memory::MemoryRequirements {
        buffer.requirements
    }

    fn bind_buffer_memory(&mut self, heap: &native::Heap, offset: u64, buffer: UnboundBuffer) -> Result<native::Buffer, buffer::CreationError> {
        if offset + buffer.requirements.size > heap.size {
            return Err(buffer::CreationError::OutOfHeap)
        }

        let mut resource = ptr::null_mut();
        let init_state = heap.default_state; //TODO?
        let desc = winapi::D3D12_RESOURCE_DESC {
            Dimension: winapi::D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: buffer.requirements.size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: winapi::DXGI_FORMAT_UNKNOWN,
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: winapi::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: winapi::D3D12_RESOURCE_FLAGS(0),
        };

        assert_eq!(winapi::S_OK, unsafe {
            self.inner.CreatePlacedResource(
                &*heap.inner as *const _ as *mut _, offset,
                &desc, init_state, ptr::null(),
                &dxguid::IID_ID3D12Resource, &mut resource)
        });
        Ok(native::Buffer {
            resource: ComPtr::new(resource as *mut _),
            size: buffer.requirements.size as u32,
        })
    }

    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError>
    {
        let (width, height, depth, aa) = kind.get_dimensions();
        let dimension = match kind {
            image::Kind::D1(..) |
            image::Kind::D1Array(..) => winapi::D3D12_RESOURCE_DIMENSION_TEXTURE1D,
            image::Kind::D2(..) |
            image::Kind::D2Array(..) => winapi::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            image::Kind::D3(..) |
            image::Kind::Cube(..) |
            image::Kind::CubeArray(..) => winapi::D3D12_RESOURCE_DIMENSION_TEXTURE3D,
        };
        let desc = winapi::D3D12_RESOURCE_DESC {
            Dimension: dimension,
            Alignment: 0,
            Width: width as u64,
            Height: height as u32,
            DepthOrArraySize: cmp::max(1, depth),
            MipLevels: mip_levels as u16,
            Format: match data::map_format(format, false) {
                Some(format) => format,
                None => return Err(image::CreationError::BadFormat),
            },
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: aa.get_num_fragments() as u32,
                Quality: 0,
            },
            Layout: winapi::D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: winapi::D3D12_RESOURCE_FLAGS(0),
        };

        let mut alloc_info = unsafe { mem::zeroed() };
        unsafe {
            self.inner.GetResourceAllocationInfo(0, 1, &desc, &mut alloc_info)
        };

        Ok(UnboundImage {
            desc,
            requirements: memory::MemoryRequirements {
                size: alloc_info.SizeInBytes,
                alignment: alloc_info.Alignment,
            },
            kind,
            usage,
        })
    }

    fn get_image_requirements(&mut self, image: &UnboundImage) -> memory::MemoryRequirements {
        image.requirements
    }

    fn bind_image_memory(&mut self, heap: &native::Heap, offset: u64, image: UnboundImage) -> Result<native::Image, image::CreationError> {
        if offset + image.requirements.size > heap.size {
            return Err(image::CreationError::OutOfHeap)
        }

        let mut resource = ptr::null_mut();
        let init_state = heap.default_state; //TODO?

        assert_eq!(winapi::S_OK, unsafe {
            self.inner.CreatePlacedResource(
                &*heap.inner as *const _ as *mut _, offset,
                &image.desc, init_state, ptr::null(),
                &dxguid::IID_ID3D12Resource, &mut resource)
        });
        Ok(native::Image {
            resource: ComPtr::new(resource as *mut _),
            kind: image.kind,
        })
    }

    fn view_buffer_as_constant(&mut self, buffer: &native::Buffer, offset: usize, size: usize) -> Result<native::ConstantBufferView, f::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &native::Image, format: format::Format) -> Result<native::RenderTargetView, f::TargetViewError> {
        let handle = self.rtv_pool.alloc_handles(1);

        // create descriptor
        unsafe {
            self.inner.CreateRenderTargetView(
                image.resource.as_mut_ptr(),
                ptr::null_mut(),
                handle);
        }

        Ok(native::RenderTargetView { handle })
    }

    fn view_image_as_shader_resource(&mut self, image: &native::Image, format: format::Format) -> Result<native::ShaderResourceView, f::TargetViewError> {
        let handle = self.srv_pool.alloc_handles(1);

        let dimension = match image.kind {
            image::Kind::D1(..) |
            image::Kind::D1Array(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE1D,
            image::Kind::D2(..) |
            image::Kind::D2Array(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE2D,
            image::Kind::D3(..) |
            image::Kind::Cube(..) |
            image::Kind::CubeArray(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE3D,
        };

        let mut desc = winapi::D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: match data::map_format(format, false) {
                Some(format) => format,
                None => return Err(f::TargetViewError::BadFormat),
            },
            ViewDimension: dimension,
            Shader4ComponentMapping: 1<<3 + 2<<6 + 3<<9, //TODO: map swizzle
            u: unsafe { mem::zeroed() },
        };

        match image.kind {
            image::Kind::D2(_, _, image::AaMode::Single) => {
                *unsafe{ desc.Texture2D_mut() } = winapi::D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: !0,
                    PlaneSlice: 0,
                    ResourceMinLODClamp: 0.0,
                }
            }
            _ => unimplemented!()
        }

        unsafe {
            self.inner.CreateShaderResourceView(
                image.resource.as_mut_ptr(),
                &desc,
                handle);
        }

        Ok(native::ShaderResourceView { handle })
    }

    fn view_image_as_unordered_access(&mut self, image: &native::Image, format: format::Format) -> Result<native::UnorderedAccessView, f::TargetViewError> {
        unimplemented!()
    }

    fn create_descriptor_heap(&mut self, ty: f::DescriptorHeapType, size: usize) -> native::DescriptorHeap {
        let native_type = match ty {
            f::DescriptorHeapType::SrvCbvUav => winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            f::DescriptorHeapType::Sampler => winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
        };
        Self::create_descriptor_heap_impl(&mut self.inner, native_type, true, size)
    }

    fn create_descriptor_set_pool(&mut self, heap: &native::DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[f::DescriptorPoolDesc]) -> native::DescriptorSetPool {
        native::DescriptorSetPool {
            heap: heap.clone(),
            pools: descriptor_pools.to_vec(),
            offset: offset as u64,
            size: 0,
            max_size: max_sets as u64,
        }
    }

    fn create_descriptor_set_layout(&mut self, bindings: &[f::DescriptorSetLayoutBinding]) -> native::DescriptorSetLayout {
        native::DescriptorSetLayout { bindings: bindings.to_vec() }
    }

    fn create_descriptor_sets(&mut self, set_pool: &mut native::DescriptorSetPool, layouts: &[&native::DescriptorSetLayout]) -> Vec<native::DescriptorSet> {
        layouts.iter().map(|layout| native::DescriptorSet {
            ranges: layout.bindings.iter().map(|binding| native::DescriptorRange {
                handle: set_pool.alloc_handles(binding.count as u64),
                ty: binding.ty,
                count: binding.count,
                handle_size: set_pool.heap.handle_size,
            }).collect()
        }).collect()
    }

    fn reset_descriptor_set_pool(&mut self, pool: &mut native::DescriptorSetPool) {
        unimplemented!()
    }

    fn update_descriptor_sets(&mut self, writes: &[f::DescriptorSetWrite<R>]) {
        self.update_descriptor_sets_impl(writes,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            |dw, starts| match *dw {
                f::DescriptorWrite::SampledImage(ref images) => {
                    starts.extend(images.iter().map(|&(ref srv, _layout)| srv.handle))
                }
                f::DescriptorWrite::Sampler(_) => (), // done separately
                _ => unimplemented!()
            });

        self.update_descriptor_sets_impl(writes,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            |dw, starts| match *dw {
                f::DescriptorWrite::Sampler(ref samplers) => {
                    starts.extend(samplers.iter().map(|sm| sm.handle))
                }
                _ => ()
            });
    }

    /// Acquire a mapping Reader.
    fn read_mapping<'a, T>(&self, buf: &'a native::Buffer, offset: u64, size: u64)
                               -> Result<mapping::Reader<'a, R, T>, mapping::Error>
        where T: Copy
    {
        unimplemented!()
    }

    /// Acquire a mapping Writer
    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a native::Buffer, offset: u64, size: u64)
                                -> Result<mapping::Writer<'a, R, T>, mapping::Error>
        where T: Copy
    {
        if offset + size > buf.size as u64 {
            return Err(mapping::Error::OutOfBounds);
        }

        let range = winapi::D3D12_RANGE {
            Begin: offset,
            End: offset + size,
        };
        let mut ptr = ptr::null_mut();
        assert_eq!(winapi::S_OK, unsafe {
            buf.resource.clone().Map(0, &range, &mut ptr)
        });
        let count = size as usize / mem::size_of::<T>();

        Ok(unsafe {
            let slice = slice::from_raw_parts_mut(ptr as *mut T, count);
            let mapping = Mapping {};
            mapping::Writer::new(slice, mapping)
        })
    }

    fn create_semaphore(&mut self) -> native::Semaphore {
        unimplemented!()
    }

    fn create_fence(&mut self, signaled: bool) -> native::Fence {
        unimplemented!()
    }

    fn reset_fences(&mut self, fences: &[&native::Fence]) {
        unimplemented!()
    }

    fn destroy_heap(&mut self, heap: native::Heap) {
        unimplemented!()
    }

    fn destroy_shader_lib(&mut self, shader_lib: native::ShaderLib) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, rp: native::RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, pl: native::PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: native::GraphicsPipeline) {
        unimplemented!()
    }

    fn destroy_compute_pipeline(&mut self, pipeline: native::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, fb: native::FrameBuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&mut self, buffer: native::Buffer) {
        unimplemented!()
    }

    fn destroy_image(&mut self, image: native::Image) {
        unimplemented!()
    }

    fn destroy_render_target_view(&mut self, rtv: native::RenderTargetView) {
        unimplemented!()
    }

    fn destroy_depth_stencil_view(&mut self, dsv: native::DepthStencilView) {
        unimplemented!()
    }

    fn destroy_constant_buffer_view(&mut self, _: native::ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, srv: native::ShaderResourceView) {
        unimplemented!()
    }

    fn destroy_unordered_access_view(&mut self, uav: native::UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, sampler: native::Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_heap(&mut self, heap: native::DescriptorHeap) {
        unimplemented!()
    }

    fn destroy_descriptor_set_pool(&mut self, pool: native::DescriptorSetPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, layout: native::DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, fence: native::Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, semaphore: native::Semaphore) {
        unimplemented!()
    }
}
