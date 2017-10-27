use {Backend};
use {native as n, command};
use {QueueFamily};
use conversions::*;

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{cmp, mem, ptr, slice};

use hal::{self,
        image, pass, format, mapping, memory, buffer, pso};
use hal::device::{WaitFor, BindError, OutOfMemory, FramebufferError, ShaderError, Extent};
use hal::pool::CommandPoolCreateFlags;
use hal::pso::{DescriptorSetWrite, DescriptorType, DescriptorSetLayoutBinding, AttributeDesc};
use hal::pass::{Subpass};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::{self, MTLFeatureSet, MTLLanguageVersion, MTLArgumentAccess, MTLDataType, MTLPrimitiveType, MTLPrimitiveTopologyClass};
use metal::{MTLVertexStepFunction, MTLSamplerMinMagFilter, MTLSamplerMipFilter, MTLStorageMode, MTLResourceOptions, MTLTextureType};
use foreign_types::ForeignType;
use objc::runtime::Object as ObjcObject;
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
/// Emit error during shader module creation. Used if we execute an query command.
fn gen_query_error(err: SpirvErrorCode) -> ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unknown query error".into(),
    };
    ShaderError::CompilationFailed(msg)
}


#[derive(Clone, Copy)]
struct PrivateCapabilities {
    resource_heaps: bool,
    argument_buffers: bool,
    max_buffers_per_stage: usize,
    max_textures_per_stage: usize,
    max_samplers_per_stage: usize,
}

#[derive(Clone)]
pub struct Device {
    device: metal::Device,
    private_caps: PrivateCapabilities,
    limits: hal::Limits,
    queue: Arc<command::QueueInner>,
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

pub struct PhysicalDevice(pub(crate) metal::Device);

impl PhysicalDevice {
    fn supports_any(&self, features_sets: &[MTLFeatureSet]) -> bool {
        features_sets.iter().cloned().any(|x| self.0.supports_feature_set(x))
    }
}


impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(self, mut families: Vec<(QueueFamily, usize)>) -> hal::Gpu<Backend> {
        use self::memory::Properties;

        assert_eq!(families.len(), 1);
        let mut queue_group = hal::queue::RawQueueGroup::new(families.remove(0).0);
        let queue_raw = command::CommandQueue::new(&self.0);
        let queue = queue_raw.0.clone();
        queue_group.add_queue(queue_raw);

        let is_mac = self.0.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v1);

        let private_caps = PrivateCapabilities {
            resource_heaps: self.supports_any(RESOURCE_HEAP_SUPPORT),
            argument_buffers: self.supports_any(ARGUMENT_BUFFER_SUPPORT) && false, //TODO
            max_buffers_per_stage: 31,
            max_textures_per_stage: if is_mac {128} else {31},
            max_samplers_per_stage: 31,
        };

        let device = Device {
            device: self.0.clone(),
            private_caps,
            limits: hal::Limits {
                max_texture_size: 4096, // TODO: feature set
                max_patch_size: 0, // No tesselation
                max_viewports: 1,

                min_buffer_copy_offset_alignment: if is_mac {256} else {64},
                min_buffer_copy_pitch_alignment: 4, // TODO: made this up
                min_uniform_buffer_offset_alignment: 1, // TODO

                max_compute_group_count: [0; 3], // TODO
                max_compute_group_size: [0; 3], // TODO
            },
            queue,
        };

        let memory_types = vec![
            hal::MemoryType {
                id: 0,
                properties: Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                heap_index: 0,
            },
            hal::MemoryType {
                id: 1,
                properties: Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                heap_index: 0,
            },
            hal::MemoryType {
                id: 2,
                properties: Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                heap_index: 0,
            },
            hal::MemoryType {
                id: 3,
                properties: Properties::DEVICE_LOCAL,
                heap_index: 1,
            },
        ];
        let memory_heaps = vec![!0, !0]; //TODO

