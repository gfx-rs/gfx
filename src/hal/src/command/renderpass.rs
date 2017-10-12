use std::ops::Range;
use {pso, target, Backend, IndexCount, InstanceCount, VertexCount, VertexOffset};
use buffer::IndexBufferView;
use queue::{Supports, Graphics};
use super::{AttachmentClear, ClearValue, CommandBuffer, RawCommandBuffer};


/// Specifies how commands for the following renderpasses will be recorded.
pub enum SubpassContents {
    ///
    Inline,
    ///
    SecondaryBuffers,
}

///
pub struct RenderPassInlineEncoder<'a, B: Backend>(pub(crate) &'a mut B::CommandBuffer)
where B::CommandBuffer: 'a;

impl<'a, B: Backend> RenderPassInlineEncoder<'a, B> {
    ///
    pub fn new<C>(
        cmd_buffer: &'a mut CommandBuffer<B, C>,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
    ) -> Self
    where
        C: Supports<Graphics>,
    {
        cmd_buffer.raw.begin_renderpass(
            render_pass,
            frame_buffer,
            render_area,
            clear_values,
            SubpassContents::Inline);
        RenderPassInlineEncoder(cmd_buffer.raw)
    }

    ///
    pub fn next_subpass_inline(self) -> Self {
        self.0.next_subpass(SubpassContents::Inline);
        self
    }

    ///
    pub fn clear_attachments(&mut self, clears: &[AttachmentClear], rects: &[target::Rect]) {
        self.0.clear_attachments(clears, rects)
    }

    ///
    pub fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        self.0.draw(vertices, instances)
    }
    ///
    pub fn draw_indexed(&mut self, indices: Range<IndexCount>, base_vertex: VertexOffset, instances: Range<InstanceCount>) {
        self.0.draw_indexed(indices, base_vertex, instances)
    }
    ///
    pub fn draw_indirect(&mut self, buffer: &B::Buffer, offset: u64, draw_count: u32, stride: u32) {
        self.0.draw_indirect(buffer, offset, draw_count, stride)
    }
    ///
    pub fn draw_indexed_indirect(&mut self, buffer: &B::Buffer, offset: u64, draw_count: u32, stride: u32) {
        self.0.draw_indexed_indirect(buffer, offset, draw_count, stride)
    }

    /// Bind index buffer view.
    pub fn bind_index_buffer(&mut self, ibv: IndexBufferView<B>) {
        self.0.bind_index_buffer(ibv)
    }

    /// Bind vertex buffers.
    pub fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<B>) {
        self.0.bind_vertex_buffers(vbs);
    }

    /// Bind a graphics pipeline.
    ///
    /// There is only *one* pipeline slot for compute and graphics.
    /// Calling the corresponding `bind_pipeline` functions will override the slot.
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.0.bind_graphics_pipeline(pipeline)
    }
}

impl<'a, B: Backend> Drop for RenderPassInlineEncoder<'a, B> {
    fn drop(&mut self) {
        self.0.end_renderpass();
    }
}
