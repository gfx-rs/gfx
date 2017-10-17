use {Backend};
use native;

use std::ops::{Deref, Range};
use std::sync::{Arc};
use std::cell::UnsafeCell;

use core::{self, memory, target, pool, pso};
use core::{VertexCount, VertexOffset, InstanceCount, IndexCount, Viewport};
use core::{RawSubmission};
use core::buffer::{IndexBufferView};
use core::image::{ImageLayout, SubresourceRange};
use core::command::{AttachmentClear, ClearColor, ClearDepthStencil, ClearValue, BufferImageCopy, BufferCopy};
use core::command::{ImageCopy, SubpassContents};
use core::command::{ImageResolve};

use metal::*;
use cocoa::foundation::NSUInteger;
use block::{ConcreteBlock};


pub struct CommandQueue(Arc<QueueInner>);

struct QueueInner {
    queue: MTLCommandQueue,
}

unsafe impl Send for QueueInner {}
unsafe impl Sync for QueueInner {}

impl Drop for QueueInner {
    fn drop(&mut self) {
        unsafe {
            self.queue.release();
        }
    }
}

pub struct CommandPool {
    queue: Arc<QueueInner>,
    managed: Option<Vec<CommandBuffer>>,
}

unsafe impl Send for CommandPool {
}
unsafe impl Sync for CommandPool {
}

#[derive(Clone)]
pub struct CommandBuffer {
    inner: Arc<UnsafeCell<CommandBufferInner>>,
    queue: Option<Arc<QueueInner>>,
}

#[derive(Debug)]
struct StageResources {
    buffers: Vec<Option<(MTLBuffer, pso::BufferOffset)>>,
    textures: Vec<Option<MTLTexture>>,
    samplers: Vec<Option<MTLSamplerState>>,
}

impl StageResources {
    fn new() -> Self {
        StageResources {
            buffers: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buffers.clear();
        self.textures.clear();
        self.samplers.clear();
    }

    fn add_buffer(&mut self, slot: usize, buffer: MTLBuffer, offset: usize) {
        while self.buffers.len() <= slot {
            self.buffers.push(None)
        }
        self.buffers[slot] = Some((buffer, offset));
    }

    fn add_textures(&mut self, start: usize, textures: &[(MTLTexture, ImageLayout)]) {
        while self.textures.len() < start + textures.len() {
            self.textures.push(None)
        }
        for (out, &(texture, _)) in self.textures[start..].iter_mut().zip(textures.iter()) {
            *out = Some(texture);
        }
    }

    fn add_samplers(&mut self, start: usize, samplers: &[MTLSamplerState]) {
        while self.samplers.len() < start + samplers.len() {
            self.samplers.push(None)
        }
        for (out, sampler) in self.samplers[start..].iter_mut().zip(samplers.iter()) {
            *out = Some(*sampler);
        }
    }
}

struct CommandBufferInner {
    command_buffer: MTLCommandBuffer,
    //TODO: would be cleaner to move the cache into `CommandBuffer` iself
    // it doesn't have to be in `Inner`
    encoder_state: EncoderState,
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    pipeline_state: Option<MTLRenderPipelineState>, // Unretained
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
}

impl CommandBufferInner {
    fn reset(&mut self, queue: &QueueInner) {
        let old = self.command_buffer;
        self.command_buffer = MTLCommandBuffer::nil();
        unsafe { old.release(); }
        self.command_buffer = queue.queue.new_command_buffer();

        self.resources_vs.clear();
        self.resources_fs.clear();
    }

