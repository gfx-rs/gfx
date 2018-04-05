use std::borrow::Borrow;
use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;
use std::{ffi, mem, ptr, slice};

use spirv_cross::{hlsl, spirv, ErrorCode as SpirvErrorCode};

use winapi::Interface;
use winapi::um::{d3d12, d3dcommon, d3dcompiler, synchapi, winbase, winnt};
use winapi::shared::minwindef::{FALSE, TRUE, UINT};
use winapi::shared::{dxgi, dxgi1_2, dxgi1_4, dxgiformat, dxgitype, winerror};
use wio::com::ComPtr;

use hal::{self, buffer, device as d, error, format, image, mapping, memory, pass, pso, query};
use hal::format::Aspects;
use hal::memory::Requirements;
use hal::pool::CommandPoolCreateFlags;
use hal::queue::{RawCommandQueue, QueueFamilyId};
use hal::range::RangeArg;

use {
    conv, free_list, native as n, root_constants, window as w,
    Backend as B, Device, MemoryGroup, QUEUE_FAMILIES, MAX_VERTEX_BUFFERS, NUM_HEAP_PROPERTIES,
};
use pool::RawCommandPool;
use root_constants::RootConstant;

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

fn shader_bytecode(shader: *mut d3dcommon::ID3DBlob) -> d3d12::D3D12_SHADER_BYTECODE {
    unsafe {
        d3d12::D3D12_SHADER_BYTECODE {
            pShaderBytecode: if !shader.is_null() {
                (*shader).GetBufferPointer() as *const _
            } else {
                ptr::null_mut()
            },
            BytecodeLength: if !shader.is_null() {
                (*shader).GetBufferSize()
            } else {
                0
            },
        }
    }
}

#[derive(Clone)]
struct ViewInfo {
    resource: *mut d3d12::ID3D12Resource,
    kind: image::Kind,
    flags: image::StorageFlags,
    view_kind: image::ViewKind,
    format: dxgiformat::DXGI_FORMAT,
    range: image::SubresourceRange,
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
    #[derivative(Debug="ignore")]
    desc: d3d12::D3D12_RESOURCE_DESC,
    dsv_format: dxgiformat::DXGI_FORMAT,
    requirements: memory::Requirements,
    kind: image::Kind,
    usage: image::Usage,
    aspects: Aspects,
    storage_flags: image::StorageFlags,
    //TODO: use hal::format::FormatDesc
    bytes_per_block: u8,
    // Dimension of a texel block (compressed formats).
    block_dim: (u8, u8),
    num_levels: image::Level,
}

