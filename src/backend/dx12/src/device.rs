use std::collections::BTreeMap;
use std::ops::Range;
use std::{ffi, mem, ptr, slice};

use d3d12;
use d3dcompiler;
use dxguid;
use kernel32;
use spirv_cross::{hlsl, spirv, ErrorCode as SpirvErrorCode};
use winapi;
use wio::com::ComPtr;

use hal::{buffer, device as d, format, image, mapping, memory, pass, pso};
use hal::{Features, Limits, MemoryType};
use hal::memory::Requirements;
use hal::pool::CommandPoolCreateFlags;

use {conv, free_list, native as n, shade, Backend as B, Device, QueueFamily};
use pool::RawCommandPool;


/// Emit error during shader module creation. Used if we don't expect an error
/// but might panic due to an exception in SPIRV-Cross.
fn gen_unexpected_error(err: SpirvErrorCode) -> d::ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unexpected error".into(),
    };
    d::ShaderError::CompilationFailed(msg)
}

/// Emit error during shader module creation. Used if we execute an query command.
fn gen_query_error(err: SpirvErrorCode) -> d::ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unknown query error".into(),
    };
    d::ShaderError::CompilationFailed(msg)
}

pub(crate) enum CommandSignature {
    Draw,
    DrawIndexed,
    Dispatch,
}

#[derive(Debug)]
pub struct UnboundBuffer {
    requirements: memory::Requirements,
    stride: u64,
    usage: buffer::Usage,
}

#[derive(Debug)]
pub struct UnboundImage {
    desc: winapi::D3D12_RESOURCE_DESC,
    dsv_format: winapi::DXGI_FORMAT,
    requirements: memory::Requirements,
    kind: image::Kind,
    usage: image::Usage,
    aspects: image::AspectFlags,
    bits_per_texel: u8,
    num_levels: image::Level,
    num_layers: image::Layer,
}

impl Device {
    /// Compile a single shader entry point from a HLSL text shader
    fn compile_shader(
        stage: pso::Stage,
        shader_model: hlsl::ShaderModel,
        entry: &str,
        code: &[u8],
    ) -> Result<*mut winapi::ID3DBlob, d::ShaderError> {
        let stage_to_str = |stage, shader_model| {
            let stage = match stage {
                pso::Stage::Vertex => "vs",
                pso::Stage::Fragment => "ps",
                pso::Stage::Compute => "cs",
                _ => unimplemented!(),
            };

            let model = match shader_model {
                hlsl::ShaderModel::V5_0 => "5_0",
                hlsl::ShaderModel::V5_1 => "5_1",
                hlsl::ShaderModel::V6_0 => "6_0",
                _ => unimplemented!(),
            };

            format!("{}_{}\0", stage, model)
        };

        let mut blob = ptr::null_mut();
        let mut error = ptr::null_mut();
        let entry = ffi::CString::new(entry).unwrap();
        let hr = unsafe {
            d3dcompiler::D3DCompile(
                code.as_ptr() as *const _,
                code.len() as u64,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                entry.as_ptr() as *const _,
                stage_to_str(stage, shader_model).as_ptr() as *const i8,
                1,
                0,
                &mut blob as *mut *mut _,
                &mut error as *mut *mut _)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("D3DCompile error {:x}", hr);
            let mut error = unsafe { ComPtr::<winapi::ID3DBlob>::new(error) };
            let message = unsafe {
                let pointer = error.GetBufferPointer();
                let size = error.GetBufferSize();
                let slice = slice::from_raw_parts(pointer as *const u8, size as usize);
                String::from_utf8_lossy(slice).into_owned()
            };
            Err(d::ShaderError::CompilationFailed(message))
        } else {
            Ok(blob)
        }
    }

    /// Create a shader module from HLSL with a single entry point
    pub fn create_shader_module_from_source(
        &self,
        stage: pso::Stage,
        hlsl_entry: &str,
        entry_point: &str,
        code: &[u8],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        let mut shader_map = BTreeMap::new();
        let blob = Self::compile_shader(stage, hlsl::ShaderModel::V5_1, hlsl_entry, code)?;
        shader_map.insert(entry_point.into(), blob);
        Ok(n::ShaderModule { shaders: shader_map })
    }

    pub(crate) fn create_command_signature(
        device: &mut ComPtr<winapi::ID3D12Device>,
        ty: CommandSignature,
    ) -> ComPtr<winapi::ID3D12CommandSignature> {
        let mut signature = ptr::null_mut();

        let (arg_ty, stride) = match ty {
            CommandSignature::Draw => (
                winapi::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
                16,
            ),
            CommandSignature::DrawIndexed => (
                winapi::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
                20,
            ),
            CommandSignature::Dispatch => (
                winapi::D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
                12,
            ),
        };

        let arg = winapi::D3D12_INDIRECT_ARGUMENT_DESC {
            Type: arg_ty,
            .. unsafe { mem::zeroed() }
        };

        let desc = winapi::D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: stride,
            NumArgumentDescs: 1,
            pArgumentDescs: &arg,
            NodeMask: 0,
        };

        let hr = unsafe {
            device.CreateCommandSignature(
                &desc,
                ptr::null_mut(),
                &dxguid::IID_ID3D12CommandSignature,
                &mut signature as *mut *mut _ as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            error!("error on command signature creation: {:x}", hr);
        }
        unsafe { ComPtr::new(signature) }
    }

