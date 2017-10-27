use {Backend};
use native;

use std::ops::{Deref, Range};
use std::sync::{Arc};
use std::cell::UnsafeCell;

use hal::{memory, pool, pso};
use hal::{VertexCount, VertexOffset, InstanceCount, IndexCount};
use hal::buffer::{IndexBufferView};
use hal::image::{ImageLayout, SubresourceRange};
use hal::command::{
    AttachmentClear, ClearColor, ClearDepthStencil, ClearValue,
    BufferImageCopy, BufferCopy, ImageCopy, ImageResolve,
    SubpassContents, RawCommandBuffer,
    ColorValue, StencilValue, Rect, Viewport,
};
use hal::queue::{RawCommandQueue, RawSubmission};

use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLClearColor, MTLSize, MTLOrigin};
use cocoa::foundation::NSUInteger;
use block::{ConcreteBlock};


pub struct CommandQueue(pub(crate) Arc<QueueInner>);

pub(crate) struct QueueInner {
    queue: metal::CommandQueue,
}

unsafe impl Send for QueueInner {}
unsafe impl Sync for QueueInner {}

pub struct CommandPool {
    pub(crate) queue: Arc<QueueInner>,
    pub(crate) managed: Option<Vec<CommandBuffer>>,
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
    buffers: Vec<Option<(metal::Buffer, pso::BufferOffset)>>,
    textures: Vec<Option<metal::Texture>>,
    samplers: Vec<Option<metal::SamplerState>>,
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

    fn add_buffer(&mut self, slot: usize, buffer: &metal::BufferRef, offset: usize) {
        while self.buffers.len() <= slot {
            self.buffers.push(None)
        }
        self.buffers[slot] = Some((buffer.to_owned(), offset));
    }

    fn add_textures(&mut self, start: usize, textures: &[Option<(metal::Texture, ImageLayout)>]) {
        while self.textures.len() < start + textures.len() {
            self.textures.push(None)
        }
        for (out, entry) in self.textures[start..].iter_mut().zip(textures.iter()) {
            *out = entry.as_ref().map(|&(ref texture, _)| texture.clone());
        }
    }

    fn add_samplers(&mut self, start: usize, samplers: &[Option<metal::SamplerState>]) {
        while self.samplers.len() < start + samplers.len() {
            self.samplers.push(None)
        }
        for (out, sampler) in self.samplers[start..].iter_mut().zip(samplers.iter()) {
            *out = sampler.clone();
        }
    }
}

struct CommandBufferInner {
    command_buffer: metal::CommandBuffer,
    //TODO: would be cleaner to move the cache into `CommandBuffer` iself
    // it doesn't have to be in `Inner`
    encoder_state: EncoderState,
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    pipeline_state: Option<metal::RenderPipelineState>,
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
}

impl CommandBufferInner {
    fn reset(&mut self, queue: &QueueInner) {
        self.command_buffer = queue.queue.new_command_buffer().to_owned();

        self.resources_vs.clear();
        self.resources_fs.clear();
    }

    fn begin_renderpass(&mut self, encoder: metal::RenderCommandEncoder) {
        self.encoder_state = EncoderState::Render(encoder);
        let encoder = if let EncoderState::Render(ref encoder) = self.encoder_state {
            encoder
        } else {
            unreachable!()
        };
        // Apply previously bound values for this command buffer
        if let Some(viewport) = self.viewport {
            encoder.set_viewport(viewport);
        }
        if let Some(scissors) = self.scissors {
            encoder.set_scissor_rect(scissors);
        }
        if let Some(ref pipeline_state) = self.pipeline_state {
            encoder.set_render_pipeline_state(pipeline_state);
        }
        // inherit vertex resources
        for (i, resource) in self.resources_vs.buffers.iter().enumerate() {
            if let Some((ref buffer, offset)) = *resource {
                encoder.set_vertex_buffer(i as _, offset as _, Some(buffer));
            }
        }
        for (i, resource) in self.resources_vs.textures.iter().enumerate() {
            if let Some(ref texture) = *resource {
                encoder.set_vertex_texture(i as _, Some(texture));
            }
        }
        for (i, resource) in self.resources_vs.samplers.iter().enumerate() {
            if let Some(ref sampler) = *resource {
                encoder.set_vertex_sampler_state(i as _, Some(sampler));
            }
        }
        // inherit fragment resources
        for (i, resource) in self.resources_fs.buffers.iter().enumerate() {
            if let Some((ref buffer, offset)) = *resource {
                encoder.set_fragment_buffer(i as _, offset as _, Some(buffer));
            }
        }
        for (i, resource) in self.resources_fs.textures.iter().enumerate() {
            if let Some(ref texture) = *resource {
                encoder.set_fragment_texture(i as _, Some(texture));
            }
        }
        for (i, resource) in self.resources_fs.samplers.iter().enumerate() {
            if let Some(ref sampler) = *resource {
                encoder.set_fragment_sampler_state(i as _, Some(sampler));
            }
        }
    }
}

