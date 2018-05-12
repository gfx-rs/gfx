use {AutoreleasePool, Backend, PrivateCapabilities, QueueFamily, Surface, Swapchain};
use {native as n, command, soft};
use conversions::*;
use internal::Channel;

use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{cmp, mem, slice};

use hal::{self, error, image, pass, format, mapping, memory, buffer, pso, query};
use hal::command::BufferCopy;
use hal::device::{BindError, OutOfMemory, FramebufferError, ShaderError};
use hal::memory::Properties;
use hal::pool::CommandPoolCreateFlags;
use hal::pso::{DescriptorType, DescriptorSetLayoutBinding, AttributeDesc, DepthTest, StencilTest};
use hal::queue::{QueueFamily as HalQueueFamily, QueueFamilyId, Queues};
use hal::range::RangeArg;

use cocoa::foundation::{NSRange, NSUInteger};
use metal::{self,
    MTLFeatureSet, MTLLanguageVersion, MTLArgumentAccess, MTLDataType, MTLPrimitiveType, MTLPrimitiveTopologyClass,
    MTLVertexStepFunction, MTLSamplerBorderColor, MTLSamplerMipFilter, MTLStorageMode, MTLResourceOptions, MTLTextureType,
    CaptureManager
};
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
    pub(crate) device: metal::Device,
    private_caps: PrivateCapabilities,
    command_shared: Arc<command::Shared>,
    memory_types: [hal::MemoryType; 3],
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        if cfg!(debug_assertions) || cfg!(feature = "metal_default_capture_scope") {
            let shared_capture_manager = CaptureManager::shared();
            if let Some(default_capture_scope) = shared_capture_manager.default_capture_scope() {
                default_capture_scope.end_scope();
            }
            shared_capture_manager.stop_capture();
        }
    }
}

pub struct PhysicalDevice {
    raw: metal::Device,
    memory_types: [hal::MemoryType; 3],
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