        hal::Gpu {
            device,
            queue_groups: vec![queue_group],
            memory_types,
            memory_heaps,
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
            Ok(lib) => Ok(n::ShaderModule::Compiled(lib)),
            Err(err) => Err(ShaderError::CompilationFailed(err.into())),
        }
    }

    fn compile_shader_library(
        &self,
        raw_data: &[u8],
        overrides: &HashMap<msl::ResourceBindingLocation, msl::ResourceBinding>,
    ) -> Result<metal::Library, ShaderError> {
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
        // fill the resource overrides
        compiler_options.resource_binding_overrides = overrides.clone();

        ast.set_compiler_options(&compiler_options)
            .map_err(|err| {
                let msg = match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unexpected error".into(),
                };
                ShaderError::CompilationFailed(msg)
            })?;

        let shader_code = ast.compile()
            .map_err(|err| {
                let msg =  match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                };
                ShaderError::CompilationFailed(msg)
            })?;

        // done
        debug!("SPIRV-Cross generated shader:\n{}", shader_code);

        let options = metal::CompileOptions::new();
        options.set_language_version(MTLLanguageVersion::V1_1);
        self.device
            .new_library_with_source(shader_code.as_ref(), &options)
            .map_err(|err| ShaderError::CompilationFailed(err.into()))
    }

    fn describe_argument(ty: DescriptorType, index: usize, count: usize) -> metal::ArgumentDescriptor {
        let arg = metal::ArgumentDescriptor::new().to_owned();
        arg.set_array_length(count as NSUInteger);

        match ty {
            DescriptorType::Sampler => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Sampler);
                arg.set_index(index as NSUInteger);
            }
            DescriptorType::SampledImage => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Texture);
                arg.set_index(index as NSUInteger);
            }
            _ => unimplemented!()
        }

        arg
    }

    fn create_graphics_pipeline<'a>(
        &self,
        &(ref shader_set, pipeline_layout, ref pass_descriptor, pipeline_desc):
        &(pso::GraphicsShaderSet<'a, Backend>, &n::PipelineLayout, Subpass<'a, Backend>, &pso::GraphicsPipelineDesc),
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let pipeline =  metal::RenderPipelineDescriptor::new();

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

        // Shaders
        let vs_lib = match shader_set.vertex.module {
            &n::ShaderModule::Compiled(ref lib) => lib.to_owned(),
            &n::ShaderModule::Raw(ref data) => {
                //TODO: cache them all somewhere!
                self.compile_shader_library(data, &pipeline_layout.res_overrides).unwrap()
            },
        };
        let mtl_vertex_function = vs_lib
            .get_function(shader_set.vertex.entry)
            .ok_or_else(|| {
                error!("invalid vertex shader entry point");
                pso::CreationError::Other
            })?;
        pipeline.set_vertex_function(Some(&mtl_vertex_function));
        let fs_lib = if let Some(fragment_entry) = shader_set.fragment {
            let fs_lib = match fragment_entry.module {
                &n::ShaderModule::Compiled(ref lib) => lib.to_owned(),
                &n::ShaderModule::Raw(ref data) => {
                    self.compile_shader_library(data, &pipeline_layout.res_overrides).unwrap()
                }
            };
            let mtl_fragment_function = fs_lib
                .get_function(fragment_entry.entry)
                .ok_or_else(|| {
                    error!("invalid pixel shader entry point");
                    pso::CreationError::Other
                })?;
            pipeline.set_fragment_function(Some(&mtl_fragment_function));
            Some(fs_lib)
        } else {
            None
        };
        if shader_set.hull.is_some() {
            error!("Metal tesselation shaders are not supported");
            return Err(pso::CreationError::Other);
        }
        if shader_set.domain.is_some() {
            error!("Metal tesselation shaders are not supported");
            return Err(pso::CreationError::Other);
        }
        if shader_set.geometry.is_some() {
            error!("Metal geometry shaders are not supported");
            return Err(pso::CreationError::Other);
        }

        // Copy color target info from Subpass
        for (i, attachment) in pass_descriptor.main_pass.attachments.iter().enumerate() {
            let descriptor = pipeline.color_attachments().object_at(i).expect("too many color attachments");

            let (mtl_format, is_depth) = map_format(attachment.format).expect("unsupported color format for Metal");
            if is_depth {
                continue;
            }

            descriptor.set_pixel_format(mtl_format);
        }

        // Blending
        for (i, color_desc) in pipeline_desc.blender.targets.iter().enumerate() {
            let descriptor = pipeline.color_attachments().object_at(i).expect("too many color attachments");

            descriptor.set_write_mask(map_write_mask(color_desc.mask));
            descriptor.set_blending_enabled(color_desc.color.is_some() | color_desc.alpha.is_some());

            if let Some(blend) = color_desc.color {
                descriptor.set_source_rgb_blend_factor(map_blend_factor(blend.source, false));
                descriptor.set_destination_rgb_blend_factor(map_blend_factor(blend.destination, false));
                descriptor.set_rgb_blend_operation(map_blend_op(blend.equation));
            }

            if let Some(blend) = color_desc.alpha {
                descriptor.set_source_alpha_blend_factor(map_blend_factor(blend.source, true));
                descriptor.set_destination_alpha_blend_factor(map_blend_factor(blend.destination, true));
                descriptor.set_alpha_blend_operation(map_blend_op(blend.equation));
            }
        }

        // Vertex buffers
        let vertex_descriptor = metal::VertexDescriptor::new();
        for (i, vertex_buffer) in pipeline_desc.vertex_buffers.iter().enumerate() {
            let mtl_buffer_desc = vertex_descriptor.layouts().object_at(i).expect("too many vertex descriptor layouts");
            mtl_buffer_desc.set_stride(vertex_buffer.stride as u64);
            match vertex_buffer.rate {
                0 => {
                    // FIXME: should this use MTLVertexStepFunction::Constant?
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerVertex);
                },
                1 => {
                    // FIXME: how to determine instancing in this case?
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerVertex);
                },
                c => {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                    mtl_buffer_desc.set_step_rate(c as u64);
                }
            }
        }
        for (i, &AttributeDesc { binding, element, ..}) in pipeline_desc.attributes.iter().enumerate() {
            let mtl_vertex_format = map_vertex_format(element.format).expect("unsupported vertex format for Metal");

            let mtl_attribute_desc = vertex_descriptor.attributes().object_at(i).expect("too many vertex attributes");
            mtl_attribute_desc.set_buffer_index(binding as NSUInteger); // TODO: Might be binding, not location?
            mtl_attribute_desc.set_offset(element.offset as NSUInteger);
            mtl_attribute_desc.set_format(mtl_vertex_format);
        }

        pipeline.set_vertex_descriptor(Some(&vertex_descriptor));

        let mut err_ptr: *mut ObjcObject = ptr::null_mut();
        let pso: *mut metal::MTLRenderPipelineState = unsafe {
            msg_send![&*self.device, newRenderPipelineStateWithDescriptor:&*pipeline error: &mut err_ptr]
        };

        if pso.is_null() {
            error!("PSO creation failed: {}", unsafe { n::objc_err_description(err_ptr) });
            unsafe { msg_send![err_ptr, release] };
            Err(pso::CreationError::Other)
        } else {
            Ok(n::GraphicsPipeline {
                vs_lib,
                fs_lib,
                raw: unsafe { metal::RenderPipelineState::from_ptr(pso) },
                primitive_type,
            })
        }
    }
}

