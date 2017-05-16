use ::{Surface, Resources};
use ::native;

use std::ops::DerefMut;

use core::{self, mapping, memory, target, pso, state, pool, queue, command};
use core::{VertexCount, VertexOffset};
use core::buffer::{IndexBufferView};
use core::command::{InstanceParams, ClearColor, ClearValue, BufferImageCopy, BufferCopy, Encoder};

pub struct QueueFamily {
}

pub struct CommandQueue {
}

pub struct CommandPool {
}

pub struct CommandBuffer {
}

pub struct SubmitInfo {
}

impl core::QueueFamily for QueueFamily {
    type Surface = Surface;

    fn supports_present(&self, _surface: &Surface) -> bool { true }
    fn num_queues(&self) -> u32 { unimplemented!() }
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
        unimplemented!()
    }

    fn wait_idle(&mut self) {
        unimplemented!()
    }
}

impl core::CommandPool for CommandPool {
    type Queue = CommandQueue;
    type PoolBuffer = CommandBuffer;

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, CommandBuffer> {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }
}

impl pool::GraphicsCommandPool for CommandPool {
    fn from_queue<Q>(queue: &mut Q, capacity: usize) -> CommandPool
        where Q: Into<queue::GraphicsQueue<CommandQueue>> + DerefMut<Target=CommandQueue>
    {
        unimplemented!()
    }
}

impl core::CommandBuffer for CommandBuffer {
    type SubmitInfo = SubmitInfo;

    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

impl core::PrimaryCommandBuffer<Resources> for CommandBuffer {
    fn pipeline_barrier<'a>(&mut self, memory_barriers: &[memory::MemoryBarrier], buffer_barriers: &[memory::BufferBarrier<'a, Resources>], image_barriers: &[memory::ImageBarrier<'a, Resources>]) {
        unimplemented!()
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }
}

impl core::GraphicsCommandBuffer<Resources> for CommandBuffer {
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
        unimplemented!()
    }

    fn set_viewports(&mut self, rects: &[target::Rect]) {
        unimplemented!()
    }
    fn set_scissors(&mut self, rects: &[target::Rect]) {
        unimplemented!()
    }
    fn set_ref_values(&mut self, values: state::RefValues) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        unimplemented!()
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
        unimplemented!()
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
        unimplemented!()
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

pub struct RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc: 'cb> {
    command_buffer: &'cb mut command::Encoder<'enc, CommandBuffer>,
    render_pass: &'rp native::RenderPass,
    framebuffer: &'fb native::FrameBuffer,
}

pub struct RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc: 'cb> {
    command_buffer: &'cb mut command::Encoder<'enc, CommandBuffer>,
    render_pass: &'rp native::RenderPass,
    framebuffer: &'fb native::FrameBuffer,
}

impl<'cb, 'rp, 'fb, 'enc> command::RenderPassEncoder<'cb, 'rp, 'fb, 'enc, CommandBuffer, Resources> for RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc>
{
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc>;

    fn begin(command_buffer: &'cb mut Encoder<'enc, CommandBuffer>,
             render_pass: &'rp native::RenderPass,
             framebuffer: &'fb native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[ClearValue]) -> Self
    {
        unimplemented!()
    }

    fn next_subpass(self) -> Self::SecondaryEncoder {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> Self::InlineEncoder {
        unimplemented!()
    }
}


impl<'cb, 'rp, 'fb, 'enc> command::RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, CommandBuffer, Resources> for RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc> {
    fn clear_attachment(&mut self) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instance: Option<InstanceParams>) {
        unimplemented!()
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instance: Option<InstanceParams>) {
        unimplemented!()
    }

    fn draw_indirect(&mut self) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Resources>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        unimplemented!()
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        unimplemented!()
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        unimplemented!()
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        unimplemented!()
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }
}

impl<'cb, 'rp, 'fb, 'enc> command::RenderPassEncoder<'cb, 'rp, 'fb, 'enc, CommandBuffer, Resources> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc> {
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc>;

    fn begin(command_buffer: &'cb mut Encoder<'enc, CommandBuffer>,
             render_pass: &'rp native::RenderPass,
             framebuffer: &'fb native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[command::ClearValue]
    ) -> Self {
        RenderPassSecondaryEncoder {
            command_buffer: command_buffer,
            render_pass: render_pass,
            framebuffer: framebuffer,
        }
    }

    fn next_subpass(self) -> Self::SecondaryEncoder {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> Self::InlineEncoder {
        unimplemented!()
    }
}

impl<'cb, 'rp, 'fb, 'enc> command::RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, CommandBuffer, Resources> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc> {

}

