use {Backend};
use {native as n, command};
use conversions::*;

use std::ops::Range;
use std::cmp;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{mem, ptr, slice};

use core::{self,
        image, pass, format, mapping, memory, buffer, pso};
use core::{Limits, Features, QueueType, Gpu, HeapType};
use core::device::{WaitFor, ResourceHeapError, ResourceHeapType, TargetViewError};
use core::device::{ShaderError, Extent};
use core::pso::{DescriptorSetWrite, DescriptorType, DescriptorSetLayoutBinding, AttributeDesc};
use core::pass::{Subpass};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::*;
use objc::runtime::Object as ObjcObject;

pub struct Adapter {
    pub(crate) device: MTLDevice,
    pub(crate) adapter_info: core::AdapterInfo,
    pub(crate) queue_families: [(n::QueueFamily, QueueType); 1],
}

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe { self.device.release(); }
    }
}

pub struct Device {
    device: MTLDevice,
    private_caps: PrivateCapabilities,
    limits: Limits,
}

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
    fn open(&self, queue_descs: &[(&n::QueueFamily, QueueType, u32)]) -> Gpu<Backend> {
        let mut general_queues = Vec::new();
        let mut graphics_queues = Vec::new();
        let mut compute_queues = Vec::new();
        let mut transfer_queues = Vec::new();

        for &(_, queue_type, count) in queue_descs {
            match queue_type {
                QueueType::General => general_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                QueueType::Graphics => graphics_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                QueueType::Compute => compute_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
                QueueType::Transfer => transfer_queues
                    .push(unsafe { core::CommandQueue::new(command::CommandQueue::new(self.device)) }),
            }
        }

        assert!(queue_descs.len() == 1, "Metal only supports one queue family");
        let (_, _, queue_count) = queue_descs[0];

        let resource_heaps = [
            MTLFeatureSet::iOS_GPUFamily1_v3,
            MTLFeatureSet::iOS_GPUFamily2_v3,
            MTLFeatureSet::iOS_GPUFamily3_v2,
            MTLFeatureSet::iOS_GPUFamily1_v4,
            MTLFeatureSet::iOS_GPUFamily2_v4,
            MTLFeatureSet::iOS_GPUFamily3_v3,

            MTLFeatureSet::tvOS_GPUFamily1_v2,
            MTLFeatureSet::tvOS_GPUFamily1_v3,

            MTLFeatureSet::macOS_GPUFamily1_v3,
        ].iter().cloned().any(|x| self.device.supports_feature_set(x));

        unsafe { self.device.retain(); }
        let device = Device {
            device: self.device,
            private_caps: PrivateCapabilities {
                resource_heaps,
                indirect_arguments: true, //TEMP
            },
            limits: Limits {
                max_texture_size: 4096, // TODO: feature set
                max_patch_size: 0, // No tesselation
                max_viewports: 1,

                min_buffer_copy_offset_alignment: 4, // TODO: Lower on iOS
                min_buffer_copy_pitch_alignment: 4, // TODO: made this up

                max_compute_group_size: (1, 1, 1), // FIXME
            },
        };

        let heap_types = vec![
            core::HeapType {
                id: 0,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::HeapType {
                id: 1,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED | memory::WRITE_COMBINED,
                heap_index: 1,
            },
            core::HeapType {
                id: 2,
                properties: memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                heap_index: 2,
            },
            core::HeapType {
                id: 3,
                properties: memory::CPU_VISIBLE | memory::COHERENT 
                    | memory::CPU_CACHED | memory::WRITE_COMBINED,
                heap_index: 3,
            },
            core::HeapType {
                id: 4,
                properties: memory::DEVICE_LOCAL,
                heap_index: 4,
            },
        ];
        let memory_heaps = Vec::new();

        Gpu {
            device,
            general_queues,
            graphics_queues,
            compute_queues,
            transfer_queues,
            heap_types,
            memory_heaps,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.adapter_info
    }

    fn get_queue_families(&self) -> &[(n::QueueFamily, QueueType)] {
        &self.queue_families
    }
}

#[derive(Clone, Copy)]
struct PrivateCapabilities {
    resource_heaps: bool,
    indirect_arguments: bool,
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
        &mut self,
        path: P,
    ) -> Result<n::ShaderModule, ShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &mut self,
        source: S,
        version: LanguageVersion,
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
            Ok(lib) => Ok(n::ShaderModule(lib)),
            Err(err) => Err(ShaderError::CompilationFailed(err.into())),
        }
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
}

impl core::Device<Backend> for Device {
    fn get_features(&self) -> &Features {
        unimplemented!()
    }