unsafe impl Send for CommandBuffer {
}

enum EncoderState {
    None,
    Blit(metal::BlitCommandEncoder),
    Render(metal::RenderCommandEncoder),
}

impl CommandQueue {
    pub fn new(device: &metal::DeviceRef) -> CommandQueue {
        CommandQueue(Arc::new(QueueInner {
            queue: device.new_command_queue(),
        }))
    }

    pub unsafe fn device(&self) -> &metal::DeviceRef {
        msg_send![&*self.0.queue, device]
    }
}

impl RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(&mut self, submit: RawSubmission<Backend>, fence: Option<&native::Fence>) {
        // FIXME: wait for semaphores!

        // FIXME: multiple buffers signaling!
        let signal_block = if !submit.signal_semaphores.is_empty() {
            let semaphores_copy: Vec<_> = submit.signal_semaphores.iter().map(|semaphore| {
                semaphore.0
            }).collect();
            Some(ConcreteBlock::new(move |_cb: *mut ()| -> () {
                for semaphore in semaphores_copy.iter() {
                    native::dispatch_semaphore_signal(*semaphore);
                }
            }).copy())
        } else {
            None
        };

        for buffer in submit.cmd_buffers {
            let command_buffer: &metal::CommandBufferRef = &(&mut *buffer.inner.get()).command_buffer;
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer, addCompletedHandler: signal_block.deref() as *const _];
            }
            // only append the fence handler to the last command buffer
            if buffer as *const _ == submit.cmd_buffers.last().unwrap() as *const _ {
                if let Some(ref fence) = fence {
                    let value_ptr = fence.0.clone();
                    let fence_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                        *value_ptr.lock().unwrap() = true;
                    }).copy();
                    msg_send![command_buffer, addCompletedHandler: fence_block.deref() as *const _];
                }
            }
            command_buffer.commit();
        }
    }
}

