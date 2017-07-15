use {Resources};
use native as n;
use conversions::*;

use std::cell::Cell;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{mem, ptr, slice};

use core::{Factory as CoreFactory, SubPass, HeapType,
    factory as f, image, pass, format, mapping, memory, buffer, pso, shade};

use cocoa::foundation::{NSRange, NSUInteger};
use metal::*;
use objc::runtime::Object as ObjcObject;


struct PrivateCapabilities {
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

pub struct Factory {
    device: MTLDevice,
    private_caps: PrivateCapabilities,
}

impl Drop for Factory {
    fn drop(&mut self) {
        unsafe {
            self.device.release();
        }
    }
}

pub fn create_factory(device: MTLDevice) -> Factory {
    unsafe { device.retain(); }
    Factory {
        device,
        private_caps: PrivateCapabilities {
            indirect_arguments: true, //TEMP
        },
    }
}

impl Factory {
    pub fn create_shader_library_from_file<P>(
        &mut self,
        path: P,
    ) -> Result<n::ShaderLib, shade::CreateShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &mut self,
        source: S,
        version: LanguageVersion,
    ) -> Result<n::ShaderLib, shade::CreateShaderError> where S: AsRef<str> {
        let options = MTLCompileOptions::new();
        options.set_language_version(match version {
            LanguageVersion { major: 1, minor: 0 } => MTLLanguageVersion::V1_0,
            LanguageVersion { major: 1, minor: 1 } => MTLLanguageVersion::V1_1,
            LanguageVersion { major: 1, minor: 2 } => MTLLanguageVersion::V1_2,
            LanguageVersion { major: 2, minor: 0 } => MTLLanguageVersion::V2_0,
            _ => return Err(shade::CreateShaderError::ModelNotSupported)
        });
        match self.device.new_library_with_source(source.as_ref(), options) { // Returns retained
            Ok(lib) => Ok(n::ShaderLib(lib)),
            Err(err) => Err(shade::CreateShaderError::CompilationFailed(err.into())),
        }
    }

    fn describe_argument(ty: f::DescriptorType, index: usize, count: usize) -> MTLArgumentDescriptor {
        let arg = MTLArgumentDescriptor::new();
        arg.set_array_length(count as NSUInteger);

        match ty {
            f::DescriptorType::Sampler => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Sampler);
                arg.set_index(index as NSUInteger);
            }
            f::DescriptorType::SampledImage => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Texture);
                arg.set_index(index as NSUInteger);
            }
            _ => unimplemented!()
        }

        arg
    }
}

impl CoreFactory<Resources> for Factory {
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

