
use std::ops::Range;
use pso;
use {Backend, IndexCount, InstanceCount, VertexCount, VertexOffset};
use buffer::IndexBufferView;
use image::{ImageLayout, SubresourceRange};
use memory::Barrier;
use super::{
    ColorValue, StencilValue, Rect, Viewport,
    AttachmentClear, BufferCopy, BufferImageCopy,
    ClearColor, ClearDepthStencil, ClearValue,
    ImageCopy, ImageResolve, SubpassContents,
};

///
pub trait RawCommandBuffer<B: Backend>: Clone + Send {
    ///
    fn begin(&mut self);

    ///
    fn finish(&mut self);

    ///
    fn reset(&mut self, release_resources: bool);

    ///
    fn pipeline_barrier(
        &mut self,
        stages: Range<pso::PipelineStage>,
        barriers: &[Barrier<B>],
    );

    ///
    fn fill_buffer(
        &mut self,
        buffer: &B::Buffer,
        range: Range<u64>,
        data: u32,
    );

    ///
    fn update_buffer(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        data: &[u8],
    );

    /// Clear color image
    fn clear_color_image(
        &mut self,
        &B::Image,
        ImageLayout,
        SubresourceRange,
        ClearColor,
    );

    /// Clear depth-stencil image
    fn clear_depth_stencil_image(
        &mut self,
        &B::Image,
        ImageLayout,
        SubresourceRange,
        ClearDepthStencil,
    );

    ///
    fn clear_attachments(&mut self, &[AttachmentClear], &[Rect]);

    ///
    fn resolve_image(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[ImageResolve],
    );

    /// Bind index buffer view.
    fn bind_index_buffer(&mut self, IndexBufferView<B>);

    /// Bind vertex buffers.
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<B>);

    /// Set the viewport parameters for the rasterizer.
    ///
    /// Every other viewport, which is not specified in this call,
    /// will be disabled.
    ///
    /// Ensure that the number of set viewports at draw time is equal
    /// (or higher) to the number specified in the bound pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - Number of viewports must be between 1 and `max_viewports`.
    /// - Only queues with graphics capability support this function.
    fn set_viewports(&mut self, &[Viewport]);

    /// Set the scissor rectangles for the rasterizer.
    ///
    /// Every other scissor, which is not specified in this call,
    /// will be disabled.
    ///
    /// Each scissor corresponds to the viewport with the same index.
    ///
    /// Ensure that the number of set scissors at draw time is equal (or higher)
    /// to the number of viewports specified in the bound pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - Number of scissors must be between 1 and `max_viewports`.
    /// - Only queues with graphics capability support this function.
    fn set_scissors(&mut self, &[Rect]);

    ///
    fn set_stencil_reference(&mut self, front: StencilValue, back: StencilValue);

    ///
    fn set_blend_constants(&mut self, ColorValue);

    ///
    fn begin_renderpass(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    );
    ///
    fn next_subpass(&mut self, contents: SubpassContents);

    ///
    fn end_renderpass(&mut self);

    /// Bind a graphics pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - Only queues with graphics capability support this function.
    fn bind_graphics_pipeline(&mut self, &B::GraphicsPipeline);

    ///
    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    );

    /// Bind a compute pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - Only queues with compute capability support this function.
    fn bind_compute_pipeline(&mut self, &B::ComputePipeline);

    ///
    fn bind_compute_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    );

    /// Execute a workgroup in the compute pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - A compute pipeline must be bound using `bind_compute_pipeline`.
    /// - Only queues with compute capability support this function.
    /// - This function must be called outside of a renderpass.
    /// - `(x, y, z)` must be less than or equal to `Limits::max_compute_group_size`
    ///
    /// TODO:
    fn dispatch(&mut self, x: u32, y: u32, z: u32);

    ///
    fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: u64);

    ///
    fn copy_buffer(
        &mut self,
        src: &B::Buffer,
        dst: &B::Buffer,
        regions: &[BufferCopy],
    );

    ///
    fn copy_image(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[ImageCopy],
    );

    ///
    fn copy_buffer_to_image(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[BufferImageCopy],
    );

    ///
    fn copy_image_to_buffer(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Buffer,
        regions: &[BufferImageCopy],
    );

    ///
    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    );

    ///
    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    );

    ///
    fn draw_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    );

    ///
    fn draw_indexed_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    );
}