    pub(crate) fn new(raw: metal::Device) -> Self {
        let private_caps = PrivateCapabilities {
            resource_heaps: Self::supports_any(&raw, RESOURCE_HEAP_SUPPORT),
            argument_buffers: Self::supports_any(&raw, ARGUMENT_BUFFER_SUPPORT) && false, //TODO
            format_depth24_stencil8: raw.d24_s8_supported(),
            format_depth32_stencil8: false, //TODO: crashing the Metal validation layer upon copying from buffer
            format_min_srgb_channels: if Self::is_mac(&raw) {4} else {1},
            format_b5: !Self::is_mac(&raw),
            max_buffers_per_stage: 31,
            max_textures_per_stage: if Self::is_mac(&raw) {128} else {31},
            max_samplers_per_stage: 16,
        };
        PhysicalDevice {
            raw,
            memory_types: [
                hal::MemoryType {
                    properties: Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                    heap_index: 0,
                },
                hal::MemoryType {
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                    heap_index: 0,
                },
                hal::MemoryType {
                    properties: Properties::DEVICE_LOCAL,
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
        let id = family.id();

        let command_shared = Arc::new(command::Shared::new(self.raw.clone()));
        let mut queue_group = hal::backend::RawQueueGroup::new(family);
        queue_group.add_queue(command_shared.clone());

        let device = Device {
            device: self.raw.clone(),
            private_caps: self.private_caps.clone(),
            command_shared,
            memory_types: self.memory_types,
        };

        if cfg!(debug_assertions) || cfg!(feature = "metal_default_capture_scope") {
            let shared_capture_manager = CaptureManager::shared();
            let default_capture_scope = shared_capture_manager.new_capture_scope_with_device(&device.device);
            shared_capture_manager.set_default_capture_scope(default_capture_scope);
            shared_capture_manager.start_capture_with_scope(&default_capture_scope);
            default_capture_scope.begin_scope();
        }

        let mut queues = HashMap::new();
        queues.insert(id, queue_group);

        Ok(hal::Gpu {
            device,
            queues: Queues::new(queues),
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
        &self, format: format::Format, dimensions: u8, _tiling: image::Tiling,
        _usage: image::Usage, _storage_flags: image::StorageFlags,
    ) -> Option<image::FormatProperties> {
        //TODO: actually query this data
        let width = 4096;
        let height = if dimensions >= 2 { 4096 } else { 1 };
        let depth = if dimensions >= 3 { 4096 } else { 1 };
        let max_dimension = 4096f32; // Max of {width, height, depth}

        self.private_caps.map_format(format).map(|_| image::FormatProperties {
            max_extent: image::Extent { width, height, depth },
            max_levels: max_dimension.log2().ceil() as u8 + 1,
            // 3D images enforce a single layer
            max_layers: if dimensions == 3 { 1 } else { 2048 },
            sample_count_mask: 0x1,
            //TODO: buffers and textures have separate limits
            // Max buffer size is determined by feature set
            // Max texture size does not appear to be documented publicly
            max_resource_size: 1 << 31,
        })
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        hal::MemoryProperties {
            memory_heaps: vec![!0, !0], //TODO
            memory_types: self.memory_types.to_vec(),
        }
    }

    fn features(&self) -> hal::Features {
        hal::Features::ROBUST_BUFFER_ACCESS
    }

    fn limits(&self) -> hal::Limits {
        hal::Limits {
            max_texture_size: 4096, // TODO: feature set
            max_patch_size: 0, // No tessellation
            max_viewports: 1,

            min_buffer_copy_offset_alignment: if Self::is_mac(&self.raw) {256} else {64},
            min_buffer_copy_pitch_alignment: 4, // TODO: made this up
            min_texel_buffer_offset_alignment: 1, //TODO
            min_uniform_buffer_offset_alignment: 1, //TODO
            min_storage_buffer_offset_alignment: 1, //TODO

            max_compute_group_count: [16; 3], // TODO
            max_compute_group_size: [64; 3], // TODO

            framebuffer_color_samples_count: 0b101, // TODO
            framebuffer_depth_samples_count: 0b101, // TODO
            framebuffer_stencil_samples_count: 0b101, // TODO

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
    fn is_heap_coherent(&self, heap: &n::MemoryHeap) -> bool {
        match *heap {
            n::MemoryHeap::Emulated(memory_type) => self.memory_types[memory_type.0].properties.contains(Properties::COHERENT),
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
        options.set_language_version(match version {
            LanguageVersion { major: 1, minor: 0 } => MTLLanguageVersion::V1_0,
            LanguageVersion { major: 1, minor: 1 } => MTLLanguageVersion::V1_1,
            LanguageVersion { major: 1, minor: 2 } => MTLLanguageVersion::V1_2,
            LanguageVersion { major: 2, minor: 0 } => MTLLanguageVersion::V2_0,
            _ => return Err(ShaderError::CompilationFailed("shader model not supported".into()))
        });
        match self.device.new_library_with_source(source.as_ref(), &options) {
            Ok(library) => Ok(n::ShaderModule::Compiled {
                library,
                entry_point_map: HashMap::new(),
            }),
            Err(err) => Err(ShaderError::CompilationFailed(err.into())),
        }
    }

    fn compile_shader_library(
        &self,
        raw_data: &[u8],
        overrides: &HashMap<msl::ResourceBindingLocation, msl::ResourceBinding>,
    ) -> Result<(metal::Library, HashMap<String, spirv::EntryPoint>), ShaderError> {
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
        compiler_options.vertex.invert_y = true;
        // fill the overrides
        compiler_options.resource_binding_overrides = overrides.clone();

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

        let mut entry_point_map = HashMap::new();
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
        options.set_language_version(MTLLanguageVersion::V1_2);

        let library = self.device
            .new_library_with_source(shader_code.as_ref(), &options)
            .map_err(|err| ShaderError::CompilationFailed(err.into()))?;

        Ok((library, entry_point_map))
    }

    fn load_shader(
        &self, ep: &pso::EntryPoint<Backend>, layout: &n::PipelineLayout
    ) -> Result<(metal::Library, metal::Function, metal::MTLSize), pso::CreationError> {
        let entries_owned;
        let (lib, entry_point_map) = match *ep.module {
            n::ShaderModule::Compiled {ref library, ref entry_point_map} => {
                (library.to_owned(), entry_point_map)
            }
            n::ShaderModule::Raw(ref data) => {
                let raw = self.compile_shader_library(data, &layout.res_overrides).unwrap();
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
        ty: DescriptorType, index: pso::DescriptorBinding, count: usize
    ) -> metal::ArgumentDescriptor {
        let arg = metal::ArgumentDescriptor::new().to_owned();
        arg.set_array_length(count as _);

        match ty {
            DescriptorType::Sampler => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Sampler);
                arg.set_index(index as _);
            }
            DescriptorType::SampledImage => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Texture);
                arg.set_index(index as _);
            }
            DescriptorType::UniformBuffer => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Struct);
                arg.set_index(index as _);
            }
            DescriptorType::StorageBuffer => {
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
        &self, _family: QueueFamilyId, flags: CommandPoolCreateFlags
    ) -> command::CommandPool {
        command::CommandPool {
            shared: self.command_shared.clone(),
            managed: if flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
                None
            } else {
                Some(Vec::new())
            },
        }
    }

    fn destroy_command_pool(&self, pool: command::CommandPool) {
        if let Some(vec) = pool.managed {
            for cmd_buf in vec {
                cmd_buf
                    .borrow_mut()
                    .reset(&self.command_shared);
            }
        }
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
        let _ap = AutoreleasePool::new(); // for RP descriptor and attachments
        //TODO: subpasses, dependencies
        let pass = metal::RenderPassDescriptor::new().to_owned();

        let attachments = attachments.into_iter()
            .map(|attachment| attachment.borrow().clone())
            .collect::<Vec<_>>();
        let mut color_attachment_index = 0;
        for attachment in &attachments {
            let is_depth = match attachment.format {
                Some(f) => f.is_depth(),
                None => continue,
            };

            let mtl_attachment: &metal::RenderPassAttachmentDescriptorRef = if is_depth {
                pass
                    .depth_attachment()
                    .expect("no depth attachement")
            } else {
                color_attachment_index += 1;
                pass
                    .color_attachments()
                    .object_at(color_attachment_index - 1)
                    .expect("too many color attachments")
            };

            mtl_attachment.set_load_action(map_load_operation(attachment.ops.load));
            mtl_attachment.set_store_action(map_store_operation(attachment.ops.store));
        }

        n::RenderPass {
            desc: pass,
            attachments,
            num_colors: color_attachment_index,
        }
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        _push_constant_ranges: IR,
    ) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>
    {
        use hal::pso::ShaderStageFlags;

        struct Counters {
            buffers: usize,
            textures: usize,
            samplers: usize,
        }
        let mut stage_infos = [
            (ShaderStageFlags::VERTEX,   spirv::ExecutionModel::Vertex,    Counters { buffers:0, textures:0, samplers:0 }),
            (ShaderStageFlags::FRAGMENT, spirv::ExecutionModel::Fragment,  Counters { buffers:0, textures:0, samplers:0 }),
            (ShaderStageFlags::COMPUTE,  spirv::ExecutionModel::GlCompute, Counters { buffers:0, textures:0, samplers:0 }),
        ];
        let mut res_overrides = HashMap::new();

        for (set_index, set_layout) in set_layouts.into_iter().enumerate() {
            match set_layout.borrow() {
                &n::DescriptorSetLayout::Emulated(ref set_bindings) => {
                    for set_binding in set_bindings {
                        for &mut(stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                            if !set_binding.stage_flags.contains(stage_bit) {
                                continue
                            }
                            let mut res = msl::ResourceBinding {
                                buffer_id: !0,
                                texture_id: !0,
                                sampler_id: !0,
                                force_used: false,
                            };
                            match set_binding.ty {
                                DescriptorType::UniformBuffer |
                                DescriptorType::StorageBuffer => {
                                    res.buffer_id = counters.buffers as _;
                                    counters.buffers += 1;
                                }
                                DescriptorType::SampledImage |
                                DescriptorType::StorageImage |
                                DescriptorType::UniformTexelBuffer |
                                DescriptorType::StorageTexelBuffer |
                                DescriptorType::InputAttachment => {
                                    res.texture_id = counters.textures as _;
                                    counters.textures += 1;
                                }
                                DescriptorType::Sampler => {
                                    res.sampler_id = counters.samplers as _;
                                    counters.samplers += 1;
                                }
                                DescriptorType::CombinedImageSampler => {
                                    res.texture_id = counters.textures as _;
                                    res.sampler_id = counters.samplers as _;
                                    counters.textures += 1;
                                    counters.samplers += 1;
                                }
                                DescriptorType::UniformBufferDynamic |
                                DescriptorType::UniformImageDynamic => unimplemented!(),
                            };
                            assert_eq!(set_binding.count, 1); //TODO
                            let location = msl::ResourceBindingLocation {
                                stage,
                                desc_set: set_index as _,
                                binding: set_binding.binding as _,
                            };
                            res_overrides.insert(location, res);
                        }
                    }
                }
                &n::DescriptorSetLayout::ArgumentBuffer(_, stage_flags) => {
                    for &mut(stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
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

        // TODO: return an `Err` when HAL signature of the function supports it
        for &(_, _, ref counters) in &stage_infos {
            assert!(counters.buffers <= self.private_caps.max_buffers_per_stage);
            assert!(counters.textures <= self.private_caps.max_textures_per_stage);
            assert!(counters.samplers <= self.private_caps.max_samplers_per_stage);
        }

        n::PipelineLayout {
            attribute_buffer_index: stage_infos[0].2.buffers as _,
            res_overrides,
        }
    }

    fn create_graphics_pipeline<'a>(
        &self,
        pipeline_desc: &pso::GraphicsPipelineDesc<'a, Backend>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let pipeline = metal::RenderPipelineDescriptor::new();
        let pipeline_layout = &pipeline_desc.layout;
        let pass_descriptor = &pipeline_desc.subpass;

        if pipeline_layout.attribute_buffer_index as usize + pipeline_desc.vertex_buffers.len() > self.private_caps.max_buffers_per_stage {
            let msg = format!("Too many buffers inputs of the vertex stage: {} attributes + {} resources",
                pipeline_desc.vertex_buffers.len(), pipeline_layout.attribute_buffer_index);
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
        let (vs_lib, vs_function, _) = self.load_shader(&pipeline_desc.shaders.vertex, pipeline_layout)?;
        pipeline.set_vertex_function(Some(&vs_function));

        // Fragment shader
        let fs_function;
        let fs_lib = match pipeline_desc.shaders.fragment {
            Some(ref ep) => {
                let (lib, fun, _) = self.load_shader(ep, pipeline_layout)?;
                fs_function = fun;
                pipeline.set_fragment_function(Some(&fs_function));
                Some(lib)
            }
            None => None,
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
            if format.is_depth() {
                pipeline.set_depth_attachment_pixel_format(mtl_format);
            } else {
                let descriptor = pipeline
                    .color_attachments()
                    .object_at(i)
                    .expect("too many color attachments");
                descriptor.set_pixel_format(mtl_format);
            }
        }

        // Blending
        for (i, color_desc) in pipeline_desc.blender.targets.iter().enumerate() {
            let descriptor = pipeline
                .color_attachments()
                .object_at(i)
                .expect("too many color attachments");
            descriptor.set_write_mask(map_write_mask(color_desc.0));

            if let pso::BlendState::On { ref color, ref alpha } = color_desc.1 {
                descriptor.set_blending_enabled(true);
                let (color_op, color_src, color_dst) = map_blend_op(color);
                let (alpha_op, alpha_src, alpha_dst) = map_blend_op(alpha);

                descriptor.set_rgb_blend_operation(color_op);
                descriptor.set_source_rgb_blend_factor(color_src);
                descriptor.set_destination_rgb_blend_factor(color_dst);

                descriptor.set_alpha_blend_operation(alpha_op);
                descriptor.set_source_alpha_blend_factor(alpha_src);
                descriptor.set_destination_alpha_blend_factor(alpha_dst);
            }
        }

        let depth_stencil_state = pipeline_desc.depth_stencil.map(|depth_stencil| {
            let desc = metal::DepthStencilDescriptor::new();

            match depth_stencil.depth {
                DepthTest::On { fun, write } => {
                    desc.set_depth_compare_function(map_compare_function(fun));
                    desc.set_depth_write_enabled(write);
                }
                DepthTest::Off => {}
            }

            match depth_stencil.stencil {
                StencilTest::On { .. } => {
                    unimplemented!()
                }
                StencilTest::Off => {}
            }

            self.device.new_depth_stencil_state(&desc)
        });

        // Vertex buffers
        let vertex_descriptor = metal::VertexDescriptor::new();
        for vertex_buffer in &pipeline_desc.vertex_buffers {
            let mtl_buffer_index = pipeline_layout.attribute_buffer_index as usize + vertex_buffer.binding as usize;
            let mtl_buffer_desc = vertex_descriptor
                .layouts()
                .object_at(mtl_buffer_index)
                .expect("too many vertex descriptor layouts");
            mtl_buffer_desc.set_stride(vertex_buffer.stride as u64);
            match vertex_buffer.rate {
                0 => {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerVertex);
                }
                c => {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                    mtl_buffer_desc.set_step_rate(c as u64);
                }
            }
        }
        for (i, &AttributeDesc { binding, element, ..}) in pipeline_desc.attributes.iter().enumerate() {
            let mtl_vertex_format = map_vertex_format(element.format)
                .expect("unsupported vertex format");
            let mtl_attribute_desc = vertex_descriptor
                .attributes()
                .object_at(i)
                .expect("too many vertex attributes");
            let mtl_buffer_index = pipeline_layout.attribute_buffer_index + binding;

            mtl_attribute_desc.set_buffer_index(mtl_buffer_index as _);
            mtl_attribute_desc.set_offset(element.offset as NSUInteger);
            mtl_attribute_desc.set_format(mtl_vertex_format);
        }

        pipeline.set_vertex_descriptor(Some(&vertex_descriptor));

        self.device.new_render_pipeline_state(&pipeline)
            .map(|raw|
                n::GraphicsPipeline {
                    vs_lib,
                    fs_lib,
                    raw,
                    primitive_type,
                    attribute_buffer_index: pipeline_layout.attribute_buffer_index,
                    depth_stencil_state,
                    baked_states: pipeline_desc.baked_states.clone(),
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
        let pipeline = metal::ComputePipelineDescriptor::new();

        let (cs_lib, cs_function, work_group_size) = self.load_shader(&pipeline_desc.shader, &pipeline_desc.layout)?;
        pipeline.set_compute_function(Some(&cs_function));

        self.device.new_compute_pipeline_state(&pipeline)
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
    ) -> Result<n::FrameBuffer, FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>
    {
        let _ap = AutoreleasePool::new(); // for attachments
        let descriptor = unsafe {
            let desc: metal::RenderPassDescriptor = msg_send![renderpass.desc, copy];
            desc.set_render_target_array_length(extent.depth as NSUInteger);
            desc
        };

        let mut attachments = attachments.into_iter();
        for i in 0..renderpass.num_colors {
            let mtl_attachment = descriptor.color_attachments().object_at(i).expect("too many color attachments");
            let attachment = attachments.next().expect("Not enough colour attachments provided");
            mtl_attachment.set_texture(Some(&attachment.borrow().0));
        }

        let depth_attachment = attachments.next();
        if let Some(_) = attachments.next() {
            panic!("Metal does not support multiple depth attachments")
        }

        if let Some(attachment) = depth_attachment {
            let mtl_attachment = descriptor.depth_attachment().unwrap();
            mtl_attachment.set_texture(Some(&attachment.borrow().0));
            // TODO: stencil
        }

        Ok(n::FrameBuffer(descriptor))
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        //TODO: we can probably at least parse here and save the `Ast`
        let depends_on_pipeline_layout = true; //TODO: !self.private_caps.argument_buffers
        Ok(if depends_on_pipeline_layout {
            n::ShaderModule::Raw(raw_data.to_vec())
        } else {
            let (library, entry_point_map) = self.compile_shader_library(raw_data, &HashMap::new())?;
            n::ShaderModule::Compiled {
                library,
                entry_point_map,
            }
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let descriptor = metal::SamplerDescriptor::new();

        descriptor.set_min_filter(map_filter(info.min_filter));
        descriptor.set_mag_filter(map_filter(info.min_filter));
        descriptor.set_mip_filter(match info.mip_filter {
            image::Filter::Nearest => MTLSamplerMipFilter::Nearest,
            image::Filter::Linear => MTLSamplerMipFilter::Linear,
        });

        if let image::Anisotropic::On(aniso) = info.anisotropic {
            descriptor.set_max_anisotropy(aniso as _);
        }

        let (r, s, t) = info.wrap_mode;
        descriptor.set_address_mode_r(map_wrap_mode(r));
        descriptor.set_address_mode_s(map_wrap_mode(s));
        descriptor.set_address_mode_t(map_wrap_mode(t));

        descriptor.set_lod_bias(info.lod_bias.into());
        descriptor.set_lod_min_clamp(info.lod_range.start.into());
        descriptor.set_lod_max_clamp(info.lod_range.end.into());

        if let Some(fun) = info.comparison {
            descriptor.set_compare_function(map_compare_function(fun));
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

        n::Sampler(self.device.new_sampler(&descriptor))
    }

    fn destroy_sampler(&self, _sampler: n::Sampler) {
    }

    fn map_memory<R: RangeArg<u64>>(
        &self, memory: &n::Memory, generic_range: R
    ) -> Result<*mut u8, mapping::Error> {
        let range = memory.resolve(&generic_range);
        debug!("mapping memory {:?} at {:?}", memory, range);
        if self.is_heap_coherent(&memory.heap) {
            // Note from the Vulkan spec:
            //> Mapping non-coherent memory does not implicitly invalidate the mapped memory,
            //> and device writes that have not been invalidated *must* be made visible before
            //> the host reads or overwrites them.
            self.invalidate_mapped_memory_ranges(&[(memory, generic_range)]);
            memory.initialized.set(true);
        }

        let base_ptr = memory.cpu_buffer.as_ref().unwrap().contents() as *mut u8;
        let ptr = unsafe { base_ptr.offset(range.start as _) };

        let mut mapping = memory.mapping.lock().unwrap();
        assert!(mapping.is_none(), "Only one mapping per `Memory` at a time is allowed");
        *mapping = Some(range);

        Ok(ptr)
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        debug!("unmapping memory {:?}", memory);
        let range = memory.mapping.lock().unwrap().take().expect("Unmapping non-mapped memory?");
        if self.is_heap_coherent(&memory.heap) {
            // See note from the Vulkan spec above ^
            self.flush_mapped_memory_ranges(&[(memory, range)]);
        }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        const ALIGN_MASK: buffer::Offset = 3;
        let _ap = AutoreleasePool::new(); // for the encoder
        debug!("flushing mapped ranges");

        // temporary command buffer to copy the contents from
        // allocated CPU-visible buffers to the target buffers
        let mut pool = self.command_shared.queue_pool.lock().unwrap();
        let cmd_buffer = pool.borrow_command_buffer(&self.command_shared);
        let encoder = cmd_buffer.new_blit_command_encoder();

        for item in iter {
            let (memory, ref generic_range) = *item.borrow();
            let range = memory.resolve(generic_range);
            debug!("\trange {:?}", range);

            let cpu_buffer = memory.cpu_buffer.as_ref().unwrap();
            cpu_buffer.did_modify_range(NSRange {
                location: range.start as _,
                length: (range.end - range.start) as _,
            });

            let allocations = memory.allocations.lock().unwrap();
            for (alloc_range, dst) in allocations.find(&range) {
                let start = alloc_range.start.max(range.start);
                let end = alloc_range.end.min(range.end);
                let other = start - alloc_range.start;
                assert_eq!(other & ALIGN_MASK, 0);
                if start & ALIGN_MASK != 0 || end & ALIGN_MASK != 0 {
                    warn!("Invalidation of {:?} doesn't respect `non_coherent_atom_size`",
                        start .. end);
                }
                command::exec_blit(encoder, &soft::BlitCommand::CopyBuffer {
                    src: cpu_buffer.clone(),
                    dst,
                    region: BufferCopy {
                        src: start & !ALIGN_MASK,
                        dst: other,
                        size: end & !ALIGN_MASK - start &!ALIGN_MASK,
                    },
                });
            }
        }

        encoder.end_encoding();
        cmd_buffer.commit();
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        const ALIGN_MASK: buffer::Offset = 3;
        let _ap = AutoreleasePool::new(); // for the encoder
        debug!("invalidating mapped ranges");

        // temporary command buffer to copy the contents from
        // the given buffers into the allocated CPU-visible buffers
        let mut pool = self.command_shared.queue_pool.lock().unwrap();
        let (queue_id, cmd_buffer) = pool.make_command_buffer(&self.device);
        let encoder = cmd_buffer.new_blit_command_encoder();
        let mut num_copies = 0;

        for item in iter {
            let (memory, ref generic_range) = *item.borrow();
            if !memory.initialized.get() {
                continue
            }
            let range = memory.resolve(generic_range);
            debug!("\trange {:?}", range);
            let cpu_buffer = memory.cpu_buffer.as_ref().unwrap();

            let allocations = memory.allocations.lock().unwrap();
            for (alloc_range, src) in allocations.find(&range) {
                let start = alloc_range.start.max(range.start);
                let end = alloc_range.end.min(range.end);
                let other = start - alloc_range.start;
                assert_eq!(other & ALIGN_MASK, 0);
                if start & ALIGN_MASK != 0 || end & ALIGN_MASK != 0 {
                    warn!("Invalidation of {:?} doesn't respect `non_coherent_atom_size`",
                        start .. end);
                }
                command::exec_blit(encoder, &soft::BlitCommand::CopyBuffer {
                    src,
                    dst: cpu_buffer.clone(),
                    region: BufferCopy {
                        src: other,
                        dst: start & !ALIGN_MASK,
                        size: end & !ALIGN_MASK - start & !ALIGN_MASK,
                    },
                });
            }

            num_copies += 1;
            encoder.synchronize_resource(cpu_buffer.as_ref());
        }

        encoder.end_encoding();
        pool.release_command_buffer(queue_id);
        if num_copies != 0 {
            debug!("\twaiting...");
            cmd_buffer.commit();
            cmd_buffer.wait_until_completed();
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        unsafe { n::Semaphore(n::dispatch_semaphore_create(1)) } // Returns retained
    }

    fn create_descriptor_pool<I>(&self, _max_sets: usize, descriptor_ranges: I) -> n::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        if !self.private_caps.argument_buffers {
            return n::DescriptorPool::Emulated;
        }

        let mut num_samplers = 0;
        let mut num_textures = 0;
        let mut num_uniforms = 0;

        let arguments = descriptor_ranges.into_iter().map(|desc| {
            let desc = desc.borrow();
            let offset_ref = match desc.ty {
                DescriptorType::Sampler => &mut num_samplers,
                DescriptorType::SampledImage => &mut num_textures,
                DescriptorType::UniformBuffer | DescriptorType::StorageBuffer => &mut num_uniforms,
                _ => unimplemented!()
            };
            let index = *offset_ref;
            *offset_ref += desc.count;
            Self::describe_argument(desc.ty, index as _, desc.count)
        }).collect::<Vec<_>>();

        let arg_array = metal::Array::from_owned_slice(&arguments);
        let encoder = self.device.new_argument_encoder(&arg_array);

        let total_size = encoder.encoded_length();
        let buffer = self.device.new_buffer(total_size, MTLResourceOptions::empty());

        n::DescriptorPool::ArgumentBuffer {
            buffer,
            total_size,
            offset: 0,
        }
    }

    fn create_descriptor_set_layout<I>(&self, bindings: I) -> n::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
    {
        if !self.private_caps.argument_buffers {
            return n::DescriptorSetLayout::Emulated(
                bindings.into_iter().map(|desc| desc.borrow().clone()).collect()
            )
        }

        let mut stage_flags = pso::ShaderStageFlags::empty();
        let arguments = bindings.into_iter().map(|desc| {
            let desc = desc.borrow();
            stage_flags |= desc.stage_flags;
            Self::describe_argument(desc.ty, desc.binding, desc.count)
        }).collect::<Vec<_>>();
        let arg_array = metal::Array::from_owned_slice(&arguments);
        let encoder = self.device.new_argument_encoder(&arg_array);

        n::DescriptorSetLayout::ArgumentBuffer(encoder, stage_flags)
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        for write in write_iter {
            match *write.set {
                n::DescriptorSet::Emulated(ref inner) => {
                    let mut set = inner.lock().unwrap();
                    let mut array_offset = write.array_offset;
                    let mut binding = write.binding;

                    for descriptor in write.descriptors {
                        while array_offset >= set.layout.iter()
                                .find(|layout| layout.binding == binding)
                                .expect("invalid descriptor set binding index")
                                .count
                        {
                            array_offset = 0;
                            binding += 1;
                        }
                        match (descriptor.borrow(), set.bindings.get_mut(&binding).unwrap()) {
                            (&pso::Descriptor::Sampler(sampler), &mut n::DescriptorSetBinding::Sampler(ref mut vec)) => {
                                vec[array_offset] = Some(sampler.0.clone());
                            }
                            (&pso::Descriptor::Image(image, layout), &mut n::DescriptorSetBinding::Image(ref mut vec)) => {
                                vec[array_offset] = Some((image.0.clone(), layout));
                            }
                            (&pso::Descriptor::CombinedImageSampler(image, layout, sampler), &mut n::DescriptorSetBinding::Combined(ref mut vec)) => {
                                vec[array_offset] = Some((image.0.clone(), layout, sampler.0.clone()));
                            }
                            (&pso::Descriptor::UniformTexelBuffer(view), &mut n::DescriptorSetBinding::Image(ref mut vec)) |
                            (&pso::Descriptor::StorageTexelBuffer(view), &mut n::DescriptorSetBinding::Image(ref mut vec)) => {
                                vec[array_offset] = Some((view.raw.clone(), image::Layout::General));
                            }
                            (&pso::Descriptor::Buffer(buffer, ref range), &mut n::DescriptorSetBinding::Buffer(ref mut vec)) => {
                                let buf_length = buffer.raw.length();
                                let start = range.start.unwrap_or(0);
                                let end = range.end.unwrap_or(buf_length);
                                assert!(end <= buf_length);
                                vec[array_offset] = Some((buffer.raw.clone(), start));
                            }
                            (&pso::Descriptor::Sampler(..), _) |
                            (&pso::Descriptor::Image(..), _) |
                            (&pso::Descriptor::CombinedImageSampler(..), _) |
                            (&pso::Descriptor::Buffer(..), _) |
                            (&pso::Descriptor::UniformTexelBuffer(..), _) |
                            (&pso::Descriptor::StorageTexelBuffer(..), _) => {
                                panic!("mismatched descriptor set type")
                            }
                        }
                    }
                }
                n::DescriptorSet::ArgumentBuffer { ref buffer, offset, ref encoder, .. } => {
                    debug_assert!(self.private_caps.argument_buffers);

                    encoder.set_argument_buffer(buffer, offset);
                    //TODO: range checks, need to keep some layout metadata around
                    assert_eq!(write.array_offset, 0); //TODO

                    for descriptor in write.descriptors {
                        match *descriptor.borrow() {
                            pso::Descriptor::Sampler(sampler) => {
                                encoder.set_sampler_states(&[&sampler.0], write.binding as _);
                            }
                            pso::Descriptor::Image(image, _layout) => {
                                encoder.set_textures(&[&image.0], write.binding as _);
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

    fn destroy_framebuffer(&self, _buffer: n::FrameBuffer) {
    }

    fn destroy_semaphore(&self, semaphore: n::Semaphore) {
        unsafe { n::dispatch_release(semaphore.0) }
    }

    fn allocate_memory(&self, memory_type: hal::MemoryTypeId, size: u64) -> Result<n::Memory, OutOfMemory> {
        let memory_properties = self.memory_types[memory_type.0].properties;
        let (storage, cache) = map_memory_properties_to_storage_and_cache(memory_properties);

        //TODO: use aliasing when `resource_heaps` are supported
        let cpu_buffer = match storage {
            MTLStorageMode::Shared |
            MTLStorageMode::Managed => {
                let buffer = self.device.new_buffer(size, MTLResourceOptions::StorageModeManaged);
                Some(buffer)
            },
            MTLStorageMode::Private => None,
        };

        // Heaps cannot be used for CPU coherent resources
        //TEMP: MacOS supports Private only, iOS and tvOS can do private/shared
        let heap = if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared && false {
            let descriptor = metal::HeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            let heap_raw = self.device.new_heap(&descriptor);
            n::MemoryHeap::Native(heap_raw)
        } else {
            n::MemoryHeap::Emulated(memory_type)
        };

        Ok(n::Memory::new(heap, size, cpu_buffer))
    }

    fn free_memory(&self, _memory: n::Memory) {
    }

    fn create_buffer(
        &self, size: u64, _usage: buffer::Usage
    ) -> Result<n::UnboundBuffer, buffer::CreationError> {
        Ok(n::UnboundBuffer {
            size
        })
    }

    fn get_buffer_requirements(&self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        let mut max_size = buffer.size;
        let mut max_alignment = 1;
        let mut type_mask = (1 << self.memory_types.len()) - 1;

        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the buffer with, so we test them
            // all get the most stringent ones.
            for (i, mt) in self.memory_types.iter().enumerate() {
                // Note: we ignore COHERENT because heaps can't use the associated Shared memory type
                if mt.properties.contains(memory::Properties::COHERENT) {
                    type_mask ^= 1 << i;
                    continue
                }
                let options = map_memory_properties_to_options(mt.properties);
                let requirements = self.device.heap_buffer_size_and_align(buffer.size, options);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
        }

        // based on Metal validation error for view creation:
        // failed assertion `BytesPerRow of a buffer-backed texture with pixelFormat(XXX) must be aligned to 256 bytes
        const SIZE_MASK: u64 = 0xFF;

        memory::Requirements {
            size: (max_size + SIZE_MASK) & !SIZE_MASK,
            alignment: max_alignment,
            type_mask,
        }
    }

    fn bind_buffer_memory(
        &self, memory: &n::Memory, offset: u64, buffer: n::UnboundBuffer
    ) -> Result<n::Buffer, BindError> {
        let (raw, res_options, is_mappable) = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode(),
                );
                let raw = heap.new_buffer(buffer.size, resource_options)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.device.new_buffer(buffer.size, resource_options)
                    });
                let is_mappable = heap.storage_mode() != MTLStorageMode::Private;
                (raw, resource_options, is_mappable)
            }
            n::MemoryHeap::Emulated(mt) => {
                // Note: buffer backing is always `Private`. CPU access is routed through
                // a CPU-visible buffer associated with `Memory` object.
                let res_options = MTLResourceOptions::StorageModePrivate;
                let raw = self.device.new_buffer(buffer.size, res_options);
                let props = self.memory_types[mt.0].properties;
                let is_mappable = props.contains(memory::Properties::CPU_VISIBLE);
                if is_mappable && props.contains(memory::Properties::COHERENT) {
                    warn!("COHERENT buffer memory is not yet supported properly");
                }
                (raw, res_options, is_mappable)
            }
        };

        Ok(n::Buffer {
            allocations: if is_mappable {
                memory.initialized.set(true); // this is conservative, can be improved
                memory.allocations.lock().unwrap().insert(offset .. (offset + buffer.size), raw.clone());
                Some(memory.allocations.clone())
            } else {
                None
            },
            raw,
            offset,
            res_options,
        })
    }

    fn destroy_buffer(&self, buffer: n::Buffer) {
        if let Some(alloc) = buffer.allocations {
            alloc.lock().unwrap().remove(buffer.offset .. (buffer.offset + buffer.raw.length()));
        }
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self, buffer: &n::Buffer, format_maybe: Option<format::Format>, range: R
    ) -> Result<n::BufferView, buffer::ViewError> {
        let start = buffer.offset + *range.start().unwrap_or(&0);
        let end_rough = *range.end().unwrap_or(&buffer.raw.length());
        let format = match format_maybe {
            Some(fmt) => fmt,
            None => return Err(buffer::ViewError::Unsupported),
        };
        let format_desc = format.surface_desc();
        if format_desc.aspects != format::Aspects::COLOR {
            // no depth/stencil support for buffer views here
            return Err(buffer::ViewError::Unsupported)
        }
        let block_count = (end_rough - start) * 8 / format_desc.bits as u64;
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(buffer::ViewError::Unsupported)?;

        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_width(format_desc.dim.0 as u64 * block_count);
        descriptor.set_height(format_desc.dim.1 as u64);
        descriptor.set_mipmap_level_count(1);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_resource_options(buffer.res_options);
        descriptor.set_storage_mode(buffer.raw.storage_mode());

        //TODO:
        //The offset and bytesPerRow parameters must be byte aligned to the size returned by the
        // minimumLinearTextureAlignmentForPixelFormat: method. The bytesPerRow parameter must also be
        // greater than or equal to the size of one pixel, in bytes, multiplied by the pixel width of one row.

        const STRIDE_MASK: u64 = 0xFF;
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
        _tiling: image::Tiling,
        usage: image::Usage,
        flags: image::StorageFlags,
    ) -> Result<n::UnboundImage, image::CreationError> {
        let is_cube = flags.contains(image::StorageFlags::CUBE_VIEW);
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(image::CreationError::Format(format))?;

        let descriptor = metal::TextureDescriptor::new();

        let mtl_type = match kind {
            image::Kind::D1(_, 1) if mip_levels == 1=> {
                assert!(!is_cube);
                MTLTextureType::D1
            }
            image::Kind::D1(_, layers) if mip_levels == 1 => {
                assert!(!is_cube);
                descriptor.set_array_length(layers as u64);
                MTLTextureType::D1Array
            }
            image::Kind::D1(_, 1) |
            image::Kind::D2(_, _, 1, 1) => {
                MTLTextureType::D2
            }
            image::Kind::D1(_, layers) |
            image::Kind::D2(_, _, layers, 1) => {
                if is_cube && layers > 6 {
                    assert_eq!(layers % 6, 0);
                    descriptor.set_array_length(layers as u64 / 6);
                    MTLTextureType::CubeArray
                } else if is_cube {
                    assert_eq!(layers, 6);
                    MTLTextureType::Cube
                } else if layers > 1 {
                    descriptor.set_array_length(layers as u64);
                    MTLTextureType::D2Array
                } else {
                    MTLTextureType::D2
                }
            }
            image::Kind::D2(_, _, 1, samples) if !is_cube => {
                descriptor.set_sample_count(samples as u64);
                MTLTextureType::D2Multisample
            }
            image::Kind::D2(..) => {
                error!("Multi-sampled array textures or cubes are not supported: {:?}", kind);
                return Err(image::CreationError::Kind)
            }
            image::Kind::D3(..) => {
                assert!(!is_cube);
                MTLTextureType::D3
            }
        };

        descriptor.set_texture_type(mtl_type);
        let extent = kind.extent();
        descriptor.set_width(extent.width as u64);
        descriptor.set_height(extent.height as u64);
        descriptor.set_depth(extent.depth as u64);
        descriptor.set_mipmap_level_count(mip_levels as u64);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_usage(map_texture_usage(usage));

        Ok(n::UnboundImage {
            texture_desc: descriptor,
            format,
            extent,
        })
    }

    fn get_image_requirements(&self, image: &n::UnboundImage) -> memory::Requirements {
        let mut type_mask = if !image.format.surface_desc().aspects.contains(format::Aspects::COLOR) {
            0x4 // DEVICE_LOCAL only
        } else {
            0x7 // any type
        };
        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the image with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            let mut max_size = 0;
            let mut max_alignment = 0;
            for (i, mt) in self.memory_types.iter().enumerate() {
                if type_mask & (1 << i) == 0 {
                    continue
                }
                // Note: we ignore COHERENT because heaps can't use the associated Shared memory type
                if mt.properties.contains(memory::Properties::COHERENT) {
                    type_mask ^= 1 << i;
                    continue
                }
                let options = map_memory_properties_to_options(mt.properties);
                image.texture_desc.set_resource_options(options);
                let requirements = self.device.heap_texture_size_and_align(&image.texture_desc);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
            memory::Requirements {
                size: max_size,
                alignment: max_alignment,
                type_mask,
            }
        } else {
            memory::Requirements {
                size: 1, // TODO: something sensible
                alignment: 4,
                type_mask,
            }
        }
    }

    fn bind_image_memory(
        &self, memory: &n::Memory, _offset: u64, image: n::UnboundImage
    ) -> Result<n::Image, BindError> {
        let raw = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.texture_desc.set_resource_options(resource_options);
                heap.new_texture(&image.texture_desc)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.device.new_texture(&image.texture_desc)
                    })
            },
            n::MemoryHeap::Emulated(memory_type) => {
                // TODO: disable hazard tracking?
                let memory_properties = self.memory_types[memory_type.0].properties;
                let resource_options = map_memory_properties_to_options(memory_properties);
                image.texture_desc.set_resource_options(resource_options);
                self.device.new_texture(&image.texture_desc)
            }
        };

        let base = image.format.base_format();

        Ok(n::Image {
            raw,
            extent: image.extent,
            format_desc: base.0.desc(),
            shader_channel: match base.1 {
                format::ChannelType::Unorm |
                format::ChannelType::Inorm |
                format::ChannelType::Ufloat |
                format::ChannelType::Float |
                format::ChannelType::Uscaled |
                format::ChannelType::Iscaled |
                format::ChannelType::Srgb => Channel::Float,
                format::ChannelType::Uint => Channel::Uint,
                format::ChannelType::Int => Channel::Int,
            },
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
        _kind: image::ViewKind,
        format: format::Format,
        _swizzle: format::Swizzle,
        _range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        // TODO: subresource range

        let mtl_format = match self.private_caps.map_format(format) {
            Some(f) => f,
            None => {
                error!("failed to find corresponding Metal format for {:?}", format);
                return Err(image::ViewError::BadFormat);
            },
        };

        Ok(n::ImageView(image.raw.new_texture_view(mtl_format)))
    }

    fn destroy_image_view(&self, _view: n::ImageView) {
    }

    // Emulated fence implementations
    #[cfg(not(feature = "native_fence"))]
    fn create_fence(&self, signaled: bool) -> n::Fence {
        n::Fence(Arc::new(Mutex::new(signaled)))
    }
    fn reset_fence(&self, fence: &n::Fence) {
        *fence.0.lock().unwrap() = false;
    }
    fn wait_for_fence(&self, fence: &n::Fence, mut timeout_ms: u32) -> bool {
        use std::{thread, time};
        let tick = 1;
        debug!("waiting for fence {:?} for {} ms", fence, timeout_ms);
        loop {
            if *fence.0.lock().unwrap() {
                return true
            }
            if timeout_ms < tick {
                return false
            }
            timeout_ms -= tick;
            thread::sleep(time::Duration::from_millis(tick as u64));
        }
    }
    fn get_fence_status(&self, fence: &n::Fence) -> bool {
        *fence.0.lock().unwrap()
    }
    #[cfg(not(feature = "native_fence"))]
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
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        self.build_swapchain(surface, config)
    }

    fn destroy_swapchain(&self, _swapchain: Swapchain) {
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        self.command_shared.queue_pool
            .lock()
            .unwrap()
            .wait_idle(&self.device);
        Ok(())
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send+Sync>() {}
    foo::<Device>()
}
