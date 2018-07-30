use {
    Backend, PrivateCapabilities, QueueFamily,
    Shared, Surface, Swapchain, validate_line_width, BufferPtr, SamplerPtr, TexturePtr,
};
use {conversions as conv, command, native as n};
use internal::FastStorageMap;
use native;
use range_alloc::RangeAllocator;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::{cmp, mem, slice, thread, time};

use hal::{self, error, image, pass, format, mapping, memory, buffer, pso, query, window};
use hal::device::{BindError, OutOfMemory, FramebufferError, ShaderError};
use hal::memory::Properties;
use hal::pool::CommandPoolCreateFlags;
use hal::queue::{QueueFamilyId, Queues};
use hal::range::RangeArg;

use cocoa::foundation::{NSRange, NSUInteger};
use foreign_types::ForeignType;
use metal::{self,
    MTLFeatureSet, MTLLanguageVersion, MTLArgumentAccess, MTLDataType, MTLPrimitiveType, MTLPrimitiveTopologyClass,
    MTLCPUCacheMode, MTLStorageMode, MTLResourceOptions,
    MTLVertexStepFunction, MTLSamplerBorderColor, MTLSamplerMipFilter, MTLTextureType,
    CaptureManager
};
use objc::rc::autoreleasepool;
use parking_lot::Mutex;
use smallvec::SmallVec;
use spirv_cross::{msl, spirv, ErrorCode as SpirvErrorCode};


const RESOURCE_HEAP_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v3,
    MTLFeatureSet::iOS_GPUFamily2_v3,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::tvOS_GPUFamily1_v2,
];

const ARGUMENT_BUFFER_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::macOS_GPUFamily1_v3,
];

const BASE_INSTANCE_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::iOS_GPUFamily3_v1,
];

const PUSH_CONSTANTS_DESC_SET: u32 = !0;
const PUSH_CONSTANTS_DESC_BINDING: u32 = 0;

//The offset and bytesPerRow parameters must be byte aligned to the size returned by the
// minimumLinearTextureAlignmentForPixelFormat: method. The bytesPerRow parameter must also be
// greater than or equal to the size of one pixel, in bytes, multiplied by the pixel width of one row.
const STRIDE_MASK: u64 = 0xFF;

/// Emit error during shader module parsing.
fn gen_parse_error(err: SpirvErrorCode) -> ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unknown parse error".into(),
    };
    ShaderError::CompilationFailed(msg)
}

fn create_function_constants(specialization: &[pso::Specialization]) -> metal::FunctionConstantValues {
    let constants_raw = metal::FunctionConstantValues::new();
    for constant in specialization {
        unsafe {
            let (ty, value) = match constant.value {
                pso::Constant::Bool(ref v) => (MTLDataType::Bool, v as *const _ as *const _),
                pso::Constant::U32(ref v) => (MTLDataType::UInt, v as *const _ as *const _),
                pso::Constant::I32(ref v) => (MTLDataType::Int, v as *const _ as *const _),
                pso::Constant::F32(ref v) => (MTLDataType::Float, v as *const _ as *const _),
                _ => panic!("Unsupported specialization constant type"),
            };
            constants_raw.set_constant_value_at_index(constant.id as u64, ty, value);
        }
    }
    constants_raw
}

fn get_final_function(library: &metal::LibraryRef, entry: &str, specialization: &[pso::Specialization]) -> Result<metal::Function, ()> {
    let initial_constants = if specialization.is_empty() {
        None
    } else {
        Some(create_function_constants(specialization))
    };

    let mut mtl_function = library
        .get_function(entry, initial_constants)
        .map_err(|_| {
            error!("Invalid vertex shader entry point");
            ()
        })?;
    let has_more_function_constants = unsafe {
        let dictionary = mtl_function.function_constants_dictionary();
        let count: NSUInteger = msg_send![dictionary, count];
        count > 0
    };
    if has_more_function_constants {
        // TODO: check that all remaining function constants are optional, otherwise return an error
        if specialization.is_empty() {
            // These may be optional function constants, in which case we need to specialize the function with an empty set of constants
            // or we'll get an error when we make the PSO
            mtl_function = library
                .get_function(entry, Some(create_function_constants(&[])))
                .map_err(|_| {
                    error!("Invalid vertex shader entry point");
                    ()
                })?;
        }
    }

    Ok(mtl_function)
}

//#[derive(Clone)]
pub struct Device {
    pub(crate) shared: Arc<Shared>,
    pub(crate) private_caps: PrivateCapabilities,
    memory_types: [hal::MemoryType; 4],
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        if cfg!(feature = "auto-capture") {
            info!("Metal capture stop");
            let shared_capture_manager = CaptureManager::shared();
            if let Some(default_capture_scope) = shared_capture_manager.default_capture_scope() {
                default_capture_scope.end_scope();
            }
            shared_capture_manager.stop_capture();
        }
    }
}

bitflags! {
    /// Memory type bits.
    struct MemoryTypes: u64 {
        const PRIVATE = 1<<0;
        const SHARED = 1<<1;
        const MANAGED_UPLOAD = 1<<2;
        const MANAGED_DOWNLOAD = 1<<3;
    }
}

impl MemoryTypes {
    fn describe(index: usize) -> (MTLStorageMode, MTLCPUCacheMode) {
        match Self::from_bits(1 << index).unwrap() {
            Self::PRIVATE          => (MTLStorageMode::Private, MTLCPUCacheMode::DefaultCache),
            Self::SHARED           => (MTLStorageMode::Shared,  MTLCPUCacheMode::DefaultCache),
            Self::MANAGED_UPLOAD   => (MTLStorageMode::Managed, MTLCPUCacheMode::WriteCombined),
            Self::MANAGED_DOWNLOAD => (MTLStorageMode::Managed, MTLCPUCacheMode::DefaultCache),
            _ => unreachable!()
        }
    }
}

pub struct PhysicalDevice {
    shared: Arc<Shared>,
    memory_types: [hal::MemoryType; 4],
    private_caps: PrivateCapabilities,
}
unsafe impl Send for PhysicalDevice {}
unsafe impl Sync for PhysicalDevice {}