impl hal::Device<Backend> for Device {
    fn get_features(&self) -> &hal::Features {
        unimplemented!()
    }

    fn get_limits(&self) -> &hal::Limits {
        &self.limits
    }

    fn create_command_pool(
        &self, _family: &QueueFamily, flags: CommandPoolCreateFlags
    ) -> command::CommandPool {
        command::CommandPool {
            queue: self.queue.clone(),
            managed: if flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
                None
            } else {
                Some(Vec::new())
            },
        }
    }

    fn destroy_command_pool(&self, _pool: command::CommandPool) {
        //TODO?
    }

    fn create_render_pass(
        &self,
        attachments: &[pass::Attachment],
        _subpasses: &[pass::SubpassDesc],
        _dependencies: &[pass::SubpassDependency],
    ) -> n::RenderPass {
        //TODO: subpasses, dependencies
        let pass = metal::RenderPassDescriptor::new().to_owned();

        let mut color_attachment_index = 0;
        //let mut depth_attachment_index = 0;
        for attachment in attachments {
            let (_format, is_depth) = map_format(attachment.format).expect("unsupported attachment format");

            let mtl_attachment: &metal::RenderPassAttachmentDescriptorRef;
            if !is_depth {
                let color_attachment = pass.color_attachments().object_at(color_attachment_index).expect("too many color attachments");
                color_attachment_index += 1;

                mtl_attachment = color_attachment;
            } else {
                unimplemented!()
            }

            mtl_attachment.set_load_action(map_load_operation(attachment.ops.load));
            mtl_attachment.set_store_action(map_store_operation(attachment.ops.store));
        }

        n::RenderPass {
            desc: pass,
            attachments: attachments.into(),
            num_colors: color_attachment_index,
        }
    }

    fn create_pipeline_layout(&self, set_layouts: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        use hal::pso::ShaderStageFlags;

        struct Counters {
            buffers: usize,
            textures: usize,
            samplers: usize,
        }
        let mut stage_infos = [
            (ShaderStageFlags::VERTEX,   spirv::ExecutionModel::Vertex,   Counters { buffers:0, textures:0, samplers:0 }),
            (ShaderStageFlags::FRAGMENT, spirv::ExecutionModel::Fragment, Counters { buffers:0, textures:0, samplers:0 }),
        ];
        let mut res_overrides = HashMap::new();

        for (set_index, set_layout) in set_layouts.iter().enumerate() {
            match set_layout {
                &&n::DescriptorSetLayout::Emulated(ref set_bindings) => {
                    for set_binding in set_bindings {
                        for &mut(stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                            if !set_binding.stage_flags.contains(stage_bit) {
                                continue
                            }
                            let count = match set_binding.ty {
                                DescriptorType::UniformBuffer |
                                DescriptorType::StorageBuffer => &mut counters.buffers,
                                DescriptorType::SampledImage => &mut counters.textures,
                                DescriptorType::Sampler => &mut counters.samplers,
                                _ => unimplemented!()
                            };
                            for i in 0 .. set_binding.count {
                                let location = msl::ResourceBindingLocation {
                                    stage,
                                    desc_set: set_index as _,
                                    binding: (set_binding.binding + i) as _,
                                };
                                let res_binding = msl::ResourceBinding {
                                    resource_id: *count as _,
                                    force_used: false,
                                };
                                *count += 1;
                                res_overrides.insert(location, res_binding);
                            }
                        }
                    }
                }
                &&n::DescriptorSetLayout::ArgumentBuffer(_, stage_flags) => {
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
                            resource_id: counters.buffers as _,
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

        n::PipelineLayout { res_overrides }
    }

    fn create_graphics_pipelines<'a>(
        &self,
        params: &[(pso::GraphicsShaderSet<'a, Backend>, &n::PipelineLayout, Subpass<'a, Backend>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        let mut output = Vec::with_capacity(params.len());
        for param in params {
            output.push(self.create_graphics_pipeline(param));
        }
        output
    }

    fn create_compute_pipelines<'a>(
        &self,
        _pipelines: &[(pso::EntryPoint<'a, Backend>, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &self, renderpass: &n::RenderPass, attachments: &[&n::ImageView], extent: Extent
    ) -> Result<n::FrameBuffer, FramebufferError> {
        let descriptor = unsafe {
            let desc: metal::RenderPassDescriptor = msg_send![renderpass.desc, copy];

            msg_send![&*desc, setRenderTargetArrayLength: extent.depth as usize];

            for (i, attachment) in attachments[..renderpass.num_colors].iter().enumerate() {
                let mtl_attachment = desc.color_attachments().object_at(i).expect("too many color attachments");
                mtl_attachment.set_texture(Some(&attachment.0));
            }

            assert!(renderpass.num_colors + 1 >= attachments.len(),
                "Metal does not support multiple depth attachments");

            if let Some(attachment) = attachments.get(renderpass.num_colors) {
                let mtl_attachment = desc.depth_attachment().unwrap();
                mtl_attachment.set_texture(Some(&attachment.0));
                // TODO: stencil
            }

            desc
        };

        Ok(n::FrameBuffer(descriptor))
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        //TODO: we can probably at least parse here and save the `Ast`
        let depends_on_pipeline_layout = true; //TODO: !self.private_caps.argument_buffers
        if depends_on_pipeline_layout {
            Ok(n::ShaderModule::Raw(raw_data.to_vec()))
        } else {
            self.compile_shader_library(raw_data, &HashMap::new())
                .map(n::ShaderModule::Compiled)
        }
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let descriptor = metal::SamplerDescriptor::new();

        use self::image::FilterMethod::*;
        let (min_mag, mipmap) = match info.filter {
            Scale => (MTLSamplerMinMagFilter::Nearest, MTLSamplerMipFilter::NotMipmapped),
            Mipmap => (MTLSamplerMinMagFilter::Nearest, MTLSamplerMipFilter::Nearest),
            Bilinear => {
                (MTLSamplerMinMagFilter::Linear, MTLSamplerMipFilter::NotMipmapped)
            }
            Trilinear => (MTLSamplerMinMagFilter::Linear, MTLSamplerMipFilter::Linear),
            Anisotropic(max) => {
                descriptor.set_max_anisotropy(max as u64);
                (MTLSamplerMinMagFilter::Linear, MTLSamplerMipFilter::NotMipmapped)
            }
        };

        descriptor.set_min_filter(min_mag);
        descriptor.set_mag_filter(min_mag);
        descriptor.set_mip_filter(mipmap);

        // FIXME: more state

        n::Sampler(self.device.new_sampler(&descriptor))
    }

    fn destroy_sampler(&self, _sampler: n::Sampler) {
    }

    fn acquire_mapping_raw(
        &self, buf: &n::Buffer, read: Option<Range<u64>>
    ) -> Result<*mut u8, mapping::Error> {
        let base_ptr = buf.0.contents() as *mut u8;

        if base_ptr.is_null() {
            return Err(mapping::Error::InvalidAccess);
        }

        if let Some(range) = read {
            if range.end > buf.0.length() {
                return Err(mapping::Error::OutOfBounds);
            }
        }

        Ok(base_ptr)
    }

    fn release_mapping_raw(&self, buffer: &n::Buffer, wrote: Option<Range<u64>>) {
        if let Some(range) = wrote {
            if buffer.0.storage_mode() != MTLStorageMode::Shared {
                buffer.0.did_modify_range(NSRange {
                    location: range.start as NSUInteger,
                    length: (range.end - range.start) as NSUInteger,
                });
            }
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        unsafe { n::Semaphore(n::dispatch_semaphore_create(1)) } // Returns retained
    }

    fn create_descriptor_pool(
        &self, _max_sets: usize, descriptor_ranges: &[pso::DescriptorRangeDesc]
    ) -> n::DescriptorPool {
        if !self.private_caps.argument_buffers {
            return n::DescriptorPool::Emulated;
        }

        let mut num_samplers = 0;
        let mut num_textures = 0;

        let arguments = descriptor_ranges.iter().map(|desc| {
            let offset_ref = match desc.ty {
                DescriptorType::Sampler => &mut num_samplers,
                DescriptorType::SampledImage => &mut num_textures,
                _ => unimplemented!()
            };
            let index = *offset_ref;
            *offset_ref += desc.count;
            Self::describe_argument(desc.ty, index, desc.count)
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

    fn create_descriptor_set_layout(
        &self, bindings: &[DescriptorSetLayoutBinding]
    ) -> n::DescriptorSetLayout {
        if !self.private_caps.argument_buffers {
            return n::DescriptorSetLayout::Emulated(bindings.to_vec())
        }

        let mut stage_flags = pso::ShaderStageFlags::empty();
        let arguments = bindings.iter().map(|desc| {
            stage_flags |= desc.stage_flags;
            Self::describe_argument(desc.ty, desc.binding, desc.count)
        }).collect::<Vec<_>>();
        let arg_array = metal::Array::from_owned_slice(&arguments);
        let encoder = self.device.new_argument_encoder(&arg_array);

        n::DescriptorSetLayout::ArgumentBuffer(encoder, stage_flags)
    }

    fn update_descriptor_sets(&self, writes: &[DescriptorSetWrite<Backend>]) {
        use hal::pso::DescriptorWrite::*;

        let mut mtl_samplers = Vec::new();
        let mut mtl_textures = Vec::new();

        for write in writes {
            match *write.set {
                n::DescriptorSet::Emulated(ref inner) => {
                    let mut set = inner.lock().unwrap();

                    // Find layout entry
                    let layout = set.layout.iter()
                        .find(|layout| layout.binding == write.binding)
                        .expect("invalid descriptor set binding index")
                        .clone();

                    match (&write.write, set.bindings.get_mut(&write.binding)) {
                        (&Sampler(ref samplers), Some(&mut n::DescriptorSetBinding::Sampler(ref mut vec))) => {
                            if write.array_offset + samplers.len() > layout.count {
                                panic!("out of range descriptor write");
                            }

                            let target_iter = vec[write.array_offset..(write.array_offset + samplers.len())].iter_mut();

                            for (new, old) in samplers.iter().zip(target_iter) {
                                *old = Some(new.0.clone());
                            }
                        },
                        (&SampledImage(ref images), Some(&mut n::DescriptorSetBinding::SampledImage(ref mut vec))) => {
                            if write.array_offset + images.len() > layout.count {
                                panic!("out of range descriptor write");
                            }

                            let target_iter = vec[write.array_offset..(write.array_offset + images.len())].iter_mut();

                            for (new, old) in images.iter().zip(target_iter) {
                                *old = Some(((new.0).0.clone(), new.1));
                            }
                        },
                        (&Sampler(_), _) | (&SampledImage(_), _) => panic!("mismatched descriptor set type"),
                        _ => unimplemented!(),
                    }
                }
                n::DescriptorSet::ArgumentBuffer { ref buffer, offset, ref encoder, .. } => {
                    debug_assert!(self.private_caps.argument_buffers);

                    encoder.set_argument_buffer(buffer, offset);
                    //TODO: range checks, need to keep some layout metadata around
                    assert_eq!(write.array_offset, 0); //TODO

                    match write.write {
                        Sampler(ref samplers) => {
                            mtl_samplers.clear();
                            mtl_samplers.extend(samplers.iter().map(|sampler| &*sampler.0));
                            encoder.set_sampler_states(&mtl_samplers, write.binding as _);
                        },
                        SampledImage(ref images) => {
                            mtl_textures.clear();
                            mtl_textures.extend(images.iter().map(|image| &*((image.0).0)));
                            encoder.set_textures(&mtl_textures, write.binding as _);
                        },
                        _ => unimplemented!(),
                    }
                }
            }
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

    fn destroy_renderpass(&self, _pass: n::RenderPass) {
    }

    fn destroy_graphics_pipeline(&self, _pipeline: n::GraphicsPipeline) {
    }

    fn destroy_compute_pipeline(&self, _pipeline: n::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&self, _buffer: n::FrameBuffer) {
    }

    fn destroy_semaphore(&self, semaphore: n::Semaphore) {
        unsafe { n::dispatch_release(semaphore.0) }
    }

    fn allocate_memory(&self, memory_type: &hal::MemoryType, size: u64) -> Result<n::Memory, OutOfMemory> {
        let (storage, cache) = map_memory_properties_to_storage_and_cache(memory_type.properties);

        // Heaps cannot be used for CPU coherent resources
        //TEMP: MacOS supports Private only, iOS and tvOS can do private/shared
        if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared && false {
            let descriptor = metal::HeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            Ok(n::Memory::Native(self.device.new_heap(&descriptor)))
        } else {
            Ok(n::Memory::Emulated { memory_type: *memory_type, size })
        }
    }

    fn free_memory(&self, _memory: n::Memory) {
    }

    fn create_buffer(
        &self, size: u64, _stride: u64, _usage: buffer::Usage
    ) -> Result<n::UnboundBuffer, buffer::CreationError> {
        Ok(n::UnboundBuffer {
            size
        })
    }

    fn get_buffer_requirements(&self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        let mut max_size = buffer.size;
        let mut max_alignment = 1;

        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the buffer with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            for &options in [
                MTLResourceOptions::StorageModeManaged,
                MTLResourceOptions::StorageModeManaged | MTLResourceOptions::CPUCacheModeWriteCombined,
                MTLResourceOptions::StorageModePrivate,
            ].iter() {
                let requirements = self.device.heap_buffer_size_and_align(buffer.size, options);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
        }

        memory::Requirements {
            size: max_size,
            alignment: max_alignment,
            type_mask: 0x1F, //TODO
        }
    }

    fn bind_buffer_memory(
        &self, memory: &n::Memory, _offset: u64, buffer: n::UnboundBuffer
    ) -> Result<n::Buffer, BindError> {
        Ok(n::Buffer(match *memory {
            n::Memory::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                heap.new_buffer(buffer.size, resource_options)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.device.new_buffer(buffer.size, resource_options)
                    })
            }
            n::Memory::Emulated { ref memory_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_memory_properties_to_options(memory_type.properties);
                self.device.new_buffer(buffer.size, resource_options)
            }
        }))
    }

    fn destroy_buffer(&self, _buffer: n::Buffer) {
    }

    fn create_buffer_view(
        &self, _buffer: &n::Buffer, _format: format::Format, _range: Range<u64>
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn destroy_buffer_view(&self, _view: n::BufferView) {
        unimplemented!()
    }

    fn create_image(
        &self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<n::UnboundImage, image::CreationError>
    {
        let (mtl_format, _) = map_format(format).ok_or(image::CreationError::Format(format.0, Some(format.1)))?;

        let descriptor = metal::TextureDescriptor::new();

        match kind {
            image::Kind::D2(width, height, _aa) => {
                descriptor.set_texture_type(MTLTextureType::D2);
                descriptor.set_width(width as u64);
                descriptor.set_height(height as u64);
            },
            _ => unimplemented!(),
        }

        descriptor.set_mipmap_level_count(mip_levels as u64);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_usage(map_texture_usage(usage));

        Ok(n::UnboundImage(descriptor))
    }

    fn get_image_requirements(&self, image: &n::UnboundImage) -> memory::Requirements {
        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the image with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            let mut max_size = 0;
            let mut max_alignment = 0;
            for &options in [
                MTLResourceOptions::StorageModeManaged,
                MTLResourceOptions::StorageModeManaged | MTLResourceOptions::CPUCacheModeWriteCombined,
                MTLResourceOptions::StorageModePrivate,
            ].iter() {
                image.0.set_resource_options(options);
                let requirements = self.device.heap_texture_size_and_align(&image.0);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
            memory::Requirements {
                size: max_size,
                alignment: max_alignment,
                type_mask: 0x1F, //TODO
            }
        } else {
            memory::Requirements {
                size: 1, // TODO: something sensible
                alignment: 4,
                type_mask: 0x1F, //TODO
            }
        }
    }

    fn bind_image_memory(
        &self, memory: &n::Memory, _offset: u64, image: n::UnboundImage
    ) -> Result<n::Image, BindError> {
        Ok(n::Image(match *memory {
            n::Memory::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.0.set_resource_options(resource_options);
                heap.new_texture(&image.0)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.device.new_texture(&image.0)
                    })
            },
            n::Memory::Emulated { ref memory_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_memory_properties_to_options(memory_type.properties);
                image.0.set_resource_options(resource_options);
                self.device.new_texture(&image.0)
            }
        }))
    }

    fn destroy_image(&self, _image: n::Image) {
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        format: format::Format,
        _swizzle: format::Swizzle,
        _range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        // TODO: subresource range

        let (mtl_format, _) = match map_format(format) {
            Some(f) => f,
            None => {
                error!("failed to find corresponding Metal format for {:?}", format);
                return Err(image::ViewError::BadFormat);
            },
        };

        Ok(n::ImageView(image.0.new_texture_view(mtl_format)))
    }

    fn destroy_image_view(&self, _view: n::ImageView) {
    }

    // Emulated fence implementations
    #[cfg(not(feature = "native_fence"))]
    fn create_fence(&self, signaled: bool) -> n::Fence {
        n::Fence(Arc::new(Mutex::new(signaled)))
    }
    fn reset_fences(&self, fences: &[&n::Fence]) {
        for fence in fences {
            *fence.0.lock().unwrap() = false;
        }
    }
    fn wait_for_fences(&self, fences: &[&n::Fence], wait: WaitFor, mut timeout_ms: u32) -> bool {
        use std::{thread, time};
        let tick = 1;
        loop {
            let done = match wait {
                WaitFor::Any => fences.iter().any(|fence| *fence.0.lock().unwrap()),
                WaitFor::All => fences.iter().all(|fence| *fence.0.lock().unwrap()),
            };
            if done {
                return true
            }
            if timeout_ms < tick {
                return false
            }
            timeout_ms -= tick;
            thread::sleep(time::Duration::from_millis(tick as u64));
        }
    }
    #[cfg(not(feature = "native_fence"))]
    fn destroy_fence(&self, _fence: n::Fence) {
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send+Sync>() {}
    foo::<Device>()
}
