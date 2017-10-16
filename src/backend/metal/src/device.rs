use {Backend};
use {native as n, command};
use conversions::*;

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{cmp, mem, ptr, slice};

use core::{self,
        image, pass, format, mapping, memory, buffer, pso};
use core::device::{WaitFor, BindError, OutOfMemory, FramebufferError, ShaderError, Extent};
use core::pso::{DescriptorSetWrite, DescriptorType, DescriptorSetLayoutBinding, AttributeDesc};
use core::pass::{Subpass};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::*;
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

pub struct Adapter {
    pub(crate) device: MTLDevice,
    pub(crate) adapter_info: core::AdapterInfo,
    pub(crate) queue_families: [(n::QueueFamily, core::QueueType); 1],
}

impl Adapter {
    fn supports_any(&self, features_sets: &[MTLFeatureSet]) -> bool {
        features_sets.iter().cloned().any(|x| self.device.supports_feature_set(x))
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe { self.device.release(); }
    }
}

#[derive(Clone, Copy)]
struct PrivateCapabilities {
    resource_heaps: bool,
    argument_buffers: bool,
    max_buffers_per_stage: usize,
    max_textures_per_stage: usize,
    max_samplers_per_stage: usize,
}

pub struct Device {
    device: MTLDevice,
    private_caps: PrivateCapabilities,
    limits: core::Limits,
}
unsafe impl Send for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.device.release();
        }
    }
}

