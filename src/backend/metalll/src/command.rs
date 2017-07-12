use ::{Surface, Resources};
use ::native;
use ::conversions::*;

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::cell::UnsafeCell;

use core::{self, mapping, memory, target, pso, state, pool, queue, command, shade};
use core::{VertexCount, VertexOffset};
use core::buffer::{IndexBufferView};
use core::command::{InstanceParams, ClearColor, ClearValue, BufferImageCopy, BufferCopy, Encoder};
use core::command::{RenderPassInlineEncoder, RenderPassSecondaryEncoder};

use metal::*;
use cocoa::foundation::NSUInteger;
use block::{Block, ConcreteBlock};

pub struct QueueFamily {
}

pub struct CommandQueue(Arc<QueueInner>);

struct QueueInner {
    queue: MTLCommandQueue,
}

impl Drop for QueueInner {
    fn drop(&mut self) {
        unsafe {
            self.queue.release();
        }
    }
}

pub struct CommandPool {
    queue: Arc<QueueInner>,
    active_buffers: Vec<CommandBuffer>,
}

pub struct CommandBuffer {
    command_buffer: MTLCommandBuffer,
    encoder_state: EncoderState,
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    pipeline_state: Option<MTLRenderPipelineState>, // Unretained
    vertex_buffers: Vec<(MTLBuffer, pso::BufferOffset)>, // Unretained
    descriptor_sets: Vec<Option<Arc<Mutex<native::DescriptorSetInner>>>>,
}

impl Drop for CommandBuffer {
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

pub struct SubmitInfo {
    command_buffer: MTLCommandBuffer
}

impl Drop for SubmitInfo {
    fn drop(&mut self) {
        unsafe { self.command_buffer.release(); }
    }
}

impl core::QueueFamily for QueueFamily {
    type Surface = Surface;

    fn supports_present(&self, _surface: &Surface) -> bool { true }
    fn num_queues(&self) -> u32 { 1 }
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

impl core::CommandQueue for CommandQueue {
    type R = Resources;
    type GeneralCommandBuffer = CommandBuffer;
    type GraphicsCommandBuffer = CommandBuffer;
    type ComputeCommandBuffer = CommandBuffer;
    type TransferCommandBuffer = CommandBuffer;
    type SubpassCommandBuffer = CommandBuffer;
    type SubmitInfo = SubmitInfo;

    unsafe fn submit<'a, C>(&mut self, submit_infos: &[core::QueueSubmit<C, Self::R>], fence: Option<&'a mut native::Fence>)
        where C: core::CommandBuffer<SubmitInfo = SubmitInfo>
    {
        for submit in submit_infos {
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
                let command_buffer = buffer.get_info().command_buffer;
                if let Some(ref signal_block) = signal_block {
                    msg_send![command_buffer.0, addCompletedHandler: signal_block.deref() as *const _];
                }
                // only append the fence handler to the last command buffer
                if submit as *const _ == submit_infos.last().unwrap() as *const _ &&
                   buffer as *const _ == submit.cmd_buffers.last().unwrap() as *const _ {
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
}

impl core::CommandPool for CommandPool {
    type Queue = CommandQueue;
    type PoolBuffer = CommandBuffer;

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, CommandBuffer> {
        unsafe {
            // TODO: maybe use unretained command buffer for efficiency?
            let command_buffer = self.queue.queue.new_command_buffer(); // Returns retained
            defer_on_unwind! { command_buffer.release() }

            self.active_buffers.push(CommandBuffer {
                command_buffer,
                encoder_state: EncoderState::None,
                viewport: None,
                scissors: None,
                pipeline_state: None,
                vertex_buffers: Vec::new(),
                descriptor_sets: Vec::new(),
            });
            Encoder::new(self.active_buffers.last_mut().unwrap())
        }
    }

    fn reset(&mut self) {
        self.active_buffers.clear();
    }

    fn reserve(&mut self, additional: usize) {
    }
}

impl pool::GraphicsCommandPool for CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> CommandPool
        where Q: Into<queue::GraphicsQueue<CommandQueue>> + DerefMut<Target=CommandQueue>
    {
        CommandPool {
            queue: queue.0.clone(),
            active_buffers: Vec::new(),
        }
    }
}

impl CommandBuffer {
    fn encode_blit(&mut self) -> MTLBlitCommandEncoder {
        unsafe {
            match self.encoder_state {
                EncoderState::None => {},
                EncoderState::Blit(blit_encoder) => return blit_encoder,
                EncoderState::Render(render_encoder) => {
                    render_encoder.end_encoding();
                    render_encoder.release();
                }
            }

            let blit_encoder = self.command_buffer.new_blit_command_encoder(); // Returns retained
            self.encoder_state = EncoderState::Blit(blit_encoder);
            blit_encoder
        }
    }
}

impl core::CommandBuffer for CommandBuffer {
    type SubmitInfo = SubmitInfo;

    unsafe fn end(&mut self) -> SubmitInfo {
        match self.encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(blit_encoder) => blit_encoder.end_encoding(),
            EncoderState::Render(render_encoder) => render_encoder.end_encoding(),
        }

        self.command_buffer.retain();
        SubmitInfo {
            command_buffer: self.command_buffer,
        }
    }
}

impl core::PrimaryCommandBuffer<Resources> for CommandBuffer {
    fn pipeline_barrier<'a>(&mut self, memory_barriers: &[memory::MemoryBarrier], buffer_barriers: &[memory::BufferBarrier<'a, Resources>], image_barriers: &[memory::ImageBarrier<'a, Resources>]) {
        //unimplemented!() FIXME
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }
}

impl core::GraphicsCommandBuffer<Resources> for CommandBuffer {
    type InlineBuffer = RenderPassInlineBuffer;
    type SecondaryBuffer = RenderPassSecondaryBuffer;

