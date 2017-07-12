use ::Resources;
use ::native::*;
use ::conversions::*;

use std::cell::Cell;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{mem, ptr, slice};

use core::{self, image, pass, format, mapping, memory, buffer, pso, shade};
use core::factory::*;
use core::shade::CreateShaderError;
use metal::*;
use objc::runtime::Object as ObjcObject;

pub struct Factory {
    device: MTLDevice,
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
    }
}

impl Factory {
    pub fn create_shader_library_from_file<P>(
        &mut self,
        path: P,
    ) -> Result<ShaderLib, CreateShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &mut self,
        source: S,
    ) -> Result<ShaderLib, CreateShaderError> where S: AsRef<str> {
        match self.device.new_library_with_source(source.as_ref(), MTLCompileOptions::nil()) { // Returns retained
            Ok(lib) => Ok(ShaderLib(lib)),
            Err(err) => Err(CreateShaderError::CompilationFailed(err.into())),
        }
    }
}

impl core::Factory<Resources> for Factory {
    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> RenderPass {
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

            RenderPass(pass)
        }
    }

    fn create_pipeline_layout(&mut self, sets: &[&DescriptorSetLayout]) -> PipelineLayout {
        PipelineLayout {}
    }

    fn create_graphics_pipelines<'a>(&mut self, params: &[(&ShaderLib, &PipelineLayout, core::SubPass<'a, Resources>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<GraphicsPipeline, pso::CreationError>> {
        unsafe {
            params.iter().map(|&(&ShaderLib(shader_lib), pipeline_layout, ref pass_descriptor, pipeline_desc)| {
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
                    error!("PSO creation failed: {}", objc_err_description(err_ptr));
                    return Err(pso::CreationError);
                } else {
                    Ok(GraphicsPipeline(pso))
                }
            }).collect()
        }
    }

    fn create_compute_pipelines(&mut self, params: &[(&ShaderLib, pso::EntryPoint, &PipelineLayout)]) -> Vec<Result<ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &RenderPass,
        color_attachments: &[&RenderTargetView], depth_stencil_attachments: &[&DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> FrameBuffer {
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

            FrameBuffer(descriptor)
        }
    }

    fn create_sampler(&mut self, info: image::SamplerInfo) -> Sampler {
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

            Sampler(self.device.new_sampler(descriptor))
        }
    }

    fn view_buffer_as_constant(&mut self, buffer: &Buffer, offset: usize, size: usize) -> Result<ConstantBufferView, TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &Image, format: format::Format) -> Result<RenderTargetView, TargetViewError> {
        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(RenderTargetView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_shader_resource(&mut self, image: &Image, format: format::Format) -> Result<ShaderResourceView, TargetViewError> {
        let (mtl_format, _) = map_format(format).ok_or_else(|| {
            error!("failed to find corresponding Metal format for {:?}", format);
            panic!(); // TODO: return TargetViewError once it is implemented
        })?;

        unsafe {
            Ok(ShaderResourceView(image.0.new_texture_view(mtl_format))) // Returns retained
        }
    }

    fn view_image_as_unordered_access(&mut self, image: &Image, format: format::Format) -> Result<UnorderedAccessView, TargetViewError> {
        unimplemented!()
    }

    fn read_mapping<'a, T>(&self, buf: &'a Buffer, offset: u64, size: u64)
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
                Mapping(MappingInner::Read), // TODO
            ))
        }
    }

    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a Buffer, offset: u64, size: u64)
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
                Mapping(MappingInner::Write(buf.0, nsrange)), // TODO
            ))
        }
    }

    fn create_semaphore(&mut self) -> Semaphore {
        unsafe { Semaphore(dispatch_semaphore_create(1)) } // Returns retained
    }

    fn create_descriptor_heap(&mut self, ty: DescriptorHeapType, size: usize) -> DescriptorHeap {
        DescriptorHeap {}
    }

    fn create_descriptor_set_pool(&mut self, heap: &DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[DescriptorPoolDesc]) -> DescriptorSetPool {
        DescriptorSetPool {}
    }

    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> DescriptorSetLayout {
        DescriptorSetLayout(bindings.to_vec())
    }

    fn create_descriptor_sets(&mut self, set_pool: &mut DescriptorSetPool, layout: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        use factory::DescriptorType::*;

        layout.iter().map(|layout| {
            let bindings = layout.0.iter().map(|layout| {
                let binding = match layout.ty {
                    Sampler => {
                        DescriptorSetBinding::Sampler((0..layout.count).map(|_| MTLSamplerState::nil()).collect())
                    },
                    SampledImage => {
                        DescriptorSetBinding::SampledImage((0..layout.count).map(|_| (MTLTexture::nil(), memory::ImageLayout::General)).collect())
                    },
                    _ => unimplemented!(),
                };
                (layout.binding, binding)
            }).collect();

            DescriptorSet(Arc::new(Mutex::new(DescriptorSetInner {
                layout: layout.0.clone(),
                bindings,
            })))
        }).collect()
    }

    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<Resources>]) {
        use factory::DescriptorWrite::*;

        for write in writes.iter() {
            let mut set = write.set.0.lock().unwrap();
            let set: &mut DescriptorSetInner = &mut*set;

            // Find layout entry
            let layout = set.layout.iter().find(|layout| layout.binding == write.binding)
                .expect("invalid descriptor set binding index");

            match write.write {
                Sampler(ref samplers) => {
                    if write.array_offset + samplers.len() > layout.count {
                        panic!("out of range descriptor write");
                    }
                    let target = if let &mut DescriptorSetBinding::Sampler(ref mut vec) = set.bindings.get_mut(&write.binding).unwrap() {
                        vec
                    } else {
                        panic!("mismatched descriptor set type");
                    };

                    let target_range = &mut target[write.array_offset..(write.array_offset + samplers.len())];

                    unsafe {
                        for (new, old) in samplers.iter().zip(target_range.iter_mut()) {
                            old.release();
                            new.0.retain();
                            *old = new.0;
                        }
                    }
                },
                SampledImage(ref images) => {
                    if write.array_offset + images.len() > layout.count {
                        panic!("out of range descriptor write");
                    }
                    let target = if let &mut DescriptorSetBinding::SampledImage(ref mut vec) = set.bindings.get_mut(&write.binding).unwrap() {
                        vec
                    } else {
                        panic!("mismatched descriptor set type");
                    };

                    let target_range = &mut target[write.array_offset..(write.array_offset + images.len())];

                    unsafe {
                        for (new, old) in images.iter().zip(target_range.iter_mut()) {
                            old.0.release();
                            (new.0).0.retain();
                            *old = ((new.0).0, new.1);
                        }
                    }
                },
                _ => unimplemented!(),
            }
        }
    }

    fn reset_descriptor_set_pool(&mut self, pool: &mut DescriptorSetPool) {
    }

    fn destroy_descriptor_heap(&mut self, heap: DescriptorHeap) {
    }

    fn destroy_descriptor_set_pool(&mut self, pool: DescriptorSetPool) {
    }

    fn destroy_descriptor_set_layout(&mut self, layout: DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&mut self, pipeline_layout: PipelineLayout) {
    }

    fn destroy_shader_lib(&mut self, lib: ShaderLib) {
        unsafe { lib.0.release(); }
    }

    fn destroy_renderpass(&mut self, pass: RenderPass) {
        unsafe { pass.0.release(); }
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: GraphicsPipeline) {
        unsafe { pipeline.0.release(); }
    }

    fn destroy_compute_pipeline(&mut self, pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, buffer: FrameBuffer) {
        unsafe { buffer.0.release(); }
    }

    fn destroy_buffer(&mut self, buffer: Buffer) {
        unsafe { buffer.0.release(); }
    }

    fn destroy_image(&mut self, image: Image) {
        unsafe { image.0.release(); }
    }

    fn destroy_render_target_view(&mut self, view: RenderTargetView) {
        unsafe { view.0.release(); }
    }

    fn destroy_depth_stencil_view(&mut self, view: DepthStencilView) {
        unsafe { view.0.release(); }
    }

    fn destroy_constant_buffer_view(&mut self, view: ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, view: ShaderResourceView) {
        unsafe { view.0.release(); }
    }

    fn destroy_unordered_access_view(&mut self, view: UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, sampler: Sampler) {
        unsafe { sampler.0.release(); }
    }

    fn destroy_semaphore(&mut self, semaphore: Semaphore) {
        unsafe { dispatch_release(semaphore.0) }
    }

    // Emulated heap implementations
    #[cfg(not(feature = "native_heap"))]
    fn create_heap(&mut self, heap_type: &core::HeapType, size: u64) -> Heap {
        Heap { heap_type: *heap_type, size }
    }
    #[cfg(not(feature = "native_heap"))]
    fn destroy_heap(&mut self, heap: Heap) {
    }

    #[cfg(not(feature = "native_heap"))]
    fn create_buffer(&mut self, size: u64, _stride: u64, _usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        // TODO: map usage
        Ok(UnboundBuffer(self.device.new_buffer(size, MTLResourceOptions::empty())))
    }

    #[cfg(not(feature = "native_heap"))]
    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> memory::MemoryRequirements {
        memory::MemoryRequirements {
            size: buffer.0.length(),
            alignment: 1,
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn bind_buffer_memory(&mut self, heap: &Heap, offset: u64, buffer: UnboundBuffer) -> Result<Buffer, buffer::CreationError> {
        Ok(Buffer(buffer.0))
    }

    #[cfg(not(feature = "native_heap"))]
    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError>
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

            Ok(UnboundImage(self.device.new_texture(descriptor))) // Returns retained
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn get_image_requirements(&mut self, image: &UnboundImage) -> memory::MemoryRequirements {
        unsafe {
            memory::MemoryRequirements {
                size: 1, // TODO
                alignment: 1,
            }
        }
    }

    #[cfg(not(feature = "native_heap"))]
    fn bind_image_memory(&mut self, heap: &Heap, offset: u64, image: UnboundImage) -> Result<Image, image::CreationError> {
        Ok(Image(image.0))
    }

    // Emulated fence implementations
    #[cfg(not(feature = "native_fence"))]
    fn create_fence(&mut self, signaled: bool) -> Fence {
        Fence(Arc::new(Mutex::new(signaled)))
    }
    fn reset_fences(&mut self, fences: &[&Fence]) {
        for fence in fences {
            *fence.0.lock().unwrap() = false;
        }
    }
    fn wait_for_fences(&mut self, fences: &[&Fence], wait: WaitFor, mut timeout_ms: u32) -> bool {
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
    fn destroy_fence(&mut self, _fence: Fence) {
    }
}