impl Clone for Device {
    fn clone(&self) -> Device {
        unsafe { self.device.retain(); }
        Device { device: self.device, private_caps: self.private_caps, limits: self.limits }
    }
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&n::QueueFamily, core::QueueType, u32)]) -> core::Gpu<Backend> {
        let mut general_queues = Vec::new();
        let mut graphics_queues = Vec::new();
        let mut compute_queues = Vec::new();
        let mut transfer_queues = Vec::new();

        for &(_, queue_type, count) in queue_descs {
            assert_eq!(count, 1);
            match queue_type {
                core::QueueType::General => general_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                core::QueueType::Graphics => graphics_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                core::QueueType::Compute => compute_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                core::QueueType::Transfer => transfer_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
            }
        }
        assert!(queue_descs.len() == 1, "Metal only supports one queue family");

        let is_mac = self.device.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v1);

        let private_caps = PrivateCapabilities {
            resource_heaps: self.supports_any(RESOURCE_HEAP_SUPPORT),
            argument_buffers: self.supports_any(ARGUMENT_BUFFER_SUPPORT) && false, //TODO
            max_buffers_per_stage: 31,
            max_textures_per_stage: if is_mac {128} else {31},
            max_samplers_per_stage: 31,
        };

        unsafe { self.device.retain(); }
        let device = Device {
            device: self.device,
            private_caps,
            limits: core::Limits {
                max_texture_size: 4096, // TODO: feature set
                max_patch_size: 0, // No tesselation
                max_viewports: 1,

                min_buffer_copy_offset_alignment: if is_mac {256} else {64},
                min_buffer_copy_pitch_alignment: 4, // TODO: made this up
                min_uniform_buffer_offset_alignment: 1, // TODO

                max_compute_group_count: [0; 3], // TODO
                max_compute_group_size: [0; 3], // TODO
            },
        };

        let memory_types = vec![
            core::MemoryType {
                id: 0,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::MemoryType {
                id: 1,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::MemoryType {
                id: 2,
                properties: memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::MemoryType {
                id: 3,
                properties: memory::DEVICE_LOCAL,
                heap_index: 1,
            },
        ];
        let memory_heaps = vec![!0, !0]; //TODO

        core::Gpu {
            device,
            general_queues,
            graphics_queues,
            compute_queues,
            transfer_queues,
            memory_types,
            memory_heaps,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.adapter_info
    }

    fn get_queue_families(&self) -> &[(n::QueueFamily, core::QueueType)] {
        &self.queue_families
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
        &mut self, _path: P,
    ) -> Result<n::ShaderModule, ShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &mut self, source: S, version: LanguageVersion,
    ) -> Result<n::ShaderModule, ShaderError> where S: AsRef<str> {
        let options = MTLCompileOptions::new();
        options.set_language_version(match version {
            LanguageVersion { major: 1, minor: 0 } => MTLLanguageVersion::V1_0,
            LanguageVersion { major: 1, minor: 1 } => MTLLanguageVersion::V1_1,
            LanguageVersion { major: 1, minor: 2 } => MTLLanguageVersion::V1_2,
            LanguageVersion { major: 2, minor: 0 } => MTLLanguageVersion::V2_0,
            _ => return Err(ShaderError::CompilationFailed("shader model not supported".into()))
        });
        match self.device.new_library_with_source(source.as_ref(), options) { // Returns retained
            Ok(lib) => Ok(n::ShaderModule::Compiled(lib)),
            Err(err) => Err(ShaderError::CompilationFailed(err.into())),
        }
    }

    fn compile_shader_library(
        &mut self,
        raw_data: &[u8],
        overrides: &HashMap<msl::ResourceBindingLocation, msl::ResourceBinding>,
    ) -> Result<MTLLibrary, ShaderError> {
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

        let options = MTLCompileOptions::new();
        options.set_language_version(MTLLanguageVersion::V1_1);
        self.device
            .new_library_with_source(shader_code.as_ref(), options) // Returns retained
            .map_err(|err| ShaderError::CompilationFailed(err.into()))
    }

    fn describe_argument(ty: DescriptorType, index: usize, count: usize) -> MTLArgumentDescriptor {
        let arg = MTLArgumentDescriptor::new();
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
        &mut self,
        &(ref shader_set, pipeline_layout, ref pass_descriptor, pipeline_desc):
        &(pso::GraphicsShaderSet<'a, Backend>, &n::PipelineLayout, Subpass<'a, Backend>, &pso::GraphicsPipelineDesc),
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        let pipeline =  MTLRenderPipelineDescriptor::alloc().init(); // Returns retained

        // FIXME: lots missing

        let (primitive_class, primitive_type) = match pipeline_desc.input_assembler.primitive {
            core::Primitive::PointList => (MTLPrimitiveTopologyClass::Point, MTLPrimitiveType::Point),
            core::Primitive::LineList => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::Line),
            core::Primitive::LineStrip => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::LineStrip),
            core::Primitive::TriangleList => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::Triangle),
            core::Primitive::TriangleStrip => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::TriangleStrip),
            _ => (MTLPrimitiveTopologyClass::Unspecified, MTLPrimitiveType::Point) //TODO: double-check
        };
        pipeline.set_input_primitive_topology(primitive_class);

        // Shaders
        let vs_lib = match shader_set.vertex.module {
            &n::ShaderModule::Compiled(lib) => lib,
            &n::ShaderModule::Raw(ref data) => {
                //TODO: cache them all somewhere!
                self.compile_shader_library(data, &pipeline_layout.res_overrides).unwrap()
            }
        };
        let mtl_vertex_function = vs_lib
            .get_function(shader_set.vertex.entry); // Returns retained
        if mtl_vertex_function.is_null() {
            error!("invalid vertex shader entry point");
            return Err(pso::CreationError::Other);
        }
        pipeline.set_vertex_function(mtl_vertex_function);
        unsafe { mtl_vertex_function.release() };
        let fs_lib = if let Some(fragment_entry) = shader_set.fragment {
            let fs_lib = match fragment_entry.module {
                &n::ShaderModule::Compiled(lib) => lib,
                &n::ShaderModule::Raw(ref data) => {
                    self.compile_shader_library(data, &pipeline_layout.res_overrides).unwrap()
                }
            };
            let mtl_fragment_function = fs_lib
                .get_function(fragment_entry.entry); // Returns retained
            if mtl_fragment_function.is_null() {
                error!("invalid pixel shader entry point");
                return Err(pso::CreationError::Other);
            }
            pipeline.set_fragment_function(mtl_fragment_function);
            unsafe { mtl_fragment_function.release() };
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
            let descriptor = pipeline.color_attachments().object_at(i);

            let (mtl_format, is_depth) = map_format(attachment.format).expect("unsupported color format for Metal");
            if is_depth {
                continue;
            }

            descriptor.set_pixel_format(mtl_format);
        }

        // Blending
        for (i, color_desc) in pipeline_desc.blender.targets.iter().enumerate() {
            let descriptor = pipeline.color_attachments().object_at(i);

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
        let vertex_descriptor = MTLVertexDescriptor::new();
        for (i, vertex_buffer) in pipeline_desc.vertex_buffers.iter().enumerate() {
            let mtl_buffer_desc = vertex_descriptor.layouts().object_at(i);
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

            let mtl_attribute_desc = vertex_descriptor.attributes().object_at(i);
            mtl_attribute_desc.set_buffer_index(binding as NSUInteger); // TODO: Might be binding, not location?
            mtl_attribute_desc.set_offset(element.offset as NSUInteger);
            mtl_attribute_desc.set_format(mtl_vertex_format);
        }

        pipeline.set_vertex_descriptor(vertex_descriptor);

        let mut err_ptr: *mut ObjcObject = ptr::null_mut();
        let pso: MTLRenderPipelineState = unsafe {
            msg_send![self.device.0, newRenderPipelineStateWithDescriptor:pipeline.0 error: &mut err_ptr]
        };
        unsafe {
            pipeline.release();
            vertex_descriptor.release();
        }

        if pso.is_null() {
            error!("PSO creation failed: {}", unsafe { n::objc_err_description(err_ptr) });
            unsafe { msg_send![err_ptr, release] };
            Err(pso::CreationError::Other)
        } else {
            Ok(n::GraphicsPipeline {
                vs_lib,
                fs_lib,
                raw: pso,
                primitive_type,
            })
        }
    }
}