impl pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        if let Some(ref mut managed) = self.managed {
            for cmd_buffer in managed {
                cmd_buffer.inner().reset(&self.queue);
            }
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<CommandBuffer> {
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new({
                // TODO: maybe use unretained command buffer for efficiency?
                let command_buffer = self.queue.queue.new_command_buffer().to_owned();

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
            let target = &*cmd_buf.inner().command_buffer;
            let managed = match self.managed {
                Some(ref mut vec) => vec,
                None => continue,
            };
            match managed.iter_mut().position(|b| &*b.inner().command_buffer as *const metal::CommandBufferRef == target as *const metal::CommandBufferRef) {
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

impl pool::SubpassCommandPool<Backend> for CommandPool {
}

impl CommandBuffer {
    #[inline]
    fn inner(&mut self) -> &mut CommandBufferInner {
        unsafe {
            &mut *self.inner.get()
        }
    }

    fn encode_blit(&mut self) -> &metal::BlitCommandEncoderRef {
        let inner = self.inner();
        match inner.encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(ref blit_encoder) => return blit_encoder,
            EncoderState::Render(_) => panic!("invalid inside renderpass"),
        }

        let blit_encoder = inner.command_buffer.new_blit_command_encoder().to_owned();
        inner.encoder_state = EncoderState::Blit(blit_encoder);
        if let EncoderState::Blit(ref blit_encoder) = inner.encoder_state {
            blit_encoder
        } else {
            unreachable!()
        }
    }

    fn except_renderpass(&mut self) -> &metal::RenderCommandEncoderRef {
        if let EncoderState::Render(ref encoder) = self.inner().encoder_state {
            encoder
        } else {
            panic!("only valid inside renderpass")
        }
    }
}

impl RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self) {
        if let Some(ref queue) = self.queue {
            unsafe { &mut *self.inner.get() }
                .reset(queue);
        }
    }

    fn finish(&mut self) {
        match self.inner().encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(ref blit_encoder) => {
                blit_encoder.end_encoding();
            },
            EncoderState::Render(ref render_encoder) => {
                render_encoder.end_encoding();
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
        _stages: Range<pso::PipelineStage>,
        _barriers: &[memory::Barrier<Backend>],
    ) {
        // TODO: MTLRenderCommandEncoder.textureBarrier on macOS?
    }

    fn fill_buffer(
        &mut self,
        _buffer: &native::Buffer,
        _range: Range<u64>,
        _data: u32,
    ) {
        unimplemented!()
    }

    fn update_buffer(
        &mut self,
        _buffer: &native::Buffer,
        _offset: u64,
        _data: &[u8],
    ) {
        unimplemented!()
    }

    fn clear_color_image(
        &mut self,
        _image: &native::Image,
        _layout: ImageLayout,
        _range: SubresourceRange,
        _value: ClearColor,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil_image(
        &mut self,
        _image: &native::Image,
        _layout: ImageLayout,
        _range: SubresourceRange,
        _value: ClearDepthStencil,
    ) {
        unimplemented!()
    }

    fn clear_attachments(
        &mut self,
        _clears: &[AttachmentClear],
        _rects: &[Rect],
    ) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        _src: &native::Image,
        _src_layout: ImageLayout,
        _dst: &native::Image,
        _dst_layout: ImageLayout,
        _regions: &[ImageResolve],
    ) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, _view: IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, buffer_set: pso::VertexBufferSet<Backend>) {
        let inner = self.inner();
        let buffers = &mut inner.resources_vs.buffers;
        while buffers.len() < buffer_set.0.len()    {
            buffers.push(None)
        }
        for (ref mut out, &(ref buffer, offset)) in buffers.iter_mut().zip(buffer_set.0.iter()) {
            **out = Some((buffer.0.clone(), offset));
        }
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            for (i, &(buffer, offset)) in buffer_set.0.iter().enumerate() {
                encoder.set_vertex_buffer(i as _, offset as _, Some(&buffer.0));
            }
        }
    }

    fn set_viewports(&mut self, vps: &[Viewport]) {
        let inner = self.inner();
        if vps.len() != 1 {
            panic!("Metal supports only one viewport");
        }
        let vp = &vps[0];
        let viewport = MTLViewport {
            originX: vp.rect.x as f64,
            originY: vp.rect.y as f64,
            width: vp.rect.w as f64,
            height: vp.rect.h as f64,
            znear: vp.depth.start as f64,
            zfar: vp.depth.end as f64,
        };
        inner.viewport = Some(viewport);
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_viewport(viewport);
        }
    }