    fn clear_depth_stencil(&mut self, depth_view: &native::DepthStencilView, depth_value: Option<target::Depth>, stencil_value: Option<target::Stencil>) {
        unimplemented!()
    }

    fn resolve_image(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, view: IndexBufferView<Resources>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, buffer: pso::VertexBufferSet<Resources>) {
        self.vertex_buffers.clear();
        self.vertex_buffers.extend(buffer.0.iter().map(|&(buffer, offset)| (buffer.0, offset)));
    }

    fn set_viewports(&mut self, rects: &[target::Rect]) {
        if rects.len() != 1 {
            panic!("Metal supports only one viewport");
        }
        let rect = &rects[0];
        self.viewport = Some(MTLViewport {
            originX: rect.x as f64,
            originY: rect.y as f64,
            width: rect.w as f64,
            height: rect.h as f64,
            znear: 0.0,
            zfar: 1.0,
        });
    }

    fn set_scissors(&mut self, rects: &[target::Rect]) {
        if rects.len() != 1 {
            panic!("Metal supports only one scissor");
        }
        let rect = &rects[0];
        self.scissors = Some(MTLScissorRect {
            x: rect.x as NSUInteger,
            y: rect.y as NSUInteger,
            width: rect.w as NSUInteger,
            height: rect.h as NSUInteger,
        });
    }

    fn set_ref_values(&mut self, values: state::RefValues) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        self.pipeline_state = Some(pipeline.0);
    }

    fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        for (i, set) in (first_set..sets.len() + first_set).zip(sets) {
            if let Some(existing) = self.descriptor_sets.get_mut(i) {
                *existing = Some(set.0.clone());
                continue;
            }

            while i > self.descriptor_sets.len() {
                self.descriptor_sets.push(None);
            }
            self.descriptor_sets.push(Some(set.0.clone()));
        }
    }
}