impl core::Device<Backend> for Device {
    fn get_features(&self) -> &core::Features {
        unimplemented!()
    }

    fn get_limits(&self) -> &core::Limits {
        &self.limits
    }

    fn create_renderpass(
        &mut self,
        attachments: &[pass::Attachment],
        _subpasses: &[pass::SubpassDesc],
        _dependencies: &[pass::SubpassDependency],
    ) -> n::RenderPass {
        //TODO: subpasses, dependencies
        unsafe {
            let pass = MTLRenderPassDescriptor::new(); // Returns retained
            defer_on_unwind! { pass.release() };

            let mut color_attachment_index = 0;
            //let mut depth_attachment_index = 0;
            for attachment in attachments {
                let (_format, is_depth) = map_format(attachment.format).expect("unsupported attachment format");

                let mtl_attachment: MTLRenderPassAttachmentDescriptor;
                if !is_depth {
                    let color_attachment = pass.color_attachments().object_at(color_attachment_index);
                    color_attachment_index += 1;

                    mtl_attachment = mem::transmute(color_attachment);
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
    }

    fn create_pipeline_layout(&mut self, set_layouts: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        use core::pso::{STAGE_VERTEX, STAGE_FRAGMENT};

        struct Counters {
            buffers: usize,
            textures: usize,
            samplers: usize,
        }
        let mut stage_infos = [
            (STAGE_VERTEX,   spirv::ExecutionModel::Vertex,   Counters { buffers:0, textures:0, samplers:0 }),
            (STAGE_FRAGMENT, spirv::ExecutionModel::Fragment, Counters { buffers:0, textures:0, samplers:0 }),
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
        &mut self,
        params: &[(pso::GraphicsShaderSet<'a, Backend>, &n::PipelineLayout, Subpass<'a, Backend>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        let mut output = Vec::with_capacity(params.len());
        for param in params {
            output.push(self.create_graphics_pipeline(param));
        }
        output
    }

    fn create_compute_pipelines<'a>(
        &mut self,
        pipelines: &[(pso::EntryPoint<'a, Backend>, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self, renderpass: &n::RenderPass, attachments: &[&n::ImageView], extent: Extent
    ) -> Result<n::FrameBuffer, FramebufferError> {
        let descriptor = unsafe {
            let desc: MTLRenderPassDescriptor = msg_send![renderpass.desc.0, copy]; // Returns retained
            defer_on_unwind! { desc.release() };

            msg_send![desc.0, setRenderTargetArrayLength: extent.depth as usize];

            for (i, attachment) in attachments[..renderpass.num_colors].iter().enumerate() {
                let mtl_attachment = desc.color_attachments().object_at(i);
                mtl_attachment.set_texture(attachment.0);
            }

            assert!(renderpass.num_colors + 1 >= attachments.len(),
                "Metal does not support multiple depth attachments");

            if let Some(attachment) = attachments.get(renderpass.num_colors) {
                let mtl_attachment = desc.depth_attachment();
                mtl_attachment.set_texture(attachment.0);
                // TODO: stencil
            }

            desc
        };

        Ok(n::FrameBuffer(descriptor))
    }

    fn create_shader_module(&mut self, raw_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        //TODO: we can probably at least parse here and save the `Ast`
        let depends_on_pipeline_layout = true; //TODO: !self.private_caps.argument_buffers
        if depends_on_pipeline_layout {
            Ok(n::ShaderModule::Raw(raw_data.to_vec()))
        } else {
            self.compile_shader_library(raw_data, &HashMap::new())
                .map(n::ShaderModule::Compiled)
        }
    }

    fn create_sampler(&mut self, info: image::SamplerInfo) -> n::Sampler {
        unsafe {
            let descriptor = MTLSamplerDescriptor::new(); // Returns retained
            defer! { descriptor.release() };

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

            n::Sampler(self.device.new_sampler(descriptor))
        }
    }

    fn destroy_sampler(&mut self, sampler: n::Sampler) {
        unsafe { sampler.0.release(); }
    }

    fn acquire_mapping_raw(
        &mut self, buf: &n::Buffer, read: Option<Range<u64>>
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

    fn release_mapping_raw(&mut self, buffer: &n::Buffer, wrote: Option<Range<u64>>) {
        if let Some(range) = wrote {
            if buffer.0.storage_mode() != MTLStorageMode::Shared {
                buffer.0.did_modify_range(NSRange {
                    location: range.start as NSUInteger,
                    length: (range.end - range.start) as NSUInteger,
                });
            }
        }
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
        unsafe { n::Semaphore(n::dispatch_semaphore_create(1)) } // Returns retained
    }

    fn create_descriptor_pool(
        &mut self, max_sets: usize, descriptor_ranges: &[pso::DescriptorRangeDesc]
    ) -> n::DescriptorPool {
        if !self.private_caps.argument_buffers {
            return n::DescriptorPool::Emulated;
        }

        let mut num_samplers = 0;
        let mut num_textures = 0;

        let mut arguments = descriptor_ranges.iter().map(|desc| {
            let mut offset_ref = match desc.ty {
                DescriptorType::Sampler => &mut num_samplers,
                DescriptorType::SampledImage => &mut num_textures,
                _ => unimplemented!()
            };
            let index = *offset_ref;
            *offset_ref += desc.count;
            Self::describe_argument(desc.ty, *offset_ref, desc.count)
        }).collect::<Vec<_>>();

        let arg_array = NSArray::array_with_objects(&arguments);
        let encoder = self.device.new_argument_encoder(arg_array);

        let total_size = encoder.encoded_length();
        unsafe { encoder.release() };
        let buffer = self.device.new_buffer(total_size, MTLResourceOptions::empty());

        n::DescriptorPool::ArgumentBuffer {
            buffer,
            total_size,
            offset: 0,
        }
    }

    fn create_descriptor_set_layout(
        &mut self, bindings: &[DescriptorSetLayoutBinding]
    ) -> n::DescriptorSetLayout {
        if !self.private_caps.argument_buffers {
            return n::DescriptorSetLayout::Emulated(bindings.to_vec())
        }

        let mut stage_flags = pso::ShaderStageFlags::empty();
        let mut arguments = bindings.iter().map(|desc| {
            stage_flags |= desc.stage_flags;
            Self::describe_argument(desc.ty, desc.binding, desc.count)
        }).collect::<Vec<_>>();
        let arg_array = NSArray::array_with_objects(&arguments);
        let encoder = self.device.new_argument_encoder(arg_array);

        n::DescriptorSetLayout::ArgumentBuffer(encoder, stage_flags)
    }

    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<Backend>]) {
        use core::pso::DescriptorWrite::*;

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
                                unsafe {
                                    new.0.retain();
                                    old.release();
                                }
                                *old = new.0;
                            }
                        },
                        (&SampledImage(ref images), Some(&mut n::DescriptorSetBinding::SampledImage(ref mut vec))) => {
                            if write.array_offset + images.len() > layout.count {
                                panic!("out of range descriptor write");
                            }

                            let target_iter = vec[write.array_offset..(write.array_offset + images.len())].iter_mut();

                            for (new, old) in images.iter().zip(target_iter) {
                                unsafe {
                                    (new.0).0.retain();
                                    old.0.release();
                                }
                                *old = ((new.0).0, new.1);
                            }
                        },
                        (&Sampler(_), _) | (&SampledImage(_), _) => panic!("mismatched descriptor set type"),
                        _ => unimplemented!(),
                    }
                }
                n::DescriptorSet::ArgumentBuffer { buffer, offset, ref encoder, stage_flags } => {
                    debug_assert!(self.private_caps.argument_buffers);

                    encoder.set_argument_buffer(buffer, offset);
                    //TODO: range checks, need to keep some layout metadata around
                    assert_eq!(write.array_offset, 0); //TODO

                    match write.write {
                        Sampler(ref samplers) => {
                            mtl_samplers.clear();
                            mtl_samplers.extend(samplers.iter().map(|sampler| sampler.0.clone()));
                            encoder.set_sampler_states(&mtl_samplers, write.binding as _);
                        },
                        SampledImage(ref images) => {
                            mtl_textures.clear();
                            mtl_textures.extend(images.iter().map(|image| image.0.clone().0));
                            encoder.set_textures(&mtl_textures, write.binding as _);
                        },
                        _ => unimplemented!(),
                    }
                }
            }
        }
    }