impl PhysicalDevice {
    fn is_mac(raw: &metal::DeviceRef) -> bool {
        raw.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v1)
    }
    fn supports_any(raw: &metal::DeviceRef, features_sets: &[MTLFeatureSet]) -> bool {
        features_sets.iter().cloned().any(|x| raw.supports_feature_set(x))
    }

    pub(crate) fn new(device: metal::Device) -> Self {
        let private_caps = PrivateCapabilities {
            //TODO: MSL versions only depend on the OS version, not feature sets
            msl_version: if device.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v3) {
                MTLLanguageVersion::V2_0
            } else if device.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v2) {
                MTLLanguageVersion::V1_2
            } else {
                MTLLanguageVersion::V1_1
            },
            exposed_queues: 1,
            resource_heaps: Self::supports_any(&device, RESOURCE_HEAP_SUPPORT),
            argument_buffers: Self::supports_any(&device, ARGUMENT_BUFFER_SUPPORT) && false, //TODO
            shared_textures: !Self::is_mac(&device),
            base_instance: Self::supports_any(&device, BASE_INSTANCE_SUPPORT),
            format_depth24_stencil8: device.d24_s8_supported(),
            format_depth32_stencil8: true, //TODO: crashing the Metal validation layer upon copying from buffer
            format_min_srgb_channels: if Self::is_mac(&device) {4} else {1},
            format_b5: !Self::is_mac(&device),
            max_buffers_per_stage: 31,
            max_textures_per_stage: if Self::is_mac(&device) {128} else {31},
            max_samplers_per_stage: 16,
            buffer_alignment: if Self::is_mac(&device) {256} else {64},
            max_buffer_size: if Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v2, MTLFeatureSet::macOS_GPUFamily1_v3]) {
                1 << 30 // 1GB on macOS 1.2 and up
            } else {
                1 << 28 // 256MB otherwise
            },
        };

        let shared = Arc::new(Shared::new(device));
        assert!((shared.push_constants_buffer_id as usize) < private_caps.max_buffers_per_stage);

        PhysicalDevice {
            shared,
            memory_types: [
                hal::MemoryType { // PRIVATE
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 0,
                },
                hal::MemoryType { // SHARED
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                    heap_index: 1,
                },
                hal::MemoryType { // MANAGED_UPLOAD
                    properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE,
                    heap_index: 1,
                },
                hal::MemoryType { // MANAGED_DOWNLOAD
                    properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                    heap_index: 1,
                },
            ],
            private_caps,
        }
    }
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: &[(&QueueFamily, &[hal::QueuePriority])],
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        // TODO: Handle opening a physical device multiple times
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].1.len(), 1);
        let family = *families[0].0;

        if cfg!(feature = "auto-capture") {
            info!("Metal capture start");
            let device = self.shared.device.lock();
            let shared_capture_manager = CaptureManager::shared();
            let default_capture_scope = shared_capture_manager.new_capture_scope_with_device(&*device);
            shared_capture_manager.set_default_capture_scope(default_capture_scope);
            shared_capture_manager.start_capture_with_scope(&default_capture_scope);
            default_capture_scope.begin_scope();
        }

        let mut queue_group = hal::backend::RawQueueGroup::new(family);
        for _ in 0 .. self.private_caps.exposed_queues {
            queue_group.add_queue(command::CommandQueue::new(self.shared.clone()));
        }

        let device = Device {
            shared: self.shared.clone(),
            private_caps: self.private_caps.clone(),
            memory_types: self.memory_types,
        };

        Ok(hal::Gpu {
            device,
            queues: Queues::new(vec![queue_group]),
        })
    }

    fn format_properties(&self, format: Option<format::Format>) -> format::Properties {
        match format.and_then(|f| self.private_caps.map_format(f)) {
            Some(_) => format::Properties {
                linear_tiling: format::ImageFeature::empty(),
                optimal_tiling: format::ImageFeature::all(),
                buffer_features: format::BufferFeature::all(),
            },
            None => format::Properties {
                linear_tiling: format::ImageFeature::empty(),
                optimal_tiling: format::ImageFeature::empty(),
                buffer_features: format::BufferFeature::empty(),
            },
        }
    }

    fn image_format_properties(
        &self, format: format::Format, dimensions: u8, tiling: image::Tiling,
        usage: image::Usage, storage_flags: image::StorageFlags,
    ) -> Option<image::FormatProperties> {
        if let image::Tiling::Linear = tiling {
            let format_desc = format.surface_desc();
            let host_usage = image::Usage::TRANSFER_SRC | image::Usage::TRANSFER_DST;
            if dimensions != 2 ||
                !storage_flags.is_empty() ||
                !host_usage.contains(usage) ||
                format_desc.aspects != format::Aspects::COLOR ||
                format_desc.is_compressed()
            {
                return None
            }
        }
        if dimensions == 1 && usage.intersects(image::Usage::COLOR_ATTACHMENT | image::Usage::DEPTH_STENCIL_ATTACHMENT) {
            // MTLRenderPassDescriptor texture must not be MTLTextureType1D
            return None;
        }
        //TODO: actually query this data
        let max_dimension = 4096u32;
        let max_extent = image::Extent {
            width: max_dimension,
            height: if dimensions >= 2 { max_dimension } else { 1 },
            depth: if dimensions >= 3 { max_dimension } else { 1 },
        };

        self.private_caps.map_format(format).map(|_| image::FormatProperties {
            max_extent,
            max_levels: if dimensions == 1 { 1 } else { 12 },
            // 3D images enforce a single layer
            max_layers: if dimensions == 3 { 1 } else { 2048 },
            sample_count_mask: 0x1,
            //TODO: buffers and textures have separate limits
            // Max buffer size is determined by feature set
            // Max texture size does not appear to be documented publicly
            max_resource_size: self.private_caps.max_buffer_size as _,
        })
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        hal::MemoryProperties {
            memory_heaps: vec![
                !0, //TODO: private memory limits
                self.private_caps.max_buffer_size,
            ],
            memory_types: self.memory_types.to_vec(),
        }
    }

    fn features(&self) -> hal::Features {
        hal::Features::ROBUST_BUFFER_ACCESS |
        hal::Features::DRAW_INDIRECT_FIRST_INSTANCE |
        hal::Features::DEPTH_CLAMP
    }

    fn limits(&self) -> hal::Limits {
        hal::Limits {
            max_texture_size: 4096, // TODO: feature set
            max_patch_size: 0, // No tessellation

            // Note: The maximum number of supported viewports and scissor rectangles varies by device.
            // TODO: read from Metal Feature Sets.
            max_viewports: 1,

            min_buffer_copy_offset_alignment: self.private_caps.buffer_alignment,
            min_buffer_copy_pitch_alignment: 4,
            min_texel_buffer_offset_alignment: self.private_caps.buffer_alignment,
            min_uniform_buffer_offset_alignment: self.private_caps.buffer_alignment,
            min_storage_buffer_offset_alignment: self.private_caps.buffer_alignment,

            max_compute_group_count: [16; 3], // TODO
            max_compute_group_size: [64; 3], // TODO

            max_vertex_input_attributes: 31,
            max_vertex_input_bindings: 31,
            max_vertex_input_attribute_offset: 255, // TODO
            max_vertex_input_binding_stride: 256, // TODO
            max_vertex_output_components: 16, // TODO

            framebuffer_color_samples_count: 0b101, // TODO
            framebuffer_depth_samples_count: 0b101, // TODO
            framebuffer_stencil_samples_count: 0b101, // TODO
            max_color_attachments: 1, // TODO

            // Note: we issue Metal buffer-to-buffer copies on memory flush/invalidate,
            // and those need to operate on sizes being multiples of 4.
            non_coherent_atom_size: 4,
        }
    }
}

pub struct LanguageVersion {
    pub major: u8,
    pub minor: u8,
}

impl LanguageVersion {
    pub fn new(major: u8, minor: u8) -> Self {
        LanguageVersion { major, minor }
    }
}

impl Device {
    fn _is_heap_coherent(&self, heap: &n::MemoryHeap) -> bool {
        match *heap {
            n::MemoryHeap::Private => false,
            n::MemoryHeap::Public(memory_type, _) => self.memory_types[memory_type.0].properties.contains(Properties::COHERENT),
            n::MemoryHeap::Native(ref heap) => heap.storage_mode() == MTLStorageMode::Shared,
        }
    }