    fn set_scissors(&mut self, rects: &[Rect]) {
        let inner = self.inner();
        if rects.len() != 1 {
            panic!("Metal supports only one scissor");
        }
        let rect = &rects[0];
        let scissor = MTLScissorRect {
            x: rect.x as _,
            y: rect.y as _,
            width: rect.w as _,
            height: rect.h as _,
        };
        inner.scissors = Some(scissor);
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_scissor_rect(scissor);
        }
    }

    fn set_stencil_reference(&mut self, _front: StencilValue, _back: StencilValue) {
        unimplemented!()
    }

    fn set_blend_constants(&mut self, _color: ColorValue) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        _render_pass: &native::RenderPass,
        frame_buffer: &native::FrameBuffer,
        _render_area: Rect,
        clear_values: &[ClearValue],
        _first_subpass: SubpassContents,
    ) {
        unsafe {
            let command_buffer = self.inner();

            match command_buffer.encoder_state {
                EncoderState::Render(_) => panic!("already in a renderpass"),
                EncoderState::Blit(ref blit) => {
                    blit.end_encoding();
                },
                EncoderState::None => {},
            }
            command_buffer.encoder_state = EncoderState::None;

            // FIXME: subpasses

            let pass_descriptor: metal::RenderPassDescriptor = msg_send![frame_buffer.0, copy];
            // TODO: validate number of clear colors
            for (i, value) in clear_values.iter().enumerate() {
                let color_desc = pass_descriptor.color_attachments().object_at(i).expect("too many clear values");
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

            let render_encoder = command_buffer.command_buffer.new_render_command_encoder(&pass_descriptor).to_owned();

            command_buffer.begin_renderpass(render_encoder);
        }
    }

    fn next_subpass(&mut self, _contents: SubpassContents) {
        unimplemented!()
    }

    fn end_renderpass(&mut self) {
        match self.inner().encoder_state {
            EncoderState::Render(ref encoder) => {
                encoder.end_encoding();
            },
            _ => panic!("not in a renderpass"),
        }
        self.inner().encoder_state = EncoderState::None;
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let inner = self.inner();
        let pipeline_state = pipeline.raw.to_owned();
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_render_pipeline_state(&pipeline_state);
        }
        inner.pipeline_state = Some(pipeline_state);
        inner.primitive_type = pipeline.primitive_type;
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

                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_vs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_vs.add_samplers(start, samplers.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, ref sampler) in samplers.iter().enumerate() {
                                            encoder.set_vertex_sampler_state((start + i) as _, sampler.as_ref().map(|x| &**x));
                                        }
                                    }
                                }
                                SampledImage(ref images) => {
                                    inner.resources_vs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, ref texture) in images.iter().enumerate() {
                                            encoder.set_vertex_texture((start + i) as _, texture.as_ref().map(|&(ref texture, _)| &**texture));
                                        }
                                    }
                                }
                            }
                        }
                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_fs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_fs.add_samplers(start, samplers.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, sampler) in samplers.iter().enumerate() {
                                            encoder.set_fragment_sampler_state((start + i) as _, sampler.as_ref().map(|x| &**x));
                                        }
                                    }
                                }
                                SampledImage(ref images) => {
                                    inner.resources_fs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, texture) in images.iter().enumerate() {
                                            encoder.set_fragment_texture((start + i) as _, texture.as_ref().map(|&(ref texture, _)| &**texture));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { ref buffer, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                        let slot = layout.res_overrides[&location_vs].resource_id;
                        inner.resources_vs.add_buffer(slot as _, buffer, offset as _);
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_vertex_buffer(slot as _, offset as _, Some(buffer))
                        }
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let slot = layout.res_overrides[&location_fs].resource_id;
                        inner.resources_fs.add_buffer(slot as _, &buffer, offset as _);
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_fragment_buffer(slot as _, offset as _, Some(buffer))
                        }
                    }
                }
            }
        }
    }

    fn bind_compute_pipeline(&mut self, _pipeline: &native::ComputePipeline) {
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

    fn dispatch(&mut self, _x: u32, _y: u32, _z: u32) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, _buffer: &native::Buffer, _offset: u64) {
        unimplemented!()
    }

    fn copy_buffer(
        &mut self,
        _src: &native::Buffer,
        _dst: &native::Buffer,
        _regions: &[BufferCopy],
    ) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        _src: &native::Image,
        _src_layout: ImageLayout,
        _dst: &native::Image,
        _dst_layout: ImageLayout,
        _regions: &[ImageCopy],
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
                    msg_send![encoder,
                        copyFromBuffer: &*src.0
                        sourceOffset: offset as NSUInteger
                        sourceBytesPerRow: region.buffer_row_pitch as NSUInteger
                        sourceBytesPerImage: region.buffer_slice_pitch as NSUInteger
                        sourceSize: extent
                        toTexture: &*dst.0
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
                    msg_send![encoder,
                        copyFromTexture: &*src.0
                        sourceSlice: layer as NSUInteger
                        sourceLevel: r.level as NSUInteger
                        sourceOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                        sourceSize: extent
                        toBuffer: &*dst.0
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
        let primitive_type = self.inner().primitive_type;
        let encoder = self.except_renderpass();

        unsafe {
            msg_send![encoder,
                drawPrimitives: primitive_type
                vertexStart: vertices.start as NSUInteger
                vertexCount: (vertices.end - vertices.start) as NSUInteger
                instanceCount: (instances.end - instances.start) as NSUInteger
                baseInstance: instances.start as NSUInteger
            ];
        }
    }

    fn draw_indexed(
        &mut self,
        _indeces: Range<IndexCount>,
        _base_vertex: VertexOffset,
        _instances: Range<InstanceCount>,
    ) {
        unimplemented!()
    }

    fn draw_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }
}