    fn destroy_descriptor_pool(&mut self, pool: n::DescriptorPool) {
    }

    fn destroy_descriptor_set_layout(&mut self, layout: n::DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&mut self, pipeline_layout: n::PipelineLayout) {
    }

    fn destroy_shader_module(&mut self, module: n::ShaderModule) {
        match module {
            n::ShaderModule::Compiled(lib) => unsafe {
                lib.release();
            },
            n::ShaderModule::Raw(_) => ()
        }
    }

    fn destroy_renderpass(&mut self, pass: n::RenderPass) {
        unsafe { pass.desc.release(); }
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: n::GraphicsPipeline) {
        //TODO: release associated vs/fs libraries
        unsafe { pipeline.raw.release(); }
    }

    fn destroy_compute_pipeline(&mut self, pipeline: n::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, buffer: n::FrameBuffer) {
        unsafe { buffer.0.release(); }
    }

    fn destroy_semaphore(&mut self, semaphore: n::Semaphore) {
        unsafe { n::dispatch_release(semaphore.0) }
    }

    fn allocate_memory(&mut self, memory_type: &core::MemoryType, size: u64) -> Result<n::Memory, OutOfMemory> {
        let (storage, cache) = map_memory_properties_to_storage_and_cache(memory_type.properties);

        // Heaps cannot be used for CPU coherent resources
        //TEMP: MacOS supports Private only, iOS and tvOS can do private/shared
        if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared && false {
            let descriptor = MTLHeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            Ok(n::Memory::Native(self.device.new_heap(descriptor)))
        } else {
            Ok(n::Memory::Emulated { memory_type: *memory_type, size })
        }
    }

