use std::borrow::Borrow;
use std::ops::{Range, Deref, DerefMut};
use std::marker::PhantomData;

use {buffer, pso, query};
use {Backend, DrawCount, IndexCount, InstanceCount, VertexCount, VertexOffset};
use queue::{Supports, Graphics};
use super::{
    AttachmentClear, ClearValue, ClearValueRaw, CommandBuffer, RawCommandBuffer,
    Shot, Level, Primary, Secondary, Submittable, Submit, DescriptorSetOffset,
};

/// Specifies how commands for the following renderpasses will be recorded.
pub enum SubpassContents {
    /// Contents of the subpass will be inline in the command buffer,
    /// NOT in secondary command buffers.
    Inline,
    /// Contents of the subpass will be in secondary command buffers, and
    /// the primary command buffer will only contain `execute_command()` calls
    /// until the subpass or render pass is complete.
    SecondaryBuffers,
}

/// This struct contains all methods for all commands submittable during a subpass.
/// It is used to implement the identical portions of RenderPassInlineEncoder and SubpassCommandBuffer.
///
/// Where methods are undocumented, they are identical to the methods on the `RawCommandBuffer`
/// trait with the same names.
pub struct RenderSubpassCommon<'a, B: Backend>(pub(crate) &'a mut B::CommandBuffer);

impl<'a, B: Backend> RenderSubpassCommon<'a, B> {
    ///
    pub fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
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
    pub fn draw_indirect(&mut self, buffer: &B::Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
        self.0.draw_indirect(buffer, offset, draw_count, stride)
    }
    ///
    pub fn draw_indexed_indirect(&mut self, buffer: &B::Buffer, offset: buffer::Offset, draw_count: DrawCount, stride: u32) {
        self.0.draw_indexed_indirect(buffer, offset, draw_count, stride)
    }

    ///
    pub fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<B>) {
        self.0.bind_index_buffer(ibv)
    }

    ///
    pub fn bind_vertex_buffers<I, T>(&mut self, first_binding: u32, buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<B::Buffer>,
    {
        self.0.bind_vertex_buffers(first_binding, buffers);
    }

    ///
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.0.bind_graphics_pipeline(pipeline)
    }

    ///
    pub fn bind_graphics_descriptor_sets<I, J>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<B::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<DescriptorSetOffset>,
    {
        self.0.bind_graphics_descriptor_sets(layout, first_set, sets, offsets)
    }

    ///
    pub fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        self.0.set_viewports(first_viewport, viewports)
    }

    ///
    pub fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        self.0.set_scissors(first_scissor, scissors)
    }

    ///
    pub fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.0.set_stencil_reference(faces, value);
    }

    ///
    pub fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.0.set_stencil_read_mask(faces, value);
    }

    ///
    pub fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.0.set_stencil_write_mask(faces, value);
    }

    ///
    pub fn set_blend_constants(&mut self, cv: pso::ColorValue) {
        self.0.set_blend_constants(cv)
    }

    ///
    pub fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        self.0.set_depth_bounds(bounds)
    }

    ///
    pub fn push_graphics_constants(&mut self, layout: &B::PipelineLayout, stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        self.0.push_graphics_constants(layout, stages, offset, constants);
    }

    ///
    pub fn set_line_width(&mut self, width: f32) {
        self.0.set_line_width(width);
    }

    ///
    pub fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        self.0.set_depth_bias(depth_bias);
    }

    // TODO: pipeline barrier (postponed)

    ///
    pub fn begin_query(&mut self, query: query::Query<B>, flags: query::ControlFlags) {
        self.0.begin_query(query, flags)
    }

    ///
    pub fn end_query(&mut self, query: query::Query<B>) {
        self.0.end_query(query)
    }

    ///
    pub fn write_timestamp(&mut self, stage: pso::PipelineStage, query: query::Query<B>) {
        self.0.write_timestamp(stage, query)
    }
}

/// An object that records commands into a command buffer inline, that is,
/// without secondary command buffers.
pub struct RenderPassInlineEncoder<'a, B: Backend, L: Level>(pub(crate) Option<RenderSubpassCommon<'a, B>>, PhantomData<L>)
where B::CommandBuffer: 'a;

impl<'a, B: Backend, L: Level> RenderPassInlineEncoder<'a, B, L> {
    /// Creates a new `RenderPassInlineEncoder`, starting a new render
    /// pass in the given `CommandBuffer`.
    pub fn new<C, T, S: Shot>(
        cmd_buffer: &'a mut CommandBuffer<B, C, S, L>,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
    ) -> Self
    where
        C: Supports<Graphics>,
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        let clear_values = clear_values
            .into_iter()
            .map(|cv| ClearValueRaw::from(*cv.borrow()));

        cmd_buffer.raw.begin_render_pass(
            render_pass,
            frame_buffer,
            render_area,
            clear_values,
            SubpassContents::Inline,
        );

        RenderPassInlineEncoder(
            Some(RenderSubpassCommon(cmd_buffer.raw)),
            PhantomData,
        )
    }

    /// Start the next subpass.
    pub fn next_subpass_inline(mut self) -> Self {
        self.0.as_mut().unwrap().0.next_subpass(SubpassContents::Inline);
        self
    }
}

impl<'a, B: Backend> RenderPassInlineEncoder<'a, B, Primary> {

    /// Begins recording a new subpass with secondary buffers.
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

/// An object that records commands into a command buffer where each command must
/// be a call to execute a secondary command buffer.
pub struct RenderPassSecondaryEncoder<'a, B: Backend>(pub(crate) Option<&'a mut B::CommandBuffer>)
where B::CommandBuffer: 'a;

impl<'a, B: Backend> RenderPassSecondaryEncoder<'a, B> {
    /// Wraps the given `CommandBuffer` in a `RenderPassSecondaryEncoder`,
    /// starting a new render pass where the actual commands are contained in
    /// secondary command buffers.
    pub fn new<C, T, S: Shot>(
        cmd_buffer: &'a mut CommandBuffer<B, C, S, Primary>,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
    ) -> Self
    where
        C: Supports<Graphics>,
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        let clear_values = clear_values
            .into_iter()
            .map(|cv| ClearValueRaw::from(*cv.borrow()));

        cmd_buffer.raw.begin_render_pass(
            render_pass,
            frame_buffer,
            render_area,
            clear_values,
            SubpassContents::SecondaryBuffers,
        );

        RenderPassSecondaryEncoder(
            Some(cmd_buffer.raw),
        )
    }

    /// Executes the given commands as a secondary command buffer.
    pub fn execute_commands<I>(&mut self, submits: I)
    where
        I: IntoIterator,
        I::Item: Submittable<'a, B, Subpass, Secondary>,
    {
        let submits = submits.into_iter().collect::<Vec<_>>();
        self.0.as_mut().unwrap().execute_commands(submits.into_iter().map(|submit| unsafe { submit.into_buffer() }));
    }

    /// Starts a new subpass with inline commands.
    pub fn next_subpass_inline(mut self) -> RenderPassInlineEncoder<'a, B, Primary> {
        let buffer = self.0.take().unwrap();
        buffer.next_subpass(SubpassContents::Inline);
        RenderPassInlineEncoder(Some(RenderSubpassCommon(buffer)), PhantomData)
    }

    /// Starts a new subpass with secondary command buffers.
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

    /// Wraps the given `CommandBuffer` in a `SubpassCommandBuffer`, starting
    /// to record a new subpass.
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