    pub fn create_shader_library_from_file<P>(
        &self, _path: P,
    ) -> Result<n::ShaderModule, ShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &self, source: S, version: LanguageVersion,
    ) -> Result<n::ShaderModule, ShaderError> where S: AsRef<str> {
        let options = metal::CompileOptions::new();
        let msl_version = match version {
            LanguageVersion { major: 1, minor: 0 } => MTLLanguageVersion::V1_0,
            LanguageVersion { major: 1, minor: 1 } => MTLLanguageVersion::V1_1,
            LanguageVersion { major: 1, minor: 2 } => MTLLanguageVersion::V1_2,
            LanguageVersion { major: 2, minor: 0 } => MTLLanguageVersion::V2_0,
            _ => return Err(ShaderError::CompilationFailed("shader model not supported".into()))
        };
        if msl_version > self.private_caps.msl_version {
            return Err(ShaderError::CompilationFailed("shader model too high".into()))
        }
        options.set_language_version(msl_version);

        self.shared.device
            .lock()
            .new_library_with_source(source.as_ref(), &options)
            .map(|library| n::ShaderModule::Compiled {
                library,
                entry_point_map: n::EntryPointMap::default(),
            })
            .map_err(|e| ShaderError::CompilationFailed(e.into()))
    }

    fn compile_shader_library(
        &self,
        raw_data: &[u8],
        primitive_class: MTLPrimitiveTopologyClass,
        overrides: &n::ResourceOverrideMap,
    ) -> Result<(metal::Library, n::EntryPointMap), ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        // now parse again using the new overrides
        let mut ast = spirv::Ast::<msl::Target>::parse(&module)
            .map_err(gen_parse_error)?;

        // compile with options
        let mut compiler_options = msl::CompilerOptions::default();
        compiler_options.enable_point_size_builtin = primitive_class == MTLPrimitiveTopologyClass::Point;
        compiler_options.resolve_specialized_array_lengths = true;
        compiler_options.vertex.invert_y = true;
        // fill the overrides
        compiler_options.resource_binding_overrides = overrides
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        ast.set_compiler_options(&compiler_options)
            .map_err(|err| {
                let msg = match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unexpected error".into(),
                };
                ShaderError::CompilationFailed(msg)
            })?;

        let entry_points = ast.get_entry_points()
            .map_err(|err| {
                let msg = match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unexpected error".into(),
                };
                ShaderError::CompilationFailed(msg)
            })?;

        let shader_code = ast.compile()
            .map_err(|err| {
                let msg = match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                };
                ShaderError::CompilationFailed(msg)
            })?;

        let mut entry_point_map = n::EntryPointMap::default();
        for entry_point in entry_points {
            info!("Entry point {:?}", entry_point);
            let cleansed = ast.get_cleansed_entry_point_name(&entry_point.name, entry_point.execution_model)
                .map_err(|err| {
                    let msg = match err {
                        SpirvErrorCode::CompilationError(msg) => msg,
                        SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                    };
                    ShaderError::CompilationFailed(msg)
                })?;
            entry_point_map.insert(entry_point.name, spirv::EntryPoint {
                name: cleansed,
                .. entry_point
            });
        }

        // done
        debug!("SPIRV-Cross generated shader:\n{}", shader_code);

        let options = metal::CompileOptions::new();
        options.set_language_version(self.private_caps.msl_version);

        let library = self.shared.device
            .lock()
            .new_library_with_source(shader_code.as_ref(), &options)
            .map_err(|err| ShaderError::CompilationFailed(err.into()))?;

        Ok((library, entry_point_map))
    }

    fn load_shader(
        &self,
        ep: &pso::EntryPoint<Backend>,
        layout: &n::PipelineLayout,
        primitive_class: MTLPrimitiveTopologyClass,
    ) -> Result<(metal::Library, metal::Function, metal::MTLSize), pso::CreationError> {
        let entries_owned;
        let (lib, entry_point_map) = match *ep.module {
            n::ShaderModule::Compiled {ref library, ref entry_point_map} => {
                (library.to_owned(), entry_point_map)
            }
            n::ShaderModule::Raw(ref data) => {
                let raw = self.compile_shader_library(data, primitive_class, &layout.res_overrides).unwrap();
                entries_owned = raw.1;
                (raw.0, &entries_owned)
            }
        };

        let (name, wg_size) = match entry_point_map.get(ep.entry) {
            Some(p) => (p.name.as_str(), metal::MTLSize {
                width : p.work_group_size.x as _,
                height: p.work_group_size.y as _,
                depth : p.work_group_size.z as _,
            }),
            // this can only happen if the shader came directly from the user
            None => (ep.entry, metal::MTLSize { width: 0, height: 0, depth: 0 }),
        };
        let mtl_function = get_final_function(&lib, name, ep.specialization)
            .map_err(|_| {
                error!("Invalid shader entry point");
                pso::CreationError::Other
            })?;

        Ok((lib, mtl_function, wg_size))
    }

    fn describe_argument(
        ty: pso::DescriptorType, index: pso::DescriptorBinding, count: usize
    ) -> metal::ArgumentDescriptor {
        let arg = metal::ArgumentDescriptor::new().to_owned();
        arg.set_array_length(count as _);

        match ty {
            pso::DescriptorType::Sampler => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Sampler);
                arg.set_index(index as _);
            }
            pso::DescriptorType::SampledImage => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Texture);
                arg.set_index(index as _);
            }
            pso::DescriptorType::UniformBuffer => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Struct);
                arg.set_index(index as _);
            }
            pso::DescriptorType::StorageBuffer => {
                arg.set_access(MTLArgumentAccess::ReadWrite);
                arg.set_data_type(MTLDataType::Struct);
                arg.set_index(index as _);
            }
            _ => unimplemented!()
        }

        arg
    }
}

impl hal::Device<Backend> for Device {
    fn create_command_pool(
        &self, _family: QueueFamilyId, _flags: CommandPoolCreateFlags
    ) -> command::CommandPool {
        command::CommandPool::new(&self.shared)
    }