    fn free_memory(&mut self, memory: n::Memory) {
        match memory {
            n::Memory::Emulated { .. } => {},
            n::Memory::Native(heap) => unsafe { heap.release(); },
        }
    }

    fn create_buffer(
        &mut self, size: u64, _stride: u64, _usage: buffer::Usage
    ) -> Result<n::UnboundBuffer, buffer::CreationError> {
        Ok(n::UnboundBuffer {
            size
        })
    }

    fn get_buffer_requirements(&mut self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        let mut max_size = buffer.size;
        let mut max_alignment = 1;

        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the buffer with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            for &options in [
                MTLResourceStorageModeManaged,
                MTLResourceStorageModeManaged | MTLResourceCPUCacheModeWriteCombined,
                MTLResourceStorageModePrivate,
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
        &mut self, memory: &n::Memory, offset: u64, buffer: n::UnboundBuffer
    ) -> Result<n::Buffer, BindError> {
        let bound_buffer = match *memory {
            n::Memory::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                heap.new_buffer(buffer.size, resource_options)
            }
            n::Memory::Emulated { ref memory_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_memory_properties_to_options(memory_type.properties);
                self.device.new_buffer(buffer.size, resource_options)
            }
        };
        if !bound_buffer.is_null() {
            Ok(n::Buffer(bound_buffer))
        } else {
            Err(BindError::OutOfBounds)
        }
    }

