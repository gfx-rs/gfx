use std::borrow::Borrow;
use std::ops::{Range, Deref, DerefMut};
use std::marker::PhantomData;
use {pso, Backend, IndexCount, InstanceCount, VertexCount, VertexOffset};
use buffer::IndexBufferView;
use queue::{Supports, Graphics};
use super::{
    ColorValue, StencilValue, Rect, Viewport,
    AttachmentClear, ClearValue, CommandBuffer, RawCommandBuffer,
    Shot, Level, Primary, Secondary, Submittable, Submit
};

/// Specifies how commands for the following renderpasses will be recorded.
pub enum SubpassContents {
    ///
    Inline,
    ///
    SecondaryBuffers,
}

/// This struct contains all methods for all commands submittable during a subpass.
/// It is used to implement the identical portions of RenderPassInlineEncoder and SubpassCommandBuffer.
pub struct RenderSubpassCommon<'a, B: Backend>(pub(crate) &'a mut B::CommandBuffer);

impl<'a, B: Backend> RenderSubpassCommon<'a, B> {
    ///
    pub fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<Rect>,
    {
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
    pub fn bind_graphics_descriptor_sets<T>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<B::DescriptorSet>,
    {
        self.0.bind_graphics_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn set_viewports<T>(&mut self, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Viewport>,
    {
        self.0.set_viewports(viewports)
    }

    ///
    pub fn set_scissors<T>(&mut self, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Rect>,
    {
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

    ///
    pub fn push_graphics_constants(&mut self, layout: &B::PipelineLayout, stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        self.0.push_graphics_constants(layout, stages, offset, constants);
    }

    // TODO: set_line_width
    // TODO: set_depth_bounds
    // TODO: set_depth_bias
    // TODO: set_stencil_compare_mask
    // TODO: set_stencil_write_mask
    // TODO: pipeline barrier (postponed)
    // TODO: begin/end query
}

///
pub struct RenderPassInlineEncoder<'a, B: Backend, L: Level>(pub(crate) Option<RenderSubpassCommon<'a, B>>, PhantomData<L>)
where B::CommandBuffer: 'a;

impl<'a, B: Backend, L: Level> RenderPassInlineEncoder<'a, B, L> {
    ///
    pub fn new<C, T, S: Shot>(
        cmd_buffer: &'a mut CommandBuffer<B, C, S, L>,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: T,
    ) -> Self
    where
        C: Supports<Graphics>,
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        cmd_buffer.raw.begin_render_pass(
            render_pass,
            frame_buffer,
            render_area,
            clear_values,
            SubpassContents::Inline);
        RenderPassInlineEncoder(Some(RenderSubpassCommon(cmd_buffer.raw)), PhantomData)
    }

    ///
    pub fn next_subpass_inline(mut self) -> Self {
        self.0.as_mut().unwrap().0.next_subpass(SubpassContents::Inline);
        self
    }
}

impl<'a, B: Backend> RenderPassInlineEncoder<'a, B, Primary> {

    ///
    pub fn next_subpass_secondary(mut self) -> RenderPassSecondaryEncoder<'a, B> {
        let buffer = self.0.take().unwrap();
        buffer.0.next_subpass(SubpassContents::SecondaryBuffers);
        RenderPassSecondaryEncoder(Some(buffer.0))
    }
}

impl<'a, B: Backend, L: Level> Deref for RenderPassInlineEncoder<'a, B, L> {
    type Target = RenderSubpassCommon<'a, B>;
    fn deref(&self) -> &RenderSubpassCommon<'a, B> {
        self.0.as_ref().unwrap()
    }
}

impl<'a, B: Backend, L: Level> DerefMut for RenderPassInlineEncoder<'a, B, L> {
    fn deref_mut(&mut self) -> &mut RenderSubpassCommon<'a, B> {
        self.0.as_mut().unwrap()
    }
}

impl<'a, B: Backend, L: Level> Drop for RenderPassInlineEncoder<'a, B, L> {
    fn drop(&mut self) {
        if let Some(ref mut b) = self.0 {
            b.0.end_render_pass();
        }
    }
}

///
pub struct RenderPassSecondaryEncoder<'a, B: Backend>(pub(crate) Option<&'a mut B::CommandBuffer>)
where B::CommandBuffer: 'a;

impl<'a, B: Backend> RenderPassSecondaryEncoder<'a, B> {
    ///
    pub fn new<C, T, S: Shot>(
        cmd_buffer: &'a mut CommandBuffer<B, C, S, Primary>,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: T,
    ) -> Self
    where
        C: Supports<Graphics>,
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        cmd_buffer.raw.begin_render_pass(
            render_pass,
            frame_buffer,
            render_area,
            clear_values,
            SubpassContents::SecondaryBuffers
        );
        RenderPassSecondaryEncoder(Some(cmd_buffer.raw))
    }

    ///
    pub fn execute_commands<I>(&mut self, submits: I)
    where
        I: IntoIterator,
        I::Item: Submittable<'a, B, Subpass, Secondary>,
    {
        let submits = submits.into_iter().collect::<Vec<_>>();
        self.0.as_mut().unwrap().execute_commands(submits.into_iter().map(|submit| unsafe { submit.into_buffer() }));
    }

    ///
    pub fn next_subpass_inline(mut self) -> RenderPassInlineEncoder<'a, B, Primary> {
        let buffer = self.0.take().unwrap();
        buffer.next_subpass(SubpassContents::Inline);
        RenderPassInlineEncoder(Some(RenderSubpassCommon(buffer)), PhantomData)
    }

    ///
    pub fn next_subpass_secondary(mut self) -> Self {
        self.0.as_mut().unwrap().next_subpass(SubpassContents::SecondaryBuffers);
        self
    }
}

impl<'a, B: Backend> Drop for RenderPassSecondaryEncoder<'a, B> {
    fn drop(&mut self) {
        if let Some(ref mut b) = self.0 {
            b.end_render_pass();
        }
    }
}

/// Capability used only for subpass command buffers' Submits.
pub enum Subpass { }

/// A secondary command buffer recorded entirely within a subpass.
pub struct SubpassCommandBuffer<'a, B: Backend, S: Shot>(pub(crate) RenderSubpassCommon<'a, B>, pub(crate) PhantomData<S>);
impl<'a, B: Backend, S: Shot> SubpassCommandBuffer<'a, B, S> {

    ///
    pub unsafe fn new(raw: &mut B::CommandBuffer) -> SubpassCommandBuffer<B, S> {
        SubpassCommandBuffer(RenderSubpassCommon(raw), PhantomData)
    }

    /// Finish recording commands to the command buffer.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(self) -> Submit<B, Subpass, S, Secondary> {
        Submit::new((self.0).0.clone())
    }

}

impl<'a, B: Backend, S: Shot> Deref for SubpassCommandBuffer<'a, B, S> {
    type Target = RenderSubpassCommon<'a, B>;
    fn deref(&self) -> &RenderSubpassCommon<'a, B> {
        &self.0
    }
}

impl<'a, B: Backend, S: Shot> DerefMut for SubpassCommandBuffer<'a, B, S> {
    fn deref_mut(&mut self) -> &mut RenderSubpassCommon<'a, B> {
        &mut self.0
    }
}

impl<'a, B: Backend, S: Shot> Drop for SubpassCommandBuffer<'a, B, S> {
    fn drop(&mut self) {
        (self.0).0.finish();
    }
}
