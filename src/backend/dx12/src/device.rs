use std::borrow::Borrow;
use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;
use std::{ffi, mem, ptr, slice};

use spirv_cross::{hlsl, spirv, ErrorCode as SpirvErrorCode};

use winapi::shared::minwindef::{FALSE, TRUE, UINT};
use winapi::shared::{dxgi, dxgi1_2, dxgi1_4, dxgiformat, dxgitype, winerror};
use winapi::um::{d3d12, d3dcompiler, synchapi, winbase, winnt};
use winapi::Interface;

use hal::format::{Aspects, Format};
use hal::memory::Requirements;
use hal::pool::CommandPoolCreateFlags;
use hal::queue::{QueueFamilyId, RawCommandQueue};
use hal::range::RangeArg;
use hal::{self, buffer, device as d, error, format, image, mapping, memory, pass, pso, query};

use native::command_list::IndirectArgument;
use native::descriptor;
use native::pso::{CachedPSO, PipelineStateFlags, PipelineStateSubobject, Subobject};

use pool::{CommandPoolAllocator, RawCommandPool};
use range_alloc::RangeAllocator;
use root_constants::RootConstant;
use {
    conv, descriptors_cpu, native, resource as r, root_constants, window as w, Backend as B,
    Device, MemoryGroup, MAX_VERTEX_BUFFERS, NUM_HEAP_PROPERTIES, QUEUE_FAMILIES,
};

// Register space used for root constants.
const ROOT_CONSTANT_SPACE: u32 = 0;

const MEM_TYPE_MASK: u64 = 0x7;
const MEM_TYPE_SHIFT: u64 = 3;

const MEM_TYPE_UNIVERSAL_SHIFT: u64 = MEM_TYPE_SHIFT * MemoryGroup::Universal as u64;
const MEM_TYPE_BUFFER_SHIFT: u64 = MEM_TYPE_SHIFT * MemoryGroup::BufferOnly as u64;
const MEM_TYPE_IMAGE_SHIFT: u64 = MEM_TYPE_SHIFT * MemoryGroup::ImageOnly as u64;
const MEM_TYPE_TARGET_SHIFT: u64 = MEM_TYPE_SHIFT * MemoryGroup::TargetOnly as u64;

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

#[derive(Clone, Debug)]
pub(crate) struct ViewInfo {
    pub(crate) resource: native::Resource,
    pub(crate) kind: image::Kind,
    pub(crate) caps: image::ViewCapabilities,
    pub(crate) view_kind: image::ViewKind,
    pub(crate) format: dxgiformat::DXGI_FORMAT,
    pub(crate) range: image::SubresourceRange,
}

pub(crate) enum CommandSignature {
    Draw,
    DrawIndexed,
    Dispatch,
}

#[derive(Debug)]
pub struct UnboundBuffer {
    requirements: memory::Requirements,
    usage: buffer::Usage,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnboundImage {
    #[derivative(Debug = "ignore")]
    desc: d3d12::D3D12_RESOURCE_DESC,
    dsv_format: dxgiformat::DXGI_FORMAT,
    requirements: memory::Requirements,
    format: Format,
    kind: image::Kind,
    usage: image::Usage,
    tiling: image::Tiling,
    view_caps: image::ViewCapabilities,
    //TODO: use hal::format::FormatDesc
    bytes_per_block: u8,
    // Dimension of a texel block (compressed formats).
    block_dim: (u8, u8),
    num_levels: image::Level,
}

/// Compile a single shader entry point from a HLSL text shader
pub(crate) fn compile_shader(
    stage: pso::Stage,
    shader_model: hlsl::ShaderModel,
    entry: &str,
    code: &[u8],
) -> Result<native::Blob, d::ShaderError> {
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

    let mut shader_data = native::Blob::null();
    let mut error = native::Blob::null();
    let entry = ffi::CString::new(entry).unwrap();
    let hr = unsafe {
        d3dcompiler::D3DCompile(
            code.as_ptr() as *const _,
            code.len(),
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            entry.as_ptr() as *const _,
            stage_to_str(stage, shader_model).as_ptr() as *const i8,
            1,
            0,
            shader_data.mut_void() as *mut *mut _,
            error.mut_void() as *mut *mut _,
        )
    };
    if !winerror::SUCCEEDED(hr) {
        error!("D3DCompile error {:x}", hr);
        let message = unsafe {
            let pointer = error.GetBufferPointer();
            let size = error.GetBufferSize();
            let slice = slice::from_raw_parts(pointer as *const u8, size as usize);
            String::from_utf8_lossy(slice).into_owned()
        };
        unsafe {
            error.destroy();
        }
        Err(d::ShaderError::CompilationFailed(message))
    } else {
        Ok(shader_data)
    }
}

#[repr(C)]
struct GraphicsPipelineStateSubobjectStream {
    root_signature: PipelineStateSubobject<*mut d3d12::ID3D12RootSignature>,
    vs: PipelineStateSubobject<d3d12::D3D12_SHADER_BYTECODE>,
    ps: PipelineStateSubobject<d3d12::D3D12_SHADER_BYTECODE>,
    ds: PipelineStateSubobject<d3d12::D3D12_SHADER_BYTECODE>,
    hs: PipelineStateSubobject<d3d12::D3D12_SHADER_BYTECODE>,
    gs: PipelineStateSubobject<d3d12::D3D12_SHADER_BYTECODE>,
    stream_output: PipelineStateSubobject<d3d12::D3D12_STREAM_OUTPUT_DESC>,
    blend: PipelineStateSubobject<d3d12::D3D12_BLEND_DESC>,
    sample_mask: PipelineStateSubobject<UINT>,
    rasterizer: PipelineStateSubobject<d3d12::D3D12_RASTERIZER_DESC>,
    depth_stencil: PipelineStateSubobject<d3d12::D3D12_DEPTH_STENCIL_DESC1>,
    input_layout: PipelineStateSubobject<d3d12::D3D12_INPUT_LAYOUT_DESC>,
    ib_strip_cut_value: PipelineStateSubobject<d3d12::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE>,
    primitive_topology: PipelineStateSubobject<d3d12::D3D12_PRIMITIVE_TOPOLOGY_TYPE>,
    render_target_formats: PipelineStateSubobject<d3d12::D3D12_RT_FORMAT_ARRAY>,
    depth_stencil_format: PipelineStateSubobject<dxgiformat::DXGI_FORMAT>,
    sample_desc: PipelineStateSubobject<dxgitype::DXGI_SAMPLE_DESC>,
    node_mask: PipelineStateSubobject<UINT>,
    cached_pso: PipelineStateSubobject<d3d12::D3D12_CACHED_PIPELINE_STATE>,
    flags: PipelineStateSubobject<d3d12::D3D12_PIPELINE_STATE_FLAGS>,
}

impl GraphicsPipelineStateSubobjectStream {
    fn new(
        pso_desc: &d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        depth_bounds_test_enable: bool,
    ) -> Self {
        GraphicsPipelineStateSubobjectStream {
            root_signature: PipelineStateSubobject::new(
                Subobject::RootSignature,
                pso_desc.pRootSignature,
            ),
            vs: PipelineStateSubobject::new(Subobject::VS, pso_desc.VS),
            ps: PipelineStateSubobject::new(Subobject::PS, pso_desc.PS),
            ds: PipelineStateSubobject::new(Subobject::DS, pso_desc.DS),
            hs: PipelineStateSubobject::new(Subobject::HS, pso_desc.HS),
            gs: PipelineStateSubobject::new(Subobject::GS, pso_desc.GS),
            stream_output: PipelineStateSubobject::new(
                Subobject::StreamOutput,
                pso_desc.StreamOutput,
            ),
            blend: PipelineStateSubobject::new(Subobject::Blend, pso_desc.BlendState),
            sample_mask: PipelineStateSubobject::new(Subobject::SampleMask, pso_desc.SampleMask),
            rasterizer: PipelineStateSubobject::new(
                Subobject::Rasterizer,
                pso_desc.RasterizerState,
            ),
            depth_stencil: PipelineStateSubobject::new(
                Subobject::DepthStencil1,
                d3d12::D3D12_DEPTH_STENCIL_DESC1 {
                    DepthEnable: pso_desc.DepthStencilState.DepthEnable,
                    DepthWriteMask: pso_desc.DepthStencilState.DepthWriteMask,
                    DepthFunc: pso_desc.DepthStencilState.DepthFunc,
                    StencilEnable: pso_desc.DepthStencilState.StencilEnable,
                    StencilReadMask: pso_desc.DepthStencilState.StencilReadMask,
                    StencilWriteMask: pso_desc.DepthStencilState.StencilWriteMask,
                    FrontFace: pso_desc.DepthStencilState.FrontFace,
                    BackFace: pso_desc.DepthStencilState.BackFace,
                    DepthBoundsTestEnable: depth_bounds_test_enable as _,
                },
            ),
            input_layout: PipelineStateSubobject::new(Subobject::InputLayout, pso_desc.InputLayout),
            ib_strip_cut_value: PipelineStateSubobject::new(
                Subobject::IBStripCut,
                pso_desc.IBStripCutValue,
            ),
            primitive_topology: PipelineStateSubobject::new(
                Subobject::PrimitiveTopology,
                pso_desc.PrimitiveTopologyType,
            ),
            render_target_formats: PipelineStateSubobject::new(
                Subobject::RTFormats,
                d3d12::D3D12_RT_FORMAT_ARRAY {
                    RTFormats: pso_desc.RTVFormats,
                    NumRenderTargets: pso_desc.NumRenderTargets,
                },
            ),
            depth_stencil_format: PipelineStateSubobject::new(
                Subobject::DSFormat,
                pso_desc.DSVFormat,
            ),
            sample_desc: PipelineStateSubobject::new(Subobject::SampleDesc, pso_desc.SampleDesc),
            node_mask: PipelineStateSubobject::new(Subobject::NodeMask, pso_desc.NodeMask),
            cached_pso: PipelineStateSubobject::new(Subobject::CachedPSO, pso_desc.CachedPSO),
            flags: PipelineStateSubobject::new(Subobject::Flags, pso_desc.Flags),
        }
    }
}

impl Device {
    fn parse_spirv(raw_data: &[u8]) -> Result<spirv::Ast<hlsl::Target>, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        spirv::Ast::parse(&module).map_err(|err| {
            let msg = match err {
                SpirvErrorCode::CompilationError(msg) => msg,
                SpirvErrorCode::Unhandled => "Unknown parsing error".into(),
            };
            d::ShaderError::CompilationFailed(msg)
        })
    }