                    mtl_attachment = mem::transmute(color_attachment);
                } else {
                    unimplemented!()
                }

                mtl_attachment.set_load_action(map_load_operation(attachment.load_op));
                mtl_attachment.set_store_action(map_store_operation(attachment.store_op));
            }

            n::RenderPass(pass)
        }
    }

    fn create_pipeline_layout(&mut self, sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        n::PipelineLayout {}
    }

    fn create_graphics_pipelines<'a>(&mut self, params: &[(&n::ShaderLib, &n::PipelineLayout, SubPass<'a, Resources>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        unsafe {
            params.iter().map(|&(&n::ShaderLib(shader_lib), pipeline_layout, ref pass_descriptor, pipeline_desc)| {
                let pipeline = MTLRenderPipelineDescriptor::alloc().init(); // Returns retained
                defer! { pipeline.release() };

                // FIXME: lots missing

                // Shaders
                let mtl_vertex_function = shader_lib.get_function(pipeline_desc.shader_entries.vertex_shader); // Returns retained
                if mtl_vertex_function.is_null() {
                    error!("invalid vertex shader entry point");
                    return Err(pso::CreationError);
                }
                defer! { mtl_vertex_function.release() };
                pipeline.set_vertex_function(mtl_vertex_function);
                if let Some(fragment_function_name) = pipeline_desc.shader_entries.pixel_shader {
                    let mtl_fragment_function = shader_lib.get_function(fragment_function_name); // Returns retained
                    if mtl_fragment_function.is_null() {
                        error!("invalid pixel shader entry point");
                        return Err(pso::CreationError);
                    }
                    defer! { mtl_fragment_function.release() };
                    pipeline.set_fragment_function(mtl_fragment_function);
                }
                if pipeline_desc.shader_entries.hull_shader.is_some() {
                    error!("Metal tesselation shaders are not supported");
                    return Err(pso::CreationError);
                }
                if pipeline_desc.shader_entries.domain_shader.is_some() {
                    error!("Metal tesselation shaders are not supported");
                    return Err(pso::CreationError);
                }
                if pipeline_desc.shader_entries.geometry_shader.is_some() {
                    error!("Metal geometry shaders are not supported");
                    return Err(pso::CreationError);
                }

                // Color targets
                for (i, &(target_format, color_desc)) in pipeline_desc.color_targets.iter()
                    .filter_map(|x| x.as_ref()).enumerate()
                {
                    let descriptor = pipeline.color_attachments().object_at(i);

                    let (mtl_format, is_depth) = map_format(target_format).expect("unsupported color format for Metal");
                    if is_depth {
                        error!("color targets cannot be bound with a depth format");
                        return Err(pso::CreationError);
                    }

                    descriptor.set_pixel_format(mtl_format);
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
                for (i, &(buffer_index, element)) in pipeline_desc.attributes.iter().enumerate() {
                    let mtl_vertex_format = map_vertex_format(element.format).expect("unsupported vertex format for Metal");

                    let mtl_attribute_desc = vertex_descriptor.attributes().object_at(i);
                    mtl_attribute_desc.set_buffer_index(buffer_index as u64);
                    mtl_attribute_desc.set_offset(element.offset as u64);
                    mtl_attribute_desc.set_format(mtl_vertex_format);
                }

                pipeline.set_vertex_descriptor(vertex_descriptor);

                let mut err_ptr: *mut ObjcObject = ptr::null_mut();
                let pso: MTLRenderPipelineState = msg_send![self.device.0, newRenderPipelineStateWithDescriptor:pipeline.0 error: &mut err_ptr];
                defer! { msg_send![err_ptr, release] };

                if pso.is_null() {
                    error!("PSO creation failed: {}", n::objc_err_description(err_ptr));
                    return Err(pso::CreationError);
                } else {
                    Ok(n::GraphicsPipeline(pso))
                }
            }).collect()
        }
    }

    fn create_compute_pipelines(&mut self, params: &[(&n::ShaderLib, pso::EntryPoint, &n::PipelineLayout)]) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &n::RenderPass,
        color_attachments: &[&n::RenderTargetView], depth_stencil_attachments: &[&n::DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> n::FrameBuffer {
        unsafe {
            let descriptor: MTLRenderPassDescriptor = msg_send![(renderpass.0).0, copy]; // Returns retained
            defer_on_unwind! { descriptor.release() };

            msg_send![descriptor.0, setRenderTargetArrayLength: layers as usize];

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

    fn view_buffer_as_constant(&mut self, buffer: &n::Buffer, offset: usize, size: usize) -> Result<n::ConstantBufferView, f::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &n::Image, format: format::Format) -> Result<n::RenderTargetView, f::TargetViewError> {
        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(n::RenderTargetView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_shader_resource(&mut self, image: &n::Image, format: format::Format) -> Result<n::ShaderResourceView, f::TargetViewError> {
        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(n::ShaderResourceView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_unordered_access(&mut self, image: &n::Image, format: format::Format) -> Result<n::UnorderedAccessView, f::TargetViewError> {
        unimplemented!()
    }

    fn read_mapping<'a, T>(&self, buf: &'a n::Buffer, offset: u64, size: u64)
                               -> Result<mapping::Reader<'a, Resources, T>,
                                         mapping::Error>
        where T: Copy
    {
        unsafe {
            let base_ptr = buf.0.contents() as *mut u8;
            let count = size as usize / mem::size_of::<T>();

            if base_ptr.is_null() {
                panic!("the buffer is GPU private");
            }

            if offset + size > buf.0.length() {
                panic!("offset/size out of range");
            }

            Ok(mapping::Reader::new(
                slice::from_raw_parts(base_ptr.offset(offset as isize) as *mut T, count),
                n::Mapping(n::MappingInner::Read), // TODO
            ))
        }
    }

    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a n::Buffer, offset: u64, size: u64)
                                -> Result<mapping::Writer<'a, Resources, T>,
                                          mapping::Error>
        where T: Copy
    {
        unsafe {
            let base_ptr = buf.0.contents() as *mut u8;
            let count = size as usize / mem::size_of::<T>();

            if base_ptr.is_null() {
                panic!("the buffer is GPU private");
            }

            if offset + size > buf.0.length() {
                panic!("offset/size out of range");
            }

            let nsrange = NSRange {
                location: offset,
                length: size,
            };

            Ok(mapping::Writer::new(
                slice::from_raw_parts_mut(base_ptr.offset(offset as isize) as *mut T, count),
                n::Mapping(n::MappingInner::Write(buf.0, nsrange)), // TODO
            ))
        }
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
        unsafe { n::Semaphore(n::dispatch_semaphore_create(1)) } // Returns retained
    }

    fn create_descriptor_heap(&mut self, ty: f::DescriptorHeapType, size: usize) -> n::DescriptorHeap {
        n::DescriptorHeap {}
    }

    fn create_descriptor_set_pool(&mut self, heap: &n::DescriptorHeap, max_sets: usize, offset: usize,
                                  descriptor_pools: &[f::DescriptorPoolDesc]) -> n::DescriptorSetPool {
        let mut num_samplers = 0;
        let mut num_textures = 0;

        let mut arguments = descriptor_pools.iter().map(|desc| {
            let mut offset_ref = match desc.ty {
                f::DescriptorType::Sampler => &mut num_samplers,
                f::DescriptorType::SampledImage => &mut num_textures,
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

        n::DescriptorSetPool {
            arg_buffer,
            total_size,
            offset: 0,
        }
    }

    #[cfg(feature = "argument_buffer")]
    fn create_descriptor_set_layout(&mut self, bindings: &[f::DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
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
    fn create_descriptor_set_layout(&mut self, bindings: &[f::DescriptorSetLayoutBinding]) -> n::DescriptorSetLayout {
        n::DescriptorSetLayout {
            bindings: bindings.to_vec(),
        }
    }

    #[cfg(feature = "argument_buffer")]
    fn create_descriptor_sets(&mut self, set_pool: &mut n::DescriptorSetPool, layouts: &[&n::DescriptorSetLayout]) -> Vec<n::DescriptorSet> {
        layouts.iter().map(|layout| {
            let offset = set_pool.offset;
            set_pool.offset += layout.encoder.encoded_length();

            n::DescriptorSet {
                buffer: set_pool.arg_buffer.clone(),
                offset,
                encoder: layout.encoder.clone(),
                stage_flags: layout.stage_flags,
            }
        }).collect()
    }

    #[cfg(not(feature = "argument_buffer"))]
    fn create_descriptor_sets(&mut self, set_pool: &mut n::DescriptorSetPool, layouts: &[&n::DescriptorSetLayout]) -> Vec<n::DescriptorSet> {
        layouts.iter().map(|layout| {
            let bindings = layout.bindings.iter().map(|layout| {
                let binding = match layout.ty {
                    f::DescriptorType::Sampler => {
                        n::DescriptorSetBinding::Sampler((0..layout.count).map(|_| MTLSamplerState::nil()).collect())
                    },
                    f::DescriptorType::SampledImage => {
                        n::DescriptorSetBinding::SampledImage((0..layout.count).map(|_| (MTLTexture::nil(), memory::ImageLayout::General)).collect())
                    },
                    _ => unimplemented!(),
                };
                (layout.binding, binding)
            }).collect();

            let inner = n::DescriptorSetInner {
                layout: layout.bindings.clone(),
                bindings,
            };
            n::DescriptorSet {
                inner: Arc::new(Mutex::new(inner)),
            }
        }).collect()
    }

    #[cfg(feature = "argument_buffer")]
    fn update_descriptor_sets(&mut self, writes: &[f::DescriptorSetWrite<Resources>]) {
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
    fn update_descriptor_sets(&mut self, writes: &[f::DescriptorSetWrite<Resources>]) {
        use core::factory::DescriptorWrite::*;

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

    fn reset_descriptor_set_pool(&mut self, pool: &mut n::DescriptorSetPool) {
    }

    fn destroy_descriptor_heap(&mut self, heap: n::DescriptorHeap) {
    }

    fn destroy_descriptor_set_pool(&mut self, pool: n::DescriptorSetPool) {
    }

    fn destroy_descriptor_set_layout(&mut self, layout: n::DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&mut self, pipeline_layout: n::PipelineLayout) {
    }

    fn destroy_shader_lib(&mut self, lib: n::ShaderLib) {
        unsafe { lib.0.release(); }
    }

    fn destroy_renderpass(&mut self, pass: n::RenderPass) {
        unsafe { pass.0.release(); }
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

    // Emulated heap implementations
    #[cfg(not(feature = "native_heap"))]
    fn create_heap(&mut self, heap_type: &HeapType, size: u64) -> n::Heap {
        n::Heap { heap_type: *heap_type, size }
    }
    #[cfg(not(feature = "native_heap"))]
    fn destroy_heap(&mut self, heap: n::Heap) {
    }

    #[cfg(not(feature = "native_heap"))]
    fn create_buffer(&mut self, size: u64, _stride: u64, _usage: buffer::Usage) -> Result<n::UnboundBuffer, buffer::CreationError> {
        // TODO: map usage
        Ok(n::UnboundBuffer(self.device.new_buffer(size, MTLResourceOptions::empty())))
    }

    #[cfg(not(feature = "native_heap"))]
    fn get_buffer_requirements(&mut self, buffer: &n::UnboundBuffer) -> memory::MemoryRequirements {
        memory::MemoryRequirements {
            size: buffer.0.length(),
            alignment: 1,
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn bind_buffer_memory(&mut self, heap: &n::Heap, offset: u64, buffer: n::UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        Ok(n::Buffer(buffer.0))
    }

    #[cfg(not(feature = "native_heap"))]
    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<n::UnboundImage, image::CreationError>
    {
        let (mtl_format, _) = map_format(format).expect("unsupported texture format");

        unsafe {
            let descriptor = MTLTextureDescriptor::new(); // Returns retained
            defer! { descriptor.release() }

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
            // TODO: usage

            Ok(n::UnboundImage(self.device.new_texture(descriptor))) // Returns retained
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn get_image_requirements(&mut self, image: &n::UnboundImage) -> memory::MemoryRequirements {
        unsafe {
            memory::MemoryRequirements {
                size: 1, // TODO
                alignment: 1,
            }
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn bind_image_memory(&mut self, heap: &n::Heap, offset: u64, image: n::UnboundImage) -> Result<n::Image, image::CreationError> {
        Ok(n::Image(image.0))
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
    fn wait_for_fences(&mut self, fences: &[&n::Fence], wait: f::WaitFor, mut timeout_ms: u32) -> bool {
        use std::{thread, time};
        let tick = 1;
        loop {
            let done = match wait {
                f::WaitFor::Any => fences.iter().any(|fence| *fence.0.lock().unwrap()),
                f::WaitFor::All => fences.iter().all(|fence| *fence.0.lock().unwrap()),
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
