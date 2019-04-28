use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};

use super::{
    AttachmentClear, ClearValue, ClearValueRaw, CommandBuffer, CommandBufferFlags,
    CommandBufferInheritanceInfo, DescriptorSetOffset, IntoRawCommandBuffer, MultiShot, OneShot,
    Primary, RawCommandBuffer, Secondary, Shot, Submittable,
};
use crate::queue::{Capability, Graphics, Supports};
use crate::{buffer, pass, pso, query};
use crate::{Backend, DrawCount, IndexCount, InstanceCount, VertexCount, VertexOffset};

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
pub struct RenderSubpassCommon<B, C> {
    cmb: C,
    _marker: PhantomData<B>,
}

impl<B: Backend, C: BorrowMut<B::CommandBuffer>> RenderSubpassCommon<B, C> {
    unsafe fn new(cmb: C) -> Self {
        RenderSubpassCommon {
            cmb,
            _marker: PhantomData,
        }
    }

    ///
    pub unsafe fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        self.cmb.borrow_mut().clear_attachments(clears, rects)
    }

    ///
    pub unsafe fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        self.cmb.borrow_mut().draw(vertices, instances)
    }

    ///
    pub unsafe fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        self.cmb
            .borrow_mut()
            .draw_indexed(indices, base_vertex, instances)
    }

    ///
    pub unsafe fn draw_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        draw_count: DrawCount,
        stride: u32,
    ) {
        self.cmb
            .borrow_mut()
            .draw_indirect(buffer, offset, draw_count, stride)
    }
    ///
    pub unsafe fn draw_indexed_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        draw_count: DrawCount,
        stride: u32,
    ) {
        self.cmb
            .borrow_mut()
            .draw_indexed_indirect(buffer, offset, draw_count, stride)
    }

    ///
    pub unsafe fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<B>) {
        self.cmb.borrow_mut().bind_index_buffer(ibv)
    }

    ///
    pub unsafe fn bind_vertex_buffers<I, T>(&mut self, first_binding: pso::BufferIndex, buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<B::Buffer>,
    {
        self.cmb
            .borrow_mut()
            .bind_vertex_buffers(first_binding, buffers);
    }

    ///
    pub unsafe fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.cmb.borrow_mut().bind_graphics_pipeline(pipeline)
    }

    ///
    pub unsafe fn bind_graphics_descriptor_sets<I, J>(
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
        self.cmb
            .borrow_mut()
            .bind_graphics_descriptor_sets(layout, first_set, sets, offsets)
    }

    ///
    pub unsafe fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        self.cmb
            .borrow_mut()
            .set_viewports(first_viewport, viewports)
    }

    ///
    pub unsafe fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        self.cmb.borrow_mut().set_scissors(first_scissor, scissors)
    }

    ///
    pub unsafe fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.cmb.borrow_mut().set_stencil_reference(faces, value);
    }

    ///
    pub unsafe fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.cmb.borrow_mut().set_stencil_read_mask(faces, value);
    }

    ///
    pub unsafe fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.cmb.borrow_mut().set_stencil_write_mask(faces, value);
    }

    ///
    pub unsafe fn set_blend_constants(&mut self, cv: pso::ColorValue) {
        self.cmb.borrow_mut().set_blend_constants(cv)
    }

    ///
    pub unsafe fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        self.cmb.borrow_mut().set_depth_bounds(bounds)
    }

    ///
    pub unsafe fn push_graphics_constants(
        &mut self,
        layout: &B::PipelineLayout,
        stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    ) {
        self.cmb
            .borrow_mut()
            .push_graphics_constants(layout, stages, offset, constants);
    }

    ///
    pub unsafe fn set_line_width(&mut self, width: f32) {
        self.cmb.borrow_mut().set_line_width(width);
    }

    ///
    pub unsafe fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        self.cmb.borrow_mut().set_depth_bias(depth_bias);
    }

    // TODO: pipeline barrier (postponed)

    ///
    pub unsafe fn begin_query(&mut self, query: query::Query<B>, flags: query::ControlFlags) {
        self.cmb.borrow_mut().begin_query(query, flags)
    }

    ///
    pub unsafe fn end_query(&mut self, query: query::Query<B>) {
        self.cmb.borrow_mut().end_query(query)
    }

    ///
    pub unsafe fn write_timestamp(&mut self, stage: pso::PipelineStage, query: query::Query<B>) {
        self.cmb.borrow_mut().write_timestamp(stage, query)
    }
}

