
use {pso, target, Backend, IndexCount, VertexCount, VertexOffset};
use buffer::IndexBufferView;
use queue::{Supports, Graphics};
use super::{ClearValue, CommandBuffer, InstanceParams, RawCommandBuffer};


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
        frame_buffer: &B::FrameBuffer,
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
    pub fn draw(&mut self, start: VertexCount, count: VertexCount, instance: Option<InstanceParams>) {
        self.0.draw(start, count, instance)
    }
    ///
    pub fn draw_indexed(&mut self, start: IndexCount, count: IndexCount, base: VertexOffset, instance: Option<InstanceParams>) {
        self.0.draw_indexed(start, count, base, instance)
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