    fn begin_renderpass(&mut self, encoder: MTLRenderCommandEncoder) {
        self.encoder_state = EncoderState::Render(encoder);
        // Apply previously bound values for this command buffer
        if let Some(viewport) = self.viewport {
            encoder.set_viewport(viewport);
        }
        if let Some(scissors) = self.scissors {
            encoder.set_scissor_rect(scissors);
        }
        if let Some(pipeline_state) = self.pipeline_state {
            encoder.set_render_pipeline_state(pipeline_state);
        }
        // inherit vertex resources
        for (i, resource) in self.resources_vs.buffers.iter().enumerate() {
            if let Some((buffer, offset)) = *resource {
                encoder.set_vertex_buffer(i as _, offset as _, buffer);
            }
        }
        for (i, resource) in self.resources_vs.textures.iter().enumerate() {
            if let Some(texture) = *resource {
                encoder.set_vertex_texture(i as _, texture);
            }
        }
        for (i, resource) in self.resources_vs.samplers.iter().enumerate() {
            if let Some(sampler) = *resource {
                encoder.set_vertex_sampler_state(i as _, sampler);
            }
        }
        // inherit fragment resources
        for (i, resource) in self.resources_fs.buffers.iter().enumerate() {
            if let Some((buffer, offset)) = *resource {
                encoder.set_fragment_buffer(i as _, offset as _, buffer);
            }
        }
        for (i, resource) in self.resources_fs.textures.iter().enumerate() {
            if let Some(texture) = *resource {
                encoder.set_fragment_texture(i as _, texture);
            }
        }
        for (i, resource) in self.resources_fs.samplers.iter().enumerate() {
            if let Some(sampler) = *resource {
                encoder.set_fragment_sampler_state(i as _, sampler);
            }
        }
    }
}

unsafe impl Send for CommandBuffer {
}

impl Drop for CommandBufferInner {
    fn drop(&mut self) {
        unsafe {
            self.command_buffer.release();

            match self.encoder_state {
                EncoderState::None => {},
                EncoderState::Blit(encoder) => encoder.release(),
                EncoderState::Render(encoder) => encoder.release(),
            }
        }
    }
}

enum EncoderState {
    None,
    Blit(MTLBlitCommandEncoder),
    Render(MTLRenderCommandEncoder),
}

impl CommandQueue {
    pub fn new(device: MTLDevice) -> CommandQueue {
        CommandQueue(Arc::new(QueueInner {
            queue: device.new_command_queue(),
        }))
    }

    pub unsafe fn device(&self) -> MTLDevice {
        msg_send![self.0.queue.0, device]
    }
}

impl core::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(&mut self, submit: RawSubmission<Backend>, fence: Option<&native::Fence>) {
        // FIXME: wait for semaphores!

        // FIXME: multiple buffers signaling!
        let signal_block = if !submit.signal_semaphores.is_empty() {
            let semaphores_copy: Vec<_> = submit.signal_semaphores.iter().map(|semaphore| {
                semaphore.0
            }).collect();
            Some(ConcreteBlock::new(move |cb: *mut ()| -> () {
                for semaphore in semaphores_copy.iter() {
                    native::dispatch_semaphore_signal(*semaphore);
                }
            }).copy())
        } else {
            None
        };

        for buffer in submit.cmd_buffers {
            let command_buffer = (&mut *buffer.inner.get()).command_buffer;
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer.0, addCompletedHandler: signal_block.deref() as *const _];
            }
            // only append the fence handler to the last command buffer
            if buffer as *const _ == submit.cmd_buffers.last().unwrap() as *const _ {
                if let Some(ref fence) = fence {
                    let value_ptr = fence.0.clone();
                    let fence_block = ConcreteBlock::new(move |cb: *mut ()| -> () {
                        *value_ptr.lock().unwrap() = true;
                    }).copy();
                    msg_send![command_buffer.0, addCompletedHandler: fence_block.deref() as *const _];
                }
            }
            command_buffer.commit();
        }
    }
}

impl core::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        if let Some(ref mut managed) = self.managed {
            for cmd_buffer in managed {
                cmd_buffer.inner().reset(&self.queue);
            }
        }
    }

    unsafe fn from_queue(queue: &CommandQueue, flags: pool::CommandPoolCreateFlags) -> Self {
        CommandPool {
            queue: (queue.0).clone(),
            managed: if flags.contains(pool::RESET_INDIVIDUAL) {
                None
            } else {
                Some(Vec::new())
            },
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<CommandBuffer> {
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new(unsafe {
                // TODO: maybe use unretained command buffer for efficiency?
                let command_buffer = self.queue.queue.new_command_buffer(); // Returns retained
                defer_on_unwind! { command_buffer.release() }

                UnsafeCell::new(CommandBufferInner {
                    command_buffer,
                    encoder_state: EncoderState::None,
                    viewport: None,
                    scissors: None,
                    pipeline_state: None,
                    primitive_type: MTLPrimitiveType::Point,
                    resources_vs: StageResources::new(),
                    resources_fs: StageResources::new(),
                })
            }),
            queue: if self.managed.is_some() {
                None
            } else {
                Some(self.queue.clone())
            },
        }).collect();

        if let Some(ref mut managed) = self.managed {
            managed.extend_from_slice(&buffers);
        }
        buffers
    }

    /// Free command buffers which are allocated from this pool.
    unsafe fn free(&mut self, buffers: Vec<CommandBuffer>) {
        for mut cmd_buf in buffers {
            //TODO: what else here?
            let target = cmd_buf.inner().command_buffer;
            let managed = match self.managed {
                Some(ref mut vec) => vec,
                None => continue,
            };
            match managed.iter_mut().position(|b| b.inner().command_buffer == target) {
                Some(index) => {
                    managed.swap_remove(index);
                }
                None => {
                    error!("Unable to free a command buffer!")
                }
            }
        }
    }
}