/// An object that records commands into a command buffer inline, that is,
/// without secondary command buffers.
pub struct RenderPassInlineEncoder<'a, B: Backend>(
    RenderSubpassCommon<B, &'a mut B::CommandBuffer>,
)
where
    B::CommandBuffer: 'a;

impl<'a, B: Backend> RenderPassInlineEncoder<'a, B> {
    /// Creates a new `RenderPassInlineEncoder`, starting a new render
    /// pass in the given `CommandBuffer`.
    pub unsafe fn new<C, T, S: Shot>(
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
            SubpassContents::Inline,
        );

        RenderPassInlineEncoder(RenderSubpassCommon::new(&mut cmd_buffer.raw))
    }

    /// Start the next subpass.
    pub fn next_subpass_inline(self) -> Self {
        unsafe {
            self.0.cmb.next_subpass(SubpassContents::Inline);
        }
        self
    }

    /// Begins recording a new subpass with secondary buffers.
    pub fn next_subpass_secondary(mut self) -> RenderPassSecondaryEncoder<'a, B> {
        unsafe {
            self.0.cmb.next_subpass(SubpassContents::SecondaryBuffers);
            let cmb = std::ptr::read(&mut self.0.cmb);
            std::mem::forget(self); // Prevent `end_render_pass`
            RenderPassSecondaryEncoder(cmb)
        }
    }
}

impl<'a, B: Backend> Deref for RenderPassInlineEncoder<'a, B> {
    type Target = RenderSubpassCommon<B, &'a mut B::CommandBuffer>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, B: Backend> DerefMut for RenderPassInlineEncoder<'a, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, B: Backend> Drop for RenderPassInlineEncoder<'a, B> {
    fn drop(&mut self) {
        unsafe {
            self.0.cmb.end_render_pass();
        }
    }
}

/// An object that records commands into a command buffer where each command must
/// be a call to execute a secondary command buffer.
pub struct RenderPassSecondaryEncoder<'a, B: Backend>(&'a mut B::CommandBuffer)
where
    B::CommandBuffer: 'a;

impl<'a, B: Backend> RenderPassSecondaryEncoder<'a, B> {
    /// Wraps the given `CommandBuffer` in a `RenderPassSecondaryEncoder`,
    /// starting a new render pass where the actual commands are contained in
    /// secondary command buffers.
    pub unsafe fn new<C, T, S: Shot>(
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

        RenderPassSecondaryEncoder(&mut cmd_buffer.raw)
    }

    /// Executes the given commands as a secondary command buffer.
    pub unsafe fn execute_commands<'b, T, I>(&mut self, cmd_buffers: I)
    where
        T: 'b + Submittable<B, Graphics, Secondary>,
        I: IntoIterator<Item = &'b T>,
    {
        self.0.execute_commands(cmd_buffers)
    }

    /// Starts a new subpass with inline commands.
    pub fn next_subpass_inline(mut self) -> RenderPassInlineEncoder<'a, B> {
        unsafe {
            self.0.next_subpass(SubpassContents::Inline);
            let cmb = std::ptr::read(&mut self.0);
            std::mem::forget(self); // Prevent `end_render_pass`
            RenderPassInlineEncoder(RenderSubpassCommon::new(cmb))
        }
    }

    /// Starts a new subpass with secondary command buffers.
    pub fn next_subpass_secondary(self) -> Self {
        unsafe {
            self.0.next_subpass(SubpassContents::SecondaryBuffers);
        }
        self
    }
}