impl Device {
    /// Compile a single shader entry point from a HLSL text shader
    fn compile_shader(
        stage: pso::Stage,
        shader_model: hlsl::ShaderModel,
        entry: &str,
        code: &[u8],
    ) -> Result<*mut d3dcommon::ID3DBlob, d::ShaderError> {
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
                code.len(),
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
        if !winerror::SUCCEEDED(hr) {
            error!("D3DCompile error {:x}", hr);
            let error = unsafe { ComPtr::<d3dcommon::ID3DBlob>::from_raw(error) };
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

    fn parse_spirv(raw_data: &[u8]) -> Result<spirv::Ast<hlsl::Target>, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        spirv::Ast::parse(&module)
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown parsing error".into(),
                };
                d::ShaderError::CompilationFailed(msg)
            })
    }

    fn patch_spirv_resources(
        ast: &mut spirv::Ast<hlsl::Target>,
        layout: Option<&n::PipelineLayout>,
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
            let set = ast.get_decoration(image.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(image.id, spirv::Decoration::DescriptorSet, space_offset + 2*set)
               .map_err(gen_unexpected_error)?;
        }

        for uniform_buffer in &shader_resources.uniform_buffers {
            let set = ast.get_decoration(uniform_buffer.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(uniform_buffer.id, spirv::Decoration::DescriptorSet, space_offset + 2*set)
               .map_err(gen_unexpected_error)?;
        }

        for sampler in &shader_resources.separate_samplers {
            let set = ast.get_decoration(sampler.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(sampler.id, spirv::Decoration::DescriptorSet, space_offset + 2*set+1)
               .map_err(gen_unexpected_error)?;
        }

        for image in &shader_resources.sampled_images {
            let set = ast.get_decoration(image.id, spirv::Decoration::DescriptorSet).map_err(gen_query_error)?;
            ast.set_decoration(image.id, spirv::Decoration::DescriptorSet, space_offset + 2*set)
               .map_err(gen_unexpected_error)?;
        }

        // TODO: other resources

        Ok(())
    }

    fn translate_spirv(
        ast: &mut spirv::Ast<hlsl::Target>,
        shader_model: hlsl::ShaderModel,
        layout: &n::PipelineLayout,
        stage: pso::Stage,
    ) -> Result<String, d::ShaderError> {
        let mut compile_options = hlsl::CompilerOptions::default();
        compile_options.shader_model = shader_model;
        compile_options.vertex.invert_y = true;

        let stage_flag = stage.into();
        let root_constant_layout = layout
            .root_constants
            .iter()
            .filter_map(|constant| if constant.stages.contains(stage_flag) {
                Some(hlsl::RootConstant {
                    start: constant.range.start * 4,
                    end: constant.range.end * 4,
                    binding: constant.range.start,
                    space: 0,
                })
            } else {
                None
            })
            .collect();
        ast.set_compiler_options(&compile_options)
            .map_err(gen_unexpected_error)?;
        ast.set_root_constant_layout(root_constant_layout)
            .map_err(gen_unexpected_error)?;
        ast.compile()
            .map_err(|err| {
                let msg =  match err {
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
        layout: &n::PipelineLayout,
    ) -> Result<(*mut d3dcommon::ID3DBlob, bool), d::ShaderError> {
        match *source.module {
            n::ShaderModule::Compiled(ref shaders) => {
                // TODO: do we need to check for specialization constants?
                // Use precompiled shader, ignore specialization or layout.
                shaders
                    .get(source.entry)
                    .map(|x| (*x, false))
                    .ok_or(d::ShaderError::MissingEntryPoint(source.entry.into()))
            }
            n::ShaderModule::Spirv(ref raw_data) => {
                let mut ast = Self::parse_spirv(raw_data)?;
                let spec_constants = ast
                    .get_specialization_constants()
                    .map_err(gen_query_error)?;

                for spec_constant in spec_constants {
                    if let Some(constant) = source
                        .specialization
                        .iter()
                        .find(|c| c.id == spec_constant.constant_id)
                    {
                        // Override specialization constant values
                        unsafe {
                            let value = match constant.value {
                                pso::Constant::Bool(v) => v as u64,
                                pso::Constant::U32(v) => v as u64,
                                pso::Constant::U64(v) => v,
                                pso::Constant::I32(v) => *(&v as *const _ as *const u64),
                                pso::Constant::I64(v) => *(&v as *const _ as *const u64),
                                pso::Constant::F32(v) => *(&v as *const _ as *const u64),
                                pso::Constant::F64(v) => *(&v as *const _ as *const u64),
                            };
                            ast.set_scalar_constant(spec_constant.id, value).map_err(gen_query_error)?;
                        }
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
                        let shader = Self::compile_shader(
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
    ) -> Result<n::ShaderModule, d::ShaderError> {
        let mut shader_map = BTreeMap::new();
        let blob = Self::compile_shader(stage, hlsl::ShaderModel::V5_1, hlsl_entry, code)?;
        shader_map.insert(entry_point.into(), blob);
        Ok(n::ShaderModule::Compiled(shader_map))
    }

    pub(crate) fn create_command_signature(
        device: &mut ComPtr<d3d12::ID3D12Device>,
        ty: CommandSignature,
    ) -> ComPtr<d3d12::ID3D12CommandSignature> {
        let mut signature = ptr::null_mut();

        let (arg_ty, stride) = match ty {
            CommandSignature::Draw => (
                d3d12::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
                16,
            ),
            CommandSignature::DrawIndexed => (
                d3d12::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
                20,
            ),
            CommandSignature::Dispatch => (
                d3d12::D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
                12,
            ),
        };

        let arg = d3d12::D3D12_INDIRECT_ARGUMENT_DESC {
            Type: arg_ty,
            .. unsafe { mem::zeroed() }
        };

        let desc = d3d12::D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: stride,
            NumArgumentDescs: 1,
            pArgumentDescs: &arg,
            NodeMask: 0,
        };

        let hr = unsafe {
            device.CreateCommandSignature(
                &desc,
                ptr::null_mut(),
                &d3d12::IID_ID3D12CommandSignature,
                &mut signature as *mut *mut _ as *mut *mut _,
            )
        };

        if !winerror::SUCCEEDED(hr) {
            error!("error on command signature creation: {:x}", hr);
        }
        unsafe { ComPtr::from_raw(signature) }
    }

    pub(crate) fn create_descriptor_heap_impl(
        device: &mut ComPtr<d3d12::ID3D12Device>,
        heap_type: d3d12::D3D12_DESCRIPTOR_HEAP_TYPE,
        shader_visible: bool,
        capacity: usize,
    ) -> n::DescriptorHeap {
        assert_ne!(capacity, 0);

        let desc = d3d12::D3D12_DESCRIPTOR_HEAP_DESC {
            Type: heap_type,
            NumDescriptors: capacity as u32,
            Flags: if shader_visible {
                d3d12::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
            } else {
                d3d12::D3D12_DESCRIPTOR_HEAP_FLAG_NONE
            },
            NodeMask: 0,
        };

        let mut heap: *mut d3d12::ID3D12DescriptorHeap = ptr::null_mut();

        let descriptor_size = unsafe {
            device.CreateDescriptorHeap(
                &desc,
                &d3d12::IID_ID3D12DescriptorHeap,
                &mut heap as *mut *mut _ as *mut *mut _,
            );
            device.GetDescriptorHandleIncrementSize(heap_type) as usize
        };

        let cpu_handle = unsafe { (*heap).GetCPUDescriptorHandleForHeapStart() };
        let gpu_handle = unsafe { (*heap).GetGPUDescriptorHandleForHeapStart() };

        let allocator = free_list::Allocator::new(capacity as _);

        n::DescriptorHeap {
            raw: unsafe { ComPtr::from_raw(heap) },
            handle_size: descriptor_size as _,
            total_handles: capacity as _,
            start: n::DualHandle {
                cpu: cpu_handle,
                gpu: gpu_handle,
            },
            allocator,
        }
    }

    fn view_image_as_render_target(
        &self, info: ViewInfo
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
        #![allow(non_snake_case)]

        let mut desc = d3d12::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: info.format,
            ViewDimension: 0,
            u: unsafe { mem::zeroed() },
        };

        let MipSlice = info.range.levels.start as _;
        let FirstArraySlice = info.range.layers.start as _;
        let ArraySize = (info.range.layers.end - info.range.layers.start) as _;

        assert!(info.range.layers.end <= info.kind.num_layers());
        let is_msaa = info.kind.num_samples() > 1;

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE1D;
                *unsafe{ desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_RTV {
                    MipSlice,
                }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE1DARRAY;
                *unsafe{ desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_RTV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DMS;
                *unsafe{ desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_RTV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_RTV {
                    MipSlice,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe{ desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_RTV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE2DARRAY;
                *unsafe{ desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_RTV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D3 => {
                desc.ViewDimension = d3d12::D3D12_RTV_DIMENSION_TEXTURE3D;
                *unsafe{ desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_RTV {
                    MipSlice,
                    FirstWSlice: 0,
                    WSize: info.kind.extent().depth as _,
                }
            }
            image::ViewKind::Cube |
            image::ViewKind::CubeArray => {
                unimplemented!()
            }
        };

        let handle = self.rtv_pool.lock().unwrap().alloc_handles(1).cpu;
        unsafe {
            self.raw.clone().CreateRenderTargetView(info.resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_depth_stencil(
        &self, info: ViewInfo
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
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
                *unsafe{ desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_DSV {
                    MipSlice,
                }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE1DARRAY;
                *unsafe{ desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_DSV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DMS;
                *unsafe{ desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_DSV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_DSV {
                    MipSlice,
                }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe{ desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_DSV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_DSV_DIMENSION_TEXTURE2DARRAY;
                *unsafe{ desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_DSV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D3 |
            image::ViewKind::Cube |
            image::ViewKind::CubeArray => {
                unimplemented!()
            }
        };

        let handle = self.dsv_pool.lock().unwrap().alloc_handles(1).cpu;
        unsafe {
            self.raw.clone().CreateDepthStencilView(info.resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_shader_resource(
        &self, info: ViewInfo
    ) -> Result<d3d12::D3D12_CPU_DESCRIPTOR_HANDLE, image::ViewError> {
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
        let is_cube = info.flags.contains(image::StorageFlags::CUBE_VIEW);

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE1D;
                *unsafe{ desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE1DARRAY;
                *unsafe{ desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_SRV {
                    MostDetailedMip,
                    MipLevels,
                    FirstArraySlice,
                    ArraySize,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D2 if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DMS;
                *unsafe{ desc.u.Texture2DMS_mut() } = d3d12::D3D12_TEX2DMS_SRV {
                    UnusedField_NothingToDefine: 0,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    PlaneSlice: 0, //TODO
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::D2Array if is_msaa => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY;
                *unsafe{ desc.u.Texture2DMSArray_mut() } = d3d12::D3D12_TEX2DMS_ARRAY_SRV {
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURE2DARRAY;
                *unsafe{ desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_SRV {
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
                *unsafe{ desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::Cube if is_cube => {
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURECUBE;
                *unsafe{ desc.u.TextureCube_mut() } = d3d12::D3D12_TEXCUBE_SRV {
                    MostDetailedMip,
                    MipLevels,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::CubeArray if is_cube => {
                assert_eq!(0, ArraySize % 6);
                desc.ViewDimension = d3d12::D3D12_SRV_DIMENSION_TEXTURECUBEARRAY;
                *unsafe{ desc.u.TextureCubeArray_mut() } = d3d12::D3D12_TEXCUBE_ARRAY_SRV {
                    MostDetailedMip,
                    MipLevels,
                    First2DArrayFace: FirstArraySlice,
                    NumCubes: ArraySize / 6,
                    ResourceMinLODClamp: 0.0,
                }
            }
            image::ViewKind::Cube |
            image::ViewKind::CubeArray => {
                error!("Cube views are not supported for the image, kind: {:?}", info.kind);
                return Err(image::ViewError::BadKind)
            }
        }

        let handle = self.srv_pool.lock().unwrap().alloc_handles(1).cpu;
        unsafe {
            self.raw.clone().CreateShaderResourceView(info.resource, &desc, handle);
        }

        Ok(handle)
    }

    fn view_image_as_storage(
        &self, info: ViewInfo
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
            return Err(image::ViewError::Unsupported)
        }

        match info.view_kind {
            image::ViewKind::D1 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE1D;
                *unsafe{ desc.u.Texture1D_mut() } = d3d12::D3D12_TEX1D_UAV {
                    MipSlice,
                }
            }
            image::ViewKind::D1Array => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE1DARRAY;
                *unsafe{ desc.u.Texture1DArray_mut() } = d3d12::D3D12_TEX1D_ARRAY_UAV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                }
            }
            image::ViewKind::D2 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE2D;
                *unsafe{ desc.u.Texture2D_mut() } = d3d12::D3D12_TEX2D_UAV {
                    MipSlice,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D2Array => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE2DARRAY;
                *unsafe{ desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_UAV {
                    MipSlice,
                    FirstArraySlice,
                    ArraySize,
                    PlaneSlice: 0, //TODO
                }
            }
            image::ViewKind::D3 => {
                desc.ViewDimension = d3d12::D3D12_UAV_DIMENSION_TEXTURE3D;
                *unsafe{ desc.u.Texture3D_mut() } = d3d12::D3D12_TEX3D_UAV {
                    MipSlice,
                    FirstWSlice: 0,
                    WSize: info.kind.extent().depth as _,
                }
            }
            image::ViewKind::Cube |
            image::ViewKind::CubeArray => {
                error!("Cubic images can't be viewed as UAV");
                return Err(image::ViewError::Unsupported);
            }
        }

        let handle = self.uav_pool.lock().unwrap().alloc_handles(1).cpu;
        unsafe {
            self.raw.clone().CreateUnorderedAccessView(info.resource, ptr::null_mut(), &desc, handle);
        }

        Ok(handle)
    }

    pub(crate) fn create_raw_fence(&self, signalled: bool) -> *mut d3d12::ID3D12Fence {
        let mut handle = ptr::null_mut();
        assert_eq!(winerror::S_OK, unsafe {
            self.raw.clone().CreateFence(
                if signalled { 1 } else { 0 },
                d3d12::D3D12_FENCE_FLAG_NONE,
                &d3d12::IID_ID3D12Fence,
                &mut handle,
            )
        });
        handle as *mut _
    }
}

impl d::Device<B> for Device {
    fn allocate_memory(
        &self,
        mem_type: hal::MemoryTypeId,
        size: u64,
    ) -> Result<n::Memory, d::OutOfMemory> {
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
            Alignment: 0, //Warning: has to be 4K for MSAA targets
            Flags: match mem_group {
                0 => d3d12::D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES,
                1 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
                2 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES,
                3 => d3d12::D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES,
                _ => unreachable!()
            },
        };

        let mut heap = ptr::null_mut();
        let hr = unsafe {
            self.raw.clone().CreateHeap(&desc, &d3d12::IID_ID3D12Heap, &mut heap)
        };
        if hr == winerror::E_OUTOFMEMORY {
            return Err(d::OutOfMemory);
        }
        assert_eq!(winerror::S_OK, hr);

        // The first memory heap of each group corresponds to the default heap, which is can never
        // be mapped.
        // Devices supporting heap tier 1 can only created buffers on mem group 1 (ALLOW_ONLY_BUFFERS).
        // Devices supporting heap tier 2 always expose only mem group 0 and don't have any further restrictions.
        let is_mapable = mem_base_id != 0 &&
            (mem_group == MemoryGroup::Universal as _ || mem_group == MemoryGroup::BufferOnly as _);

        // Create a buffer resource covering the whole memory slice to be able to map the whole memory.
        let resource = if is_mapable {
            let mut resource = ptr::null_mut();
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
                    heap as _,
                    0,
                    &desc,
                    d3d12::D3D12_RESOURCE_STATE_COMMON,
                    ptr::null(),
                    &d3d12::ID3D12Resource::uuidof(),
                    &mut resource as *mut *mut _ as *mut *mut _,
                )
            });

            Some(resource)
        } else {
            None
        };

        Ok(n::Memory {
            heap: unsafe { ComPtr::from_raw(heap as _) },
            type_id: mem_type,
            size,
            resource,
        })
    }

    fn create_command_pool(
        &self, family: QueueFamilyId, _create_flags: CommandPoolCreateFlags
    ) -> RawCommandPool {
        let list_type = QUEUE_FAMILIES[family.0].native_type();
        // create command allocator
        let mut command_allocator: *mut d3d12::ID3D12CommandAllocator = ptr::null_mut();
        let hr = unsafe {
            self.raw.clone().CreateCommandAllocator(
                list_type,
                &d3d12::IID_ID3D12CommandAllocator,
                &mut command_allocator as *mut *mut _ as *mut *mut _,
            )
        };
        // TODO: error handling
        if !winerror::SUCCEEDED(hr) {
            error!("error on command allocator creation: {:x}", hr);
        }

        RawCommandPool {
            inner: unsafe { ComPtr::from_raw(command_allocator) },
            device: self.raw.clone(),
            list_type,
            signatures: self.signatures.clone(),
        }
    }

    fn destroy_command_pool(&self, _pool: RawCommandPool) {
        // automatic
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> n::RenderPass
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
            Preserve,
            Undefined,
        }
        struct AttachmentInfo {
            sub_states: Vec<SubState>,
            target_state: d3d12::D3D12_RESOURCE_STATES,
            last_state: d3d12::D3D12_RESOURCE_STATES,
            barrier_start_index: usize,
        }

        let attachments = attachments.into_iter()
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
                last_state: conv::map_image_resource_state(image::Access::empty(), att.layouts.start),
                barrier_start_index: 0,
            })
            .collect::<Vec<_>>();

        // Fill out subpass known layouts
        for (sid, sub) in subpasses.iter().enumerate() {
            let sub = sub.borrow();
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
                let state = SubState::New(d3d12::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
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
            if dep.passes.start != dep.passes.end && dep.passes.start != pass::SubpassRef::External {
                if let pass::SubpassRef::Pass(sid) = dep.passes.end {
                    deps_left[sid] += 1;
                }
            }
        }

        let mut rp = n::RenderPass {
            attachments: attachments.clone(),
            subpasses: Vec::new(),
            post_barriers: Vec::new(),
        };

        while let Some(sid) = deps_left.iter().position(|count| *count == 0) {
            deps_left[sid] = !0; // mark as done
            for dep in &dependencies {
                let dep = dep.borrow();
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
                color_attachments: subpasses[sid].borrow().colors.iter().cloned().collect(),
                depth_stencil_attachment: subpasses[sid].borrow().depth_stencil.cloned(),
                input_attachments: subpasses[sid].borrow().inputs.iter().cloned().collect(),
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

    fn create_pipeline_layout<IS, IR>(
        &self,
        sets: IS,
        push_constant_ranges: IR,
    ) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
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
            let mut param = d3d12::D3D12_ROOT_PARAMETER {
                ParameterType: d3d12::D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
                ShaderVisibility: d3d12::D3D12_SHADER_VISIBILITY_ALL, //TODO
                .. unsafe { mem::zeroed() }
            };

            *unsafe{ param.u.Constants_mut() } = d3d12::D3D12_ROOT_CONSTANTS {
                ShaderRegister: root_constant.range.start as _,
                RegisterSpace: ROOT_CONSTANT_SPACE,
                Num32BitValues: (root_constant.range.end - root_constant.range.start) as _,
            };

            parameters.push(param);
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
                let bindings = &desc_set
                    .borrow()
                    .bindings;

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
            let mut table_type = n::SetTableTypes::empty();

            let mut param = d3d12::D3D12_ROOT_PARAMETER {
                ParameterType: d3d12::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                ShaderVisibility: d3d12::D3D12_SHADER_VISIBILITY_ALL, //TODO
                .. unsafe { mem::zeroed() }
            };

            let range_base = ranges.len();
            ranges.extend(set
                .bindings
                .iter()
                .filter(|bind| bind.ty != pso::DescriptorType::Sampler)
                .map(|bind| conv::map_descriptor_range(bind, (table_space_offset + 2*i) as u32, false)));

            if ranges.len() > range_base {
                *unsafe{ param.u.DescriptorTable_mut() } = d3d12::D3D12_ROOT_DESCRIPTOR_TABLE {
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
                .filter(|bind| bind.ty == pso::DescriptorType::Sampler || bind.ty == pso::DescriptorType::CombinedImageSampler)
                .map(|bind| {
                    conv::map_descriptor_range(
                        bind,
                        (table_space_offset + 2*i+1) as u32,
                        true,
                    )
                }));

            if ranges.len() > range_base {
                *unsafe{ param.u.DescriptorTable_mut() } = d3d12::D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: (ranges.len() - range_base) as _,
                    pDescriptorRanges: ranges[range_base..].as_ptr(),
                };

                parameters.push(param);
                table_type |= n::SAMPLERS;
            }

            set_tables.push(table_type);
        }

        // Ensure that we didn't reallocate!
        debug_assert_eq!(ranges.len(), total);

        ranges.get_mut(0).map(|range| {
            range.OffsetInDescriptorsFromTableStart = 0; // careful!
        });

        let desc = d3d12::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: parameters.as_ptr(),
            NumStaticSamplers: 0,
            pStaticSamplers: ptr::null(),
            Flags: d3d12::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };

        let mut signature = ptr::null_mut();
        let mut signature_raw = ptr::null_mut();
        let mut error = ptr::null_mut();

        // TODO: error handling
        unsafe {
            let _hr = d3d12::D3D12SerializeRootSignature(
                &desc,
                d3d12::D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature_raw,
                &mut error,
            );

            if !error.is_null() {
                //TODO
                let error_output = (*error).GetBufferPointer();
                let message = ::std::ffi::CStr::from_ptr(error_output as *const _ as *const _);
                error!("D3D12SerializeRootSignature error: {:?}", message.to_str().unwrap());
                (*error).Release();
            }

            self.raw.clone().CreateRootSignature(
                0,
                (*signature_raw).GetBufferPointer(),
                (*signature_raw).GetBufferSize(),
                &d3d12::IID_ID3D12RootSignature,
                &mut signature as *mut *mut _ as *mut *mut _,
            );
            (*signature_raw).Release();
        }

        n::PipelineLayout {
            raw: signature,
            tables: set_tables,
            root_constants,
            num_parameter_slots: parameters.len(),
        }
    }

    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let build_shader =
            |stage: pso::Stage, source: Option<&pso::EntryPoint<'a, B>>| {
                let source = match source {
                    Some(src) => src,
                    None => return Ok((ptr::null_mut(), false)),
                };

                Self::extract_entry_point(stage, source, desc.layout)
                    .map_err(|err| pso::CreationError::Shader(err))
            };

        let (vs, vs_destroy) = build_shader(pso::Stage::Vertex, Some(&desc.shaders.vertex))?;
        let (fs, fs_destroy) = build_shader(pso::Stage::Fragment, desc.shaders.fragment.as_ref())?;
        let (gs, gs_destroy) = build_shader(pso::Stage::Geometry, desc.shaders.geometry.as_ref())?;
        let (ds, ds_destroy) = build_shader(pso::Stage::Domain, desc.shaders.domain.as_ref())?;
        let (hs, hs_destroy) = build_shader(pso::Stage::Hull, desc.shaders.hull.as_ref())?;

        // Define input element descriptions
        let input_element_descs = desc.attributes
            .iter()
            .filter_map(|attrib| {
                let buffer_desc = if let Some(buffer_desc) = desc.vertex_buffers.get(attrib.binding as usize) {
                    buffer_desc
                } else {
                    error!("Couldn't find associated vertex buffer description {:?}", attrib.binding);
                    return Some(Err(pso::CreationError::Other));
                };

                let slot_class = match buffer_desc.rate {
                    0 => d3d12::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    _ => d3d12::D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                };
                let format = attrib.element.format;

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
                    InputSlot: attrib.binding as _,
                    AlignedByteOffset: attrib.element.offset,
                    InputSlotClass: slot_class,
                    InstanceDataStepRate: buffer_desc.rate as _,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Input slots
        let mut vertex_strides = [0; MAX_VERTEX_BUFFERS];
        for (stride, buffer) in vertex_strides.iter_mut().zip(desc.vertex_buffers.iter()) {
            *stride = buffer.stride;
        }

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
            for (rtv, target) in rtvs.iter_mut()
                .zip(pass.color_attachments.iter())
            {
                let format = desc.subpass.main_pass.attachments[target.0].format;
                *rtv = format.and_then(conv::map_format).unwrap_or(dxgiformat::DXGI_FORMAT_UNKNOWN);
                num_rtvs += 1;
            }
            (rtvs, num_rtvs)
        };

        // Setup pipeline description
        let pso_desc = d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: desc.layout.raw,
            VS: shader_bytecode(vs),
            PS: shader_bytecode(fs),
            GS: shader_bytecode(gs),
            DS: shader_bytecode(ds),
            HS: shader_bytecode(hs),
            StreamOutput: d3d12::D3D12_STREAM_OUTPUT_DESC {
                pSODeclaration: ptr::null(),
                NumEntries: 0,
                pBufferStrides: ptr::null(),
                NumStrides: 0,
                RasterizedStream: 0,
            },
            BlendState: d3d12::D3D12_BLEND_DESC {
                AlphaToCoverageEnable: if desc.blender.alpha_coverage { TRUE } else { FALSE },
                IndependentBlendEnable: TRUE,
                RenderTarget: conv::map_render_targets(&desc.blender.targets),
            },
            SampleMask: UINT::max_value(),
            RasterizerState: conv::map_rasterizer(&desc.rasterizer),
            DepthStencilState: desc.depth_stencil.as_ref().map_or(unsafe { mem::zeroed() }, conv::map_depth_stencil),
            InputLayout: d3d12::D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_element_descs.as_ptr(),
                NumElements: input_element_descs.len() as u32,
            },
            IBStripCutValue: d3d12::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED, // TODO
            PrimitiveTopologyType: conv::map_topology_type(desc.input_assembler.primitive),
            NumRenderTargets: num_rtvs,
            RTVFormats: rtvs,
            DSVFormat: pass.depth_stencil_attachment
                .and_then(|att_ref|
                    desc.subpass
                        .main_pass
                        .attachments[att_ref.0]
                        .format
                        .and_then(|f| conv::map_format_dsv(f.base_format().0))
                )
                .unwrap_or(dxgiformat::DXGI_FORMAT_UNKNOWN),
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
                Count: 1, // TODO
                Quality: 0, // TODO
            },
            NodeMask: 0,
            CachedPSO: d3d12::D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: d3d12::D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        let topology = conv::map_topology(desc.input_assembler.primitive);

        // Create PSO
        let mut pipeline = ptr::null_mut();
        let hr = unsafe {
            self.raw.clone().CreateGraphicsPipelineState(
                &pso_desc,
                &d3d12::IID_ID3D12PipelineState,
                &mut pipeline as *mut *mut _ as *mut *mut _)
        };

        let destroy_shader = |shader: *mut d3dcommon::ID3DBlob| unsafe { (*shader).Release() };

        if vs_destroy { destroy_shader(vs); }
        if fs_destroy { destroy_shader(fs); }
        if gs_destroy { destroy_shader(gs); }
        if hs_destroy { destroy_shader(hs); }
        if ds_destroy { destroy_shader(ds); }

        if winerror::SUCCEEDED(hr) {
            Ok(n::GraphicsPipeline {
                raw: pipeline,
                signature: desc.layout.raw,
                num_parameter_slots: desc.layout.num_parameter_slots,
                topology,
                constants: desc.layout.root_constants.clone(),
                vertex_strides,
                baked_states: desc.baked_states.clone(),
            })
        } else {
            Err(pso::CreationError::Other)
        }
    }

    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        let (cs, cs_destroy) =
            Self::extract_entry_point(
                pso::Stage::Compute,
                &desc.shader,
                desc.layout,
            )
            .map_err(|err| pso::CreationError::Shader(err))?;

        let pso_desc = d3d12::D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: desc.layout.raw,
            CS: shader_bytecode(cs),
            NodeMask: 0,
            CachedPSO: d3d12::D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: d3d12::D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        // Create PSO
        let mut pipeline = ptr::null_mut();
        let hr = unsafe {
            self.raw.clone().CreateComputePipelineState(
                &pso_desc,
                &d3d12::IID_ID3D12PipelineState,
                &mut pipeline as *mut *mut _ as *mut *mut _)
        };

        if cs_destroy {
            unsafe { (*cs).Release(); }
        }

        if winerror::SUCCEEDED(hr) {
            Ok(n::ComputePipeline {
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
        _renderpass: &n::RenderPass,
        attachments: I,
        _extent: image::Extent,
    ) -> Result<n::Framebuffer, d::FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>
    {
        Ok(n::Framebuffer {
            attachments: attachments.into_iter().map(|att| *att.borrow()).collect(),
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, d::ShaderError> {
        Ok(n::ShaderModule::Spirv(raw_data.into()))
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
        memory: &n::Memory,
        offset: u64,
        buffer: UnboundBuffer,
    ) -> Result<n::Buffer, d::BindError> {
        if buffer.requirements.type_mask & (1 << memory.type_id) == 0 {
            error!("Bind memory failure: supported mask 0x{:x}, given id {}",
                buffer.requirements.type_mask, memory.type_id);
            return Err(d::BindError::WrongMemory)
        }
        if offset + buffer.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds)
        }

        let mut resource = ptr::null_mut();
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
                memory.heap.as_raw(),
                offset,
                &desc,
                d3d12::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &d3d12::IID_ID3D12Resource,
                &mut resource,
            )
        });

        let clear_uav = if buffer.usage.contains(buffer::Usage::TRANSFER_DST) {
            let handles = self.uav_pool.lock().unwrap().alloc_handles(1);
            let mut desc = d3d12::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: dxgiformat::DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: d3d12::D3D12_UAV_DIMENSION_BUFFER,
                u: unsafe { mem::zeroed() },
            };

           *unsafe { desc.u.Buffer_mut() } = d3d12::D3D12_BUFFER_UAV {
                FirstElement: 0,
                NumElements: (buffer.requirements.size / 4) as _,
                StructureByteStride: 0,
                CounterOffsetInBytes: 0,
                Flags: d3d12::D3D12_BUFFER_UAV_FLAG_RAW,
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
            clear_uav,
        })
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        _buffer: &n::Buffer,
        _format: Option<format::Format>,
        _range: R,
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        flags: image::StorageFlags,
    ) -> Result<UnboundImage, image::CreationError> {
        assert!(mip_levels <= kind.num_levels());

        let base_format = format.base_format();
        let format_desc = base_format.0.desc();

        let aspects = format_desc.aspects;
        let bytes_per_block = (format_desc.bits / 8) as _;
        let block_dim = format_desc.dim;

        let extent = kind.extent();
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
            Layout: match tiling {
                image::Tiling::Optimal => d3d12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
                image::Tiling::Linear => d3d12::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            },
            Flags: conv::map_image_flags(usage),
        };

        let alloc_info = unsafe {
            self.raw.clone().GetResourceAllocationInfo(0, 1, &desc)
        };

        let type_mask_shift = if self.private_caps.heterogeneous_resource_heaps {
            MEM_TYPE_UNIVERSAL_SHIFT
        } else if usage.can_target() {
            MEM_TYPE_TARGET_SHIFT
        } else {
            MEM_TYPE_IMAGE_SHIFT
        };

        Ok(UnboundImage {
            dsv_format: conv::map_format_dsv(base_format.0)
                .unwrap_or(desc.Format),
            desc,
            requirements: memory::Requirements {
                size: alloc_info.SizeInBytes,
                alignment: alloc_info.Alignment,
                type_mask: MEM_TYPE_MASK << type_mask_shift,
            },
            kind,
            usage,
            aspects,
            storage_flags: flags,
            bytes_per_block,
            block_dim,
            num_levels: mip_levels,
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
        use self::image::Usage;

        if image.requirements.type_mask & (1 << memory.type_id) == 0 {
            error!("Bind memory failure: supported mask 0x{:x}, given id {}",
                image.requirements.type_mask, memory.type_id);
            return Err(d::BindError::WrongMemory)
        }
        if offset + image.requirements.size > memory.size {
            return Err(d::BindError::OutOfBounds)
        }

        let mut resource = ptr::null_mut();
        let num_layers = image.kind.num_layers();

        assert_eq!(winerror::S_OK, unsafe {
            self.raw.clone().CreatePlacedResource(
                memory.heap.as_raw(),
                offset,
                &image.desc,
                d3d12::D3D12_RESOURCE_STATE_COMMON,
                ptr::null(),
                &d3d12::IID_ID3D12Resource,
                &mut resource,
            )
        });

        let info = ViewInfo {
            resource: resource as *mut _,
            kind: image.kind,
            flags: image::StorageFlags::empty(),
            view_kind: match image.kind {
                image::Kind::D1(..) => image::ViewKind::D1Array,
                image::Kind::D2(..) => image::ViewKind::D2Array,
                image::Kind::D3(..) => image::ViewKind::D3,
            },
            format: image.desc.Format,
            range: image::SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0 .. 1, //TODO?
                layers: 0 .. num_layers,
            },
        };

        //TODO: the clear_Xv is incomplete. We should support clearing images created without XXX_ATTACHMENT usage.
        // for this, we need to check the format and force the `RENDER_TARGET` flag behind the user's back
        // if the format supports being rendered into, allowing us to create clear_Xv

        Ok(n::Image {
            resource: resource as *mut _,
            kind: image.kind,
            usage: image.usage,
            storage_flags: image.storage_flags,
            dxgi_format: image.desc.Format,
            bytes_per_block: image.bytes_per_block,
            block_dim: image.block_dim,
            num_levels: image.num_levels,
            clear_cv: if image.aspects.contains(Aspects::COLOR) && image.usage.contains(Usage::COLOR_ATTACHMENT) {
                Some(self.view_image_as_render_target(info.clone()).unwrap())
            } else {
                None
            },
            clear_dv: if image.aspects.contains(Aspects::DEPTH) && image.usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
                Some(self.view_image_as_depth_stencil(ViewInfo {
                    format: image.dsv_format,
                    range: image::SubresourceRange {
                        aspects: Aspects::DEPTH,
                        levels: 0 .. 1, //TODO?
                        layers: 0 .. num_layers,
                    },
                    .. info.clone()
                }).unwrap())
            } else {
                None
            },
            clear_sv: if image.aspects.contains(Aspects::STENCIL) && image.usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
                Some(self.view_image_as_depth_stencil(ViewInfo {
                    format: image.dsv_format,
                    range: image::SubresourceRange {
                        aspects: Aspects::STENCIL,
                        levels: 0 .. 1, //TODO?
                        layers: 0 .. num_layers,
                    },
                    .. info.clone()
                }).unwrap())
            } else {
                None
            },
        })
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        view_kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        let info = ViewInfo {
            resource: image.resource,
            kind: image.kind,
            flags: image.storage_flags,
            view_kind,
            format: conv::map_format(format)
                .ok_or(image::ViewError::BadFormat)?,
            range,
        };

        Ok(n::ImageView {
            resource: image.resource,
            handle_srv: if image.usage.contains(image::Usage::SAMPLED) {
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
                Some(self.view_image_as_depth_stencil(ViewInfo {
                    format: conv::map_format_dsv(format.base_format().0)
                        .ok_or(image::ViewError::BadFormat)?,
                    .. info
                })?)
            } else {
                None
            },
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let handle = self.sampler_pool.lock().unwrap().alloc_handles(1).cpu;

        let op = match info.comparison {
            Some(_) => d3d12::D3D12_FILTER_REDUCTION_TYPE_COMPARISON,
            None => d3d12::D3D12_FILTER_REDUCTION_TYPE_STANDARD,
        };
        let desc = d3d12::D3D12_SAMPLER_DESC {
            Filter: conv::map_filter(info.mag_filter, info.min_filter, info.mip_filter, op),
            AddressU: conv::map_wrap(info.wrap_mode.0),
            AddressV: conv::map_wrap(info.wrap_mode.1),
            AddressW: conv::map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.anisotropic {
                image::Anisotropic::On(max) => max as _, // TODO: check support here?
                image::Anisotropic::Off => 0,
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

    fn create_descriptor_pool<I>(
        &self,
        max_sets: usize,
        descriptor_pools: I,
    ) -> n::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>
    {
        let mut num_srv_cbv_uav = 0;
        let mut num_samplers = 0;

        let descriptor_pools = descriptor_pools.into_iter()
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
            let mut heap_srv_cbv_uav = self
                .heap_srv_cbv_uav
                .lock()
                .unwrap();

            let range = heap_srv_cbv_uav
                .allocator
                .allocate(num_srv_cbv_uav as _)
                .unwrap(); // TODO: error/resize
            n::DescriptorHeapSlice {
                heap: heap_srv_cbv_uav.raw.clone(),
                handle_size: heap_srv_cbv_uav.handle_size as _,
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
                .allocate(num_samplers as _)
                .unwrap(); // TODO: error/resize
            n::DescriptorHeapSlice {
                heap: heap_sampler.raw.clone(),
                handle_size: heap_sampler.handle_size as _,
                next: range.start as _,
                range,
                start: heap_sampler.start,
            }
        };

        n::DescriptorPool {
            heap_srv_cbv_uav,
            heap_sampler,
            pools: descriptor_pools,
            max_size: max_sets as _,
        }
    }

    fn create_descriptor_set_layout<I>(
        &self,
        bindings: I,
    )-> n::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>
    {
        n::DescriptorSetLayout {
            bindings: bindings.into_iter().map(|bind| bind.borrow().clone()).collect()
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
                            let max_size = 1u64<<12; //arbitrary
                            descriptor_update_pools.push(n::DescriptorCpuPool {
                                heap: Self::create_descriptor_heap_impl(
                                    &mut self.raw.clone(),
                                    d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                                    false,
                                    max_size as _,
                                ),
                                offset: 0,
                                size: 0,
                                max_size,
                            });
                        }
                        let heap = descriptor_update_pools.last_mut().unwrap();
                        let handle = heap.alloc_handles(1).cpu;
                        if heap.size == heap.max_size {
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
                                self.raw.CreateUnorderedAccessView(buffer.resource, ptr::null_mut(), &desc, handle);
                            }
                        } else {
                            // Making the size field of buffer requirements for uniform
                            // buffers a multiple of 256 and setting the required offset
                            // alignment to 256 allows us to patch the size here.
                            // We can always enforce the size to be aligned to 256 for
                            // CBVs without going out-of-bounds.
                            let size = ((end - start) + 255) & !255;
                            let desc = d3d12::D3D12_CONSTANT_BUFFER_VIEW_DESC {
                                BufferLocation: unsafe { (*buffer.resource).GetGPUVirtualAddress() } + start,
                                SizeInBytes: size as _,
                            };
                            unsafe { self.raw.CreateConstantBufferView(&desc, handle); }
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
                    pso::Descriptor::TexelBuffer(_) => unimplemented!()
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
            buffer_desc_pool.size = 0;
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
            if let (Some(src_range), Some(dst_range)) = (src_info.view_range.as_ref(), dst_info.view_range.as_ref()) {
                assert!(copy.src_array_offset + copy.count <= src_range.count as usize);
                assert!(copy.dst_array_offset + copy.count <= dst_range.count as usize);
                src_views.push(src_range.at(copy.src_array_offset as _));
                dst_views.push(dst_range.at(copy.dst_array_offset as _));
                num_views.push(copy.count as u32);
            }
            if let (Some(src_range), Some(dst_range)) = (src_info.sampler_range.as_ref(), dst_info.sampler_range.as_ref()) {
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

    fn map_memory<R>(&self, memory: &n::Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        if let Some(mem) = memory.resource {
            let start = range.start().unwrap_or(&0);
            let end = range.end().unwrap_or(&memory.size);
            assert!(start <= end);

            let mut ptr = ptr::null_mut();
            assert_eq!(winerror::S_OK, unsafe {
                (*mem).Map(
                    0,
                    &d3d12::D3D12_RANGE {
                        Begin: 0,
                        End: 0,
                    },
                    &mut ptr,
                )
            });
            unsafe { ptr.offset(*start as _); }
            Ok(ptr as *mut _)
        } else {
            panic!("Memory not created with a memory type exposing `CPU_VISIBLE`.")
        }
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        if let Some(mem) = memory.resource {
            unsafe {
                (*mem).Unmap(
                    0,
                    &d3d12::D3D12_RANGE {
                        Begin: 0,
                        End: 0,
                    },
                );
            }
        }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        for range in ranges {
            let &(ref memory, ref range) = range.borrow();
            if let Some(mem) = memory.resource {
                // map and immediately unmap, hoping that dx12 drivers internally cache
                // currently mapped buffers.
                assert_eq!(winerror::S_OK, unsafe {
                    (*mem).Map(
                        0,
                        &d3d12::D3D12_RANGE {
                            Begin: 0,
                            End: 0,
                        },
                        ptr::null_mut(),
                    )
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
        I::Item: Borrow<(&'a n::Memory, R)>,
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
                    (*mem).Unmap(
                        0,
                        &d3d12::D3D12_RANGE {
                            Begin: 0,
                            End: 0,
                        },
                    );
                }
            }
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        let fence = self.create_fence(false);
        n::Semaphore {
            raw: fence.raw,
        }
    }

    fn create_fence(&self, signalled: bool) -> n::Fence {
        n::Fence {
            raw: unsafe { ComPtr::from_raw(self.create_raw_fence(signalled)) },
        }
    }

    fn reset_fence(&self, fence: &n::Fence) {
        assert_eq!(winerror::S_OK, unsafe {
            fence.raw.clone().Signal(0)
        });
    }

    fn wait_for_fences<I>(&self, fences: I, wait: d::WaitFor, timeout_ms: u32) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let fences = fences.into_iter().collect::<Vec<_>>();
        let mut events = self.events.lock().unwrap();
        for _ in events.len() .. fences.len() {
            events.push(unsafe {
                synchapi::CreateEventA(
                    ptr::null_mut(),
                    FALSE,
                    FALSE,
                    ptr::null(),
                )
            });
        }

        for (&event, fence) in events.iter().zip(fences.iter()) {
            assert_eq!(winerror::S_OK, unsafe {
                synchapi::ResetEvent(event);
                fence.borrow().raw.clone().SetEventOnCompletion(1, event)
            });
        }

        let all = match wait {
            d::WaitFor::Any => FALSE,
            d::WaitFor::All => TRUE,
        };
        let hr = unsafe {
            synchapi::WaitForMultipleObjects(fences.len() as u32, events.as_ptr(), all, timeout_ms)
        };

        const WAIT_OBJECT_LAST: u32 = winbase::WAIT_OBJECT_0 + winnt::MAXIMUM_WAIT_OBJECTS;
        const WAIT_ABANDONED_LAST: u32 = winbase::WAIT_ABANDONED_0 + winnt::MAXIMUM_WAIT_OBJECTS;
        match hr {
            winbase::WAIT_OBJECT_0 ... WAIT_OBJECT_LAST => true,
            winbase::WAIT_ABANDONED_0 ... WAIT_ABANDONED_LAST => true, //TODO?
            winerror::WAIT_TIMEOUT => false,
            _ => panic!("Unexpected wait status 0x{:X}", hr),
        }
    }

    fn get_fence_status(&self, _fence: &n::Fence) -> bool {
        unimplemented!()
    }

    fn free_memory(&self, memory: n::Memory) {
        if let Some(buffer) = memory.resource {
            unsafe { (*buffer).Release(); }
        }
    }

    fn create_query_pool(&self, query_ty: query::QueryType, count: u32) -> n::QueryPool {
        let heap_ty = match query_ty {
            query::QueryType::Occlusion =>
                d3d12::D3D12_QUERY_HEAP_TYPE_OCCLUSION,
            query::QueryType::PipelineStatistics(_) =>
                d3d12::D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS,
            query::QueryType::Timestamp =>
                d3d12::D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
        };

        let desc = d3d12::D3D12_QUERY_HEAP_DESC {
            Type: heap_ty,
            Count: count,
            NodeMask: 0,
        };

        let mut handle = ptr::null_mut();
        assert_eq!(winerror::S_OK, unsafe {
            self.raw.clone().CreateQueryHeap(
                &desc,
                &d3d12::IID_ID3D12QueryHeap,
                &mut handle,
            )
        });

        n::QueryPool {
            raw: unsafe { ComPtr::from_raw(handle as *mut _) },
            ty: heap_ty,
        }
    }

    fn destroy_query_pool(&self, _pool: n::QueryPool) {
        // Just drop
    }

    fn destroy_shader_module(&self, shader_lib: n::ShaderModule) {
        if let n::ShaderModule::Compiled(shaders) = shader_lib {
            for (_, _blob) in shaders {
                //unsafe { blob.Release(); } //TODO
            }
        }
    }

    fn destroy_render_pass(&self, _rp: n::RenderPass) {
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

    fn create_swapchain(
        &self,
        surface: &mut w::Surface,
        config: hal::SwapchainConfig,
    ) -> (w::Swapchain, hal::Backbuffer<B>) {
        let mut swap_chain: *mut dxgi1_2::IDXGISwapChain1 = ptr::null_mut();

        let format = match config.color_format {
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
            Format: conv::map_format(config.color_format).unwrap(),
            ViewDimension: d3d12::D3D12_RTV_DIMENSION_TEXTURE2D,
            .. unsafe { mem::zeroed() }
        };
        let rtv_heap = Device::create_descriptor_heap_impl(
            &mut self.raw.clone(),
            d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            false,
            config.image_count as _,
        );

        // TODO: double-check values
        let desc = dxgi1_2::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: dxgi1_2::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: config.image_count,
            Width: surface.width,
            Height: surface.height,
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
                self.present_queue.as_raw() as *mut _,
                surface.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                &mut swap_chain as *mut *mut _,
            )
        };

        if !winerror::SUCCEEDED(hr) {
            error!("error on swapchain creation 0x{:x}", hr);
        }

        let swap_chain = unsafe { ComPtr::<dxgi1_4::IDXGISwapChain3>::from_raw(swap_chain as _) };

        // Get backbuffer images
        let images = (0 .. config.image_count).map(|i| {
            let mut resource: *mut d3d12::ID3D12Resource = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    i as _,
                    &d3d12::IID_ID3D12Resource,
                    &mut resource as *mut *mut _ as *mut *mut _);
            }

            let rtv_handle = rtv_heap.at(i as _).cpu;
            unsafe {
                self.raw.clone().CreateRenderTargetView(resource, &rtv_desc, rtv_handle);
            }

            let format_desc = config
                .color_format
                .base_format()
                .0
                .desc();

            let bytes_per_block = (format_desc.bits / 8) as _;
            let block_dim = format_desc.dim;

            let kind = image::Kind::D2(surface.width, surface.height, 1, 1);
            n::Image {
                resource,
                kind,
                usage: config.image_usage,
                storage_flags: image::StorageFlags::empty(),
                dxgi_format: format,
                bytes_per_block,
                block_dim,
                num_levels: 1,
                clear_cv: Some(rtv_handle),
                clear_dv: None,
                clear_sv: None,
            }
        }).collect();

        let swapchain = w::Swapchain {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            rtv_heap,
        };

        (swapchain, hal::Backbuffer::Images(images))
    }

    fn destroy_swapchain(&self, _swapchain: w::Swapchain) {
        // automatic
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        for queue in &self.queues {
            queue.wait_idle()?;
        }
        Ok(())
    }
}