    fn destroy_command_pool(&self, mut pool: command::CommandPool) {
        use hal::pool::RawCommandPool;
        pool.reset();
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        _subpasses: IS,
        _dependencies: ID,
    ) -> n::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        n::RenderPass {
            attachments: attachments.into_iter()
                .map(|at| at.borrow().clone())
                .collect(),
        }
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        push_constant_ranges: IR,
    ) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>
    {
        let mut stage_infos = [
            (pso::ShaderStageFlags::VERTEX,   spirv::ExecutionModel::Vertex,    n::ResourceCounters::new()),
            (pso::ShaderStageFlags::FRAGMENT, spirv::ExecutionModel::Fragment,  n::ResourceCounters::new()),
            (pso::ShaderStageFlags::COMPUTE,  spirv::ExecutionModel::GlCompute, n::ResourceCounters::new()),
        ];
        let mut res_overrides = n::ResourceOverrideMap::default();
        let mut offsets = Vec::new();

        for (set_index, set_layout) in set_layouts.into_iter().enumerate() {
            // remember where the resources for this set start at each shader stage
            offsets.push(n::MultiStageResourceCounters {
                vs: stage_infos[0].2.clone(),
                ps: stage_infos[1].2.clone(),
                cs: stage_infos[2].2.clone(),
            });
            match *set_layout.borrow() {
                n::DescriptorSetLayout::Emulated(ref desc_layouts, _) => {
                    for layout in desc_layouts.iter() {
                        for &mut (stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                            if !layout.stages.contains(stage_bit) {
                                continue
                            }
                            let res = msl::ResourceBinding {
                                buffer_id: if layout.content.contains(n::DescriptorContent::BUFFER) {
                                    counters.buffers += 1;
                                    (counters.buffers - 1) as _
                                } else { !0 },
                                texture_id: if layout.content.contains(n::DescriptorContent::TEXTURE) {
                                    counters.textures += 1;
                                    (counters.textures - 1) as _
                                } else { !0 },
                                sampler_id: if layout.content.contains(n::DescriptorContent::SAMPLER) {
                                    counters.samplers += 1;
                                    (counters.samplers - 1) as _
                                } else { !0 },
                                force_used: false,
                            };
                            if layout.array_index == 0 {
                                let location = msl::ResourceBindingLocation {
                                    stage,
                                    desc_set: set_index as _,
                                    binding: layout.binding,
                                };
                                res_overrides.insert(location, res);
                            }
                        }
                    }
                }
                n::DescriptorSetLayout::ArgumentBuffer(_, stage_flags) => {
                    for &mut (stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                        if !stage_flags.contains(stage_bit) {
                            continue
                        }
                        let location = msl::ResourceBindingLocation {
                            stage,
                            desc_set: set_index as _,
                            binding: 0,
                        };
                        let res_binding = msl::ResourceBinding {
                            buffer_id: counters.buffers as _,
                            texture_id: !0,
                            sampler_id: !0,
                            force_used: false,
                        };
                        res_overrides.insert(location, res_binding);
                        counters.buffers += 1;
                    }
                }
            }
        }

        let mut pc_limits = [0u32; 3];
        for pcr in push_constant_ranges {
            let (flags, range) = pcr.borrow();
            for (limit, &(stage_bit, _, _)) in pc_limits.iter_mut().zip(&stage_infos) {
                if flags.contains(stage_bit) {
                    *limit = range.end.max(*limit);
                }
            }
        }

        for (limit, &mut (_, stage, ref mut counters)) in pc_limits.iter().zip(&mut stage_infos) {
            // handle the push constant buffer assignment and shader overrides
            if *limit != 0 {
                let buffer_id = self.shared.push_constants_buffer_id;
                res_overrides.insert(
                    msl::ResourceBindingLocation {
                        stage,
                        desc_set: PUSH_CONSTANTS_DESC_SET,
                        binding: PUSH_CONSTANTS_DESC_BINDING,
                    },
                    msl::ResourceBinding {
                        buffer_id,
                        texture_id: !0,
                        sampler_id: !0,
                        force_used: false,
                    },
                );
                assert!(counters.buffers < buffer_id as usize);
            } else {
                assert!(counters.buffers <= self.private_caps.max_buffers_per_stage);
            }
            // make sure we fit the limits
            assert!(counters.textures <= self.private_caps.max_textures_per_stage);
            assert!(counters.samplers <= self.private_caps.max_samplers_per_stage);
        }

        n::PipelineLayout {
            res_overrides,
            offsets,
            total: n::MultiStageResourceCounters {
                vs: stage_infos[0].2.clone(),
                ps: stage_infos[1].2.clone(),
                cs: stage_infos[2].2.clone(),
            },
        }
    }

    fn create_graphics_pipeline<'a>(
        &self,
        pipeline_desc: &pso::GraphicsPipelineDesc<'a, Backend>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        debug!("create_graphics_pipeline {:?}", pipeline_desc);
        let pipeline = metal::RenderPipelineDescriptor::new();
        let pipeline_layout = &pipeline_desc.layout;
        let pass_descriptor = &pipeline_desc.subpass;

        if pipeline_layout.attribute_buffer_index() as usize + pipeline_desc.vertex_buffers.len() > self.private_caps.max_buffers_per_stage {
            let msg = format!("Too many buffers inputs of the vertex stage: {} attributes + {} resources",
                pipeline_desc.vertex_buffers.len(), pipeline_layout.attribute_buffer_index());
            return Err(pso::CreationError::Shader(ShaderError::InterfaceMismatch(msg)));
        }
        // FIXME: lots missing

        let (primitive_class, primitive_type) = match pipeline_desc.input_assembler.primitive {
            hal::Primitive::PointList => (MTLPrimitiveTopologyClass::Point, MTLPrimitiveType::Point),
            hal::Primitive::LineList => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::Line),
            hal::Primitive::LineStrip => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::LineStrip),
            hal::Primitive::TriangleList => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::Triangle),
            hal::Primitive::TriangleStrip => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::TriangleStrip),
            _ => (MTLPrimitiveTopologyClass::Unspecified, MTLPrimitiveType::Point) //TODO: double-check
        };
        pipeline.set_input_primitive_topology(primitive_class);

        // Vertex shader
        let (vs_lib, vs_function, _) = self.load_shader(
            &pipeline_desc.shaders.vertex,
            pipeline_layout,
            primitive_class,
        )?;
        pipeline.set_vertex_function(Some(&vs_function));

        // Fragment shader
        let fs_function;
        let fs_lib = match pipeline_desc.shaders.fragment {
            Some(ref ep) => {
                let (lib, fun, _) = self.load_shader(ep, pipeline_layout, primitive_class)?;
                fs_function = fun;
                pipeline.set_fragment_function(Some(&fs_function));
                Some(lib)
            }
            None => {
                // TODO: This is a workaround for what appears to be a Metal validation bug
                // A pixel format is required even though no attachments are provided
                if pass_descriptor.main_pass.attachments.is_empty() {
                    pipeline.set_depth_attachment_pixel_format(metal::MTLPixelFormat::Depth32Float);
                }
                None
            },
        };

        // Other shaders
        if pipeline_desc.shaders.hull.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Hull)));
        }
        if pipeline_desc.shaders.domain.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Domain)));
        }
        if pipeline_desc.shaders.geometry.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Geometry)));
        }

        let device = self.shared.device.lock();

        // Copy color target info from Subpass
        for (i, attachment) in pass_descriptor.main_pass.attachments.iter().enumerate() {
            let format = attachment.format.expect("expected color format");
            let mtl_format = match self.private_caps.map_format(format) {
                Some(f) => f,
                None => {
                    error!("Unable to convert {:?} format", format);
                    return Err(pso::CreationError::Other);
                }
            };
            if format.is_color() {
                pipeline
                    .color_attachments()
                    .object_at(i)
                    .expect("too many color attachments")
                    .set_pixel_format(mtl_format);
            }
            if format.is_depth() {
                pipeline.set_depth_attachment_pixel_format(mtl_format);
            }
            if format.is_stencil() {
                pipeline.set_stencil_attachment_pixel_format(mtl_format);
            }
        }

        // Blending
        for (i, color_desc) in pipeline_desc.blender.targets.iter().enumerate() {
            let descriptor = pipeline
                .color_attachments()
                .object_at(i)
                .expect("too many color attachments");
            descriptor.set_write_mask(conv::map_write_mask(color_desc.0));

            if let pso::BlendState::On { ref color, ref alpha } = color_desc.1 {
                descriptor.set_blending_enabled(true);
                let (color_op, color_src, color_dst) = conv::map_blend_op(color);
                let (alpha_op, alpha_src, alpha_dst) = conv::map_blend_op(alpha);

                descriptor.set_rgb_blend_operation(color_op);
                descriptor.set_source_rgb_blend_factor(color_src);
                descriptor.set_destination_rgb_blend_factor(color_dst);

                descriptor.set_alpha_blend_operation(alpha_op);
                descriptor.set_source_alpha_blend_factor(alpha_src);
                descriptor.set_destination_alpha_blend_factor(alpha_dst);
            }
        }

        // Vertex buffers
        let vertex_descriptor = metal::VertexDescriptor::new();
        let mut vertex_buffer_map = n::VertexBufferMap::default();
        let mut next_buffer_index = pipeline_layout.attribute_buffer_index();
        trace!("Vertex attribute remapping started");

        for (i, &pso::AttributeDesc { binding, element, ..}) in pipeline_desc.attributes.iter().enumerate() {
            let original = pipeline_desc.vertex_buffers
                .iter()
                .find(|vb| vb.binding == binding)
                .expect("no associated vertex buffer found");
            // handle wrapping offsets
            let elem_size = element.format.surface_desc().bits as pso::ElemOffset / 8;
            let (cut_offset, base_offset) = if original.stride == 0 || element.offset + elem_size <= original.stride {
                (element.offset, 0)
            } else {
                let remainder = element.offset % original.stride;
                if remainder + elem_size <= original.stride {
                    (remainder, element.offset - remainder)
                } else {
                    (0, element.offset)
                }
            };
            let mtl_buffer_index = match vertex_buffer_map.entry((binding, base_offset)) {
                Entry::Vacant(_) if next_buffer_index == self.shared.push_constants_buffer_id => {
                    error!("Attribute offset {} exceeds the stride {}, and there is no room for replacement.",
                        element.offset, original.stride);
                    return Err(pso::CreationError::Other);
                }
                Entry::Vacant(e) => {
                    e.insert(pso::VertexBufferDesc {
                        binding: next_buffer_index,
                        stride: original.stride,
                        rate: original.rate,
                    });
                    next_buffer_index += 1;
                    next_buffer_index - 1
                }
                Entry::Occupied(e) => e.get().binding,
            };
            trace!("\tAttribute[{}] is mapped to vertex buffer[{}] with binding {} and offsets {} + {}",
                i, binding, mtl_buffer_index, base_offset, cut_offset);
            // pass the refined data to Metal
            let mtl_attribute_desc = vertex_descriptor
                .attributes()
                .object_at(i)
                .expect("too many vertex attributes");
            let mtl_vertex_format = conv::map_vertex_format(element.format)
                .expect("unsupported vertex format");
            mtl_attribute_desc.set_format(mtl_vertex_format);
            mtl_attribute_desc.set_buffer_index(mtl_buffer_index as _);
            mtl_attribute_desc.set_offset(cut_offset as _);
        }

        const STRIDE_GRANULARITY: pso::ElemStride = 4; //TODO: work around?
        for vb in vertex_buffer_map.values() {
            let mtl_buffer_desc = vertex_descriptor
                .layouts()
                .object_at(vb.binding as usize)
                .expect("too many vertex descriptor layouts");
            if vb.stride % STRIDE_GRANULARITY != 0 {
                error!("Stride ({}) must be a multiple of {}", vb.stride, STRIDE_GRANULARITY);
                return Err(pso::CreationError::Other);
            }
            if vb.stride != 0 {
                mtl_buffer_desc.set_stride(vb.stride as u64);
                if vb.rate == 0 {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerVertex);
                } else {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                    mtl_buffer_desc.set_step_rate(vb.rate as u64);
                }
            } else {
                mtl_buffer_desc.set_stride(256); // big enough to fit all the elements
                mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                mtl_buffer_desc.set_step_rate(!0);
            }
        }
        if !vertex_buffer_map.is_empty() {
            pipeline.set_vertex_descriptor(Some(&vertex_descriptor));
        }

        if let pso::PolygonMode::Line(width) = pipeline_desc.rasterizer.polygon_mode {
            validate_line_width(width);
        }

        let rasterizer_state = Some(n::RasterizerState {
            front_winding: conv::map_winding(pipeline_desc.rasterizer.front_face),
            cull_mode: match conv::map_cull_face(pipeline_desc.rasterizer.cull_face) {
                Some(mode) => mode,
                None => {
                    //TODO - Metal validation fails with
                    // RasterizationEnabled is false but the vertex shader's return type is not void
                    error!("Culling both sides is not yet supported");
                    //pipeline.set_rasterization_enabled(false);
                    metal::MTLCullMode::None
                }
            },
            depth_clip: if pipeline_desc.rasterizer.depth_clamping {
                metal::MTLDepthClipMode::Clamp
            } else {
                metal::MTLDepthClipMode::Clip
            },
        });
        let depth_bias = pipeline_desc.rasterizer.depth_bias
            .unwrap_or(pso::State::Static(pso::DepthBias::default()));

        // prepare the depth-stencil state now
        self.shared.service_pipes
            .depth_stencil_states
            .prepare(&pipeline_desc.depth_stencil, &*device);

        let attachment_formats = pass_descriptor.main_pass.attachments
            .iter()
            .map(|at| at.format)
            .collect();

        device.new_render_pipeline_state(&pipeline)
            .map(|raw|
                n::GraphicsPipeline {
                    vs_lib,
                    fs_lib,
                    raw,
                    primitive_type,
                    attribute_buffer_index: pipeline_layout.attribute_buffer_index(),
                    rasterizer_state,
                    depth_bias,
                    depth_stencil_desc: pipeline_desc.depth_stencil.clone(),
                    baked_states: pipeline_desc.baked_states.clone(),
                    vertex_buffer_map,
                    attachment_formats,
                })
            .map_err(|err| {
                error!("PSO creation failed: {}", err);
                pso::CreationError::Other
            })
    }

    fn create_compute_pipeline<'a>(
        &self,
        pipeline_desc: &pso::ComputePipelineDesc<'a, Backend>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        debug!("create_compute_pipeline {:?}", pipeline_desc);
        let pipeline = metal::ComputePipelineDescriptor::new();

        let (cs_lib, cs_function, work_group_size) = self.load_shader(
            &pipeline_desc.shader,
            &pipeline_desc.layout,
            MTLPrimitiveTopologyClass::Unspecified,
        )?;
        pipeline.set_compute_function(Some(&cs_function));

        self.shared.device
            .lock()
            .new_compute_pipeline_state(&pipeline)
            .map(|raw| {
                n::ComputePipeline {
                    cs_lib,
                    raw,
                    work_group_size,
                }
            })
            .map_err(|err| {
                error!("PSO creation failed: {}", err);
                pso::CreationError::Other
            })
    }

    fn create_framebuffer<I>(
        &self, renderpass: &n::RenderPass, attachments: I, extent: image::Extent
    ) -> Result<n::Framebuffer, FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>
    {
        let descriptor = metal::RenderPassDescriptor::new().to_owned();
        descriptor.set_render_target_array_length(extent.depth as NSUInteger);

        let mut inner = n::FramebufferInner {
            extent,
            aspects: format::Aspects::empty(),
            colors: SmallVec::new(),
            depth_stencil: None,
        };

        autoreleasepool(|| { // for the attachments
            for (rat, attachment) in renderpass.attachments.iter().zip(attachments) {
                let format = match rat.format {
                    Some(format) => format,
                    None => continue,
                };
                let aspects = format.surface_desc().aspects;
                inner.aspects |= aspects;

                let at = attachment.borrow();
                if aspects.contains(format::Aspects::COLOR) {
                    descriptor
                        .color_attachments()
                        .object_at(inner.colors.len())
                        .expect("too many color attachments")
                        .set_texture(Some(&at.raw));
                    inner.colors.push(native::ColorAttachment {
                        mtl_format: at.mtl_format,
                        channel: format.base_format().1.into(),
                    });
                }
                if aspects.contains(format::Aspects::DEPTH) {
                    assert_eq!(inner.depth_stencil, None);
                    inner.depth_stencil = Some(at.mtl_format);
                    descriptor
                        .depth_attachment()
                        .unwrap()
                        .set_texture(Some(&at.raw));
                }
                if aspects.contains(format::Aspects::STENCIL) {
                    if let Some(old_format) = inner.depth_stencil {
                        assert_eq!(old_format, at.mtl_format);
                    } else {
                        inner.depth_stencil = Some(at.mtl_format);
                    }
                    descriptor
                        .stencil_attachment()
                        .unwrap()
                        .set_texture(Some(&at.raw));
                }
            }
        });

        Ok(n::Framebuffer {
            descriptor,
            desc_storage: FastStorageMap::default(),
            inner,
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        //TODO: we can probably at least parse here and save the `Ast`
        let depends_on_pipeline_layout = true; //TODO: !self.private_caps.argument_buffers
        Ok(if depends_on_pipeline_layout {
            n::ShaderModule::Raw(raw_data.to_vec())
        } else {
            let (library, entry_point_map) = self.compile_shader_library(
                raw_data,
                MTLPrimitiveTopologyClass::Unspecified,
                &n::ResourceOverrideMap::default(),
            )?;
            n::ShaderModule::Compiled {
                library,
                entry_point_map,
            }
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let descriptor = metal::SamplerDescriptor::new();

        descriptor.set_min_filter(conv::map_filter(info.min_filter));
        descriptor.set_mag_filter(conv::map_filter(info.mag_filter));
        descriptor.set_mip_filter(match info.mip_filter {
            // Note: this shouldn't be required, but Metal appears to be confused when mipmaps
            // are provided even with trivial LOD bias.
            image::Filter::Nearest if info.lod_range.end < image::Lod::from(0.5) => MTLSamplerMipFilter::NotMipmapped,
            image::Filter::Nearest => MTLSamplerMipFilter::Nearest,
            image::Filter::Linear => MTLSamplerMipFilter::Linear,
        });

        if let image::Anisotropic::On(aniso) = info.anisotropic {
            descriptor.set_max_anisotropy(aniso as _);
        }

        let (s, t, r) = info.wrap_mode;
        descriptor.set_address_mode_s(conv::map_wrap_mode(s));
        descriptor.set_address_mode_t(conv::map_wrap_mode(t));
        descriptor.set_address_mode_r(conv::map_wrap_mode(r));

        descriptor.set_lod_bias(info.lod_bias.into());
        descriptor.set_lod_min_clamp(info.lod_range.start.into());
        descriptor.set_lod_max_clamp(info.lod_range.end.into());
        descriptor.set_lod_average(true); // optimization

        if let Some(fun) = info.comparison {
            descriptor.set_compare_function(conv::map_compare_function(fun));
        }
        if [r, s, t].iter().any(|&am| am == image::WrapMode::Border) {
            descriptor.set_border_color(match info.border.0 {
                0x00000000 => MTLSamplerBorderColor::TransparentBlack,
                0x000000FF => MTLSamplerBorderColor::OpaqueBlack,
                0xFFFFFFFF => MTLSamplerBorderColor::OpaqueWhite,
                other => {
                    error!("Border color 0x{:X} is not supported", other);
                    MTLSamplerBorderColor::TransparentBlack
                }
            });
        }

        n::Sampler(
            self.shared.device
            .lock()
            .new_sampler(&descriptor)
        )
    }

    fn destroy_sampler(&self, _sampler: n::Sampler) {
    }

    fn map_memory<R: RangeArg<u64>>(
        &self, memory: &n::Memory, generic_range: R
    ) -> Result<*mut u8, mapping::Error> {
        let range = memory.resolve(&generic_range);
        debug!("map_memory of size {} at {:?}", memory.size, range);

        let base_ptr = match memory.heap {
            n::MemoryHeap::Public(_, ref cpu_buffer) => cpu_buffer.contents() as *mut u8,
            n::MemoryHeap::Native(_) |
            n::MemoryHeap::Private => panic!("Unable to map memory!"),
        };
        Ok(unsafe { base_ptr.offset(range.start as _) })
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        debug!("unmap_memory of size {}", memory.size);
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        debug!("flush_mapped_memory_ranges");
        for item in iter {
            let (memory, ref generic_range) = *item.borrow();
            let range = memory.resolve(generic_range);
            debug!("\trange {:?}", range);

            match memory.heap {
                n::MemoryHeap::Native(_) => unimplemented!(),
                n::MemoryHeap::Public(mt, ref cpu_buffer) if 1<<mt.0 != MemoryTypes::SHARED.bits() as usize => {
                    cpu_buffer.did_modify_range(NSRange {
                        location: range.start as _,
                        length: (range.end - range.start) as _,
                    });
                }
                n::MemoryHeap::Public(..) => continue,
                n::MemoryHeap::Private => panic!("Can't map private memory!"),
            };
        }
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let mut num_syncs = 0;
        debug!("invalidate_mapped_memory_ranges");

        // temporary command buffer to copy the contents from
        // the given buffers into the allocated CPU-visible buffers
        let cmd_queue = self.shared.queue.lock();
        let cmd_buffer = cmd_queue.spawn_temp();
        autoreleasepool(|| {
            let encoder = cmd_buffer.new_blit_command_encoder();

            for item in iter {
                let (memory, ref generic_range) = *item.borrow();
                let range = memory.resolve(generic_range);
                debug!("\trange {:?}", range);

                match memory.heap {
                    n::MemoryHeap::Native(_) => unimplemented!(),
                    n::MemoryHeap::Public(mt, ref cpu_buffer) if 1<<mt.0 != MemoryTypes::SHARED.bits() as usize => {
                        num_syncs += 1;
                        encoder.synchronize_resource(cpu_buffer);
                    }
                    n::MemoryHeap::Public(..) => continue,
                    n::MemoryHeap::Private => panic!("Can't map private memory!"),
                };
            }
            encoder.end_encoding();
        });

        if num_syncs != 0 {
            debug!("\twaiting...");
            cmd_buffer.set_label("invalidate_mapped_memory_ranges");
            cmd_buffer.commit();
            cmd_buffer.wait_until_completed();
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        n::Semaphore {
            // Semaphore synchronization between command buffers of the same queue
            // is useless, don't bother even creating one.
            system: if self.private_caps.exposed_queues > 1 {
                Some(n::SystemSemaphore::new())
            } else {
                None
            },
            image_ready: Arc::new(Mutex::new(None)),
        }
    }

    fn create_descriptor_pool<I>(&self, _max_sets: usize, descriptor_ranges: I) -> n::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        let (mut num_samplers, mut num_textures, mut num_buffers) = (0, 0, 0);

        if self.private_caps.argument_buffers {
            let mut arguments = Vec::new();
            for desc_range in descriptor_ranges {
                let desc = desc_range.borrow();
                let offset_ref = match desc.ty {
                    pso::DescriptorType::Sampler => &mut num_samplers,
                    pso::DescriptorType::SampledImage => &mut num_textures,
                    pso::DescriptorType::UniformBuffer | pso::DescriptorType::StorageBuffer => &mut num_buffers,
                    _ => unimplemented!()
                };
                let index = *offset_ref;
                *offset_ref += desc.count;
                let arg_desc = Self::describe_argument(desc.ty, index as _, desc.count);
                arguments.push(arg_desc);
            }

            let device = self.shared.device.lock();
            let arg_array = metal::Array::from_owned_slice(&arguments);
            let encoder = device.new_argument_encoder(&arg_array);

            let total_size = encoder.encoded_length();
            let raw = device.new_buffer(total_size, MTLResourceOptions::empty());

            n::DescriptorPool::ArgumentBuffer {
                raw,
                range_allocator: RangeAllocator::new(0..total_size),
            }
        } else {
            for desc_range in descriptor_ranges {
                let desc = desc_range.borrow();
                let content = n::DescriptorContent::from(desc.ty);
                if content.contains(n::DescriptorContent::BUFFER) {
                    num_buffers += desc.count;
                }
                if content.contains(n::DescriptorContent::TEXTURE) {
                    num_textures += desc.count;
                }
                if content.contains(n::DescriptorContent::SAMPLER) {
                    num_samplers += desc.count;
                }
            }
            n::DescriptorPool::new_emulated(num_samplers, num_textures, num_buffers)
        }
    }

    fn create_descriptor_set_layout<I, J>(
        &self, binding_iter: I, immutable_sampler_iter: J
    ) -> n::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<n::Sampler>,
    {
        if self.private_caps.argument_buffers {
            let mut stage_flags = pso::ShaderStageFlags::empty();
            let arguments = binding_iter
                .into_iter()
                .map(|desc| {
                    let desc = desc.borrow();
                    stage_flags |= desc.stage_flags;
                    Self::describe_argument(desc.ty, desc.binding, desc.count)
                })
                .collect::<Vec<_>>();
            let arg_array = metal::Array::from_owned_slice(&arguments);
            let encoder = self.shared.device
                .lock()
                .new_argument_encoder(&arg_array);

            n::DescriptorSetLayout::ArgumentBuffer(encoder, stage_flags)
        } else {
            let mut desc_layouts = Vec::new();
            let mut dynamic_offset_count = 0;
            let mut immutable_sampler_count = 0;

            for set_layout_binding in binding_iter {
                let slb = set_layout_binding.borrow();
                let mut content = native::DescriptorContent::from(slb.ty);
                if slb.immutable_samplers {
                    content |= native::DescriptorContent::IMMUTABLE_SAMPLER;
                }
                for array_index in 0 .. slb.count {
                    desc_layouts.push(native::DescriptorLayout {
                        content,
                        associated_data_index: if slb.immutable_samplers {
                            immutable_sampler_count += 1;
                            immutable_sampler_count - 1
                        } else if content.contains(native::DescriptorContent::DYNAMIC_BUFFER) {
                            dynamic_offset_count += 1;
                            dynamic_offset_count - 1
                        } else {
                            !0
                        },
                        stages: slb.stage_flags,
                        binding: slb.binding,
                        array_index,
                    });
                }
            }

            desc_layouts.sort_by_key(|dl| (dl.binding, dl.array_index));
            let samplers = immutable_sampler_iter
                .into_iter()
                .map(|s| s.borrow().0.clone())
                .collect::<Vec<_>>();
            assert_eq!(samplers.len(), immutable_sampler_count as usize);

            n::DescriptorSetLayout::Emulated(Arc::new(desc_layouts), samplers)
        }
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        debug!("write_descriptor_sets");
        for write in write_iter {
            match *write.set {
                n::DescriptorSet::Emulated { ref pool, ref layouts, ref sampler_range, ref texture_range, ref buffer_range } => {
                    let mut counters = n::ResourceCounters {
                        buffers: buffer_range.start as usize,
                        textures: texture_range.start as usize,
                        samplers: sampler_range.start as usize,
                    };
                    let mut start = None; //TODO: can pre-compute this
                    for (i, layout) in layouts.iter().enumerate() {
                        if layout.binding == write.binding && layout.array_index == write.array_offset {
                            start = Some(i);
                            break;
                        }
                        counters.add(layout.content);
                    }
                    let mut data = pool.write();

                    for (layout, descriptor) in layouts[start.unwrap() ..].iter().zip(write.descriptors) {
                        trace!("\t{:?} at {:?}", layout, counters);
                        match *descriptor.borrow() {
                            pso::Descriptor::Sampler(sampler) => {
                                debug_assert!(!layout.content.contains(n::DescriptorContent::IMMUTABLE_SAMPLER));
                                data.samplers[counters.samplers] = Some(SamplerPtr(sampler.0.as_ptr()));
                            }
                            pso::Descriptor::Image(image, il) => {
                                data.textures[counters.textures] = Some((TexturePtr(image.raw.as_ptr()), il));
                            }
                            pso::Descriptor::CombinedImageSampler(image, il, sampler) => {
                                if !layout.content.contains(n::DescriptorContent::IMMUTABLE_SAMPLER) {
                                    data.samplers[counters.samplers] = Some(SamplerPtr(sampler.0.as_ptr()));
                                }
                                data.textures[counters.textures] = Some((TexturePtr(image.raw.as_ptr()), il));
                            }
                            pso::Descriptor::UniformTexelBuffer(view) |
                            pso::Descriptor::StorageTexelBuffer(view) => {
                                data.textures[counters.textures] = Some((TexturePtr(view.raw.as_ptr()), image::Layout::General));
                            }
                            pso::Descriptor::Buffer(buffer, ref range) => {
                                let buf_length = buffer.raw.length();
                                let start = range.start.unwrap_or(0);
                                let end = range.end.unwrap_or(buf_length);
                                assert!(end <= buf_length);
                                data.buffers[counters.buffers] = Some((BufferPtr(buffer.raw.as_ptr()), start));
                            }
                        }
                        counters.add(layout.content);
                    }
                }
                n::DescriptorSet::ArgumentBuffer { ref raw, offset, ref encoder, .. } => {
                    debug_assert!(self.private_caps.argument_buffers);

                    encoder.set_argument_buffer(raw, offset);
                    //TODO: range checks, need to keep some layout metadata around
                    assert_eq!(write.array_offset, 0); //TODO

                    for descriptor in write.descriptors {
                        match *descriptor.borrow() {
                            pso::Descriptor::Sampler(sampler) => {
                                encoder.set_sampler_states(&[&sampler.0], write.binding as _);
                            }
                            pso::Descriptor::Image(image, _layout) => {
                                encoder.set_textures(&[&image.raw], write.binding as _);
                            }
                            pso::Descriptor::Buffer(buffer, ref range) => {
                                encoder.set_buffer(&buffer.raw, range.start.unwrap_or(0), write.binding as _);
                            }
                            pso::Descriptor::CombinedImageSampler(..) |
                            pso::Descriptor::UniformTexelBuffer(..) |
                            pso::Descriptor::StorageTexelBuffer(..) => unimplemented!(),
                        }
                    }
                }
            }
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copies: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        for _copy in copies {
            unimplemented!()
        }
    }

    fn destroy_descriptor_pool(&self, _pool: n::DescriptorPool) {
    }

    fn destroy_descriptor_set_layout(&self, _layout: n::DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&self, _pipeline_layout: n::PipelineLayout) {
    }

    fn destroy_shader_module(&self, _module: n::ShaderModule) {
    }

    fn destroy_render_pass(&self, _pass: n::RenderPass) {
    }

    fn destroy_graphics_pipeline(&self, _pipeline: n::GraphicsPipeline) {
    }

    fn destroy_compute_pipeline(&self, _pipeline: n::ComputePipeline) {
    }

    fn destroy_framebuffer(&self, _buffer: n::Framebuffer) {
    }

    fn destroy_semaphore(&self, _semaphore: n::Semaphore) {
    }

    fn allocate_memory(&self, memory_type: hal::MemoryTypeId, size: u64) -> Result<n::Memory, OutOfMemory> {
        let (storage, cache) = MemoryTypes::describe(memory_type.0);
        let device = self.shared.device.lock();
        debug!("allocate_memory type {:?} of size {}", memory_type, size);

        // Heaps cannot be used for CPU coherent resources
        //TEMP: MacOS supports Private only, iOS and tvOS can do private/shared
        let heap = if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared && false {
            let descriptor = metal::HeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            let heap_raw = device.new_heap(&descriptor);
            n::MemoryHeap::Native(heap_raw)
        } else if storage == MTLStorageMode::Private {
            n::MemoryHeap::Private
        } else {
            let options = conv::resource_options_from_storage_and_cache(storage, cache);
            let cpu_buffer = device.new_buffer(size, options);
            debug!("\tbacked by cpu buffer {:?}", cpu_buffer.as_ptr());
            n::MemoryHeap::Public(memory_type, cpu_buffer)
        };

        Ok(n::Memory::new(heap, size))
    }

    fn free_memory(&self, memory: n::Memory) {
        debug!("free_memory of size {}", memory.size);
        if let n::MemoryHeap::Public(_, ref cpu_buffer) = memory.heap {
            debug!("\tbacked by cpu buffer {:?}", cpu_buffer.as_ptr());
        }
    }

    fn create_buffer(
        &self, size: u64, usage: buffer::Usage
    ) -> Result<n::UnboundBuffer, buffer::CreationError> {
        debug!("create_buffer of size {} and usage {:?}", size, usage);
        Ok(n::UnboundBuffer {
            size,
            usage,
        })
    }

    fn get_buffer_requirements(&self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        let mut max_size = buffer.size;
        let mut max_alignment = self.private_caps.buffer_alignment;

        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the buffer with, so we test them
            // all get the most stringent ones.
            for (i, _mt) in self.memory_types.iter().enumerate() {
                let (storage, cache) = MemoryTypes::describe(i);
                let options = conv::resource_options_from_storage_and_cache(storage, cache);
                let requirements = self.shared.device
                    .lock()
                    .heap_buffer_size_and_align(buffer.size, options);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
        }

        // based on Metal validation error for view creation:
        // failed assertion `BytesPerRow of a buffer-backed texture with pixelFormat(XXX) must be aligned to 256 bytes
        const SIZE_MASK: u64 = 0xFF;
        let supports_texel_view = buffer.usage.intersects(
            buffer::Usage::UNIFORM_TEXEL |
            buffer::Usage::STORAGE_TEXEL
        );

        memory::Requirements {
            size: (max_size + SIZE_MASK) & !SIZE_MASK,
            alignment: max_alignment,
            type_mask: if !supports_texel_view || self.private_caps.shared_textures {
                MemoryTypes::all().bits()
            } else {
                (MemoryTypes::all() ^ MemoryTypes::SHARED).bits()
            },
        }
    }

    fn bind_buffer_memory(
        &self, memory: &n::Memory, offset: u64, buffer: n::UnboundBuffer
    ) -> Result<n::Buffer, BindError> {
        debug!("bind_buffer_memory of size {} at offset {}", buffer.size, offset);
        let (raw, res_options, range) = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = conv::resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode(),
                );
                let raw = heap.new_buffer(buffer.size, resource_options)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.shared.device
                            .lock()
                            .new_buffer(buffer.size, resource_options)
                    });
                (raw, resource_options, 0 .. buffer.size) //TODO?
            }
            n::MemoryHeap::Public(mt, ref cpu_buffer) => {
                debug!("\tmapped to public heap with address {:?}", cpu_buffer.as_ptr());
                let (storage, cache) = MemoryTypes::describe(mt.0);
                let options = conv::resource_options_from_storage_and_cache(storage, cache);
                (cpu_buffer.clone(), options, offset .. offset + buffer.size)
            }
            n::MemoryHeap::Private => {
                //TODO: check for aliasing
                let options = MTLResourceOptions::StorageModePrivate |
                    MTLResourceOptions::CPUCacheModeDefaultCache;
                let raw = self.shared.device
                    .lock()
                    .new_buffer(buffer.size, options);
                (raw, options, 0 .. buffer.size)
            }
        };

        Ok(n::Buffer {
            raw,
            range,
            res_options,
        })
    }

    fn destroy_buffer(&self, buffer: n::Buffer) {
        debug!("destroy_buffer {:?} occupying memory {:?}", buffer.raw.as_ptr(), buffer.range);
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self, buffer: &n::Buffer, format_maybe: Option<format::Format>, range: R
    ) -> Result<n::BufferView, buffer::ViewCreationError> {
        let start = buffer.range.start + *range.start().unwrap_or(&0);
        let end_rough = *range.end().unwrap_or(&buffer.raw.length());
        let format = match format_maybe {
            Some(fmt) => fmt,
            None => return Err(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe }),
        };
        let format_desc = format.surface_desc();
        if format_desc.aspects != format::Aspects::COLOR {
            // no depth/stencil support for buffer views here
            return Err(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe })
        }
        let block_count = (end_rough - start) * 8 / format_desc.bits as u64;
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe })?;

        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_width(format_desc.dim.0 as u64 * block_count);
        descriptor.set_height(format_desc.dim.1 as u64);
        descriptor.set_mipmap_level_count(1);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_resource_options(buffer.res_options);
        descriptor.set_storage_mode(buffer.raw.storage_mode());

        let size = block_count * (format_desc.bits as u64 / 8);
        let stride = (size + STRIDE_MASK) & !STRIDE_MASK;

        Ok(n::BufferView {
            raw: buffer.raw.new_texture_from_contents(&descriptor, start, stride),
        })
    }

    fn destroy_buffer_view(&self, _view: n::BufferView) {
        //nothing to do
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        flags: image::StorageFlags,
    ) -> Result<n::UnboundImage, image::CreationError> {
        debug!("create_image {:?} with {} mips of {:?} {:?} and usage {:?}",
            kind, mip_levels, format, tiling, usage);

        let is_cube = flags.contains(image::StorageFlags::CUBE_VIEW);
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(image::CreationError::Format(format))?;

        let descriptor = metal::TextureDescriptor::new();

        let (mtl_type, num_layers) = match kind {
            image::Kind::D1(_, 1) => {
                assert!(!is_cube);
                (MTLTextureType::D1, None)
            }
            image::Kind::D1(_, layers) => {
                assert!(!is_cube);
                (MTLTextureType::D1Array, Some(layers))
            }
            image::Kind::D2(_, _, layers, 1) => {
                if is_cube && layers > 6 {
                    assert_eq!(layers % 6, 0);
                    (MTLTextureType::CubeArray, Some(layers / 6))
                } else if is_cube {
                    assert_eq!(layers, 6);
                    (MTLTextureType::Cube, None)
                } else if layers > 1 {
                    (MTLTextureType::D2Array, Some(layers))
                } else {
                    (MTLTextureType::D2, None)
                }
            }
            image::Kind::D2(_, _, 1, samples) if !is_cube => {
                descriptor.set_sample_count(samples as u64);
                (MTLTextureType::D2Multisample, None)
            }
            image::Kind::D2(..) => {
                error!("Multi-sampled array textures or cubes are not supported: {:?}", kind);
                return Err(image::CreationError::Kind)
            }
            image::Kind::D3(..) => {
                assert!(!is_cube);
                (MTLTextureType::D3, None)
            }
        };

        descriptor.set_texture_type(mtl_type);
        if let Some(count) = num_layers {
            descriptor.set_array_length(count as u64);
        }
        let extent = kind.extent();
        descriptor.set_width(extent.width as u64);
        descriptor.set_height(extent.height as u64);
        descriptor.set_depth(extent.depth as u64);
        descriptor.set_mipmap_level_count(mip_levels as u64);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_usage(conv::map_texture_usage(usage, tiling));

        let format_desc = format.surface_desc();
        let mip_sizes = (0 .. mip_levels)
            .map(|level| {
                let pitches = n::Image::pitches_impl(extent.at_level(level), format_desc);
                num_layers.unwrap_or(1) as buffer::Offset * pitches[2]
            })
            .collect();

        let host_usage = image::Usage::TRANSFER_SRC | image::Usage::TRANSFER_DST;
        let host_visible = mtl_type == MTLTextureType::D2 &&
            mip_levels == 1 && num_layers.is_none() &&
            format_desc.aspects.contains(format::Aspects::COLOR) &&
            tiling == image::Tiling::Linear &&
            host_usage.contains(usage);

        Ok(n::UnboundImage {
            texture_desc: descriptor,
            format,
            kind,
            mip_sizes,
            host_visible,
        })
    }

    fn get_image_requirements(&self, image: &n::UnboundImage) -> memory::Requirements {
        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the image with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            let mut max_size = 0;
            let mut max_alignment = 0;
            let types = if image.host_visible {
                MemoryTypes::all()
            } else {
                MemoryTypes::PRIVATE
            };
            for (i, _) in self.memory_types.iter().enumerate() {
                if !types.contains(MemoryTypes::from_bits(1 << i).unwrap()) {
                    continue
                }
                let (storage, cache_mode) = MemoryTypes::describe(i);
                image.texture_desc.set_storage_mode(storage);
                image.texture_desc.set_cpu_cache_mode(cache_mode);

                let requirements = self.shared.device
                    .lock()
                    .heap_texture_size_and_align(&image.texture_desc);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
            memory::Requirements {
                size: max_size,
                alignment: max_alignment,
                type_mask: types.bits(),
            }
        } else if image.host_visible {
            assert_eq!(image.mip_sizes.len(), 1);
            let mask = self.private_caps.buffer_alignment - 1;
            memory::Requirements {
                size: (image.mip_sizes[0] + mask) & !mask,
                alignment: self.private_caps.buffer_alignment,
                type_mask: if self.private_caps.shared_textures {
                    MemoryTypes::all().bits()
                } else {
                    (MemoryTypes::all() ^ MemoryTypes::SHARED).bits()
                },
            }
        } else {
            memory::Requirements {
                size: image.mip_sizes.iter().sum(),
                alignment: 4,
                type_mask: MemoryTypes::PRIVATE.bits(),
            }
        }
    }

    fn get_image_subresource_footprint(
        &self, image: &n::Image, sub: image::Subresource
    ) -> image::SubresourceFootprint {
        let num_layers = image.kind.num_layers() as buffer::Offset;
        let level_offset = (0 .. sub.level).fold(0, |offset, level| {
            let pitches = image.pitches(level);
            offset + num_layers * pitches[2]
        });
        let pitches = image.pitches(sub.level);
        let layer_offset = level_offset + sub.layer as buffer::Offset * pitches[2];
        image::SubresourceFootprint {
            slice: layer_offset .. layer_offset + pitches[2],
            row_pitch: pitches[0] as _,
            depth_pitch: pitches[1] as _,
            array_pitch: pitches[2] as _,
        }
    }

    fn bind_image_memory(
        &self, memory: &n::Memory, offset: u64, image: n::UnboundImage
    ) -> Result<n::Image, BindError> {
        let base = image.format.base_format();
        let format_desc = base.0.desc();

        let raw = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = conv::resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.texture_desc.set_resource_options(resource_options);
                heap.new_texture(&image.texture_desc)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.shared.device
                            .lock()
                            .new_texture(&image.texture_desc)
                    })
            },
            n::MemoryHeap::Public(memory_type, ref cpu_buffer) => {
                let row_size = image.kind.extent().width as u64 * (format_desc.bits as u64 / 8);
                let stride = (row_size + STRIDE_MASK) & !STRIDE_MASK;

                let (storage_mode, cache_mode) = MemoryTypes::describe(memory_type.0);
                image.texture_desc.set_storage_mode(storage_mode);
                image.texture_desc.set_cpu_cache_mode(cache_mode);

                cpu_buffer.new_texture_from_contents(&image.texture_desc, offset, stride)
            }
            n::MemoryHeap::Private => {
                image.texture_desc.set_storage_mode(MTLStorageMode::Private);
                self.shared.device
                    .lock()
                    .new_texture(&image.texture_desc)
            }
        };

        Ok(n::Image {
            raw,
            kind: image.kind,
            format_desc,
            shader_channel: base.1.into(),
            mtl_format: match self.private_caps.map_format(image.format) {
                Some(format) => format,
                None => {
                    error!("failed to find corresponding Metal format for {:?}", image.format);
                    return Err(BindError::OutOfBounds);
                },
            },
            mtl_type: image.texture_desc.texture_type(),
        })
    }

    fn destroy_image(&self, _image: n::Image) {
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        let mtl_format = match self.private_caps.map_format_with_swizzle(format, swizzle) {
            Some(f) => f,
            None => {
                error!("failed to swizzle format {:?} with {:?}", format, swizzle);
                return Err(image::ViewError::BadFormat);
            },
        };

        let full_range = image::SubresourceRange {
            aspects: image.format_desc.aspects,
            levels: 0 .. image.raw.mipmap_level_count() as image::Level,
            layers: 0 .. image.kind.num_layers(),
        };
        let view = if
            mtl_format == image.mtl_format &&
            //kind == image::ViewKind::D2 && //TODO: find a better way to check this
            swizzle == format::Swizzle::NO &&
            range == full_range &&
            match (kind, image.kind) {
                (image::ViewKind::D1, image::Kind::D1(..)) |
                (image::ViewKind::D2, image::Kind::D2(..)) |
                (image::ViewKind::D3, image::Kind::D3(..)) => true,
                (image::ViewKind::D1Array, image::Kind::D1(_, layers)) if layers > 1 => true,
                (image::ViewKind::D2Array, image::Kind::D2(_, _, layers, _)) if layers > 1 => true,
                (_, _) => false, //TODO: expose more choices here?
            }
        {
            // Some images are marked as framebuffer-only, and we can't create aliases of them.
            // Also helps working around Metal bugs with aliased array textures.
            image.raw.clone()
        } else {
            image.raw.new_texture_view_from_slice(
                mtl_format,
                conv::map_texture_type(kind),
                NSRange {
                    location: range.levels.start as _,
                    length: (range.levels.end - range.levels.start) as _,
                },
                NSRange {
                    location: range.layers.start as _,
                    length: (range.layers.end - range.layers.start) as _,
                },
            )
        };

        Ok(n::ImageView { raw: view, mtl_format })
    }

    fn destroy_image_view(&self, _view: n::ImageView) {
    }

    fn create_fence(&self, signaled: bool) -> n::Fence {
        n::Fence(RefCell::new(n::FenceInner::Idle { signaled }))
    }
    fn reset_fence(&self, fence: &n::Fence) {
        *fence.0.borrow_mut() = n::FenceInner::Idle { signaled: false };
    }
    fn wait_for_fence(&self, fence: &n::Fence, timeout_ns: u64) -> bool {
        fn to_ns(duration: time::Duration) -> u64 {
            duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
        }

        debug!("wait_for_fence {:?} for {} ms", fence, timeout_ns);
        let inner = fence.0.borrow();
        let cmd_buf = match *inner {
            native::FenceInner::Idle { signaled } => return signaled,
            native::FenceInner::Pending(ref cmd_buf) => cmd_buf,
        };
        if timeout_ns == !0 {
            cmd_buf.wait_until_completed();
            return true
        }

        let start = time::Instant::now();
        loop {
            if let metal::MTLCommandBufferStatus::Completed = cmd_buf.status() {
                return true
            }
            if to_ns(start.elapsed()) >= timeout_ns {
                return false;
            }
            thread::sleep(time::Duration::from_millis(1));
        }
    }
    fn get_fence_status(&self, fence: &n::Fence) -> bool {
        match *fence.0.borrow() {
            native::FenceInner::Idle { signaled } => signaled,
            native::FenceInner::Pending(ref cmd_buf) => match cmd_buf.status() {
                metal::MTLCommandBufferStatus::Completed => true,
                _ => false,
            },
        }
    }
    fn destroy_fence(&self, _fence: n::Fence) {
    }

    fn create_query_pool(&self, _ty: query::QueryType, _count: u32) -> () {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
        old_swapchain: Option<Swapchain>,
        _extent: &window::Extent2D,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        if let Some(_swapchain) = old_swapchain {
            //swapchain is dropped here
        }
        self.build_swapchain(surface, config)
    }

    fn destroy_swapchain(&self, _swapchain: Swapchain) {
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        command::QueueInner::wait_idle(&self.shared.queue);
        Ok(())
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send+Sync>() {}
    foo::<Device>()
}