    fn get_limits(&self) -> &Limits {
        &self.limits
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> n::RenderPass {
        unsafe {
            let pass = MTLRenderPassDescriptor::new(); // Returns retained
            defer_on_unwind! { pass.release() };

            let mut color_attachment_index = 0;
            let mut depth_attachment_index = 0;
            for attachment in attachments {
                let (format, is_depth) = map_format(attachment.format).expect("unsupported attachment format");

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

            n::RenderPass { desc: pass, attachments: attachments.into() }
        }
    }

    fn create_pipeline_layout(&mut self, sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        n::PipelineLayout {}
    }

    fn create_graphics_pipelines<'a>(
        &mut self,
        params: &[(pso::GraphicsShaderSet<'a, Backend>, &n::PipelineLayout, Subpass<'a, Backend>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        unsafe {
            params.iter().map(|&(ref shader_set, pipeline_layout, ref pass_descriptor, pipeline_desc)| {
                let pipeline =  MTLRenderPipelineDescriptor::alloc().init(); // Returns retained
                defer! { pipeline.release() };

                // FIXME: lots missing

                // Shaders
                let mtl_vertex_function = (shader_set.vertex.module.0)
                    .get_function(shader_set.vertex.entry); // Returns retained
                if mtl_vertex_function.is_null() {
                    error!("invalid vertex shader entry point");
                    return Err(pso::CreationError::Other);
                }
                defer! { mtl_vertex_function.release() };
                pipeline.set_vertex_function(mtl_vertex_function);
                if let Some(fragment_entry) = shader_set.fragment {
                    let mtl_fragment_function = (fragment_entry.module.0)
                        .get_function(fragment_entry.entry); // Returns retained
                    if mtl_fragment_function.is_null() {
                        error!("invalid pixel shader entry point");
                        return Err(pso::CreationError::Other);
                    }
                    defer! { mtl_fragment_function.release() };
                    pipeline.set_fragment_function(mtl_fragment_function);
                }
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
                defer! { vertex_descriptor.release() };
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
                for (i, &AttributeDesc { location, binding, element, }) in pipeline_desc.attributes.iter().enumerate() {
                    let mtl_vertex_format = map_vertex_format(element.format).expect("unsupported vertex format for Metal");

                    let mtl_attribute_desc = vertex_descriptor.attributes().object_at(i);
                    mtl_attribute_desc.set_buffer_index(binding as NSUInteger); // TODO: Might be binding, not location?
                    mtl_attribute_desc.set_offset(element.offset as NSUInteger);
                    mtl_attribute_desc.set_format(mtl_vertex_format);
                }

                pipeline.set_vertex_descriptor(vertex_descriptor);

                let mut err_ptr: *mut ObjcObject = ptr::null_mut();
                let pso: MTLRenderPipelineState = msg_send![self.device.0, newRenderPipelineStateWithDescriptor:pipeline.0 error: &mut err_ptr];
                defer! { msg_send![err_ptr, release] };

                if pso.is_null() {
                    error!("PSO creation failed: {}", n::objc_err_description(err_ptr));
                    return Err(pso::CreationError::Other);
                } else {
                    Ok(n::GraphicsPipeline(pso))
                }
            }).collect()
        }
    }

    fn create_compute_pipelines<'a>(
        &mut self,
        pipelines: &[(pso::EntryPoint<'a, Backend>, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &n::RenderPass,
        color_attachments: &[&n::RenderTargetView], depth_stencil_attachments: &[&n::DepthStencilView],
        extent: Extent,
    ) -> n::FrameBuffer {
        unsafe {
            let descriptor: MTLRenderPassDescriptor = msg_send![renderpass.desc.0, copy]; // Returns retained
            defer_on_unwind! { descriptor.release() };

            msg_send![descriptor.0, setRenderTargetArrayLength: extent.depth as usize];

            for (i, attachment) in color_attachments.iter().enumerate() {
                let mtl_attachment = descriptor.color_attachments().object_at(i);
                mtl_attachment.set_texture(attachment.0);
            }

            if depth_stencil_attachments.len() > 1 {
                panic!("Metal does not support multiple depth attachments");
            }

            if let Some(attachment) = depth_stencil_attachments.get(0) {
                let mtl_attachment = descriptor.depth_attachment();
                mtl_attachment.set_texture(attachment.0);

                // TODO: stencil
            }

            n::FrameBuffer(descriptor)
        }
    }

    fn create_shader_module(&mut self, spirv_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        unimplemented!()
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

    fn view_buffer_as_constant(&mut self, buffer: &n::Buffer, range: Range<u64>) -> Result<n::ConstantBufferView, TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &n::Image, format: format::Format, range: image::SubresourceRange) 
        -> Result<n::RenderTargetView, TargetViewError>
    {
        // TODO: subresource range

        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(n::RenderTargetView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_shader_resource(&mut self, image: &n::Image, format: format::Format) -> Result<n::ShaderResourceView, TargetViewError> {
        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(n::ShaderResourceView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_unordered_access(&mut self, image: &n::Image, format: format::Format) -> Result<n::UnorderedAccessView, TargetViewError> {
        unimplemented!()
    }

    fn acquire_mapping_raw(&mut self, buf: &n::Buffer, read: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error> {
        unsafe {
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
    }

    fn release_mapping_raw(&mut self, buffer: &n::Buffer, wrote: Option<Range<u64>>) {
        unsafe {
            if let Some(range) = wrote {
                if buffer.0.storage_mode() != MTLStorageMode::Shared {
                    buffer.0.did_modify_range(NSRange {
                        location: range.start as NSUInteger,
                        length: (range.end - range.start) as NSUInteger,
                    });
                }
            }
        }
    }
    
    fn create_semaphore(&mut self) -> n::Semaphore {
        unsafe { n::Semaphore(n::dispatch_semaphore_create(1)) } // Returns retained
    }

    #[cfg(feature = "argument_buffer")]
    fn create_descriptor_pool(&mut self, max_sets: usize, descriptor_ranges: &[pso::DescriptorRangeDesc]) 
        -> n::DescriptorPool
    {
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
        let arg_buffer = self.device.new_buffer(total_size, MTLResourceOptions::empty());

        n::DescriptorPool {
            arg_buffer,
            total_size,
            offset: 0,
        }
    }

    #[cfg(not(feature = "argument_buffer"))]
    fn create_descriptor_pool(&mut self, max_sets: usize, descriptor_ranges: &[pso::DescriptorRangeDesc]) 
        -> n::DescriptorPool
    {
        n::DescriptorPool {}
    }

    #[cfg(feature = "argument_buffer")]
    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        let mut stage_flags = shade::StageFlags::empty();
        let mut arguments = bindings.iter().map(|desc| {
            stage_flags |= desc.stage_flags;
            Self::describe_argument(desc.ty, desc.binding, desc.count)
        }).collect::<Vec<_>>();
        let arg_array = NSArray::array_with_objects(&arguments);
        let encoder = self.device.new_argument_encoder(arg_array);

        n::DescriptorSetLayout {
            encoder,
            stage_flags,
        }
    }

    #[cfg(not(feature = "argument_buffer"))]
    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        n::DescriptorSetLayout {
            bindings: bindings.to_vec(),
        }
    }
    #[cfg(feature = "argument_buffer")]
    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<Resources>]) {
        use core::factory::DescriptorWrite::*;

        let mut mtl_samplers = Vec::new();
        let mut mtl_textures = Vec::new();

        for write in writes {
            write.set.encoder.set_argument_buffer(write.set.buffer, write.set.offset);
            //TODO: range checks, need to keep some layout metadata around

            match write.write {
                Sampler(ref samplers) => {
                    mtl_samplers.clear();
                    mtl_samplers.extend(samplers.iter().map(|sampler| sampler.0.clone()));
                    write.set.encoder.set_sampler_states(&mtl_samplers, write.array_offset as _);
                },
                SampledImage(ref images) => {
                    mtl_textures.clear();
                    mtl_textures.extend(images.iter().map(|image| image.0.clone().0));
                    write.set.encoder.set_textures(&mtl_textures, write.array_offset as _);
                },
                _ => unimplemented!(),
            }
        }
    }

    #[cfg(not(feature = "argument_buffer"))]
    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<Backend>]) {
        use core::pso::DescriptorWrite::*;

        for write in writes {
            let n::DescriptorSetInner { ref mut bindings, layout: ref set_layout } = *write.set.inner.lock().unwrap();

            // Find layout entry
            let layout = set_layout.iter().find(|layout| layout.binding == write.binding)
                .expect("invalid descriptor set binding index");

            match (&write.write, bindings.get_mut(&write.binding)) {
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
    }

    fn destroy_descriptor_pool(&mut self, pool: n::DescriptorPool) {
    }

    fn destroy_descriptor_set_layout(&mut self, layout: n::DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&mut self, pipeline_layout: n::PipelineLayout) {
    }

    fn destroy_shader_module(&mut self, module: n::ShaderModule) {
        unsafe { module.0.release(); }
    }

    fn destroy_renderpass(&mut self, pass: n::RenderPass) {
        unsafe { pass.desc.release(); }
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: n::GraphicsPipeline) {
        unsafe { pipeline.0.release(); }
    }

    fn destroy_compute_pipeline(&mut self, pipeline: n::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, buffer: n::FrameBuffer) {
        unsafe { buffer.0.release(); }
    }

    fn destroy_buffer(&mut self, buffer: n::Buffer) {
        unsafe { buffer.0.release(); }
    }

    fn destroy_image(&mut self, image: n::Image) {
        unsafe { image.0.release(); }
    }

    fn destroy_render_target_view(&mut self, view: n::RenderTargetView) {
        unsafe { view.0.release(); }
    }

    fn destroy_depth_stencil_view(&mut self, view: n::DepthStencilView) {
        unsafe { view.0.release(); }
    }

    fn destroy_constant_buffer_view(&mut self, view: n::ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, view: n::ShaderResourceView) {
        unsafe { view.0.release(); }
    }

    fn destroy_unordered_access_view(&mut self, view: n::UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, sampler: n::Sampler) {
        unsafe { sampler.0.release(); }
    }

    fn destroy_semaphore(&mut self, semaphore: n::Semaphore) {
        unsafe { n::dispatch_release(semaphore.0) }
    }

    fn create_heap(&mut self, heap_type: &HeapType, _resource_type: ResourceHeapType, size: u64) -> Result<n::Heap, ResourceHeapError> {
        let (storage, cache) = map_heap_properties_to_storage_and_cache(heap_type.properties);

        // Heaps cannot be used for CPU coherent resources
        if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared {
            let descriptor = MTLHeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            Ok(n::Heap::Native(self.device.new_heap(descriptor)))
        } else {
            Ok(n::Heap::Emulated { heap_type: *heap_type, size })
        }
    }

    fn destroy_heap(&mut self, heap: n::Heap) {
        match heap {
            n::Heap::Emulated { .. } => {},
            n::Heap::Native(heap) => unsafe { heap.release(); },
        }
    }

    fn create_buffer(&mut self, size: u64, _stride: u64, _usage: buffer::Usage) -> Result<n::UnboundBuffer, buffer::CreationError> {
        Ok(n::UnboundBuffer {
            size
        })
    }

    fn get_buffer_requirements(&mut self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        // We don't know what memory type the user will try to allocate the buffer with, so we test them
        // all get the most stringent ones. Note we don't check Shared because heaps can't use it
        let mut max_size = 0;
        let mut max_alignment = 0;
        for &options in [
            MTLResourceStorageModeManaged,
            MTLResourceStorageModeManaged | MTLResourceCPUCacheModeWriteCombined,
            MTLResourceStorageModePrivate,
        ].iter() {
            let requirements = self.device.heap_buffer_size_and_align(buffer.size, options);
            max_size = cmp::max(max_size, requirements.size);
            max_alignment = cmp::max(max_alignment, requirements.align);
        }
        memory::Requirements {
            size: max_size,
            alignment: max_alignment,
        }
    }

    fn bind_buffer_memory(&mut self, heap: &n::Heap, offset: u64, buffer: n::UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        let bound_buffer = match *heap {
            n::Heap::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                heap.new_buffer(buffer.size, resource_options)
            }
            n::Heap::Emulated { ref heap_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_heap_properties_to_options(heap_type.properties);
                self.device.new_buffer(buffer.size, resource_options)
            }
        };
        if !bound_buffer.is_null() {
            Ok(n::Buffer(bound_buffer))
        } else {
            Err(buffer::CreationError)
        }
    }

    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
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
            }
        } else {
            memory::Requirements {
                size: 1, // TODO: something sensible
                alignment: 4,
            }
        }
    }

    fn bind_image_memory(&mut self, heap: &n::Heap, offset: u64, image: n::UnboundImage) -> Result<n::Image, image::CreationError> {
        let bound_image = match *heap {
            n::Heap::Native(ref heap) => {
                let resource_options = resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.0.set_resource_options(resource_options);
                heap.new_texture(image.0)
            },
            n::Heap::Emulated { ref heap_type, size: _ } => {
                // TODO: disable hazard tracking?
                let resource_options = map_heap_properties_to_options(heap_type.properties);
                image.0.set_resource_options(resource_options);
                self.device.new_texture(image.0)
            }
        };
        unsafe { image.0.release(); }
        if !bound_image.is_null() {
            Ok(n::Image(bound_image))
        } else {
            Err(image::CreationError::OutOfHeap)
        }
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
