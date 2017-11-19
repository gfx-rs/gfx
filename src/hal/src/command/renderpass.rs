use std::ops::Range;
use {pso, Backend, IndexCount, InstanceCount, VertexCount, VertexOffset};
use buffer::IndexBufferView;
use queue::{Supports, Graphics};
use super::{
    ColorValue, StencilValue, Rect, Viewport,
    AttachmentClear, ClearValue, CommandBuffer, RawCommandBuffer,
};


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
        render_area: Rect,
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
    pub fn clear_attachments(&mut self, clears: &[AttachmentClear], rects: &[Rect]) {
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
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.0.bind_graphics_pipeline(pipeline)
    }

    ///
    pub fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    ) {
        self.0.bind_graphics_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn set_viewports(&mut self, viewports: &[Viewport]) {
        self.0.set_viewports(viewports)
    }

    ///
    pub fn set_scissors(&mut self, scissors: &[Rect]) {
        self.0.set_scissors(scissors)
    }

    ///
    pub fn set_stencil_reference(&mut self, front: StencilValue, back: StencilValue) {
        self.0.set_stencil_reference(front, back)
    }

    ///
    pub fn set_blend_constants(&mut self, cv: ColorValue) {
        self.0.set_blend_constants(cv)
    }

    // TODO: set_line_width
    // TODO: set_depth_bounds
    // TODO: set_depth_bias
    // TODO: set_stencil_compare_mask
    // TODO: set_stencil_write_mask
    // TODO: push constants
    // TODO: pipeline barrier (postponed)
    // TODO: begin/end query

}

impl<'a, B: Backend> Drop for RenderPassInlineEncoder<'a, B> {
    fn drop(&mut self) {
        self.0.end_renderpass();
    }
}