    fn destroy_buffer(&mut self, buffer: n::Buffer) {
        unsafe { buffer.0.release(); }
    }

    fn create_buffer_view(
        &mut self, _buffer: &n::Buffer, _format: format::Format, _range: Range<u64>
    ) -> Result<n::BufferView, buffer::ViewError> {
        unimplemented!()
    }

    fn destroy_buffer_view(&mut self, _view: n::BufferView) {
        unimplemented!()
    }

    fn create_image(
        &mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<n::UnboundImage, image::CreationError>
    {
        let (mtl_format, _) = map_format(format).ok_or(image::CreationError::Format(format.0, Some(format.1)))?;

        unsafe {
            let descriptor = MTLTextureDescriptor::new(); // Returns retained

            match kind {
                image::Kind::D2(width, height, aa) => {
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
    }

    fn get_image_requirements(&mut self, image: &n::UnboundImage) -> memory::Requirements {
        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the image with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            let mut max_size = 0;
            let mut max_alignment = 0;
            for &options in [
                MTLResourceStorageModeManaged,
                MTLResourceStorageModeManaged | MTLResourceCPUCacheModeWriteCombined,
                MTLResourceStorageModePrivate,
            ].iter() {
                image.0.set_resource_options(options);
                let requirements = self.device.heap_texture_size_and_align(image.0);
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
        &mut self, memory: &n::Memory, offset: u64, image: n::UnboundImage
    ) -> Result<n::Image, BindError> {
        let bound_image = match *memory {
            n::Memory::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.0.set_resource_options(resource_options);
                heap.new_texture(image.0)
            },
            n::Memory::Emulated { ref memory_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_memory_properties_to_options(memory_type.properties);
                image.0.set_resource_options(resource_options);
                self.device.new_texture(image.0)
            }
        };
        unsafe { image.0.release(); }
        if !bound_image.is_null() {
            Ok(n::Image(bound_image))
        } else {
            Err(BindError::OutOfBounds)
        }
    }

    fn destroy_image(&mut self, image: n::Image) {
        unsafe { image.0.release(); }
    }

    fn create_image_view(
        &mut self,
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

        unsafe {
            Ok(n::ImageView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn destroy_image_view(&mut self, view: n::ImageView) {
        unsafe { view.0.release(); }
    }

    // Emulated fence implementations
    #[cfg(not(feature = "native_fence"))]
    fn create_fence(&mut self, signaled: bool) -> n::Fence {
        n::Fence(Arc::new(Mutex::new(signaled)))
    }
    fn reset_fences(&mut self, fences: &[&n::Fence]) {
        for fence in fences {
            *fence.0.lock().unwrap() = false;
        }
    }
    fn wait_for_fences(&mut self, fences: &[&n::Fence], wait: WaitFor, mut timeout_ms: u32) -> bool {
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
    fn destroy_fence(&mut self, _fence: n::Fence) {
    }
}