    pub(crate) fn create_descriptor_heap_impl(
        device: &mut ComPtr<winapi::ID3D12Device>,
        heap_type: winapi::D3D12_DESCRIPTOR_HEAP_TYPE,
        shader_visible: bool,
        capacity: usize,
    ) -> n::DescriptorHeap {
        assert_ne!(capacity, 0);

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
        let mut cpu_handle = winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 };
        let mut gpu_handle = winapi::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: 0 };
        let descriptor_size = unsafe {
            device.CreateDescriptorHeap(
                &desc,
                &dxguid::IID_ID3D12DescriptorHeap,
                &mut heap as *mut *mut _ as *mut *mut _,
            );
            (*heap).GetCPUDescriptorHandleForHeapStart(&mut cpu_handle);
            (*heap).GetGPUDescriptorHandleForHeapStart(&mut gpu_handle);
            device.GetDescriptorHandleIncrementSize(heap_type) as u64
        };

        let allocator = free_list::Allocator::new(capacity as _);

        n::DescriptorHeap {
            raw: unsafe { ComPtr::new(heap) },
            handle_size: descriptor_size,
            total_handles: capacity as u64,
            start: n::DualHandle {
                cpu: cpu_handle,
                gpu: gpu_handle,
            },
            allocator,
        }
    }

    fn update_descriptor_sets_impl<F>(
        &self,
        writes: &[pso::DescriptorSetWrite<B>],
        heap_type: winapi::D3D12_DESCRIPTOR_HEAP_TYPE,
        mut fun: F,
    ) where
        F: FnMut(&pso::DescriptorWrite<B>, &mut Vec<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>),
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
                let range_binding = &sw.set.ranges[sw.binding as usize];
                let range = match (heap_type, range_binding) {
                    (winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, &n::DescriptorRangeBinding::Sampler(ref range)) => range,
                    (winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, &n::DescriptorRangeBinding::View(ref range)) => range,
                    _ => unreachable!(),
                };
                dst_starts.push(range.at(sw.array_offset));
                dst_sizes.push((src_starts.len() - old_count) as u32);
            }
        }

        if !dst_starts.is_empty() {
            unsafe {
                self.raw.clone().CopyDescriptors(
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

    fn view_image_as_render_target(
        &self,
        resource: *mut winapi::ID3D12Resource,
        kind: image::Kind,
        format: winapi::DXGI_FORMAT,
        range: &image::SubresourceRange,
    ) -> Result<winapi::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        //TODO: use subresource range
        let handle = self.rtv_pool.lock().unwrap().alloc_handles(1).cpu;

        if kind.get_dimensions().3 != image::AaMode::Single {
            error!("No MSAA supported yet!");
        }

        let mut desc = winapi::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: format,
            .. unsafe { mem::zeroed() }
        };

        match kind {
            image::Kind::D2(..) => {
                assert_eq!(range.levels.start + 1, range.levels.end);
                desc.ViewDimension = winapi::D3D12_RTV_DIMENSION_TEXTURE2D;
                *unsafe { desc.Texture2D_mut() } = winapi::D3D12_TEX2D_RTV {
                    MipSlice: range.levels.start as _,
                    PlaneSlice: 0, //TODO
                };
            },
            _ => unimplemented!()
        };

        unsafe {
            self.raw.clone().CreateRenderTargetView(resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_depth_stencil(
        &self,
        resource: *mut winapi::ID3D12Resource,
        kind: image::Kind,
        format: winapi::DXGI_FORMAT,
        range: &image::SubresourceRange,
    ) -> Result<winapi::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        //TODO: use subresource range
        let handle = self.dsv_pool.lock().unwrap().alloc_handles(1).cpu;

        if kind.get_dimensions().3 != image::AaMode::Single {
            error!("No MSAA supported yet!");
        }

        let mut desc = winapi::D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: format,
            .. unsafe { mem::zeroed() }
        };

        match kind {
            image::Kind::D2(..) => {
                assert_eq!(range.levels.start + 1, range.levels.end);
                desc.ViewDimension = winapi::D3D12_DSV_DIMENSION_TEXTURE2D;
                *unsafe { desc.Texture2D_mut() } = winapi::D3D12_TEX2D_DSV {
                    MipSlice: range.levels.start as _,
                };
            },
            _ => unimplemented!()
        };

        unsafe {
            self.raw.clone().CreateDepthStencilView(resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_shader_resource(
        &self,
        resource: *mut winapi::ID3D12Resource,
        kind: image::Kind,
        format: winapi::DXGI_FORMAT,
        range: &image::SubresourceRange,
    ) -> Result<winapi::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        let handle = self.srv_pool.lock().unwrap().alloc_handles(1).cpu;

        let dimension = match kind {
            image::Kind::D1(..) |
            image::Kind::D1Array(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE1D,
            image::Kind::D2(..) |
            image::Kind::D2Array(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE2D,
            image::Kind::D3(..) |
            image::Kind::Cube(..) |
            image::Kind::CubeArray(..) => winapi::D3D12_SRV_DIMENSION_TEXTURE3D,
        };

        let mut desc = winapi::D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: dimension,
            Shader4ComponentMapping: 0x1688, // TODO: map swizzle
            u: unsafe { mem::zeroed() },
        };

        match kind {
            image::Kind::D2(_, _, image::AaMode::Single) => {
                assert_eq!(range.levels.start, 0);
                *unsafe{ desc.Texture2D_mut() } = winapi::D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: range.levels.end as _,
                    PlaneSlice: 0, //TODO
                    ResourceMinLODClamp: 0.0,
                }
            }
            _ => unimplemented!()
        }

        unsafe {
            self.raw.clone().CreateShaderResourceView(resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_storage(
        &self,
        resource: *mut winapi::ID3D12Resource,
        kind: image::Kind,
        format: winapi::DXGI_FORMAT,
        range: &image::SubresourceRange,
    ) -> Result<winapi::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        let handle = self.uav_pool.lock().unwrap().alloc_handles(1).cpu;

        let dimension = match kind {
            image::Kind::D1(..) |
            image::Kind::D1Array(..) => winapi::D3D12_UAV_DIMENSION_TEXTURE1D,
            image::Kind::D2(..) |
            image::Kind::D2Array(..) => winapi::D3D12_UAV_DIMENSION_TEXTURE2D,
            image::Kind::D3(..) |
            image::Kind::Cube(..) |
            image::Kind::CubeArray(..) => winapi::D3D12_UAV_DIMENSION_TEXTURE3D,
        };

        let mut desc = winapi::D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: format,
            ViewDimension: dimension,
            u: unsafe { mem::zeroed() },
        };

        match kind {
            image::Kind::D2(_, _, _) => {
                *unsafe{ desc.Texture2D_mut() } = winapi::D3D12_TEX2D_UAV {
                    MipSlice: range.levels.start as _,
                    PlaneSlice: 0,
                }
            }
            _ => unimplemented!()
        }

        unsafe {
            self.raw.clone().CreateUnorderedAccessView(resource, ptr::null_mut(), &desc, handle);
        }

        Ok(handle)
    }
}

impl d::Device<B> for Device {
    fn get_features(&self) -> &Features { &self.features }
    fn get_limits(&self) -> &Limits { &self.limits }

    fn allocate_memory(
        &self,
        mem_type: &MemoryType,
        size: u64,
    ) -> Result<n::Memory, d::OutOfMemory> {
        let mut heap = ptr::null_mut();

        let properties = winapi::D3D12_HEAP_PROPERTIES {
            Type: winapi::D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: self.heap_properties[mem_type.id % 4].page_property,
            MemoryPoolPreference: self.heap_properties[mem_type.id % 4].memory_pool,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        };

        let desc = winapi::D3D12_HEAP_DESC {
            SizeInBytes: size,
            Properties: properties,
            Alignment: 0, //Warning: has to be 4K for MSAA targets
            Flags: match mem_type.id >> 2 {
                0 => winapi::D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES,
                1 => winapi::D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
                2 => winapi::D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES,
                3 => winapi::D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES,
                _ => unreachable!()
            },
        };

        let hr = unsafe {
            self.raw.clone().CreateHeap(&desc, &dxguid::IID_ID3D12Heap, &mut heap)
        };
        if hr == winapi::E_OUTOFMEMORY {
            return Err(d::OutOfMemory);
        }
        assert_eq!(winapi::S_OK, hr);

        Ok(n::Memory {
            heap: unsafe { ComPtr::new(heap as _) },
            ty: mem_type.clone(),
            size,
        })
    }

    fn create_command_pool(
        &self, family: &QueueFamily, _create_flags: CommandPoolCreateFlags
    ) -> RawCommandPool {
        let list_type = family.native_type();
        // create command allocator
        let mut command_allocator: *mut winapi::ID3D12CommandAllocator = ptr::null_mut();
        let hr = unsafe {
            self.raw.clone().CreateCommandAllocator(
                list_type,
                &dxguid::IID_ID3D12CommandAllocator,
                &mut command_allocator as *mut *mut _ as *mut *mut _,
            )
        };
        // TODO: error handling
        if !winapi::SUCCEEDED(hr) {
            error!("error on command allocator creation: {:x}", hr);
        }

        RawCommandPool {
            inner: unsafe { ComPtr::new(command_allocator) },
            device: self.raw.clone(),
            list_type,
            signatures: self.signatures.clone(),
        }
    }

    fn destroy_command_pool(&self, _pool: RawCommandPool) {
        // automatic
    }

    fn create_render_pass(
        &self,
        attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc],
        dependencies: &[pass::SubpassDependency],
    ) -> n::RenderPass {
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum SubState {
            New(winapi::D3D12_RESOURCE_STATES),
            Preserve,
            Undefined,
        }
        struct AttachmentInfo {
            sub_states: Vec<SubState>,
            target_state: winapi::D3D12_RESOURCE_STATES,
            last_state: winapi::D3D12_RESOURCE_STATES,
            barrier_start_index: usize,
        }

        let mut att_infos = attachments
            .iter()
            .map(|att| AttachmentInfo {
                sub_states: vec![SubState::Undefined; subpasses.len()],
                target_state: if att.format.0.is_depth() {
                    winapi::D3D12_RESOURCE_STATE_DEPTH_WRITE //TODO?
                } else {
                    winapi::D3D12_RESOURCE_STATE_RENDER_TARGET
                },
                last_state: conv::map_image_resource_state(image::Access::empty(), att.layouts.start),
                barrier_start_index: 0,
            })
            .collect::<Vec<_>>();

        // Fill out subpass known layouts
        for (sid, sub) in subpasses.iter().enumerate() {
            for &(id, _layout) in sub.colors {
                let state = SubState::New(att_infos[id].target_state);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &(id, _layout) in sub.depth_stencil {
                let state = SubState::New(att_infos[id].target_state);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &(id, _layout) in sub.inputs {
                let state = SubState::New(winapi::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &id in sub.preserves {
                let old = mem::replace(&mut att_infos[id].sub_states[sid], SubState::Preserve);
                debug_assert_eq!(SubState::Undefined, old);
            }
        }

        let mut deps_left = vec![0u16; subpasses.len()];
        for dep in dependencies {
            //Note: self-dependencies are ignored
            if dep.passes.start != dep.passes.end && dep.passes.start != pass::SubpassRef::External {
                if let pass::SubpassRef::Pass(sid) = dep.passes.end {
                    deps_left[sid] += 1;
                }
            }
        }

        let mut rp = n::RenderPass {
            attachments: attachments.to_vec(),
            subpasses: Vec::new(),
            post_barriers: Vec::new(),
        };

        while let Some(sid) = deps_left.iter().position(|count| *count == 0) {
            deps_left[sid] = !0; // mark as done
            for dep in dependencies {
                if dep.passes.start != dep.passes.end && dep.passes.start == pass::SubpassRef::Pass(sid) {
                    if let pass::SubpassRef::Pass(other) = dep.passes.end {
                        deps_left[other] -= 1;
                    }
                }
            }

            let mut pre_barriers = Vec::new();
            for (att_id, ai) in att_infos.iter_mut().enumerate() {
                let state_dst = match ai.sub_states[sid] {
                    SubState::Preserve => {
                        ai.barrier_start_index = rp.subpasses.len() + 1;
                        continue;
                    },
                    SubState::New(state) if state != ai.last_state => state,
                    _ => continue,
                };
                let barrier = n::BarrierDesc::new(att_id, ai.last_state .. state_dst);
                match rp.subpasses.get_mut(ai.barrier_start_index) {
                    Some(past_subpass) => {
                        let split = barrier.split();
                        past_subpass.pre_barriers.push(split.start);
                        pre_barriers.push(split.end);
                    },
                    None => pre_barriers.push(barrier),
                }
                ai.last_state = state_dst;
                ai.barrier_start_index = rp.subpasses.len() + 1;
            }

            rp.subpasses.push(n::SubpassDesc {
                color_attachments: subpasses[sid].colors.iter().cloned().collect(),
                depth_stencil_attachment: subpasses[sid].depth_stencil.cloned(),
                input_attachments: subpasses[sid].inputs.iter().cloned().collect(),
                pre_barriers,
            });
        }
        // if this fails, our graph has cycles
        assert_eq!(rp.subpasses.len(), subpasses.len());
        assert!(deps_left.into_iter().all(|count| count == !0));

        // take care of the post-pass transitions
        for (att_id, (ai, att)) in att_infos.iter().zip(attachments.iter()).enumerate() {
            let state_dst = conv::map_image_resource_state(image::Access::empty(), att.layouts.end);
            if state_dst == ai.last_state {
                continue;
            }
            let barrier = n::BarrierDesc::new(att_id, ai.last_state .. state_dst);
            match rp.subpasses.get_mut(ai.barrier_start_index) {
                Some(past_subpass) => {
                    let split = barrier.split();
                    past_subpass.pre_barriers.push(split.start);
                    rp.post_barriers.push(split.end);
                },
                None => rp.post_barriers.push(barrier),
            }
        }

        rp
    }

    fn create_pipeline_layout(&self, sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        // Pipeline layouts are implemented as RootSignature for D3D12.
        //
        // Each descriptor set layout will be one table entry of the root signature.
        // We have the additional restriction that SRV/CBV/UAV and samplers need to be
        // separated, so each set layout will actually occupy up to 2 entries!

        let total = sets.iter().map(|desc_sec| desc_sec.bindings.len()).sum();
        // guarantees that no re-allocation is done, and our pointers are valid
        let mut ranges = Vec::with_capacity(total);
        let mut parameters = Vec::with_capacity(sets.len() * 2);
        let mut set_tables = Vec::with_capacity(sets.len());

        for (i, set) in sets.iter().enumerate() {
            let mut table_type = n::SetTableTypes::empty();

            let mut param = winapi::D3D12_ROOT_PARAMETER {
                ParameterType: winapi::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                ShaderVisibility: winapi::D3D12_SHADER_VISIBILITY_ALL, //TODO
                .. unsafe { mem::zeroed() }
            };

            let range_base = ranges.len();
            ranges.extend(set
                .bindings
                .iter()
                .filter(|bind| bind.ty != pso::DescriptorType::Sampler)
                .map(|bind| conv::map_descriptor_range(bind, 2*i as u32)));

            if ranges.len() > range_base {
                *unsafe{ param.DescriptorTable_mut() } = winapi::D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: (ranges.len() - range_base) as _,
                    pDescriptorRanges: ranges[range_base..].as_ptr(),
                };

                parameters.push(param);
                table_type |= n::SRV_CBV_UAV;
            }

            let range_base = ranges.len();
            ranges.extend(set
                .bindings
                .iter()
                .filter(|bind| bind.ty == pso::DescriptorType::Sampler)
                .map(|bind| conv::map_descriptor_range(bind, (2*i +1) as u32)));

            if ranges.len() > range_base {
                *unsafe{ param.DescriptorTable_mut() } = winapi::D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: (ranges.len() - range_base) as _,
                    pDescriptorRanges: ranges[range_base..].as_ptr(),
                };

                parameters.push(param);
                table_type |= n::SAMPLERS;
            }

            set_tables.push(table_type);
        }

        ranges.get_mut(0).map(|range| {
            range.OffsetInDescriptorsFromTableStart = 0; // careful!
        });

        let desc = winapi::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: parameters.as_ptr(),
            NumStaticSamplers: 0,
            pStaticSamplers: ptr::null(),
            Flags: winapi::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };

        let mut signature = ptr::null_mut();
        let mut signature_raw = ptr::null_mut();
        let mut error = ptr::null_mut();

        // TODO: error handling
        unsafe {
            let _hr = d3d12::D3D12SerializeRootSignature(
                &desc,
                winapi::D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature_raw,
                &mut error,
            );

            if !error.is_null() {
                //TODO
                //let error_output = (*error).GetBufferPointer();
                //let message = <ffi::OsString as OsStringExt>::from_ptr(error_output)
                //    .to_string_lossy();
                //error!("D3D12SerializeRootSignature error: {}", message);
                (*error).Release();
            }

            self.raw.clone().CreateRootSignature(
                0,
                (*signature_raw).GetBufferPointer(),
                (*signature_raw).GetBufferSize(),
                &dxguid::IID_ID3D12RootSignature,
                &mut signature as *mut *mut _ as *mut *mut _,
            );
            (*signature_raw).Release();
        }

        n::PipelineLayout {
            raw: signature,
            tables: set_tables,
            num_parameter_slots: parameters.len(),
        }
    }

    fn create_graphics_pipelines<'a>(
        &self,
        descs: &[pso::GraphicsPipelineDesc<'a, B>],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        descs.iter().map(|desc| {
            let build_shader = |source: Option<pso::EntryPoint<'a, B>>| {
                // TODO: better handle case where looking up shader fails
                let shader = source.and_then(|src| src.module.shaders.get(src.entry));
                match shader {
                    Some(shader) => {
                        winapi::D3D12_SHADER_BYTECODE {
                            pShaderBytecode: unsafe { (**shader).GetBufferPointer() as *const _ },
                            BytecodeLength: unsafe { (**shader).GetBufferSize() as u64 },
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

            let vs = build_shader(Some(desc.shaders.vertex));
            let fs = build_shader(desc.shaders.fragment);
            let gs = build_shader(desc.shaders.geometry);
            let ds = build_shader(desc.shaders.domain);
            let hs = build_shader(desc.shaders.hull);

            // Define input element descriptions
            let mut vs_reflect = shade::reflect_shader(&vs);
            let input_element_descs = {
                let input_descs = shade::reflect_input_elements(&mut vs_reflect);
                desc.attributes
                    .iter()
                    .map(|attrib| {
                        let buffer_desc = if let Some(buffer_desc) = desc.vertex_buffers.get(attrib.binding as usize) {
                                buffer_desc
                            } else {
                                error!("Couldn't find associated vertex buffer description {:?}", attrib.binding);
                                return Err(pso::CreationError::Other);
                            };

                        let input_elem =
                            if let Some(input_elem) = input_descs.iter().find(|elem| elem.semantic_index == attrib.location) {
                                input_elem
                            } else {
                                error!("Couldn't find associated input element slot in the shader {:?}", attrib.location);
                                return Err(pso::CreationError::Other);
                            };

                        let slot_class = match buffer_desc.rate {
                            0 => winapi::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                            _ => winapi::D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                        };
                        let format = attrib.element.format;

                        Ok(winapi::D3D12_INPUT_ELEMENT_DESC {
                            SemanticName: input_elem.semantic_name,
                            SemanticIndex: input_elem.semantic_index,
                            Format: match conv::map_format(format) {
                                Some(fm) => fm,
                                None => {
                                    error!("Unable to find DXGI format for {:?}", format);
                                    return Err(pso::CreationError::Other);
                                }
                            },
                            InputSlot: attrib.binding as _,
                            AlignedByteOffset: attrib.element.offset,
                            InputSlotClass: slot_class,
                            InstanceDataStepRate: buffer_desc.rate as _,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            // TODO: check maximum number of rtvs
            // Get associated subpass information
            let pass = {
                let subpass = &desc.subpass;
                match subpass.main_pass.subpasses.get(subpass.index) {
                    Some(subpass) => subpass,
                    None => return Err(pso::CreationError::InvalidSubpass(subpass.index)),
                }
            };

            // Get color attachment formats from subpass
            let (rtvs, num_rtvs) = {
                let mut rtvs = [winapi::DXGI_FORMAT_UNKNOWN; 8];
                let mut num_rtvs = 0;
                for (rtv, target) in rtvs.iter_mut()
                    .zip(pass.color_attachments.iter())
                {
                    let format = desc.subpass.main_pass.attachments[target.0].format;
                    *rtv = conv::map_format(format).unwrap_or(winapi::DXGI_FORMAT_UNKNOWN);
                    num_rtvs += 1;
                }
                (rtvs, num_rtvs)
            };

            // Setup pipeline description
            let pso_desc = winapi::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: desc.layout.raw,
                VS: vs, PS: fs, GS: gs, DS: ds, HS: hs,
                StreamOutput: winapi::D3D12_STREAM_OUTPUT_DESC {
                    pSODeclaration: ptr::null(),
                    NumEntries: 0,
                    pBufferStrides: ptr::null(),
                    NumStrides: 0,
                    RasterizedStream: 0,
                },
                BlendState: winapi::D3D12_BLEND_DESC {
                    AlphaToCoverageEnable: if desc.blender.alpha_coverage { winapi::TRUE } else { winapi::FALSE },
                    IndependentBlendEnable: winapi::TRUE,
                    RenderTarget: conv::map_render_targets(&desc.blender.targets),
                },
                SampleMask: winapi::UINT::max_value(),
                RasterizerState: conv::map_rasterizer(&desc.rasterizer),
                DepthStencilState: desc.depth_stencil.as_ref().map_or(unsafe { mem::zeroed() }, conv::map_depth_stencil),
                InputLayout: winapi::D3D12_INPUT_LAYOUT_DESC {
                    pInputElementDescs: input_element_descs.as_ptr(),
                    NumElements: input_element_descs.len() as u32,
                },
                IBStripCutValue: winapi::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED, // TODO
                PrimitiveTopologyType: conv::map_topology_type(desc.input_assembler.primitive),
                NumRenderTargets: num_rtvs,
                RTVFormats: rtvs,
                DSVFormat: pass.depth_stencil_attachment
                    .and_then(|att_ref|
                        conv::map_format_dsv(
                            desc
                            .subpass
                            .main_pass
                            .attachments[att_ref.0]
                            .format
                            .0
                        )
                    )
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

            let topology = conv::map_topology(desc.input_assembler.primitive);

            // Create PSO
            let mut pipeline = ptr::null_mut();
            let hr = unsafe {
                self.raw.clone().CreateGraphicsPipelineState(
                    &pso_desc,
                    &dxguid::IID_ID3D12PipelineState,
                    &mut pipeline as *mut *mut _ as *mut *mut _)
            };

            if winapi::SUCCEEDED(hr) {
                Ok(n::GraphicsPipeline {
                    raw: pipeline,
                    signature: desc.layout.raw,
                    num_parameter_slots: desc.layout.num_parameter_slots,
                    topology,
                })
            } else {
                Err(pso::CreationError::Other)
            }
        }).collect()
    }

    fn create_compute_pipelines<'a>(
        &self,
        descs: &[pso::ComputePipelineDesc<'a, B>],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        descs.iter().map(|desc| {
            let cs = {
                // TODO: better handle case where looking up shader fails
                match desc.shader.module.shaders.get(desc.shader.entry) {
                    Some(shader) => {
                        winapi::D3D12_SHADER_BYTECODE {
                            pShaderBytecode: unsafe { (**shader).GetBufferPointer() as *const _ },
                            BytecodeLength: unsafe { (**shader).GetBufferSize() as u64 },
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

            let pso_desc = winapi::D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: desc.layout.raw,
                CS: cs,
                NodeMask: 0,
                CachedPSO: winapi::D3D12_CACHED_PIPELINE_STATE {
                    pCachedBlob: ptr::null(),
                    CachedBlobSizeInBytes: 0,
                },
                Flags: winapi::D3D12_PIPELINE_STATE_FLAG_NONE,
            };

            // Create PSO
            let mut pipeline = ptr::null_mut();
            let hr = unsafe {
                self.raw.clone().CreateComputePipelineState(
                    &pso_desc,
                    &dxguid::IID_ID3D12PipelineState,
                    &mut pipeline as *mut *mut _ as *mut *mut _)
            };

            if winapi::SUCCEEDED(hr) {
                Ok(n::ComputePipeline {
                    raw: pipeline,
                    signature: desc.layout.raw,
                    num_parameter_slots: desc.layout.num_parameter_slots,
                })
            } else {
                Err(pso::CreationError::Other)
            }
        }).collect()
    }

    fn create_framebuffer(
        &self,
        _renderpass: &n::RenderPass,
        attachments: &[&n::ImageView],
        _extent: d::Extent,
    ) -> Result<n::Framebuffer, d::FramebufferError> {
        Ok(n::Framebuffer {
            attachments: attachments.iter().cloned().cloned().collect(),
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        let mut ast = spirv::Ast::<hlsl::Target>::parse(&module)
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown parsing error".into(),
                };
                d::ShaderError::CompilationFailed(msg)
            })?;

        // Patch descriptor sets due to the splitting of descriptor heaps into
        // SrvCbvUav and sampler heap. Each set will have a new location to match
        // the layout of the root signatures.
        let shader_resources = ast.get_shader_resources().map_err(gen_query_error)?;
        for image in &shader_resources.separate_images {
            let set = ast.get_decoration(image.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(image.id, spirv::Decoration::DescriptorSet, 2*set)
               .map_err(gen_unexpected_error)?;
        }

        for sampler in &shader_resources.separate_samplers {
            let set = ast.get_decoration(sampler.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(sampler.id, spirv::Decoration::DescriptorSet, 2*set+1)
               .map_err(gen_unexpected_error)?;
        }

        let shader_model = hlsl::ShaderModel::V5_1;
        let mut compile_options = hlsl::CompilerOptions::default();
        compile_options.shader_model = shader_model;
        compile_options.vertex.invert_y = true;

        ast.set_compiler_options(&compile_options)
           .map_err(gen_unexpected_error)?;
        let shader_code = ast.compile()
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                };
                d::ShaderError::CompilationFailed(msg)
            })?;

        debug!("SPIRV-Cross generated shader:\n{}", shader_code);

        let mut shader_map = BTreeMap::new();
        let entry_points = ast.get_entry_points().map_err(gen_query_error)?;
        for entry_point in entry_points {
            let stage = match entry_point.execution_model {
                spirv::ExecutionModel::Vertex => pso::Stage::Vertex,
                spirv::ExecutionModel::Fragment => pso::Stage::Fragment,
                _ => unimplemented!(), // TODO: geometry, tessellation and compute seem to unsupported for now
            };

            let shader_blob = Self::compile_shader(
                stage,
                shader_model,
                &entry_point.name,
                shader_code.as_bytes(),
            )?;

            shader_map.insert(entry_point.name, shader_blob);
        }
        Ok(n::ShaderModule { shaders: shader_map })
    }

    fn create_buffer(
        &self,
        mut size: u64,
        stride: u64,
        usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        if usage.contains(buffer::Usage::UNIFORM) {
            // Constant buffer view sizes need to be aligned.
            // Coupled with the offset alignment we can enforce an aligned CBV size
            // on descriptor updates.
            size = (size + 255) & !255;
        }

        let requirements = memory::Requirements {
            size,
            alignment: winapi::D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as u64,
            type_mask: if self.private_caps.heterogeneous_resource_heaps { 0x7 } else { 0x7<<4 },
        };

        Ok(UnboundBuffer {
            requirements,
            stride,
            usage,
        })
    }

    fn get_buffer_requirements(&self, buffer: &UnboundBuffer) -> Requirements {
        buffer.requirements
    }

    fn bind_buffer_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        buffer: UnboundBuffer,
    ) -> Result<n::Buffer, d::BindError> {
        if buffer.requirements.type_mask & (1 << memory.ty.id) == 0 {
            error!("Bind memory failure: supported mask 0x{:x}, given id {}",
                buffer.requirements.type_mask, memory.ty.id);
            return Err(d::BindError::WrongMemory)
        }
        if offset + buffer.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds)
        }

        let mut resource = ptr::null_mut();
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
            Flags: conv::map_buffer_flags(buffer.usage),
        };

        assert_eq!(winapi::S_OK, unsafe {
            self.raw.clone().CreatePlacedResource(
                memory.heap.as_mut(),
                offset,
                &desc,
                winapi::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &dxguid::IID_ID3D12Resource,
                &mut resource,
            )
        });

        let clear_uav = if buffer.usage.contains(buffer::Usage::TRANSFER_DST) {
            let handles = self.uav_pool.lock().unwrap().alloc_handles(1);
            let mut desc = winapi::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: winapi::DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: winapi::D3D12_UAV_DIMENSION_BUFFER,
                u: unsafe { mem::zeroed() },
            };

           *unsafe { desc.Buffer_mut() } = winapi::D3D12_BUFFER_UAV {
                FirstElement: 0,
                NumElements: (buffer.requirements.size / 4) as _,
                StructureByteStride: 0,
                CounterOffsetInBytes: 0,
                Flags: winapi::D3D12_BUFFER_UAV_FLAG_RAW,
            };

            unsafe {
                self.raw.clone().CreateUnorderedAccessView(
                    resource as *mut _,
                    ptr::null_mut(),
                    &desc,
                    handles.cpu,
                );
            }
            Some(handles)
        } else {
            None
        };

        Ok(n::Buffer {
            resource: resource as *mut _,
            size_in_bytes: buffer.requirements.size as _,
            stride: buffer.stride as _,
            clear_uav,
        })
    }

    fn create_buffer_view(
        &self,
        _buffer: &n::Buffer,
        _format: format::Format,
        _range: Range<u64>,
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        usage: image::Usage,
    ) -> Result<UnboundImage, image::CreationError> {
        use self::image::AspectFlags;
        let mut aspects = AspectFlags::empty();
        let bits = format.0.describe_bits();
        if bits.color + bits.alpha != 0 {
            aspects |= AspectFlags::COLOR;
        }
        if bits.depth != 0 {
            aspects |= AspectFlags::DEPTH;
        }
        if bits.stencil != 0 {
            aspects |= AspectFlags::STENCIL;
        }

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
            DepthOrArraySize: depth,
            MipLevels: mip_levels as u16,
            Format: match conv::map_format(format) {
                Some(format) => format,
                None => return Err(image::CreationError::Format(format.0, Some(format.1))),
            },
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: aa.get_num_fragments() as u32,
                Quality: 0,
            },
            Layout: winapi::D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: conv::map_image_flags(usage),
        };

        let mut alloc_info = unsafe { mem::zeroed() };
        unsafe {
            self.raw.clone().GetResourceAllocationInfo(&mut alloc_info, 0, 1, &desc);
        }

        Ok(UnboundImage {
            dsv_format: conv::map_format_dsv(format.0).unwrap_or(desc.Format),
            desc,
            requirements: memory::Requirements {
                size: alloc_info.SizeInBytes,
                alignment: alloc_info.Alignment,
                type_mask: if self.private_caps.heterogeneous_resource_heaps { 0x7 }
                    else if usage.can_target() { 0x7<<12 } else { 0x7<<8 },
            },
            kind,
            usage,
            aspects,
            bits_per_texel: bits.total,
            num_levels: mip_levels,
            num_layers: kind.get_num_layers(),
        })
    }

    fn get_image_requirements(&self, image: &UnboundImage) -> Requirements {
        image.requirements
    }

    fn bind_image_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        image: UnboundImage,
    ) -> Result<n::Image, d::BindError> {
        use self::image::{AspectFlags, Usage};
        if image.requirements.type_mask & (1 << memory.ty.id) == 0 {
            error!("Bind memory failure: supported mask 0x{:x}, given id {}",
                image.requirements.type_mask, memory.ty.id);
            return Err(d::BindError::WrongMemory)
        }
        if offset + image.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds)
        }

        let mut resource = ptr::null_mut();

        assert_eq!(winapi::S_OK, unsafe {
            self.raw.clone().CreatePlacedResource(
                memory.heap.as_mut(),
                offset,
                &image.desc,
                winapi::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &dxguid::IID_ID3D12Resource,
                &mut resource,
            )
        });

        //TODO: the clear_Xv is incomplete. We should support clearing images created without XXX_ATTACHMENT usage.
        // for this, we need to check the format and force the `RENDER_TARGET` flag behind the user's back
        // if the format supports being rendered into, allowing us to create clear_Xv

        Ok(n::Image {
            resource: resource as *mut _,
            kind: image.kind,
            usage: image.usage,
            dxgi_format: image.desc.Format,
            bits_per_texel: image.bits_per_texel,
            num_levels: image.num_levels,
            num_layers: image.num_layers,
            clear_cv: if image.aspects.contains(AspectFlags::COLOR) && image.usage.contains(Usage::COLOR_ATTACHMENT) {
                let range = image::SubresourceRange {
                    aspects: AspectFlags::COLOR,
                    levels: 0 .. 1, //TODO?
                    layers: 0 .. image.num_layers,
                };
                Some(self.view_image_as_render_target(resource as *mut _, image.kind, image.desc.Format, &range).unwrap())
            } else {
                None
            },
            clear_dv: if image.aspects.contains(AspectFlags::DEPTH) && image.usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
                let range = image::SubresourceRange {
                    aspects: AspectFlags::DEPTH,
                    levels: 0 .. 1, //TODO?
                    layers: 0 .. image.num_layers,
                };
                Some(self.view_image_as_depth_stencil(resource as *mut _, image.kind, image.dsv_format, &range).unwrap())
            } else {
                None
            },
            clear_sv: if image.aspects.contains(AspectFlags::STENCIL) && image.usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
                let range = image::SubresourceRange {
                    aspects: AspectFlags::STENCIL,
                    levels: 0 .. 1, //TODO?
                    layers: 0 .. image.num_layers,
                };
                Some(self.view_image_as_depth_stencil(resource as *mut _, image.kind, image.dsv_format, &range).unwrap())
            } else {
                None
            },
        })
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        use self::image::Usage;
        let format_raw = conv::map_format(format).ok_or(image::ViewError::BadFormat);

        Ok(n::ImageView {
            resource: image.resource,
            handle_srv: if image.usage.contains(Usage::SAMPLED) {
                Some(self.view_image_as_shader_resource(image.resource, image.kind, format_raw.clone()?, &range)?)
            } else {
                None
            },
            handle_rtv: if image.usage.contains(Usage::COLOR_ATTACHMENT) {
                Some(self.view_image_as_render_target(image.resource, image.kind, format_raw.clone()?, &range)?)
            } else {
                None
            },
            handle_dsv: if image.usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
                let fmt = conv::map_format_dsv(format.0).ok_or(image::ViewError::BadFormat);
                Some(self.view_image_as_depth_stencil(image.resource, image.kind, fmt?, &range)?)
            } else {
                None
            },
            handle_uav: if image.usage.contains(Usage::STORAGE) {
                Some(self.view_image_as_storage(image.resource, image.kind, format_raw?, &range)?)
            } else {
                None
            },
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let handle = self.sampler_pool.lock().unwrap().alloc_handles(1).cpu;

        let op = match info.comparison {
            Some(_) => conv::FilterOp::Comparison,
            None => conv::FilterOp::Product,
        };
        let desc = winapi::D3D12_SAMPLER_DESC {
            Filter: conv::map_filter(info.filter, op),
            AddressU: conv::map_wrap(info.wrap_mode.0),
            AddressV: conv::map_wrap(info.wrap_mode.1),
            AddressW: conv::map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.filter {
                image::FilterMethod::Anisotropic(max) => max as _, // TODO: check support here?
                _ => 0,
            },
            ComparisonFunc: conv::map_comparison(info.comparison.unwrap_or(pso::Comparison::Always)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.start.into(),
            MaxLOD: info.lod_range.end.into(),
        };

        unsafe {
            self.raw.clone().CreateSampler(&desc, handle);
        }

        n::Sampler { handle }
    }

    fn create_descriptor_pool(
        &self,
        max_sets: usize,
        descriptor_pools: &[pso::DescriptorRangeDesc],
    ) -> n::DescriptorPool {
        let mut num_srv_cbv_uav = 0;
        let mut num_samplers = 0;

        for desc in descriptor_pools {
            match desc.ty {
                pso::DescriptorType::Sampler => {
                    num_samplers += desc.count as u64;
                }
                _ => {
                    num_srv_cbv_uav += desc.count as u64;
                }
            }
        }

        let heap_srv_cbv_uav = {
            let mut heap_srv_cbv_uav = self
                .heap_srv_cbv_uav
                .lock()
                .unwrap();

            let range = heap_srv_cbv_uav
                .allocator
                .allocate(num_srv_cbv_uav)
                .unwrap(); // TODO: error/resize
            n::DescriptorHeapSlice {
                heap: heap_srv_cbv_uav.raw.clone(),
                handle_size: heap_srv_cbv_uav.handle_size,
                next: range.start,
                range,
                start: heap_srv_cbv_uav.start,
            }
        };

        let heap_sampler = {
            let mut heap_sampler = self
                .heap_sampler
                .lock()
                .unwrap();

            let range = heap_sampler
                .allocator
                .allocate(num_samplers)
                .unwrap(); // TODO: error/resize
            n::DescriptorHeapSlice {
                heap: heap_sampler.raw.clone(),
                handle_size: heap_sampler.handle_size,
                next: range.start,
                range,
                start: heap_sampler.start,
            }
        };

        n::DescriptorPool {
            heap_srv_cbv_uav,
            heap_sampler,
            pools: descriptor_pools.to_vec(),
            max_size: max_sets as _,
        }
    }

    fn create_descriptor_set_layout(
        &self,
        bindings: &[pso::DescriptorSetLayoutBinding],
    )-> n::DescriptorSetLayout {
        n::DescriptorSetLayout { bindings: bindings.to_vec() }
    }

    fn update_descriptor_sets(&self, writes: &[pso::DescriptorSetWrite<B>]) {
        // Create temporary non-shader visible views for uniform and storage buffers.
        let mut num_views = 0;
        for sw in writes {
            match sw.write {
                pso::DescriptorWrite::UniformBuffer(ref views) |
                pso::DescriptorWrite::StorageBuffer(ref views) => {
                    num_views += views.len();
                },
                _ => (),
            }
        }

        let mut raw = self.raw.clone();
        let mut handles = Vec::with_capacity(num_views);
        let _buffer_heap = if num_views != 0 {
            let mut heap = n::DescriptorCpuPool {
                heap: Self::create_descriptor_heap_impl(
                    &mut raw,
                    winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                    false,
                    num_views,
                ),
                offset: 0,
                size: 0,
                max_size: num_views as _,
            };
            // Create views
            for sw in writes {
                match sw.write {
                    pso::DescriptorWrite::UniformBuffer(ref views) => {
                        handles.extend(views.iter().map(|&(buffer, ref range)| {
                            let handle = heap.alloc_handles(1).cpu;
                            // Making the size field of buffer requirements for uniform
                            // buffers a multiple of 256 and setting the required offset
                            // alignment to 256 allows us to patch the size here.
                            // We can always enforce the size to be aligned to 256 for
                            // CBVs without going out-of-bounds.
                            let size = ((range.end - range.start) + 255) & !255;
                            let desc = winapi::D3D12_CONSTANT_BUFFER_VIEW_DESC {
                                BufferLocation: unsafe { (*buffer.resource).GetGPUVirtualAddress() },
                                SizeInBytes: size as _,
                            };
                            unsafe {
                                raw.CreateConstantBufferView(&desc, handle);
                            }
                            handle
                        }));
                    }
                    pso::DescriptorWrite::StorageBuffer(ref views) => {
                        // StorageBuffer gets translated into a byte address buffer.
                        // We don't have to stride information to make it structured.
                        handles.extend(views.iter().map(|&(buffer, ref range)| {
                            let handle = heap.alloc_handles(1).cpu;
                            let mut desc = winapi::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                                Format: winapi::DXGI_FORMAT_R32_TYPELESS,
                                ViewDimension: winapi::D3D12_UAV_DIMENSION_BUFFER,
                                u: unsafe { mem::zeroed() },
                            };

                           *unsafe { desc.Buffer_mut() } = winapi::D3D12_BUFFER_UAV {
                                FirstElement: range.start as _,
                                NumElements: ((range.end - range.start) / 4) as _,
                                StructureByteStride: 0,
                                CounterOffsetInBytes: 0,
                                Flags: winapi::D3D12_BUFFER_UAV_FLAG_RAW,
                            };

                            unsafe {
                                raw.CreateUnorderedAccessView(buffer.resource, ptr::null_mut(), &desc, handle);
                            }
                            handle
                        }));
                    }
                    _ => {}
                }
            }

            Some(heap)
        } else {
            None
        };

        let mut cur_view = 0;
        self.update_descriptor_sets_impl(writes,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            |dw, starts| match *dw {
                pso::DescriptorWrite::SampledImage(ref images) => {
                    starts.extend(images.iter().map(|&(ref srv, _layout)| srv.handle_srv.unwrap()));
                }
                pso::DescriptorWrite::UniformBuffer(ref views) |
                pso::DescriptorWrite::StorageBuffer(ref views) => {
                    starts.extend(&handles[cur_view .. cur_view + views.len()]);
                    cur_view += views.len();
                }
                pso::DescriptorWrite::StorageImage(ref images) => {
                    starts.extend(images.iter().map(|&(ref uav, _layout)| uav.handle_uav.unwrap()));
                }
                pso::DescriptorWrite::Sampler(_) => (), // done separately
                _ => unimplemented!()
            });

        self.update_descriptor_sets_impl(writes,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            |dw, starts| match *dw {
                pso::DescriptorWrite::Sampler(ref samplers) => {
                    starts.extend(samplers.iter().map(|sm| sm.handle))
                }
                _ => ()
            });
    }

    fn acquire_mapping_raw(&self, buf: &n::Buffer, read: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>
    {
        let read_range = match read {
            Some(r) => winapi::D3D12_RANGE {
                Begin: r.start,
                End: r.end,
            },
            None => winapi::D3D12_RANGE {
                Begin: 0,
                End: 0,
            },
        };

        let mut ptr = ptr::null_mut();
        assert_eq!(winapi::S_OK, unsafe {
            (*buf.resource).Map(0, &read_range, &mut ptr)
        });

        Ok(ptr as *mut _)
    }

    fn release_mapping_raw(&self, buf: &n::Buffer, wrote: Option<Range<u64>>) {
        let written_range = match wrote {
            Some(w) => winapi::D3D12_RANGE {
                Begin: w.start,
                End: w.end,
            },
            None => winapi::D3D12_RANGE {
                Begin: 0,
                End: 0,
            },
        };

        unsafe { (*buf.resource).Unmap(0, &written_range) };
    }

    fn create_semaphore(&self) -> n::Semaphore {
        let fence = self.create_fence(false);
        n::Semaphore {
            raw: fence.raw,
        }
    }

    fn create_fence(&self, _signaled: bool) -> n::Fence {
        let mut handle = ptr::null_mut();
        assert_eq!(winapi::S_OK, unsafe {
            self.raw.clone().CreateFence(
                0,
                winapi::D3D12_FENCE_FLAGS(0),
                &dxguid::IID_ID3D12Fence,
                &mut handle,
            )
        });

        n::Fence {
            raw: unsafe { ComPtr::new(handle as *mut _) },
        }
    }

    fn reset_fences(&self, fences: &[&n::Fence]) {
        for fence in fences {
            assert_eq!(winapi::S_OK, unsafe {
                fence.raw.clone().Signal(0)
            });
        }
    }

    fn wait_for_fences(&self, fences: &[&n::Fence], wait: d::WaitFor, timeout_ms: u32) -> bool {
        let mut events = self.events.lock().unwrap();
        for _ in events.len() .. fences.len() {
            events.push(unsafe {
                kernel32::CreateEventA(
                    ptr::null_mut(),
                    winapi::FALSE, winapi::FALSE,
                    ptr::null(),
                )
            });
        }

        for (&event, fence) in events.iter().zip(fences.iter()) {
            assert_eq!(winapi::S_OK, unsafe {
                kernel32::ResetEvent(event);
                fence.raw.clone().SetEventOnCompletion(1, event)
            });
        }

        let all = match wait {
            d::WaitFor::Any => winapi::FALSE,
            d::WaitFor::All => winapi::TRUE,
        };
        let hr = unsafe {
            kernel32::WaitForMultipleObjects(fences.len() as u32, events.as_ptr(), all, timeout_ms)
        };

        const WAIT_OBJECT_LAST: u32 = winapi::WAIT_OBJECT_0 + winapi::MAXIMUM_WAIT_OBJECTS;
        const WAIT_ABANDONED_LAST: u32 = winapi::WAIT_ABANDONED_0 + winapi::MAXIMUM_WAIT_OBJECTS;
        match hr {
            winapi::WAIT_OBJECT_0 ... WAIT_OBJECT_LAST => true,
            winapi::WAIT_ABANDONED_0 ... WAIT_ABANDONED_LAST => true, //TODO?
            winapi::WAIT_TIMEOUT => false,
            _ => panic!("Unexpected wait status 0x{:X}", hr),
        }
    }

    fn get_fence_status(&self, fence: &n::Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, _memory: n::Memory) {
        // Just drop
    }

    fn destroy_shader_module(&self, shader_lib: n::ShaderModule) {
        for (_, _blob) in shader_lib.shaders {
            //unsafe { blob.Release(); } //TODO
        }
    }

    fn destroy_renderpass(&self, _rp: n::RenderPass) {
        // Just drop
    }

    fn destroy_pipeline_layout(&self, layout: n::PipelineLayout) {
        unsafe { (*layout.raw).Release(); }
    }

    fn destroy_graphics_pipeline(&self, pipeline: n::GraphicsPipeline) {
        unsafe { (*pipeline.raw).Release(); }
    }

    fn destroy_compute_pipeline(&self, pipeline: n::ComputePipeline) {
        unsafe { (*pipeline.raw).Release(); }
    }

    fn destroy_framebuffer(&self, _fb: n::Framebuffer) {
        // Just drop
    }

    fn destroy_buffer(&self, buffer: n::Buffer) {
        unsafe { (*buffer.resource).Release(); }
    }

    fn destroy_buffer_view(&self, _view: n::BufferView) {
        // empty
    }

    fn destroy_image(&self, image: n::Image) {
        unsafe { (*image.resource).Release(); }
    }

    fn destroy_image_view(&self, _view: n::ImageView) {
        // Just drop
    }

    fn destroy_sampler(&self, _sampler: n::Sampler) {
        // Just drop
    }

    fn destroy_descriptor_pool(&self, pool: n::DescriptorPool) {
        self.heap_srv_cbv_uav.lock().unwrap()
            .allocator.deallocate(pool.heap_srv_cbv_uav.range);
        self.heap_sampler.lock().unwrap()
            .allocator.deallocate(pool.heap_sampler.range);
    }

    fn destroy_descriptor_set_layout(&self, _layout: n::DescriptorSetLayout) {
        // Just drop
    }

    fn destroy_fence(&self, _fence: n::Fence) {
        // Just drop, ComPtr backed
    }

    fn destroy_semaphore(&self, _semaphore: n::Semaphore) {
        // Just drop, ComPtr backed
    }
}