impl<'a, B: Backend> Drop for RenderPassSecondaryEncoder<'a, B> {
    fn drop(&mut self) {
        unsafe {
            self.0.end_render_pass();
        }
    }
}

/// A secondary command buffer recorded entirely within a subpass.
pub struct SubpassCommandBuffer<B: Backend, S: Shot, R = <B as Backend>::CommandBuffer>(
    RenderSubpassCommon<B, R>,
    PhantomData<S>,
);
impl<B: Backend, S: Shot> SubpassCommandBuffer<B, S> {
    /// Wraps the given `CommandBuffer` in a `SubpassCommandBuffer`, starting
    /// to record a new subpass.
    pub unsafe fn new(raw: B::CommandBuffer) -> Self {
        SubpassCommandBuffer(RenderSubpassCommon::new(raw), PhantomData)
    }

    /// Finish recording commands to the command buffer.
    ///
    /// The command pool must be reset to able to re-record commands.
    pub unsafe fn finish(&mut self) {
        self.0.cmb.finish();
    }
}

impl<B: Backend> SubpassCommandBuffer<B, OneShot> {
    /// Begin recording a one-shot sub-pass command buffer.
    pub unsafe fn begin<'a>(
        &mut self,
        subpass: pass::Subpass<'a, B>,
        framebuffer: Option<&'a B::Framebuffer>,
    ) {
        let flags = CommandBufferFlags::RENDER_PASS_CONTINUE | CommandBufferFlags::ONE_TIME_SUBMIT;
        let inheritance_info = CommandBufferInheritanceInfo {
            subpass: Some(subpass),
            framebuffer,
            ..CommandBufferInheritanceInfo::default()
        };
        self.0.cmb.begin(flags, inheritance_info);
    }
}

impl<B: Backend> SubpassCommandBuffer<B, MultiShot> {
    /// Begin recording a one-shot sub-pass command buffer.
    pub unsafe fn begin<'a>(
        &mut self,
        allow_pending_resubmit: bool,
        subpass: pass::Subpass<'a, B>,
        framebuffer: Option<&'a B::Framebuffer>,
    ) {
        let mut flags = CommandBufferFlags::RENDER_PASS_CONTINUE;
        if allow_pending_resubmit {
            flags |= CommandBufferFlags::SIMULTANEOUS_USE;
        }
        let inheritance_info = CommandBufferInheritanceInfo {
            subpass: Some(subpass),
            framebuffer,
            ..CommandBufferInheritanceInfo::default()
        };
        self.0.cmb.begin(flags, inheritance_info);
    }
}

impl<B: Backend, S: Shot> Deref for SubpassCommandBuffer<B, S> {
    type Target = RenderSubpassCommon<B, B::CommandBuffer>;
    fn deref(&self) -> &RenderSubpassCommon<B, B::CommandBuffer> {
        &self.0
    }
}

impl<B: Backend, S: Shot> DerefMut for SubpassCommandBuffer<B, S> {
    fn deref_mut(&mut self) -> &mut RenderSubpassCommon<B, B::CommandBuffer> {
        &mut self.0
    }
}

impl<B, S, R> Borrow<R> for SubpassCommandBuffer<B, S, R>
where
    B: Backend<CommandBuffer = R>,
    S: Shot,
    R: RawCommandBuffer<B>,
{
    fn borrow(&self) -> &R {
        &self.0.cmb
    }
}

impl<B, C, S> Submittable<B, C, Secondary> for SubpassCommandBuffer<B, S>
where
    B: Backend,
    C: Capability + Supports<Graphics>,
    S: Shot,
{
}

impl<B, S> IntoRawCommandBuffer<B, Graphics> for SubpassCommandBuffer<B, S>
where
    B: Backend,
    S: Shot,
{
    fn into_raw(self) -> B::CommandBuffer {
        self.0.cmb
    }
}