    fn patch_spirv_resources(
        ast: &mut spirv::Ast<hlsl::Target>,
        layout: Option<&r::PipelineLayout>,
    ) -> Result<(), d::ShaderError> {
        // Patch descriptor sets due to the splitting of descriptor heaps into
        // SrvCbvUav and sampler heap. Each set will have a new location to match
        // the layout of the root signatures.
        let space_offset = match layout {
            Some(layout) if !layout.root_constants.is_empty() => 1,
            _ => 0,
        };

        let shader_resources = ast.get_shader_resources().map_err(gen_query_error)?;
        for image in &shader_resources.separate_images {
            let set = ast
                .get_decoration(image.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                image.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for uniform_buffer in &shader_resources.uniform_buffers {
            let set = ast
                .get_decoration(uniform_buffer.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                uniform_buffer.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for storage_buffer in &shader_resources.storage_buffers {
            let set = ast
                .get_decoration(storage_buffer.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                storage_buffer.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for image in &shader_resources.storage_images {
            let set = ast
                .get_decoration(image.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                image.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for sampler in &shader_resources.separate_samplers {
            let set = ast
                .get_decoration(sampler.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                sampler.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for image in &shader_resources.sampled_images {
            let set = ast
                .get_decoration(image.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                image.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        for input in &shader_resources.subpass_inputs {
            let set = ast
                .get_decoration(input.id, spirv::Decoration::DescriptorSet)
                .map_err(gen_query_error)?;
            ast.set_decoration(
                input.id,
                spirv::Decoration::DescriptorSet,
                space_offset + set,
            ).map_err(gen_unexpected_error)?;
        }

        // TODO: other resources

        Ok(())
    }

    fn translate_spirv(
        ast: &mut spirv::Ast<hlsl::Target>,
        shader_model: hlsl::ShaderModel,
        layout: &r::PipelineLayout,
        stage: pso::Stage,
    ) -> Result<String, d::ShaderError> {
        let mut compile_options = hlsl::CompilerOptions::default();
        compile_options.shader_model = shader_model;
        compile_options.vertex.invert_y = true;

        let stage_flag = stage.into();
        let root_constant_layout = layout
            .root_constants
            .iter()
            .filter_map(|constant| {
                if constant.stages.contains(stage_flag) {
                    Some(hlsl::RootConstant {
                        start: constant.range.start * 4,
                        end: constant.range.end * 4,
                        binding: constant.range.start,
                        space: 0,
                    })
                } else {
                    None
                }
            })
            .collect();
        ast.set_compiler_options(&compile_options)
            .map_err(gen_unexpected_error)?;
        ast.set_root_constant_layout(root_constant_layout)
            .map_err(gen_unexpected_error)?;
        ast.compile().map_err(|err| {
            let msg = match err {
                SpirvErrorCode::CompilationError(msg) => msg,
                SpirvErrorCode::Unhandled => "Unknown compile error".into(),
            };
            d::ShaderError::CompilationFailed(msg)
        })
    }

    // Extract entry point from shader module on pipeline creation.
    // Returns compiled shader blob and bool to indicate if the shader should be
    // destroyed after pipeline creation
    fn extract_entry_point(
        stage: pso::Stage,
        source: &pso::EntryPoint<B>,
        layout: &r::PipelineLayout,
    ) -> Result<(native::Blob, bool), d::ShaderError> {
        match *source.module {
            r::ShaderModule::Compiled(ref shaders) => {
                // TODO: do we need to check for specialization constants?
                // Use precompiled shader, ignore specialization or layout.
                shaders
                    .get(source.entry)
                    .map(|src| (*src, false))
                    .ok_or(d::ShaderError::MissingEntryPoint(source.entry.into()))
            }
            r::ShaderModule::Spirv(ref raw_data) => {
                let mut ast = Self::parse_spirv(raw_data)?;
                let spec_constants = ast.get_specialization_constants().map_err(gen_query_error)?;

                //TODO: move this out into `auxil`
                for spec_constant in spec_constants {
                    if let Some(constant) = source.specialization.constants
                        .iter()
                        .find(|c| c.id == spec_constant.constant_id)
                    {
                        // Override specialization constant values
                        let value = source.specialization
                            .data[constant.range.start as usize .. constant.range.end as usize]
                            .iter()
                            .rev()
                            .fold(0u64, |u, &b| (u<<8) + b as u64);
                        ast.set_scalar_constant(spec_constant.id, value)
                            .map_err(gen_query_error)?;
                    }
                }

                Self::patch_spirv_resources(&mut ast, Some(layout))?;
                let shader_model = hlsl::ShaderModel::V5_1;
                let shader_code = Self::translate_spirv(&mut ast, shader_model, layout, stage)?;
                debug!("SPIRV-Cross generated shader:\n{}", shader_code);

                let real_name = ast
                    .get_cleansed_entry_point_name(source.entry, conv::map_stage(stage))
                    .map_err(gen_query_error)?;
                // TODO: opt: don't query *all* entry points.
                let entry_points = ast.get_entry_points().map_err(gen_query_error)?;
                entry_points
                    .iter()
                    .find(|entry_point| entry_point.name == real_name)
                    .ok_or(d::ShaderError::MissingEntryPoint(source.entry.into()))
                    .and_then(|entry_point| {
                        let stage = conv::map_execution_model(entry_point.execution_model);
                        let shader = compile_shader(
                            stage,
                            shader_model,
                            &entry_point.name,
                            shader_code.as_bytes(),
                        )?;
                        Ok((shader, true))
                    })
            }
        }
    }

    /// Create a shader module from HLSL with a single entry point
    pub fn create_shader_module_from_source(
        &self,
        stage: pso::Stage,
        hlsl_entry: &str,
        entry_point: &str,
        code: &[u8],
    ) -> Result<r::ShaderModule, d::ShaderError> {
        let mut shader_map = BTreeMap::new();
        let blob = compile_shader(stage, hlsl::ShaderModel::V5_1, hlsl_entry, code)?;
        shader_map.insert(entry_point.into(), blob);
        Ok(r::ShaderModule::Compiled(shader_map))
    }

    pub(crate) fn create_command_signature(
        device: native::Device,
        ty: CommandSignature,
    ) -> native::CommandSignature {
        let (arg, stride) = match ty {
            CommandSignature::Draw => (IndirectArgument::draw(), 16),
            CommandSignature::DrawIndexed => (IndirectArgument::draw_indexed(), 20),
            CommandSignature::Dispatch => (IndirectArgument::dispatch(), 12),
        };

        let (signature, hr) =
            device.create_command_signature(native::RootSignature::null(), &[arg], stride, 0);

        if !winerror::SUCCEEDED(hr) {
            error!("error on command signature creation: {:x}", hr);
        }
        signature
    }

    pub(crate) fn create_descriptor_heap_impl(
        device: native::Device,
        heap_type: descriptor::HeapType,
        shader_visible: bool,
        capacity: usize,
    ) -> r::DescriptorHeap {
        assert_ne!(capacity, 0);

        let (heap, hr) = device.create_descriptor_heap(
            capacity as _,
            heap_type,
            if shader_visible {
                descriptor::HeapFlags::SHADER_VISIBLE
            } else {
                descriptor::HeapFlags::empty()
            },
            0,
        );

        let descriptor_size = device.get_descriptor_increment_size(heap_type);
        let cpu_handle = heap.start_cpu_descriptor();
        let gpu_handle = heap.start_gpu_descriptor();

        let range_allocator = RangeAllocator::new(0..(capacity as u64));

        r::DescriptorHeap {
            raw: heap,
            handle_size: descriptor_size as _,
            total_handles: capacity as _,
            start: r::DualHandle {
                cpu: cpu_handle,
                gpu: gpu_handle,
                size: 0,
            },
            range_allocator,
        }
    }

    pub(crate) fn view_image_as_render_target_impl(
        device: native::Device,
        handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
        info: ViewInfo,
    ) -> Result<(), image::ViewError> {
        #![allow(non_snake_case)]

        let mut desc = d3d12::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: info.format,
            ViewDimension: 0,
            u: unsafe { mem::zeroed() },
        };

        let MipSlice = info.range.levels.start as _;
        let FirstArraySlice = info.range.layers.start as _;
        let ArraySize = (info.range.layers.end - info.range.layers.start) as _;
        assert_eq!(info.range.levels.start + 1, info.range.levels.end);
        assert!(info.range.layers.end <= info.kind.num_layers());
        let is_msaa = info.kind.num_samples() > 1;

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE1D;
                *unsafe { desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_RTV { MipSlice }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE1DARRAY;
                *unsafe { desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_RTV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DMS;
                *unsafe { desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_RTV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2D;
                *unsafe { desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_RTV {
                    MipSlice,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe { desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_RTV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DARRAY;
                *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_RTV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D3 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE3D;
                *unsafe { desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_RTV {
                    MipSlice,
                    FirstWSlice: 0,
                    WSize: info.kind.extent().depth as _,
                }
            }
            image::ViewKind::Cube | image::ViewKind::CubeArray => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DARRAY;
                //TODO: double-check if any *6 are needed
                *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_RTV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                }
            }
        };

        unsafe {
            device.CreateRenderTargetView(info.resource.as_mut_ptr(), &desc, handle);
        }

        Ok(())
    }

    fn view_image_as_render_target(
        &self,
        info: ViewInfo,
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        let handle = self.rtv_pool.lock().unwrap().alloc_handle();
        Self::view_image_as_render_target_impl(self.raw, handle, info).map(|_| handle)
    }

    pub(crate) fn view_image_as_depth_stencil_impl(
        device: native::Device,
        handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
        info: ViewInfo,
    ) -> Result<(), image::ViewError> {
        #![allow(non_snake_case)]

        let mut desc = d3d12::D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: info.format,
            ViewDimension: 0,
            Flags: 0,
            u: unsafe { mem::zeroed() },
        };

        let MipSlice = info.range.levels.start as _;
        let FirstArraySlice = info.range.layers.start as _;
        let ArraySize = (info.range.layers.end - info.range.layers.start) as _;
        assert_eq!(info.range.levels.start + 1, info.range.levels.end);
        assert!(info.range.layers.end <= info.kind.num_layers());
        let is_msaa = info.kind.num_samples() > 1;

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE1D;
                *unsafe { desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_DSV { MipSlice }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE1DARRAY;
                *unsafe { desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_DSV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DMS;
                *unsafe { desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_DSV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2D;
                *unsafe { desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_DSV { MipSlice }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe { desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_DSV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DARRAY;
                *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_DSV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D3 | image::ViewKind::Cube | image::ViewKind::CubeArray => {
                unimplemented!()
            }
        };

        unsafe {
            device.CreateDepthStencilView(info.resource.as_mut_ptr(), &desc, handle);
        }

        Ok(())
    }

    fn view_image_as_depth_stencil(
        &self,
        info: ViewInfo,
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        let handle = self.dsv_pool.lock().unwrap().alloc_handle();
        Self::view_image_as_depth_stencil_impl(self.raw, handle, info).map(|_| handle)
    }

    pub(crate) fn build_image_as_shader_resource_desc(
        info: &ViewInfo,
    ) -> Result<d3d12::D3D12_SHADER_RESOURCE_VIEW_DESC, image::ViewError> {
        #![allow(non_snake_case)]

        let mut desc = d3d12::D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: info.format,
            ViewDimension: 0,
            Shader4ComponentMapping: 0x1688, // TODO: map swizzle
            u: unsafe { mem::zeroed() },
        };

        let MostDetailedMip = info.range.levels.start as _;
        let MipLevels = (info.range.levels.end - info.range.levels.start) as _;
        let FirstArraySlice = info.range.layers.start as _;
        let ArraySize = (info.range.layers.end - info.range.layers.start) as _;

        assert!(info.range.layers.end <= info.kind.num_layers());
        let is_msaa = info.kind.num_samples() > 1;
        let is_cube = info.caps.contains(image::ViewCapabilities::KIND_CUBE);

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE1D;
                *unsafe { desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE1DARRAY;
                *unsafe { desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_SRV {
                    MostDetailedMip,
                    MipLevels,
                    FirstArraySlice,
                    ArraySize,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DMS;
                *unsafe { desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_SRV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2D;
                *unsafe { desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    PlaneSlice: 0, //TODO
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe { desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_SRV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DARRAY;
                *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_SRV {
                    MostDetailedMip,
                    MipLevels,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D3 => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE3D;
                *unsafe { desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::Cube if is_cube => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURECUBE;
                *unsafe { desc.u.TextureCube_mut() } = d3d12::D3D12_TEXCUBE_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::CubeArray if is_cube => {
                assert_eq!(0, ArraySize % 6);
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURECUBEARRAY;
                *unsafe { desc.u.TextureCubeArray_mut() } = d3d12::D3D12_TEXCUBE_ARRAY_SRV {
                    MostDetailedMip,
                    MipLevels,
                    First2DArrayFace: FirstArraySlice,
                    NumCubes: ArraySize / 6,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::Cube | image::ViewKind::CubeArray => {
                error!(
                    "Cube views are not supported for the image, kind: {:?}",
                    info.kind
                );
                return Err(image::ViewError::BadKind);
            }
        }

        Ok(desc)
    }

    fn view_image_as_shader_resource(
        &self,
        mut info: ViewInfo,
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        #![allow(non_snake_case)]

        // Depth-stencil formats can't be used for SRVs.
        info.format = match info.format {
            dxgiformat::DXGI_FORMAT_D16_UNORM => dxgiformat::DXGI_FORMAT_R16_UNORM,
            dxgiformat::DXGI_FORMAT_D32_FLOAT => dxgiformat::DXGI_FORMAT_R32_FLOAT,
            format => format,
        };

        let desc = Self::build_image_as_shader_resource_desc(&info)?;
        let handle = self.srv_uav_pool.lock().unwrap().alloc_handle();
        unsafe {
            self.raw
                .CreateShaderResourceView(info.resource.as_mut_ptr(), &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_storage(
        &self,
        info: ViewInfo,
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        #![allow(non_snake_case)]
        assert_eq!(info.range.levels.start + 1, info.range.levels.end);

        let mut desc = d3d12::D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: info.format,
            ViewDimension: 0,
            u: unsafe { mem::zeroed() },
        };

        let MipSlice = info.range.levels.start as _;
        let FirstArraySlice = info.range.layers.start as _;
        let ArraySize = (info.range.layers.end - info.range.layers.start) as _;

        assert!(info.range.layers.end <= info.kind.num_layers());
        if info.kind.num_samples() > 1 {
            error!("MSAA images can't be viewed as UAV");
            return Err(image::ViewError::Unsupported);
        }

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE1D;
                *unsafe { desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_UAV { MipSlice }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE1DARRAY;
                *unsafe { desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_UAV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE2D;
                *unsafe { desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_UAV {
                    MipSlice,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE2DARRAY;
                *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_UAV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D3 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE3D;
                *unsafe { desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_UAV {
                    MipSlice,
                    FirstWSlice: 0,
                    WSize: info.kind.extent().depth as _,
                }
            }
            image::ViewKind::Cube | image::ViewKind::CubeArray => {
                error!("Cubic images can't be viewed as UAV");
                return Err(image::ViewError::Unsupported);
            }
        }

        let handle = self.srv_uav_pool.lock().unwrap().alloc_handle();
        unsafe {
            self.raw.CreateUnorderedAccessView(
                info.resource.as_mut_ptr(),
                ptr::null_mut(),
                &desc,
                handle,
            );
        }

        Ok(handle)
    }

    pub(crate) fn create_raw_fence(&self, signalled: bool) -> native::Fence {
        let mut handle = native::Fence::null();
        assert_eq!(winerror::S_OK, unsafe {
            self.raw.CreateFence(
                if signalled { 1 } else { 0 },
                d3d12::D3D12_FENCE_FLAG_NONE,
                &d3d12::ID3D12Fence::uuidof(),
                handle.mut_void(),
            )
        });
        handle
    }
}

impl d::Device<B> for Device {
    fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<r::Memory, d::OutOfMemory> {
        let mem_type = mem_type.0;
        let mem_base_id = mem_type % NUM_HEAP_PROPERTIES;
        let heap_property = &self.heap_properties[mem_base_id];

        let properties = d3d12::D3D12_HEAP_PROPERTIES {
            Type: d3d12::D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: heap_property.page_property,
            MemoryPoolPreference: heap_property.memory_pool,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        };

        // Exposed memory types are grouped according to their capabilities.
        // See `MemoryGroup` for more details.
        let mem_group = mem_type / NUM_HEAP_PROPERTIES;

        let desc = d3d12::D3D12_HEAP_DESC {
            SizeInBytes: size,
            Properties: properties,
            Alignment: d3d12::D3D12_DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT as _, // TODO: not always..?
            Flags: match mem_group {
                0 => d3d12::D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES,
                1 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
                2 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES,
                3 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES,
                _ => unreachable!(),
            },
        };

        let mut heap = native::Heap::null();
        let hr = unsafe {
            self.raw
                .clone()
                .CreateHeap(&desc, &d3d12::ID3D12Heap::uuidof(), heap.mut_void())
        };
        if hr == winerror::E_OUTOFMEMORY {
            return Err(d::OutOfMemory);
        }
        assert_eq!(winerror::S_OK, hr);

        // The first memory heap of each group corresponds to the default heap, which is can never
        // be mapped.
        // Devices supporting heap tier 1 can only created buffers on mem group 1 (ALLOW_ONLY_BUFFERS).
        // Devices supporting heap tier 2 always expose only mem group 0 and don't have any further restrictions.
        let is_mapable = mem_base_id != 0
            && (mem_group == MemoryGroup::Universal as _
                || mem_group == MemoryGroup::BufferOnly as _);

        // Create a buffer resource covering the whole memory slice to be able to map the whole memory.
        let resource = if is_mapable {
            let mut resource = native::Resource::null();
            let desc = d3d12::D3D12_RESOURCE_DESC {
                Dimension: d3d12::D3D12_RESOURCE_DIMENSION_BUFFER,
                Alignment: 0,
                Width: size,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: dxgiformat::DXGI_FORMAT_UNKNOWN,
                SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: d3d12::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: d3d12::D3D12_RESOURCE_FLAG_NONE,
            };

            assert_eq!(winerror::S_OK, unsafe {
                self.raw.clone().CreatePlacedResource(
                    heap.as_mut_ptr(),
                    0,
                    &desc,
                    d3d12::D3D12_RESOURCE_STATE_COMMON,
                    ptr::null(),
                    &d3d12::ID3D12Resource::uuidof(),
                    resource.mut_void(),
                )
            });

            Some(resource)
        } else {
            None
        };

        Ok(r::Memory {
            heap,
            type_id: mem_type,
            size,
            resource,
        })
    }

    fn create_command_pool(
        &self,
        family: QueueFamilyId,
        create_flags: CommandPoolCreateFlags,
    ) -> RawCommandPool {
        let list_type = QUEUE_FAMILIES[family.0].native_type();

        let allocator = if create_flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            // Allocators are created per individual ID3D12GraphicsCommandList
            CommandPoolAllocator::Individual(Vec::new())
        } else {
            let (command_allocator, hr) = self.raw.create_command_allocator(list_type);

            // TODO: error handling
            if !winerror::SUCCEEDED(hr) {
                error!("error on command allocator creation: {:x}", hr);
            }

            CommandPoolAllocator::Shared(command_allocator)
        };

        RawCommandPool {
            allocator,
            device: self.raw,
            list_type,
            shared: self.shared.clone(),
        }
    }

    fn destroy_command_pool(&self, pool: RawCommandPool) {
        pool.destroy();
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> r::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum SubState {
            New(d3d12::D3D12_RESOURCE_STATES),
            // Color attachment which will be resolved at the end of the subpass
            Resolve(d3d12::D3D12_RESOURCE_STATES),
            Preserve,
            Undefined,
        }
        struct AttachmentInfo {
            sub_states: Vec<SubState>,
            target_state: d3d12::D3D12_RESOURCE_STATES,
            last_state: d3d12::D3D12_RESOURCE_STATES,
            barrier_start_index: usize,
        }

        let attachments = attachments
            .into_iter()
            .map(|attachment| attachment.borrow().clone())
            .collect::<Vec<_>>();
        let subpasses = subpasses.into_iter().collect::<Vec<_>>();
        let dependencies = dependencies.into_iter().collect::<Vec<_>>();
        let mut att_infos = attachments
            .iter()
            .map(|att| AttachmentInfo {
                sub_states: vec![SubState::Undefined; subpasses.len()],
                target_state: if att.format.map_or(false, |f| f.is_depth()) {
                    d3d12::D3D12_RESOURCE_STATE_DEPTH_WRITE //TODO?
                } else {
                    d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET
                },
                last_state: conv::map_image_resource_state(
                    image::Access::empty(),
                    att.layouts.start,
                ),
                barrier_start_index: 0,
            })
            .collect::<Vec<_>>();

        // Fill out subpass known layouts
        for (sid, sub) in subpasses.iter().enumerate() {
            let sub = sub.borrow();
            for (i, &(id, _layout)) in sub.colors.iter().enumerate() {
                let dst_state = att_infos[id].target_state;
                let state = match sub.resolves.get(i) {
                    Some(_) => SubState::Resolve(dst_state),
                    None => SubState::New(dst_state),
                };
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &(id, _layout) in sub.depth_stencil {
                let state = SubState::New(att_infos[id].target_state);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &(id, _layout) in sub.inputs {
                let state = SubState::New(d3d12::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &(id, _layout) in sub.resolves {
                let state = SubState::New(d3d12::D3D12_RESOURCE_STATE_RESOLVE_DEST);
                let old = mem::replace(&mut att_infos[id].sub_states[sid], state);
                debug_assert_eq!(SubState::Undefined, old);
            }
            for &id in sub.preserves {
                let old = mem::replace(&mut att_infos[id].sub_states[sid], SubState::Preserve);
                debug_assert_eq!(SubState::Undefined, old);
            }
        }

        let mut deps_left = vec![0u16; subpasses.len()];
        for dep in &dependencies {
            let dep = dep.borrow();
            //Note: self-dependencies are ignored
            if dep.passes.start != dep.passes.end && dep.passes.start != pass::SubpassRef::External
            {
                if let pass::SubpassRef::Pass(sid) = dep.passes.end {
                    deps_left[sid] += 1;
                }
            }
        }

        let mut rp = r::RenderPass {
            attachments: attachments.clone(),
            subpasses: Vec::new(),
            post_barriers: Vec::new(),
        };

        while let Some(sid) = deps_left.iter().position(|count| *count == 0) {
            deps_left[sid] = !0; // mark as done
            for dep in &dependencies {
                let dep = dep.borrow();
                if dep.passes.start != dep.passes.end
                    && dep.passes.start == pass::SubpassRef::Pass(sid)
                {
                    if let pass::SubpassRef::Pass(other) = dep.passes.end {
                        deps_left[other] -= 1;
                    }
                }
            }

            // Subpass barriers
            let mut pre_barriers = Vec::new();
            let mut post_barriers = Vec::new();
            for (att_id, ai) in att_infos.iter_mut().enumerate() {
                // Barrier from previous subpass to current or following subpasses.
                match ai.sub_states[sid] {
                    SubState::Preserve => {
                        ai.barrier_start_index = rp.subpasses.len() + 1;
                    }
                    SubState::New(state) if state != ai.last_state => {
                        let barrier = r::BarrierDesc::new(att_id, ai.last_state..state);
                        match rp.subpasses.get_mut(ai.barrier_start_index) {
                            Some(past_subpass) => {
                                let split = barrier.split();
                                past_subpass.pre_barriers.push(split.start);
                                pre_barriers.push(split.end);
                            }
                            None => pre_barriers.push(barrier),
                        }
                        ai.last_state = state;
                        ai.barrier_start_index = rp.subpasses.len() + 1;
                    }
                    SubState::Resolve(state) => {
                        // 1. Standard pre barrier to update state from previous pass into desired substate.
                        if state != ai.last_state {
                            let barrier = r::BarrierDesc::new(att_id, ai.last_state..state);
                            match rp.subpasses.get_mut(ai.barrier_start_index) {
                                Some(past_subpass) => {
                                    let split = barrier.split();
                                    past_subpass.pre_barriers.push(split.start);
                                    pre_barriers.push(split.end);
                                }
                                None => pre_barriers.push(barrier),
                            }
                        }

                        // 2. Post Barrier at the end of the subpass into RESOLVE_SOURCE.
                        let resolve_state = d3d12::D3D12_RESOURCE_STATE_RESOLVE_SOURCE;
                        let barrier = r::BarrierDesc::new(att_id, state..resolve_state);
                        post_barriers.push(barrier);

                        ai.last_state = resolve_state;
                        ai.barrier_start_index = rp.subpasses.len() + 1;
                    }
                    _ => {}
                };
            }

            rp.subpasses.push(r::SubpassDesc {
                color_attachments: subpasses[sid].borrow().colors.iter().cloned().collect(),
                depth_stencil_attachment: subpasses[sid].borrow().depth_stencil.cloned(),
                input_attachments: subpasses[sid].borrow().inputs.iter().cloned().collect(),
                resolve_attachments: subpasses[sid].borrow().resolves.iter().cloned().collect(),
                pre_barriers,
                post_barriers,
            });
        }
        // if this fails, our graph has cycles
        assert_eq!(rp.subpasses.len(), subpasses.len());
        assert!(deps_left.into_iter().all(|count| count == !0));

        // take care of the post-pass transitions at the end of the renderpass.
        for (att_id, (ai, att)) in att_infos.iter().zip(attachments.iter()).enumerate() {
            let state_dst = conv::map_image_resource_state(image::Access::empty(), att.layouts.end);
            if state_dst == ai.last_state {
                continue;
            }
            let barrier = r::BarrierDesc::new(att_id, ai.last_state..state_dst);
            match rp.subpasses.get_mut(ai.barrier_start_index) {
                Some(past_subpass) => {
                    let split = barrier.split();
                    past_subpass.pre_barriers.push(split.start);
                    rp.post_barriers.push(split.end);
                }
                None => rp.post_barriers.push(barrier),
            }
        }

        rp
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        sets: IS,
        push_constant_ranges: IR,
    ) -> r::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<r::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        // Pipeline layouts are implemented as RootSignature for D3D12.
        //
        // Each descriptor set layout will be one table entry of the root signature.
        // We have the additional restriction that SRV/CBV/UAV and samplers need to be
        // separated, so each set layout will actually occupy up to 2 entries!
        //
        // Root signature layout:
        //     Root Constants: Register: Offest/4, Space: 0
        //       ...
        //     DescriptorTable0: Space: 2 (+1) (SrvCbvUav)
        //     DescriptorTable0: Space: 3 (+1) (Sampler)
        //     DescriptorTable1: Space: 4 (+1) (SrvCbvUav)
        //     ...

        let sets = sets.into_iter().collect::<Vec<_>>();
        let root_constants = root_constants::split(push_constant_ranges)
            .iter()
            .map(|constant| {
                assert!(constant.range.start <= constant.range.end);
                RootConstant {
                    stages: constant.stages,
                    range: constant.range.start..constant.range.end,
                }
            })
            .collect::<Vec<_>>();

        // guarantees that no re-allocation is done, and our pointers are valid
        let mut parameters = Vec::with_capacity(root_constants.len() + sets.len() * 2);

        for root_constant in root_constants.iter() {
            parameters.push(native::descriptor::RootParameter::constants(
                native::descriptor::ShaderVisibility::All, // TODO
                root_constant.range.start as _,
                ROOT_CONSTANT_SPACE,
                (root_constant.range.end - root_constant.range.start) as _,
            ));
        }

        // Offest of `spaceN` for descriptor tables. Root constants will be in
        // `space0`.
        let table_space_offset = if !root_constants.is_empty() { 1 } else { 0 };

        // Collect the whole number of bindings we will create upfront.
        // It allows us to preallocate enough storage to avoid reallocation,
        // which could cause invalid pointers.
        let total = sets
            .iter()
            .map(|desc_set| {
                let mut sum = 0;
                let bindings = &desc_set.borrow().bindings;

                for binding in bindings {
                    sum += if binding.ty == pso::DescriptorType::CombinedImageSampler {
                        2
                    } else {
                        1
                    };
                }

                sum
            })
            .sum();
        let mut ranges = Vec::with_capacity(total);
        let mut set_tables = Vec::with_capacity(sets.len());

        for (i, set) in sets.iter().enumerate() {
            let set = set.borrow();
            let mut table_type = r::SetTableTypes::empty();

            let range_base = ranges.len();
            ranges.extend(
                set.bindings
                    .iter()
                    .filter(|bind| bind.ty != pso::DescriptorType::Sampler)
                    .map(|bind| {
                        conv::map_descriptor_range(bind, (table_space_offset + i) as u32, false)
                    }),
            );

            if ranges.len() > range_base {
                parameters.push(native::descriptor::RootParameter::descriptor_table(
                    native::descriptor::ShaderVisibility::All, // TODO
                    &ranges[range_base..],
                ));
                table_type |= r::SRV_CBV_UAV;
            }

            let range_base = ranges.len();
            ranges.extend(
                set.bindings
                    .iter()
                    .filter(|bind| {
                        bind.ty == pso::DescriptorType::Sampler
                            || bind.ty == pso::DescriptorType::CombinedImageSampler
                    })
                    .map(|bind| {
                        conv::map_descriptor_range(bind, (table_space_offset + i) as u32, true)
                    }),
            );

            if ranges.len() > range_base {
                parameters.push(native::descriptor::RootParameter::descriptor_table(
                    native::descriptor::ShaderVisibility::All, // TODO
                    &ranges[range_base..],
                ));
                table_type |= r::SAMPLERS;
            }

            set_tables.push(table_type);
        }

        // Ensure that we didn't reallocate!
        debug_assert_eq!(ranges.len(), total);

        // TODO: error handling
        let ((signature_raw, error), _hr) = native::RootSignature::serialize(
            native::descriptor::RootSignatureVersion::V1_0,
            &parameters,
            &[],
            native::descriptor::RootSignatureFlags::ALLOW_IA_INPUT_LAYOUT,
        );

        if !error.is_null() {
            error!("Root signature serialization error: {:?}", unsafe {
                error.as_c_str().to_str().unwrap()
            });
            unsafe {
                error.destroy();
            }
        }

        // TODO: error handling
        let (signature, _hr) = self.raw.create_root_signature(signature_raw, 0);
        unsafe {
            signature_raw.destroy();
        }

        r::PipelineLayout {
            raw: signature,
            tables: set_tables,
            root_constants,
            num_parameter_slots: parameters.len(),
        }
    }

    fn create_pipeline_cache(&self) -> () {
        ()
    }

    fn destroy_pipeline_cache(&self, _: ()) {
        //empty
    }

    fn merge_pipeline_caches<I>(&self, _: &(), _: I)
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
    {
        //empty
    }

    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<r::GraphicsPipeline, pso::CreationError> {
        enum ShaderBc {
            Owned(native::Blob),
            Borrowed(native::Blob),
            None,
        }
        impl ShaderBc {
            pub fn shader(&self) -> native::Shader {
                match *self {
                    ShaderBc::Owned(ref bc) | ShaderBc::Borrowed(ref bc) => {
                        native::Shader::from_blob(*bc)
                    }
                    ShaderBc::None => native::Shader::null(),
                }
            }
        }

        let build_shader = |stage: pso::Stage, source: Option<&pso::EntryPoint<'a, B>>| {
            let source = match source {
                Some(src) => src,
                None => return Ok(ShaderBc::None),
            };

            match Self::extract_entry_point(stage, source, desc.layout) {
                Ok((shader, true)) => Ok(ShaderBc::Owned(shader)),
                Ok((shader, false)) => Ok(ShaderBc::Borrowed(shader)),
                Err(err) => Err(pso::CreationError::Shader(err)),
            }
        };

        let vs = build_shader(pso::Stage::Vertex, Some(&desc.shaders.vertex))?;
        let ps = build_shader(pso::Stage::Fragment, desc.shaders.fragment.as_ref())?;
        let gs = build_shader(pso::Stage::Geometry, desc.shaders.geometry.as_ref())?;
        let ds = build_shader(pso::Stage::Domain, desc.shaders.domain.as_ref())?;
        let hs = build_shader(pso::Stage::Hull, desc.shaders.hull.as_ref())?;

        // Rebind vertex buffers, see native.rs for more details.
        let mut vertex_bindings = [None; MAX_VERTEX_BUFFERS];
        let mut vertex_strides = [0; MAX_VERTEX_BUFFERS];

        for buffer in &desc.vertex_buffers {
            vertex_strides[buffer.binding as usize] = buffer.stride;
        }
        // Fill in identity mapping where we don't need to adjust anything.
        for attrib in &desc.attributes {
            let binding = attrib.binding as usize;
            let stride = vertex_strides[attrib.binding as usize];
            if attrib.element.offset < stride {
                vertex_bindings[binding] = Some(r::VertexBinding {
                    stride: vertex_strides[attrib.binding as usize],
                    offset: 0,
                    mapped_binding: binding,
                });
            }
        }

        // Define input element descriptions
        let input_element_descs = desc
            .attributes
            .iter()
            .filter_map(|attrib| {
                let buffer_desc = match desc
                    .vertex_buffers
                    .iter()
                    .find(|buffer_desc| buffer_desc.binding == attrib.binding)
                {
                    Some(buffer_desc) => buffer_desc,
                    None => {
                        error!(
                            "Couldn't find associated vertex buffer description {:?}",
                            attrib.binding
                        );
                        return Some(Err(pso::CreationError::Other));
                    }
                };

                let slot_class = match buffer_desc.rate {
                    0 => d3d12::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    _ => d3d12::D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                };
                let format = attrib.element.format;

                // Check if we need to add a new remapping in-case the offset is
                // higher than the vertex stride.
                // In this case we rebase the attribute to zero offset.
                let binding = attrib.binding as usize;
                let stride = vertex_strides[binding];
                let offset = attrib.element.offset;
                let (input_slot, offset) = if stride <= offset {
                    // Number of input attributes may not exceed bindings, see limits.
                    // We will always find at least one free binding.
                    let mapping = vertex_bindings.iter().position(Option::is_none).unwrap();
                    vertex_bindings[mapping] = Some(r::VertexBinding {
                        stride: vertex_strides[binding],
                        offset: offset,
                        mapped_binding: binding,
                    });

                    (mapping, 0)
                } else {
                    (binding, offset)
                };

                Some(Ok(d3d12::D3D12_INPUT_ELEMENT_DESC {
                    SemanticName: "TEXCOORD\0".as_ptr() as *const _, // Semantic name used by SPIRV-Cross
                    SemanticIndex: attrib.location,
                    Format: match conv::map_format(format) {
                        Some(fm) => fm,
                        None => {
                            error!("Unable to find DXGI format for {:?}", format);
                            return Some(Err(pso::CreationError::Other));
                        }
                    },
                    InputSlot: input_slot as _,
                    AlignedByteOffset: offset,
                    InputSlotClass: slot_class,
                    InstanceDataStepRate: buffer_desc.rate as _,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

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
            let mut rtvs = [dxgiformat::DXGI_FORMAT_UNKNOWN; 8];
            let mut num_rtvs = 0;
            for (rtv, target) in rtvs.iter_mut().zip(pass.color_attachments.iter()) {
                let format = desc.subpass.main_pass.attachments[target.0].format;
                *rtv = format
                    .and_then(conv::map_format)
                    .unwrap_or(dxgiformat::DXGI_FORMAT_UNKNOWN);
                num_rtvs += 1;
            }
            (rtvs, num_rtvs)
        };

        let sample_desc = dxgitype::DXGI_SAMPLE_DESC {
            Count: match desc.multisampling {
                Some(ref ms) => ms.rasterization_samples as _,
                None => 1,
            },
            Quality: 0,
        };

        // Setup pipeline description
        let pso_desc = d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: desc.layout.raw.as_mut_ptr(),
            VS: *vs.shader(),
            PS: *ps.shader(),
            GS: *gs.shader(),
            DS: *ds.shader(),
            HS: *hs.shader(),
            StreamOutput: d3d12::D3D12_STREAM_OUTPUT_DESC {
                pSODeclaration: ptr::null(),
                NumEntries: 0,
                pBufferStrides: ptr::null(),
                NumStrides: 0,
                RasterizedStream: 0,
            },
            BlendState: d3d12::D3D12_BLEND_DESC {
                AlphaToCoverageEnable: desc.multisampling.as_ref().map_or(FALSE, |ms| {
                    if ms.alpha_coverage {
                        TRUE
                    } else {
                        FALSE
                    }
                }),
                IndependentBlendEnable: TRUE,
                RenderTarget: conv::map_render_targets(&desc.blender.targets),
            },
            SampleMask: UINT::max_value(),
            RasterizerState: conv::map_rasterizer(&desc.rasterizer),
            DepthStencilState: conv::map_depth_stencil(&desc.depth_stencil),
            InputLayout: d3d12::D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: if input_element_descs.is_empty() {
                    ptr::null()
                } else {
                    input_element_descs.as_ptr()
                },
                NumElements: input_element_descs.len() as u32,
            },
            IBStripCutValue: d3d12::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED, // TODO
            PrimitiveTopologyType: conv::map_topology_type(desc.input_assembler.primitive),
            NumRenderTargets: num_rtvs,
            RTVFormats: rtvs,
            DSVFormat: pass
                .depth_stencil_attachment
                .and_then(|att_ref| {
                    desc.subpass.main_pass.attachments[att_ref.0]
                        .format
                        .and_then(|f| conv::map_format_dsv(f.base_format().0))
                })
                .unwrap_or(dxgiformat::DXGI_FORMAT_UNKNOWN),
            SampleDesc: sample_desc,
            NodeMask: 0,
            CachedPSO: d3d12::D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: d3d12::D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        let topology = conv::map_topology(desc.input_assembler.primitive);

        // Create PSO
        let mut pipeline = native::PipelineState::null();
        let hr = if desc.depth_stencil.depth_bounds {
            // The DepthBoundsTestEnable option isn't available in the original D3D12_GRAPHICS_PIPELINE_STATE_DESC struct.
            // Instead, we must use the newer subobject stream method.
            let (device2, hr) = unsafe { self.raw.cast::<d3d12::ID3D12Device2>() };
            if winerror::SUCCEEDED(hr) {
                let mut pss_stream = GraphicsPipelineStateSubobjectStream::new(&pso_desc, true);
                let pss_desc = d3d12::D3D12_PIPELINE_STATE_STREAM_DESC {
                    SizeInBytes: mem::size_of_val(&pss_stream),
                    pPipelineStateSubobjectStream: &mut pss_stream as *mut _ as _,
                };
                unsafe {
                    device2.CreatePipelineState(
                        &pss_desc,
                        &d3d12::ID3D12PipelineState::uuidof(),
                        pipeline.mut_void(),
                    )
                }
            } else {
                hr
            }
        } else {
            unsafe {
                self.raw.clone().CreateGraphicsPipelineState(
                    &pso_desc,
                    &d3d12::ID3D12PipelineState::uuidof(),
                    pipeline.mut_void(),
                )
            }
        };

        let destroy_shader = |shader: ShaderBc| {
            if let ShaderBc::Owned(bc) = shader {
                unsafe {
                    bc.destroy();
                }
            }
        };

        destroy_shader(vs);
        destroy_shader(ps);
        destroy_shader(gs);
        destroy_shader(hs);
        destroy_shader(ds);

        if winerror::SUCCEEDED(hr) {
            let mut baked_states = desc.baked_states.clone();
            if !desc.depth_stencil.depth_bounds {
                baked_states.depth_bounds = None;
            }

            Ok(r::GraphicsPipeline {
                raw: pipeline,
                signature: desc.layout.raw,
                num_parameter_slots: desc.layout.num_parameter_slots,
                topology,
                constants: desc.layout.root_constants.clone(),
                vertex_bindings,
                baked_states,
            })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        _cache: Option<&()>,
    ) -> Result<r::ComputePipeline, pso::CreationError> {
        let (cs, cs_destroy) =
            Self::extract_entry_point(pso::Stage::Compute, &desc.shader, desc.layout)
                .map_err(|err| pso::CreationError::Shader(err))?;

        let (pipeline, hr) = self.raw.create_compute_pipeline_state(
            desc.layout.raw,
            native::Shader::from_blob(cs),
            0,
            CachedPSO::null(),
            PipelineStateFlags::empty(),
        );

        if cs_destroy {
            unsafe {
                cs.destroy();
            }
        }

        if winerror::SUCCEEDED(hr) {
            Ok(r::ComputePipeline {
                raw: pipeline,
                signature: desc.layout.raw,
                num_parameter_slots: desc.layout.num_parameter_slots,
                constants: desc.layout.root_constants.clone(),
            })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_framebuffer<I>(
        &self,
        _renderpass: &r::RenderPass,
        attachments: I,
        extent: image::Extent,
    ) -> Result<r::Framebuffer, d::FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<r::ImageView>,
    {
        Ok(r::Framebuffer {
            attachments: attachments.into_iter().map(|att| *att.borrow()).collect(),
            layers: extent.depth as _,
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<r::ShaderModule, d::ShaderError> {
        Ok(r::ShaderModule::Spirv(raw_data.into()))
    }

    fn create_buffer(
        &self,
        mut size: u64,
        usage: buffer::Usage,
    ) -> Result<UnboundBuffer, buffer::CreationError> {
        if usage.contains(buffer::Usage::UNIFORM) {
            // Constant buffer view sizes need to be aligned.
            // Coupled with the offset alignment we can enforce an aligned CBV size
            // on descriptor updates.
            size = (size + 255) & !255;
        }
        if usage.contains(buffer::Usage::TRANSFER_DST) {
            // minimum of 1 word for the clear UAV
            size = size.max(4);
        }

        let type_mask_shift = if self.private_caps.heterogeneous_resource_heaps {
            MEM_TYPE_UNIVERSAL_SHIFT
        } else {
            MEM_TYPE_BUFFER_SHIFT
        };

        let requirements = memory::Requirements {
            size,
            alignment: d3d12::D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as u64,
            type_mask: MEM_TYPE_MASK << type_mask_shift,
        };

        Ok(UnboundBuffer {
            requirements,
            usage,
        })
    }

    fn get_buffer_requirements(&self, buffer: &UnboundBuffer) -> Requirements {
        buffer.requirements
    }

    fn bind_buffer_memory(
        &self,
        memory: &r::Memory,
        offset: u64,
        buffer: UnboundBuffer,
    ) -> Result<r::Buffer, d::BindError> {
        if buffer.requirements.type_mask & (1 << memory.type_id) == 0 {
            error!(
                "Bind memory failure: supported mask 0x{:x}, given id {}",
                buffer.requirements.type_mask, memory.type_id
            );
            return Err(d::BindError::WrongMemory);
        }
        if offset + buffer.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds);
        }

        let mut resource = native::Resource::null();
        let desc = d3d12::D3D12_RESOURCE_DESC {
            Dimension: d3d12::D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: buffer.requirements.size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: dxgiformat::DXGI_FORMAT_UNKNOWN,
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: d3d12::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: conv::map_buffer_flags(buffer.usage),
        };

        assert_eq!(winerror::S_OK, unsafe {
            self.raw.clone().CreatePlacedResource(
                memory.heap.as_mut_ptr(),
                offset,
                &desc,
                d3d12::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &d3d12::ID3D12Resource::uuidof(),
                resource.mut_void(),
            )
        });

        let clear_uav = if buffer.usage.contains(buffer::Usage::TRANSFER_DST) {
            let handle = self.srv_uav_pool.lock().unwrap().alloc_handle();
            let mut view_desc = d3d12::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: dxgiformat::DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: d3d12::D3D12_UAV_DIMENSION_BUFFER,
                u: unsafe { mem::zeroed() },
            };

            *unsafe { view_desc.u.Buffer_mut() } = d3d12::D3D12_BUFFER_UAV {
                FirstElement: 0,
                NumElements: (buffer.requirements.size / 4) as _,
                StructureByteStride: 0,
                CounterOffsetInBytes: 0,
                Flags: d3d12::D3D12_BUFFER_UAV_FLAG_RAW,
            };

            unsafe {
                self.raw.CreateUnorderedAccessView(
                    resource.as_mut_ptr(),
                    ptr::null_mut(),
                    &view_desc,
                    handle,
                );
            }
            Some(handle)
        } else {
            None
        };

        Ok(r::Buffer {
            resource,
            size_in_bytes: buffer.requirements.size as _,
            clear_uav,
        })
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        buffer: &r::Buffer,
        format: Option<format::Format>,
        range: R,
    ) -> Result<r::BufferView, buffer::ViewCreationError> {
        let buffer_features = {
            let idx = format.map(|fmt| fmt as usize).unwrap_or(0);
            self.format_properties[idx].buffer_features
        };
        let (format, format_desc) = match format.and_then(conv::map_format) {
            Some(fmt) => (fmt, format.unwrap().surface_desc()),
            None => return Err(buffer::ViewCreationError::UnsupportedFormat { format }),
        };

        let start = *range.start().unwrap_or(&0);
        let end = *range.end().unwrap_or(&(buffer.size_in_bytes as _));

        let bytes_per_texel = (format_desc.bits / 8) as u64;
        // Check if it adheres to the texel buffer offset limit
        assert_eq!(start % bytes_per_texel, 0);
        let first_element = start / bytes_per_texel;
        let num_elements = (end - start) / bytes_per_texel; // rounds down to next smaller size

        let handle_srv = if buffer_features.contains(format::BufferFeature::UNIFORM_TEXEL) {
            let mut desc = d3d12::D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: format,
                ViewDimension: d3d12::D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: 0x1688, // TODO: verify
                u: unsafe { mem::zeroed() },
            };

            *unsafe { desc.u.Buffer_mut() } = d3d12::D3D12_BUFFER_SRV {
                FirstElement: first_element,
                NumElements: num_elements as _,
                StructureByteStride: bytes_per_texel as _,
                Flags: d3d12::D3D12_BUFFER_SRV_FLAG_NONE,
            };

            let handle = self.srv_uav_pool.lock().unwrap().alloc_handle();
            unsafe {
                self.raw.clone().CreateShaderResourceView(
                    buffer.resource.as_mut_ptr(),
                    &desc,
                    handle,
                );
            }
            handle
        } else {
            d3d12::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 }
        };

        let handle_uav = if buffer_features.intersects(
            format::BufferFeature::STORAGE_TEXEL | format::BufferFeature::STORAGE_TEXEL_ATOMIC,
        ) {
            let mut desc = d3d12::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: format,
                ViewDimension: d3d12::D3D12_UAV_DIMENSION_BUFFER,
                u: unsafe { mem::zeroed() },
            };

            *unsafe { desc.u.Buffer_mut() } = d3d12::D3D12_BUFFER_UAV {
                FirstElement: first_element,
                NumElements: num_elements as _,
                StructureByteStride: bytes_per_texel as _,
                Flags: d3d12::D3D12_BUFFER_UAV_FLAG_NONE,
                CounterOffsetInBytes: 0,
            };

            let handle = self.srv_uav_pool.lock().unwrap().alloc_handle();
            unsafe {
                self.raw.clone().CreateUnorderedAccessView(
                    buffer.resource.as_mut_ptr(),
                    ptr::null_mut(),
                    &desc,
                    handle,
                );
            }
            handle
        } else {
            d3d12::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 }
        };

        return Ok(r::BufferView {
            handle_srv,
            handle_uav,
        });
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Result<UnboundImage, image::CreationError> {
        assert!(mip_levels <= kind.num_levels());

        let base_format = format.base_format();
        let format_desc = base_format.0.desc();
        let bytes_per_block = (format_desc.bits / 8) as _;
        let block_dim = format_desc.dim;
        let extent = kind.extent();

        let format_properties = &self.format_properties[format as usize];
        let (layout, features) = match tiling {
            image::Tiling::Optimal => (
                d3d12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                format_properties.optimal_tiling,
            ),
            image::Tiling::Linear => (
                d3d12::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                format_properties.linear_tiling,
            ),
        };

        let desc = d3d12::D3D12_RESOURCE_DESC {
            Dimension: match kind {
                image::Kind::D1(..) => d3d12::D3D12_RESOURCE_DIMENSION_TEXTURE1D,
                image::Kind::D2(..) => d3d12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                image::Kind::D3(..) => d3d12::D3D12_RESOURCE_DIMENSION_TEXTURE3D,
            },
            Alignment: 0,
            Width: extent.width as _,
            Height: extent.height as _,
            DepthOrArraySize: if extent.depth > 1 {
                extent.depth as _
            } else {
                kind.num_layers() as _
            },
            MipLevels: mip_levels as _,
            Format: match conv::map_format(format) {
                Some(format) => format,
                None => return Err(image::CreationError::Format(format)),
            },
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: kind.num_samples() as _,
                Quality: 0,
            },
            Layout: layout,
            Flags: conv::map_image_flags(usage, features),
        };

        let alloc_info = unsafe { self.raw.clone().GetResourceAllocationInfo(0, 1, &desc) };

        // Image usages which require RT/DS heap due to internal implementation.
        let target_usage = image::Usage::COLOR_ATTACHMENT
            | image::Usage::DEPTH_STENCIL_ATTACHMENT
            | image::Usage::TRANSFER_DST;

        let type_mask_shift = if self.private_caps.heterogeneous_resource_heaps {
            MEM_TYPE_UNIVERSAL_SHIFT
        } else if usage.intersects(target_usage) {
            MEM_TYPE_TARGET_SHIFT
        } else {
            MEM_TYPE_IMAGE_SHIFT
        };

        Ok(UnboundImage {
            dsv_format: conv::map_format_dsv(base_format.0).unwrap_or(desc.Format),
            desc,
            requirements: memory::Requirements {
                size: alloc_info.SizeInBytes,
                alignment: alloc_info.Alignment,
                type_mask: MEM_TYPE_MASK << type_mask_shift,
            },
            format,
            kind,
            usage,
            tiling,
            view_caps,
            bytes_per_block,
            block_dim,
            num_levels: mip_levels,
        })
    }

    fn get_image_requirements(&self, image: &UnboundImage) -> Requirements {
        image.requirements
    }

    fn get_image_subresource_footprint(
        &self,
        image: &r::Image,
        sub: image::Subresource,
    ) -> image::SubresourceFootprint {
        let mut num_rows = 0;
        let mut total_bytes = 0;
        let footprint = unsafe {
            let mut footprint = mem::zeroed();
            let desc = (*image.resource).GetDesc();
            self.raw.GetCopyableFootprints(
                &desc,
                image.calc_subresource(sub.level as _, sub.layer as _, 0),
                1,
                0,
                &mut footprint,
                &mut num_rows,
                ptr::null_mut(), // row size in bytes
                &mut total_bytes,
            );
            footprint
        };

        let depth_pitch = (footprint.Footprint.RowPitch * num_rows) as buffer::Offset;
        let array_pitch = footprint.Footprint.Depth as buffer::Offset * depth_pitch;
        image::SubresourceFootprint {
            slice: footprint.Offset..footprint.Offset + total_bytes,
            row_pitch: footprint.Footprint.RowPitch as _,
            depth_pitch,
            array_pitch,
        }
    }

    fn bind_image_memory(
        &self,
        memory: &r::Memory,
        offset: u64,
        image: UnboundImage,
    ) -> Result<r::Image, d::BindError> {
        use self::image::Usage;

        if image.requirements.type_mask & (1 << memory.type_id) == 0 {
            error!(
                "Bind memory failure: supported mask 0x{:x}, given id {}",
                image.requirements.type_mask, memory.type_id
            );
            return Err(d::BindError::WrongMemory);
        }
        if offset + image.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds);
        }

        let mut resource = native::Resource::null();
        let num_layers = image.kind.num_layers();

        assert_eq!(winerror::S_OK, unsafe {
            self.raw.clone().CreatePlacedResource(
                memory.heap.as_mut_ptr(),
                offset,
                &image.desc,
                d3d12::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &d3d12::ID3D12Resource::uuidof(),
                resource.mut_void(),
            )
        });

        let info = ViewInfo {
            resource,
            kind: image.kind,
            caps: image::ViewCapabilities::empty(),
            view_kind: match image.kind {
                image::Kind::D1(..) => image::ViewKind::D1Array,
                image::Kind::D2(..) => image::ViewKind::D2Array,
                image::Kind::D3(..) => image::ViewKind::D3,
            },
            format: image.desc.Format,
            range: image::SubresourceRange {
                aspects: Aspects::empty(),
                levels: 0..0,
                layers: 0..0,
            },
        };

        //TODO: the clear_Xv is incomplete. We should support clearing images created without XXX_ATTACHMENT usage.
        // for this, we need to check the format and force the `RENDER_TARGET` flag behind the user's back
        // if the format supports being rendered into, allowing us to create clear_Xv
        let format_properties = &self.format_properties[image.format as usize];
        let props = match image.tiling {
            image::Tiling::Optimal => format_properties.optimal_tiling,
            image::Tiling::Linear => format_properties.linear_tiling,
        };
        let can_clear_color = image
            .usage
            .intersects(Usage::TRANSFER_DST | Usage::COLOR_ATTACHMENT)
            && props.contains(format::ImageFeature::COLOR_ATTACHMENT);
        let can_clear_depth = image
            .usage
            .intersects(Usage::TRANSFER_DST | Usage::DEPTH_STENCIL_ATTACHMENT)
            && props.contains(format::ImageFeature::DEPTH_STENCIL_ATTACHMENT);
        let aspects = image.format.surface_desc().aspects;

        Ok(r::Image {
            resource: resource,
            place: r::Place::Heap {
                raw: memory.heap.clone(),
                offset,
            },
            surface_type: image.format.base_format().0,
            kind: image.kind,
            usage: image.usage,
            view_caps: image.view_caps,
            descriptor: image.desc,
            bytes_per_block: image.bytes_per_block,
            block_dim: image.block_dim,
            clear_cv: if aspects.contains(Aspects::COLOR) && can_clear_color {
                (0..num_layers)
                    .map(|layer| {
                        self.view_image_as_render_target(ViewInfo {
                            range: image::SubresourceRange {
                                aspects: Aspects::COLOR,
                                levels: 0..1, //TODO?
                                layers: layer..layer + 1,
                            },
                            ..info.clone()
                        }).unwrap()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            clear_dv: if aspects.contains(Aspects::DEPTH) && can_clear_depth {
                (0..num_layers)
                    .map(|layer| {
                        self.view_image_as_depth_stencil(ViewInfo {
                            format: image.dsv_format,
                            range: image::SubresourceRange {
                                aspects: Aspects::DEPTH,
                                levels: 0..1, //TODO?
                                layers: layer..layer + 1,
                            },
                            ..info.clone()
                        }).unwrap()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            clear_sv: if aspects.contains(Aspects::STENCIL) && can_clear_depth {
                (0..num_layers)
                    .map(|layer| {
                        self.view_image_as_depth_stencil(ViewInfo {
                            format: image.dsv_format,
                            range: image::SubresourceRange {
                                aspects: Aspects::STENCIL,
                                levels: 0..1, //TODO?
                                layers: layer..layer + 1,
                            },
                            ..info.clone()
                        }).unwrap()
                    })
                    .collect()
            } else {
                Vec::new()
            },
        })
    }

    fn create_image_view(
        &self,
        image: &r::Image,
        view_kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<r::ImageView, image::ViewError> {
        let mip_levels = (range.levels.start, range.levels.end);
        let layers = (range.layers.start, range.layers.end);

        let info = ViewInfo {
            resource: image.resource,
            kind: image.kind,
            caps: image.view_caps,
            view_kind,
            format: conv::map_format(format).ok_or(image::ViewError::BadFormat)?,
            range,
        };

        Ok(r::ImageView {
            resource: image.resource,
            handle_srv: if image
                .usage
                .intersects(image::Usage::SAMPLED | image::Usage::INPUT_ATTACHMENT)
            {
                Some(self.view_image_as_shader_resource(info.clone())?)
            } else {
                None
            },
            handle_rtv: if image.usage.contains(image::Usage::COLOR_ATTACHMENT) {
                Some(self.view_image_as_render_target(info.clone())?)
            } else {
                None
            },
            handle_uav: if image.usage.contains(image::Usage::STORAGE) {
                Some(self.view_image_as_storage(info.clone())?)
            } else {
                None
            },
            handle_dsv: if image.usage.contains(image::Usage::DEPTH_STENCIL_ATTACHMENT) {
                Some(
                    self.view_image_as_depth_stencil(ViewInfo {
                        format: conv::map_format_dsv(format.base_format().0)
                            .ok_or(image::ViewError::BadFormat)?,
                        ..info
                    })?,
                )
            } else {
                None
            },
            dxgi_format: image.descriptor.Format,
            num_levels: image.descriptor.MipLevels as image::Level,
            mip_levels,
            layers,
            kind: info.kind,
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> r::Sampler {
        let handle = self.sampler_pool.lock().unwrap().alloc_handle();

        let op = match info.comparison {
            Some(_) => d3d12::D3D12_FILTER_REDUCTION_TYPE_COMPARISON,
            None => d3d12::D3D12_FILTER_REDUCTION_TYPE_STANDARD,
        };
        self.raw.create_sampler(
            handle,
            conv::map_filter(
                info.mag_filter,
                info.min_filter,
                info.mip_filter,
                op,
                info.anisotropic,
            ),
            [
                conv::map_wrap(info.wrap_mode.0),
                conv::map_wrap(info.wrap_mode.1),
                conv::map_wrap(info.wrap_mode.2),
            ],
            info.lod_bias.into(),
            match info.anisotropic {
                image::Anisotropic::On(max) => max as _, // TODO: check support here?
                image::Anisotropic::Off => 0,
            },
            conv::map_comparison(info.comparison.unwrap_or(pso::Comparison::Always)),
            info.border.into(),
            info.lod_range.start.into()..info.lod_range.end.into(),
        );

        r::Sampler { handle }
    }

    fn create_descriptor_pool<I>(&self, max_sets: usize, descriptor_pools: I) -> r::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        let mut num_srv_cbv_uav = 0;
        let mut num_samplers = 0;

        let descriptor_pools = descriptor_pools
            .into_iter()
            .map(|desc| *desc.borrow())
            .collect::<Vec<_>>();

        for desc in &descriptor_pools {
            match desc.ty {
                pso::DescriptorType::Sampler => {
                    num_samplers += desc.count;
                }
                pso::DescriptorType::CombinedImageSampler => {
                    num_samplers += desc.count;
                    num_srv_cbv_uav += desc.count;
                }
                _ => {
                    num_srv_cbv_uav += desc.count;
                }
            }
        }

        let heap_srv_cbv_uav = {
            let mut heap_srv_cbv_uav = self.heap_srv_cbv_uav.lock().unwrap();

            let range = match num_srv_cbv_uav {
                0 => 0..0,
                _ => heap_srv_cbv_uav
                    .range_allocator
                    .allocate_range(num_srv_cbv_uav as _)
                    .unwrap(), // TODO: error/resize
            };

            r::DescriptorHeapSlice {
                heap: heap_srv_cbv_uav.raw.clone(),
                handle_size: heap_srv_cbv_uav.handle_size as _,
                range_allocator: RangeAllocator::new(range),
                start: heap_srv_cbv_uav.start,
            }
        };

        let heap_sampler = {
            let mut heap_sampler = self.heap_sampler.lock().unwrap();

            let range = match num_samplers {
                0 => 0..0,
                _ => heap_sampler
                    .range_allocator
                    .allocate_range(num_samplers as _)
                    .unwrap(), // TODO: error/resize
            };

            r::DescriptorHeapSlice {
                heap: heap_sampler.raw.clone(),
                handle_size: heap_sampler.handle_size as _,
                range_allocator: RangeAllocator::new(range),
                start: heap_sampler.start,
            }
        };

        r::DescriptorPool {
            heap_srv_cbv_uav,
            heap_sampler,
            pools: descriptor_pools,
            max_size: max_sets as _,
        }
    }

    fn create_descriptor_set_layout<I, J>(
        &self,
        bindings: I,
        _immutable_samplers: J,
    ) -> r::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<r::Sampler>,
    {
        r::DescriptorSetLayout {
            bindings: bindings.into_iter().map(|b| b.borrow().clone()).collect(),
        }
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, B, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, B>>,
    {
        let mut descriptor_update_pools = self.descriptor_update_pools.lock().unwrap();
        let mut update_pool_index = 0;

        //TODO: combine destination ranges
        let mut dst_samplers = Vec::new();
        let mut dst_views = Vec::new();
        let mut src_samplers = Vec::new();
        let mut src_views = Vec::new();
        let mut num_samplers = Vec::new();
        let mut num_views = Vec::new();

        for write in write_iter {
            let mut offset = write.array_offset as u64;
            let mut target_binding = write.binding as usize;
            let mut bind_info = &write.set.binding_infos[target_binding];
            for descriptor in write.descriptors {
                // spill over the writes onto the next binding
                while offset >= bind_info.count {
                    assert_eq!(offset, bind_info.count);
                    target_binding += 1;
                    bind_info = &write.set.binding_infos[target_binding];
                    offset = 0;
                }
                match *descriptor.borrow() {
                    pso::Descriptor::Buffer(buffer, ref range) => {
                        if update_pool_index == descriptor_update_pools.len() {
                            let max_size = 1u64 << 12; //arbitrary
                            descriptor_update_pools.push(descriptors_cpu::HeapLinear::new(
                                self.raw,
                                descriptor::HeapType::CbvSrvUav,
                                max_size as _,
                            ));
                        }
                        let heap = descriptor_update_pools.last_mut().unwrap();
                        let handle = heap.alloc_handle();
                        if heap.is_full() {
                            // pool is full, move to the next one
                            update_pool_index += 1;
                        }
                        let start = range.start.unwrap_or(0);
                        let end = range.end.unwrap_or(buffer.size_in_bytes as _);

                        if bind_info.is_uav {
                            assert_eq!((end - start) % 4, 0);
                            let mut desc = d3d12::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                                Format: dxgiformat::DXGI_FORMAT_R32_TYPELESS,
                                ViewDimension: d3d12::D3D12_UAV_DIMENSION_BUFFER,
                                u: unsafe { mem::zeroed() },
                            };
                            *unsafe { desc.u.Buffer_mut() } = d3d12::D3D12_BUFFER_UAV {
                                FirstElement: start as _,
                                NumElements: ((end - start) / 4) as _,
                                StructureByteStride: 0,
                                CounterOffsetInBytes: 0,
                                Flags: d3d12::D3D12_BUFFER_UAV_FLAG_RAW,
                            };
                            unsafe {
                                self.raw.CreateUnorderedAccessView(
                                    buffer.resource.as_mut_ptr(),
                                    ptr::null_mut(),
                                    &desc,
                                    handle,
                                );
                            }
                        } else {
                            // Making the size field of buffer requirements for uniform
                            // buffers a multiple of 256 and setting the required offset
                            // alignment to 256 allows us to patch the size here.
                            // We can always enforce the size to be aligned to 256 for
                            // CBVs without going out-of-bounds.
                            let size = ((end - start) + 255) & !255;
                            let desc = d3d12::D3D12_CONSTANT_BUFFER_VIEW_DESC {
                                BufferLocation: unsafe { (*buffer.resource).GetGPUVirtualAddress() }
                                    + start,
                                SizeInBytes: size as _,
                            };
                            unsafe {
                                self.raw.CreateConstantBufferView(&desc, handle);
                            }
                        }

                        src_views.push(handle);
                        dst_views.push(bind_info.view_range.as_ref().unwrap().at(offset));
                        num_views.push(1);
                    }
                    pso::Descriptor::Image(image, _layout) => {
                        let handle = if bind_info.is_uav {
                            image.handle_uav.unwrap()
                        } else {
                            image.handle_srv.unwrap()
                        };
                        src_views.push(handle);
                        dst_views.push(bind_info.view_range.as_ref().unwrap().at(offset));
                        num_views.push(1);
                    }
                    pso::Descriptor::CombinedImageSampler(image, _layout, sampler) => {
                        src_views.push(image.handle_srv.unwrap());
                        dst_views.push(bind_info.view_range.as_ref().unwrap().at(offset));
                        num_views.push(1);
                        src_samplers.push(sampler.handle);
                        dst_samplers.push(bind_info.sampler_range.as_ref().unwrap().at(offset));
                        num_samplers.push(1);
                    }
                    pso::Descriptor::Sampler(sampler) => {
                        src_samplers.push(sampler.handle);
                        dst_samplers.push(bind_info.sampler_range.as_ref().unwrap().at(offset));
                        num_samplers.push(1);
                    }
                    pso::Descriptor::UniformTexelBuffer(buffer_view) => {
                        let handle = buffer_view.handle_srv;
                        if handle.ptr != 0 {
                            src_views.push(handle);
                            dst_views.push(bind_info.view_range.as_ref().unwrap().at(offset));
                            num_views.push(1);
                        } else {
                            error!("SRV handle of the uniform texel buffer is zero (not supported by specified format).");
                        }
                    }
                    pso::Descriptor::StorageTexelBuffer(buffer_view) => {
                        let handle = buffer_view.handle_uav;
                        if handle.ptr != 0 {
                            src_views.push(handle);
                            dst_views.push(bind_info.view_range.as_ref().unwrap().at(offset));
                            num_views.push(1);
                        } else {
                            error!("UAV handle of the storage texel buffer is zero (not supported by specified format).");
                        }
                    }
                }
                offset += 1;
            }
        }

        if !num_views.is_empty() {
            unsafe {
                self.raw.clone().CopyDescriptors(
                    dst_views.len() as u32,
                    dst_views.as_ptr(),
                    num_views.as_ptr(),
                    src_views.len() as u32,
                    src_views.as_ptr(),
                    num_views.as_ptr(),
                    d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                );
            }
        }
        if !num_samplers.is_empty() {
            unsafe {
                self.raw.clone().CopyDescriptors(
                    dst_samplers.len() as u32,
                    dst_samplers.as_ptr(),
                    num_samplers.as_ptr(),
                    src_samplers.len() as u32,
                    src_samplers.as_ptr(),
                    num_samplers.as_ptr(),
                    d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                );
            }
        }

        // reset the temporary CPU-size descriptor pools
        for buffer_desc_pool in descriptor_update_pools.iter_mut() {
            buffer_desc_pool.clear();
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copy_iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>,
    {
        let mut dst_samplers = Vec::new();
        let mut dst_views = Vec::new();
        let mut src_samplers = Vec::new();
        let mut src_views = Vec::new();
        let mut num_samplers = Vec::new();
        let mut num_views = Vec::new();

        for copy_wrap in copy_iter {
            let copy = copy_wrap.borrow();
            let src_info = &copy.src_set.binding_infos[copy.src_binding as usize];
            let dst_info = &copy.dst_set.binding_infos[copy.dst_binding as usize];
            if let (Some(src_range), Some(dst_range)) =
                (src_info.view_range.as_ref(), dst_info.view_range.as_ref())
            {
                assert!(copy.src_array_offset + copy.count <= src_range.count as usize);
                assert!(copy.dst_array_offset + copy.count <= dst_range.count as usize);
                src_views.push(src_range.at(copy.src_array_offset as _));
                dst_views.push(dst_range.at(copy.dst_array_offset as _));
                num_views.push(copy.count as u32);
            }
            if let (Some(src_range), Some(dst_range)) = (
                src_info.sampler_range.as_ref(),
                dst_info.sampler_range.as_ref(),
            ) {
                assert!(copy.src_array_offset + copy.count <= src_range.count as usize);
                assert!(copy.dst_array_offset + copy.count <= dst_range.count as usize);
                src_samplers.push(src_range.at(copy.src_array_offset as _));
                dst_samplers.push(dst_range.at(copy.dst_array_offset as _));
                num_samplers.push(copy.count as u32);
            }
        }

        if !num_views.is_empty() {
            unsafe {
                self.raw.clone().CopyDescriptors(
                    dst_views.len() as u32,
                    dst_views.as_ptr(),
                    num_views.as_ptr(),
                    src_views.len() as u32,
                    src_views.as_ptr(),
                    num_views.as_ptr(),
                    d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                );
            }
        }
        if !num_samplers.is_empty() {
            unsafe {
                self.raw.clone().CopyDescriptors(
                    dst_samplers.len() as u32,
                    dst_samplers.as_ptr(),
                    num_samplers.as_ptr(),
                    src_samplers.len() as u32,
                    src_samplers.as_ptr(),
                    num_samplers.as_ptr(),
                    d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                );
            }
        }
    }

    fn map_memory<R>(&self, memory: &r::Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        if let Some(mem) = memory.resource {
            let start = range.start().unwrap_or(&0);
            let end = range.end().unwrap_or(&memory.size);
            assert!(start <= end);

            let mut ptr = ptr::null_mut();
            assert_eq!(winerror::S_OK, unsafe {
                (*mem).Map(0, &d3d12::D3D12_RANGE { Begin: 0, End: 0 }, &mut ptr)
            });
            unsafe {
                ptr = ptr.offset(*start as _);
            }
            Ok(ptr as *mut _)
        } else {
            panic!("Memory not created with a memory type exposing `CPU_VISIBLE`.")
        }
    }

    fn unmap_memory(&self, memory: &r::Memory) {
        if let Some(mem) = memory.resource {
            unsafe {
                (*mem).Unmap(0, &d3d12::D3D12_RANGE { Begin: 0, End: 0 });
            }
        }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a r::Memory, R)>,
        R: RangeArg<u64>,
    {
        for range in ranges {
            let &(ref memory, ref range) = range.borrow();
            if let Some(mem) = memory.resource {
                // map and immediately unmap, hoping that dx12 drivers internally cache
                // currently mapped buffers.
                assert_eq!(winerror::S_OK, unsafe {
                    (*mem).Map(0, &d3d12::D3D12_RANGE { Begin: 0, End: 0 }, ptr::null_mut())
                });

                let start = *range.start().unwrap_or(&0);
                let end = *range.end().unwrap_or(&memory.size); // TODO: only need to be end of current mapping

                unsafe {
                    (*mem).Unmap(
                        0,
                        &d3d12::D3D12_RANGE {
                            Begin: start as _,
                            End: end as _,
                        },
                    );
                }
            }
        }
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a r::Memory, R)>,
        R: RangeArg<u64>,
    {
        for range in ranges {
            let &(ref memory, ref range) = range.borrow();
            if let Some(mem) = memory.resource {
                let start = *range.start().unwrap_or(&0);
                let end = *range.end().unwrap_or(&memory.size); // TODO: only need to be end of current mapping

                // map and immediately unmap, hoping that dx12 drivers internally cache
                // currently mapped buffers.
                assert_eq!(winerror::S_OK, unsafe {
                    (*mem).Map(
                        0,
                        &d3d12::D3D12_RANGE {
                            Begin: start as _,
                            End: end as _,
                        },
                        ptr::null_mut(),
                    )
                });

                unsafe {
                    (*mem).Unmap(0, &d3d12::D3D12_RANGE { Begin: 0, End: 0 });
                }
            }
        }
    }

    fn create_semaphore(&self) -> r::Semaphore {
        let fence = self.create_fence(false);
        r::Semaphore { raw: fence.raw }
    }

    fn create_fence(&self, signalled: bool) -> r::Fence {
        r::Fence {
            raw: self.create_raw_fence(signalled),
        }
    }

    fn reset_fence(&self, fence: &r::Fence) {
        assert_eq!(winerror::S_OK, fence.raw.signal(0));
    }

    fn wait_for_fences<I>(&self, fences: I, wait: d::WaitFor, timeout_ns: u64) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<r::Fence>,
    {
        let fences = fences.into_iter().collect::<Vec<_>>();
        let mut events = self.events.lock().unwrap();
        for _ in events.len()..fences.len() {
            events.push(unsafe {
                synchapi::CreateEventA(ptr::null_mut(), FALSE, FALSE, ptr::null())
            });
        }

        for (&event, fence) in events.iter().zip(fences.iter()) {
            assert_eq!(winerror::S_OK, unsafe {
                synchapi::ResetEvent(event);
                fence.borrow().raw.set_event_on_completion(event, 1)
            });
        }

        let all = match wait {
            d::WaitFor::Any => FALSE,
            d::WaitFor::All => TRUE,
        };
        let hr = unsafe {
            // This block handles overflow when converting to u32 and always rounds up
            // The Vulkan specification allows to wait more than specified
            let timeout_ms = {
                if timeout_ns > (<u32>::max_value() as u64) * 1_000_000 {
                    <u32>::max_value()
                } else {
                    ((timeout_ns + 999_999) / 1_000_000) as u32
                }
            };

            synchapi::WaitForMultipleObjects(fences.len() as u32, events.as_ptr(), all, timeout_ms)
        };

        const WAIT_OBJECT_LAST: u32 = winbase::WAIT_OBJECT_0 + winnt::MAXIMUM_WAIT_OBJECTS;
        const WAIT_ABANDONED_LAST: u32 = winbase::WAIT_ABANDONED_0 + winnt::MAXIMUM_WAIT_OBJECTS;
        match hr {
            winbase::WAIT_OBJECT_0...WAIT_OBJECT_LAST => true,
            winbase::WAIT_ABANDONED_0...WAIT_ABANDONED_LAST => true, //TODO?
            winerror::WAIT_TIMEOUT => false,
            _ => panic!("Unexpected wait status 0x{:X}", hr),
        }
    }

    fn get_fence_status(&self, _fence: &r::Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, memory: r::Memory) {
        unsafe {
            memory.heap.destroy();
            if let Some(buffer) = memory.resource {
                buffer.destroy();
            }
        }
    }

    fn create_query_pool(
        &self,
        query_ty: query::Type,
        count: query::Id,
    ) -> Result<r::QueryPool, query::Error> {
        let heap_ty = match query_ty {
            query::Type::Occlusion => native::query::HeapType::Occlusion,
            query::Type::PipelineStatistics(_) => native::query::HeapType::PipelineStatistics,
            query::Type::Timestamp => native::query::HeapType::Timestamp,
        };

        let (query_heap, hr) = self.raw.create_query_heap(heap_ty, count, 0);
        assert_eq!(winerror::S_OK, hr);

        Ok(r::QueryPool {
            raw: query_heap,
            ty: heap_ty,
        })
    }

    fn destroy_query_pool(&self, pool: r::QueryPool) {
        unsafe {
            pool.raw.destroy();
        }
    }

    fn get_query_pool_results(
        &self,
        _pool: &r::QueryPool,
        _queries: Range<query::Id>,
        _data: &mut [u8],
        _stride: buffer::Offset,
        _flags: query::ResultFlags,
    ) -> Result<bool, query::Error> {
        unimplemented!()
    }

    fn destroy_shader_module(&self, shader_lib: r::ShaderModule) {
        if let r::ShaderModule::Compiled(shaders) = shader_lib {
            for (_, blob) in shaders {
                unsafe {
                    blob.destroy();
                }
            }
        }
    }

    fn destroy_render_pass(&self, _rp: r::RenderPass) {
        // Just drop
    }

    fn destroy_pipeline_layout(&self, layout: r::PipelineLayout) {
        unsafe {
            layout.raw.destroy();
        }
    }

    fn destroy_graphics_pipeline(&self, pipeline: r::GraphicsPipeline) {
        unsafe {
            pipeline.raw.destroy();
        }
    }

    fn destroy_compute_pipeline(&self, pipeline: r::ComputePipeline) {
        unsafe {
            pipeline.raw.destroy();
        }
    }

    fn destroy_framebuffer(&self, _fb: r::Framebuffer) {
        // Just drop
    }

    fn destroy_buffer(&self, buffer: r::Buffer) {
        unsafe {
            buffer.resource.destroy();
        }
    }

    fn destroy_buffer_view(&self, _view: r::BufferView) {
        // empty
    }

    fn destroy_image(&self, image: r::Image) {
        unsafe {
            image.resource.destroy();
        }
    }

    fn destroy_image_view(&self, _view: r::ImageView) {
        // Just drop
    }

    fn destroy_sampler(&self, _sampler: r::Sampler) {
        // Just drop
    }

    fn destroy_descriptor_pool(&self, _pool: r::DescriptorPool) {
        // Just drop
        // Allocated descriptor sets don't need to be freed beforehand.
    }

    fn destroy_descriptor_set_layout(&self, _layout: r::DescriptorSetLayout) {
        // Just drop
    }

    fn destroy_fence(&self, fence: r::Fence) {
        unsafe {
            fence.raw.destroy();
        }
    }

    fn destroy_semaphore(&self, semaphore: r::Semaphore) {
        unsafe {
            semaphore.raw.destroy();
        }
    }

    fn create_swapchain(
        &self,
        surface: &mut w::Surface,
        config: hal::SwapchainConfig,
        old_swapchain: Option<w::Swapchain>,
    ) -> (w::Swapchain, hal::Backbuffer<B>) {
        if let Some(old_swapchain) = old_swapchain {
            self.destroy_swapchain(old_swapchain);
        }

        let mut swap_chain1 = native::WeakPtr::<dxgi1_2::IDXGISwapChain1>::null();

        let format = match config.format {
            // Apparently, swap chain doesn't like sRGB, but the RTV can still have some:
            // https://www.gamedev.net/forums/topic/670546-d3d12srgb-buffer-format-for-swap-chain/
            // [15716] DXGI ERROR: IDXGIFactory::CreateSwapchain: Flip model swapchains
            //                     (DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL and DXGI_SWAP_EFFECT_FLIP_DISCARD) only support the following Formats:
            //                     (DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM),
            //                     assuming the underlying Device does as well.
            format::Format::Bgra8Srgb => format::Format::Bgra8Unorm,
            format::Format::Rgba8Srgb => format::Format::Rgba8Unorm,
            format => format,
        };

        let format = conv::map_format(format).unwrap(); // TODO: error handling

        let rtv_desc = d3d12::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: conv::map_format(config.format).unwrap(),
            ViewDimension: d3d12::D3D12_RTV_DIMENSION_TEXTURE2D,
            ..unsafe { mem::zeroed() }
        };
        let rtv_heap = Device::create_descriptor_heap_impl(
            self.raw,
            descriptor::HeapType::Rtv,
            false,
            config.image_count as _,
        );

        // TODO: double-check values
        let desc = dxgi1_2::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: dxgi1_2::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: config.image_count,
            Width: config.extent.width,
            Height: config.extent.height,
            Format: format,
            Flags: 0,
            BufferUsage: dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Scaling: dxgi1_2::DXGI_SCALING_STRETCH,
            Stereo: FALSE,
            SwapEffect: dxgi::DXGI_SWAP_EFFECT_FLIP_DISCARD,
        };

        let hr = unsafe {
            // TODO
            surface.factory.CreateSwapChainForHwnd(
                self.present_queue.as_mut_ptr() as *mut _,
                surface.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                swap_chain1.mut_void() as *mut *mut _,
            )
        };

        if !winerror::SUCCEEDED(hr) {
            error!("error on swapchain creation 0x{:x}", hr);
        }

        let (swap_chain3, hr3) = unsafe { swap_chain1.cast::<dxgi1_4::IDXGISwapChain3>() };
        if !winerror::SUCCEEDED(hr3) {
            error!("error on swapchain cast 0x{:x}", hr3);
        }

        unsafe {
            swap_chain1.destroy();
        }

        // Get backbuffer images
        let mut resources: Vec<native::Resource> = Vec::new();
        let images = (0..config.image_count)
            .map(|i| {
                let mut resource = native::Resource::null();
                unsafe {
                    swap_chain3.GetBuffer(
                        i as _,
                        &d3d12::ID3D12Resource::uuidof(),
                        resource.mut_void(),
                    );
                }

                let rtv_handle = rtv_heap.at(i as _, 0).cpu;
                unsafe {
                    self.raw
                        .CreateRenderTargetView(resource.as_mut_ptr(), &rtv_desc, rtv_handle);
                }
                resources.push(resource);

                let surface_type = config.format.base_format().0;
                let format_desc = surface_type.desc();

                let bytes_per_block = (format_desc.bits / 8) as _;
                let block_dim = format_desc.dim;
                let kind = image::Kind::D2(config.extent.width, config.extent.height, 1, 1);

                r::Image {
                    resource,
                    place: r::Place::SwapChain,
                    surface_type,
                    kind,
                    usage: config.image_usage,
                    view_caps: image::ViewCapabilities::empty(),
                    descriptor: d3d12::D3D12_RESOURCE_DESC {
                        Dimension: d3d12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                        Alignment: 0,
                        Width: config.extent.width as _,
                        Height: config.extent.height as _,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        Format: format,
                        SampleDesc: desc.SampleDesc.clone(),
                        Layout: d3d12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                        Flags: 0,
                    },
                    bytes_per_block,
                    block_dim,
                    clear_cv: vec![rtv_handle],
                    clear_dv: Vec::new(),
                    clear_sv: Vec::new(),
                }
            })
            .collect();

        let swapchain = w::Swapchain {
            inner: swap_chain3,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            rtv_heap,
            resources,
        };

        (swapchain, hal::Backbuffer::Images(images))
    }

    fn destroy_swapchain(&self, swapchain: w::Swapchain) {
        unsafe {
            for resource in &swapchain.resources {
                resource.destroy();
            }
            swapchain.inner.destroy();
            swapchain.rtv_heap.destroy();
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        for queue in &self.queues {
            queue.wait_idle()?;
        }
        Ok(())
    }
}