impl core::SubpassCommandPool<Backend> for CommandPool {
}

impl CommandBuffer {
    #[inline]
    fn inner(&mut self) -> &mut CommandBufferInner {
        unsafe {
            &mut *self.inner.get()
        }
    }

    fn encode_blit(&mut self) -> MTLBlitCommandEncoder {
        match self.inner().encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(blit_encoder) => return blit_encoder,
            EncoderState::Render(render_encoder) => panic!("invalid inside renderpass"),
        }

        let blit_encoder = self.inner().command_buffer.new_blit_command_encoder(); // Returns retained
        self.inner().encoder_state = EncoderState::Blit(blit_encoder);
        blit_encoder
    }

    fn except_renderpass(&mut self) -> MTLRenderCommandEncoder {
        if let EncoderState::Render(encoder) = self.inner().encoder_state {
            encoder
        } else {
            panic!("only valid inside renderpass")
        }
    }
}

impl core::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self) {
        if let Some(ref queue) = self.queue {
            unsafe { &mut *self.inner.get() }
                .reset(queue);
        }
    }

    fn finish(&mut self) {
        match self.inner().encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(blit_encoder) => {
                blit_encoder.end_encoding();
                unsafe { blit_encoder.release(); }
            },
            EncoderState::Render(render_encoder) => {
                render_encoder.end_encoding();
                unsafe { render_encoder.release(); }
            },
        }
        self.inner().encoder_state = EncoderState::None;
    }

    fn reset(&mut self, _release_resources: bool) {
        unsafe { &mut *self.inner.get() }
            .reset(self.queue.as_ref().unwrap());
    }

    fn pipeline_barrier(
        &mut self,
        stages: Range<pso::PipelineStage>,
        barriers: &[memory::Barrier<Backend>],
    ) {
        // TODO: MTLRenderCommandEncoder.textureBarrier on macOS?
    }

    fn fill_buffer(
        &mut self,
        buffer: &native::Buffer,
        range: Range<u64>,
        data: u32,
    ) {
        unimplemented!()
    }

    fn update_buffer(
        &mut self,
        buffer: &native::Buffer,
        offset: u64,
        data: &[u8],
    ) {
        unimplemented!()
    }

    fn clear_color_image(
        &mut self,
        image: &native::Image,
        layout: ImageLayout,
        range: SubresourceRange,
        value: ClearColor,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil_image(
        &mut self,
        image: &native::Image,
        layout: ImageLayout,
        range: SubresourceRange,
        value: ClearDepthStencil,
    ) {
        unimplemented!()
    }

    fn clear_attachments(
        &mut self,
        clears: &[AttachmentClear],
        rects: &[target::Rect],
    ) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        src: &native::Image,
        src_layout: ImageLayout,
        dst: &native::Image,
        dst_layout: ImageLayout,
        regions: &[ImageResolve],
    ) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, view: IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, buffer_set: pso::VertexBufferSet<Backend>) {
        let inner = self.inner();
        let buffers = &mut inner.resources_vs.buffers;
        while buffers.len() < buffer_set.0.len()    {
            buffers.push(None)
        }
        for (out, &(buffer, offset)) in buffers.iter_mut().zip(buffer_set.0.iter()) {
            *out = Some((buffer.0, offset));
        }
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            for (i, &(buffer, offset)) in buffer_set.0.iter().enumerate() {
                encoder.set_vertex_buffer(i as _, offset as _, buffer.0);
            }
        }
    }

    fn set_viewports(&mut self, rects: &[Viewport]) {
        let inner = self.inner();
        if rects.len() != 1 {
            panic!("Metal supports only one viewport");
        }
        let rect = &rects[0];
        let vp = MTLViewport {
            originX: rect.x as f64,
            originY: rect.y as f64,
            width: rect.w as f64,
            height: rect.h as f64,
            znear: rect.near as f64,
            zfar: rect.far as f64,
        };
        inner.viewport = Some(vp);
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_viewport(vp);
        }
    }

    fn set_scissors(&mut self, rects: &[target::Rect]) {
        let inner = self.inner();
        if rects.len() != 1 {
            panic!("Metal supports only one scissor");
        }
        let rect = &rects[0];
        let scissor = MTLScissorRect {
            x: rect.x as NSUInteger,
            y: rect.y as NSUInteger,
            width: rect.w as NSUInteger,
            height: rect.h as NSUInteger,
        };
        inner.scissors = Some(scissor);
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_scissor_rect(scissor);
        }
    }

    fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil) {
        unimplemented!()
    }

    fn set_blend_constants(&mut self, color: target::ColorValue) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &native::RenderPass,
        frame_buffer: &native::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    ) {
        unsafe {
            let command_buffer = self.inner();

            match command_buffer.encoder_state {
                EncoderState::Render(_) => panic!("already in a renderpass"),
                EncoderState::Blit(blit) => {
                    blit.end_encoding();
                    blit.release();
                    command_buffer.encoder_state = EncoderState::None;
                },
                EncoderState::None => {},
            }

            // FIXME: subpasses

            let pass_descriptor: MTLRenderPassDescriptor = msg_send![(frame_buffer.0).0, copy]; // Returns retained
            defer! { pass_descriptor.release() }
            // TODO: validate number of clear colors
            for (i, value) in clear_values.iter().enumerate() {
                let color_desc = pass_descriptor.color_attachments().object_at(i);
                let mtl_color = match *value {
                    ClearValue::Color(ClearColor::Float(values)) => MTLClearColor::new(
                        values[0] as f64,
                        values[1] as f64,
                        values[2] as f64,
                        values[3] as f64,
                    ),
                    _ => unimplemented!(),
                };
                color_desc.set_clear_color(mtl_color);
            }

            let render_encoder = command_buffer.command_buffer.new_render_command_encoder(pass_descriptor); // Returns retained
            defer_on_unwind! { render_encoder.release() };

            command_buffer.begin_renderpass(render_encoder);
        }
    }

    fn next_subpass(&mut self, contents: SubpassContents) {
        unimplemented!()
    }

    fn end_renderpass(&mut self) {
        match self.inner().encoder_state {
            EncoderState::Render(encoder) => {
                encoder.end_encoding();
                unsafe {
                    encoder.release();
                }
            },
            _ => panic!("not in a renderpass"),
        }
        self.inner().encoder_state = EncoderState::None;
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let inner = self.inner();
        inner.pipeline_state = Some(pipeline.raw);
        inner.primitive_type = pipeline.primitive_type;
        if let EncoderState::Render(encoder) = inner.encoder_state {
            encoder.set_render_pipeline_state(pipeline.raw);
        }
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &native::PipelineLayout,
        first_set: usize,
        sets: &[&native::DescriptorSet],
    ) {
        use spirv_cross::{msl, spirv};
        let inner = self.inner();

        for (set_index, &desc_set) in sets.iter().enumerate() {
            let location_vs = msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::Vertex,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            };
            let location_fs = msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::Fragment,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            };
            match *desc_set {
                native::DescriptorSet::Emulated(ref desc_inner) => {
                    use native::DescriptorSetBinding::*;
                    let set = desc_inner.lock().unwrap();
                    for (&binding, values) in set.bindings.iter() {
                        let desc_layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                        if desc_layout.stage_flags.contains(pso::STAGE_VERTEX) {
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_vs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_vs.add_samplers(start, samplers.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, &sampler) in samplers.iter().enumerate() {
                                            encoder.set_vertex_sampler_state((start + i) as _, sampler);
                                        }
                                    }
                                },
                                SampledImage(ref images) => {
                                    inner.resources_vs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, &texture) in images.iter().enumerate() {
                                            encoder.set_vertex_texture((start + i) as _, texture.0);
                                        }
                                    }
                                },
                                _ => unimplemented!(),
                            }
                        }
                        if desc_layout.stage_flags.contains(pso::STAGE_FRAGMENT) {
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_fs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_fs.add_samplers(start, samplers.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, &sampler) in samplers.iter().enumerate() {
                                            encoder.set_fragment_sampler_state((start + i) as _, sampler);
                                        }
                                    }
                                },
                                SampledImage(ref images) => {
                                    inner.resources_fs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, &texture) in images.iter().enumerate() {
                                            encoder.set_fragment_texture((start + i) as _, texture.0);
                                        }
                                    }
                                },
                                _ => unimplemented!(),
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { buffer, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::STAGE_VERTEX) {
                        let slot = layout.res_overrides[&location_vs].resource_id;
                        inner.resources_vs.add_buffer(slot as _, buffer, offset as _);
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_vertex_buffer(slot as _, offset as _, buffer)
                        }
                    }
                    if stage_flags.contains(pso::STAGE_FRAGMENT) {
                        let slot = layout.res_overrides[&location_fs].resource_id;
                        inner.resources_fs.add_buffer(slot as _, buffer, offset as _);
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_fragment_buffer(slot as _, offset as _, buffer)
                        }
                    }
                }
            }
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        unimplemented!()
    }

    fn bind_compute_descriptor_sets(
        &mut self,
        _layout: &native::PipelineLayout,
        _first_set: usize,
        _sets: &[&native::DescriptorSet],
    ) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, buffer: &native::Buffer, offset: u64) {
        unimplemented!()
    }

    fn copy_buffer(
        &mut self,
        src: &native::Buffer,
        dst: &native::Buffer,
        regions: &[BufferCopy],
    ) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        src: &native::Image,
        src_layout: ImageLayout,
        dst: &native::Image,
        dst_layout: ImageLayout,
        regions: &[ImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_buffer_to_image(
        &mut self,
        src: &native::Buffer,
        dst: &native::Image,
        _dst_layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        let encoder = self.encode_blit();
        let extent = MTLSize {
            width: dst.0.width(),
            height: dst.0.height(),
            depth: dst.0.depth(),
        };
        // FIXME: layout

        for region in regions {
            let image_offset = &region.image_offset;
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + region.buffer_slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                unsafe {
                    msg_send![encoder.0,
                        copyFromBuffer: (src.0).0
                        sourceOffset: offset as NSUInteger
                        sourceBytesPerRow: region.buffer_row_pitch as NSUInteger
                        sourceBytesPerImage: region.buffer_slice_pitch as NSUInteger
                        sourceSize: extent
                        toTexture: (dst.0).0
                        destinationSlice: layer as NSUInteger
                        destinationLevel: r.level as NSUInteger
                        destinationOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                    ]
                }
            }
        }
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &native::Image,
        _src_layout: ImageLayout,
        dst: &native::Buffer,
        regions: &[BufferImageCopy],
    ) {
        let encoder = self.encode_blit();
        let extent = MTLSize {
            width: src.0.width(),
            height: src.0.height(),
            depth: src.0.depth(),
        };
        // FIXME: layout

        for region in regions {
            let image_offset = &region.image_offset;
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + region.buffer_slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                unsafe {
                    msg_send![encoder.0,
                        copyFromTexture: (src.0).0
                        sourceSlice: layer as NSUInteger
                        sourceLevel: r.level as NSUInteger
                        sourceOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                        sourceSize: extent
                        toBuffer: (dst.0).0
                        destinationOffset: offset as NSUInteger
                        destinationBytesPerRow: region.buffer_row_pitch as NSUInteger
                        destinationBytesPerImage: region.buffer_slice_pitch as NSUInteger
                    ]
                }
            }
        }
    }

    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    ) {
        let encoder = self.except_renderpass();

        unsafe {
            msg_send![encoder.0,
                drawPrimitives: self.inner().primitive_type
                vertexStart: vertices.start as NSUInteger
                vertexCount: (vertices.end - vertices.start) as NSUInteger
                instanceCount: (instances.end - instances.start) as NSUInteger
                baseInstance: instances.start as NSUInteger
            ];
        }
    }

    fn draw_indexed(
        &mut self,
        indeces: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        unimplemented!()
    }

    fn draw_indirect(
        &mut self,
        buffer: &native::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &native::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unimplemented!()
    }
}