impl core::TransferCommandBuffer<Resources> for CommandBuffer {
    fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &native::Buffer, dest: &native::Buffer, regions: &[BufferCopy]) {
        unimplemented!()
    }
    fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
        unimplemented!()
    }
    fn copy_buffer_to_image(&mut self, src: &native::Buffer, dst: &native::Image, layout: memory::ImageLayout, regions: &[BufferImageCopy]) {
        let encoder = self.encode_blit();
        let extent = unsafe { MTLSize {
            width: dst.0.width(),
            height: dst.0.height(),
            depth: dst.0.depth(),
        }};
        // FIXME: layout

        for region in regions {
            let image_offset = &region.image_offset;

            // TODO multiple layers
            assert!(region.image_layers == 1, "multiple layer copies not implemented");
            unsafe {
                msg_send![encoder.0,
                    copyFromBuffer: (src.0).0
                    sourceOffset: region.buffer_offset as NSUInteger
                    sourceBytesPerRow: region.buffer_row_pitch as NSUInteger
                    sourceBytesPerImage: region.buffer_slice_pitch as NSUInteger
                    sourceSize: extent
                    toTexture: (dst.0).0
                    destinationSlice: region.image_base_layer as NSUInteger
                    destinationLevel: region.image_mip_level as NSUInteger
                    destinationOrigin: MTLOrigin { x: image_offset.x as NSUInteger, y: image_offset.y as NSUInteger, z: image_offset.z as NSUInteger }
                ]
            }
        }
    }
    fn copy_image_to_buffer(&mut self) {
        unimplemented!()
    }
}

impl core::ProcessingCommandBuffer<Resources> for CommandBuffer {
    fn clear_color(&mut self, target_view: &native::RenderTargetView, color: ClearColor) {
        unimplemented!()
    }
    fn clear_buffer(&mut self) {
        unimplemented!()
    }

    fn bind_descriptor_heaps(&mut self, srv_cbv_uav: Option<&native::DescriptorHeap>, samplers: Option<&native::DescriptorHeap>) {
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }
}

impl core::ComputeCommandBuffer<Resources> for CommandBuffer {
    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        unimplemented!()
    }
    fn dispatch(&mut self, a: u32, b: u32, c: u32) {
        unimplemented!()
    }
    fn dispatch_indirect(&mut self) {
        unimplemented!()
    }
}

impl core::SecondaryCommandBuffer<Resources> for CommandBuffer {
    fn pipeline_barrier(&mut self) {
        unimplemented!()
    }
}

impl core::SubpassCommandBuffer<Resources> for CommandBuffer {
    fn clear_attachment(&mut self) {
        unimplemented!();
    }
    fn draw(&mut self, start: VertexCount, count: VertexCount, instance_params: Option<InstanceParams>) {
        unimplemented!();
    }
    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instance_params: Option<InstanceParams>) {
        unimplemented!();
    }
    fn draw_indirect(&mut self) {
        unimplemented!();
    }
    fn draw_indexed_indirect(&mut self) {
        unimplemented!();
    }

    fn bind_index_buffer(&mut self, view: IndexBufferView<Resources>) {
        unimplemented!();
    }
    fn bind_vertex_buffers(&mut self, buffer: pso::VertexBufferSet<Resources>) {
        unimplemented!();
    }

    fn set_viewports(&mut self, rects: &[target::Rect]) {
        unimplemented!();
    }
    fn set_scissors(&mut self, rects: &[target::Rect]) {
        unimplemented!();
    }
    fn set_ref_values(&mut self, values: state::RefValues) {
        unimplemented!();
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        unimplemented!();
    }
    fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        unimplemented!();
    }
    fn push_constants(&mut self) {
        unimplemented!();
    }
}

pub struct RenderPassInlineBuffer {
    render_encoder: MTLRenderCommandEncoder,
}

impl Drop for RenderPassInlineBuffer {
    fn drop(&mut self) {
        unsafe { self.render_encoder.release(); }
    }
}

pub struct RenderPassSecondaryBuffer {
}

impl command::RenderPassInlineBuffer<CommandBuffer, Resources> for RenderPassInlineBuffer
{
    fn begin(command_buffer: &mut Encoder<CommandBuffer>,
             render_pass: &native::RenderPass,
             framebuffer: &native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[ClearValue]) -> Self
    {
        unsafe {
            // FIXME: subpasses

            let pass_descriptor = framebuffer.0;
            // TODO: we may want to copy here because we will modify the Framebuffer (by setting
            // clear colors)
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

            let render_encoder = command_buffer.command_buffer.new_render_command_encoder(pass_descriptor);
            defer_on_unwind! { render_encoder.release() };

            // Apply previously bound values for this command buffer
            if let Some(viewport) = command_buffer.viewport {
                render_encoder.set_viewport(viewport);
            }
            if let Some(scissors) = command_buffer.scissors {
                render_encoder.set_scissor_rect(scissors);
            }
            if let Some(pipeline_state) = command_buffer.pipeline_state {
                render_encoder.set_render_pipeline_state(pipeline_state);
            } else {
                panic!("missing bound pipeline state object");
            }
            for (i, &(buffer, offset)) in command_buffer.vertex_buffers.iter().enumerate() {
                render_encoder.set_vertex_buffer(i as u64, offset as u64, buffer);
            }
            // Interpret descriptor sets
            for set in command_buffer.descriptor_sets.iter().filter_map(|x| x.as_ref()) {
                use native::DescriptorSetBinding::*;

                let set = set.lock().unwrap();

                for (&binding, values) in set.bindings.iter() {
                    let layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                    if layout.stage_flags.contains(shade::STAGE_PIXEL) {
                        match *values {
                            Sampler(ref samplers) => {
                                if samplers.len() > 1 {
                                    unimplemented!()
                                }

                                let sampler = samplers[0];
                                render_encoder.set_fragment_sampler_state(binding as u64, sampler);
                            },
                            SampledImage(ref images) => {
                                if images.len() > 1 {
                                    unimplemented!()
                                }

                                let (image, layout) = images[0]; // TODO: layout?
                                render_encoder.set_fragment_texture(binding as u64, image);
                            },
                            _ => unimplemented!(),
                        }
                    }
                    if layout.stage_flags.contains(shade::STAGE_VERTEX) {
                        unimplemented!()
                    }
                }
            }

            RenderPassInlineBuffer {
                render_encoder,
            }
        }
    }

    fn finish(&mut self,
              command_buffer: &mut CommandBuffer,
              render_pass: &native::RenderPass,
              framebuffer: &native::FrameBuffer) {
        unsafe {
            self.render_encoder.end_encoding();
        }
    }

    fn next_subpass(&mut self) -> RenderPassSecondaryBuffer {
        unimplemented!()
    }

    fn next_subpass_inline(&mut self) -> Self {
        unimplemented!()
    }

    fn clear_attachment(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>) {
        unimplemented!()
    }

    fn draw(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, start: VertexCount, count: VertexCount, instance: Option<InstanceParams>) {
        if let Some(instance) = instance {
            unimplemented!()
        } else {
            // FIXME: primitive type
            encoder.pass_buffer.render_encoder.draw_primitives(MTLPrimitiveType::Triangle, start as u64, count as u64);
        }
    }

    fn draw_indexed(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, start: VertexCount, count: VertexCount, base: VertexOffset, instance: Option<InstanceParams>) {
        unimplemented!()
    }

    fn draw_indirect(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>) {
        unimplemented!()
    }

    fn draw_indexed_indirect(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>) {
        unimplemented!()
    }

    fn bind_index_buffer(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, ibv: IndexBufferView<Resources>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, vbs: pso::VertexBufferSet<Resources>) {
        unimplemented!()
    }

    fn set_viewports(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, viewports: &[target::Rect]) {
        unimplemented!()
    }

    fn set_scissors(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, scissors: &[target::Rect]) {
        unimplemented!()
    }

    fn set_ref_values(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, rv: state::RefValues) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, pipeline: &native::GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        unimplemented!()
    }

    fn push_constants(encoder: &mut RenderPassInlineEncoder<CommandBuffer, Resources>) {
        unimplemented!()
    }
}

impl command::RenderPassSecondaryBuffer<CommandBuffer, Resources> for RenderPassSecondaryBuffer {
    fn begin(command_buffer: &mut Encoder<CommandBuffer>,
             render_pass: &native::RenderPass,
             framebuffer: &native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[ClearValue]) -> Self
    {
        unimplemented!()
    }

    fn finish(&mut self,
              command_buffer: &mut CommandBuffer,
              render_pass: &native::RenderPass,
              framebuffer: &native::FrameBuffer) {
        unimplemented!()
    }

    fn next_subpass(&mut self) -> Self {
        unimplemented!()
    }

    fn next_subpass_inline(&mut self) -> RenderPassInlineBuffer {
        unimplemented!()
    }
}
