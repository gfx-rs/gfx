use std::any::Any;
use std::borrow::Borrow;
use std::ops::Range;

use {buffer, pso};
use {Backend, IndexCount, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use image::{Filter, Layout, SubresourceRange};
use memory::{Barrier, Dependencies};
use query::{Query, QueryControl, QueryId};
use super::{
    AttachmentClear, BufferCopy, BufferImageCopy,
    ClearColor, ClearDepthStencil, ClearValue,
    ImageBlit, ImageCopy, ImageResolve, SubpassContents,
};

/// Unsafe variant of `ClearColor`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ClearColorRaw {
    /// `f32` variant
    pub float32: [f32; 4],
    /// `i32` variant
    pub int32: [i32; 4],
    /// `u32` variant
    pub uint32: [u32; 4],
    _align: [u32; 4],
}

/// A variant of `ClearDepthStencil` that has a `#[repr(C)]` layout
/// and so is used when a known layout is needed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ClearDepthStencilRaw {
    /// Depth value
    pub depth: f32,
    /// Stencil value
    pub stencil: u32,
}

/// Unsafe variant of `ClearValue`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ClearValueRaw {
    /// Clear color
    pub color: ClearColorRaw,
    /// Clear depth and stencil
    pub depth_stencil: ClearDepthStencilRaw,
    _align: [u32; 4],
}

bitflags! {
    /// Option flags for various command buffer settings.
    #[derive(Default)]
    pub struct CommandBufferFlags: u32 {
        // TODO: Remove once 'const fn' is stabilized: https://github.com/rust-lang/rust/issues/24111
        /// No flags.
        const EMPTY = 0x0;

        /// Says that the command buffer will be recorded, submitted only once, and then reset and re-filled
        /// for another submission.
        const ONE_TIME_SUBMIT = 0x1;

        /// If set on a secondary command buffer, it says the command buffer takes place entirely inside
        /// a render pass. Ignored on primary command buffer.
        const RENDER_PASS_CONTINUE = 0x2;

        // TODO: I feel like this could be better.
        /// Says that a command buffer can be recorded into multiple primary command buffers,
        /// and submitted to a queue while it is still pending.
        const SIMULTANEOUS_USE = 0x4;
    }
}

/// An enum that indicates at runtime whether a command buffer
/// is primary or secondary, similar to what `command::Primary`
/// and `command::Secondary` do at compile-time.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Primary,
    Secondary,
}

/// A trait that describes all the operations that must be
/// provided by a `Backend`'s command buffer.
pub trait RawCommandBuffer<B: Backend>: Clone + Any + Send + Sync {
    /// Begins recording commands to a command buffer.
    fn begin(&mut self, flags: CommandBufferFlags);

    /// Finish recording commands to a command buffer.
    fn finish(&mut self);

    /// Empties the command buffer, optionally releasing all
    /// resources from the commands that have been submitted.
    fn reset(&mut self, release_resources: bool);

    // TODO: This REALLY needs to be deeper, but it's complicated.
    // Should probably be a whole book chapter on synchronization and stuff really.
    /// Inserts a synchronization dependency between pipeline stages
    /// in the command buffer.
    fn pipeline_barrier<'a, T>(
        &mut self,
        stages: Range<pso::PipelineStage>,
        dependencies: Dependencies,
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'a, B>>;

    /// Fill a buffer with the given `u32` value.
    fn fill_buffer(
        &mut self,
        buffer: &B::Buffer,
        range: Range<buffer::Offset>,
        data: u32,
    );

    /// Copy data from the given slice into a buffer.
    fn update_buffer(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    );

    /// Clears an image to the given color.
    /// Just calls `clear_color_raw` with some minor type conversion.
    fn clear_color_image(
        &mut self,
        image: &B::Image,
        layout: Layout,
        range: SubresourceRange,
        cv: ClearColor,
    ) {
        self.clear_color_image_raw(
            image,
            layout,
            range,
            cv.into(),
        )
    }

    /// Clears an image to the given color.
    fn clear_color_image_raw(
        &mut self,
        &B::Image,
        Layout,
        SubresourceRange,
        ClearColorRaw,
    );

    /// Clear a depth-stencil image to the given value.
    /// Just calls `clear_depth_stencil_image_raw` with some minor type conversion.
    fn clear_depth_stencil_image(
        &mut self,
        image: &B::Image,
        layout: Layout,
        range: SubresourceRange,
        cv: ClearDepthStencil,
    ) {
        let cv = ClearDepthStencilRaw {
            depth: cv.0,
            stencil: cv.1,
        };
        self.clear_depth_stencil_image_raw(image, layout, range, cv)
    }

    /// Clear a depth-stencil image to the given value.
    fn clear_depth_stencil_image_raw(
        &mut self,
        &B::Image,
        Layout,
        SubresourceRange,
        ClearDepthStencilRaw,
    );

    /// Takes an iterator of attachments and an iterator of rect's,
    /// and clears the given rect's for *each* attachment.
    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::Rect>;

    /// "Resolves" a multisampled image, converting it into a non-multisampled
    /// image. Takes an iterator of regions to apply the resolution to.
    fn resolve_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: Layout,
        dst: &B::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageResolve>;

    /// Copies regions from the source to destination image,
    /// applying scaling, filtering and potentially format conversion.
    fn blit_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: Layout,
        dst: &B::Image,
        dst_layout: Layout,
        filter: Filter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageBlit>;

    /// Bind the index buffer view, making it the "current" one that draw commands
    /// will operate on.
    fn bind_index_buffer(&mut self, buffer::IndexBufferView<B>);

    /// Bind the vertex buffer set, making it the "current" one that draw commands
    /// will operate on.
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
    fn set_viewports<T>(&mut self, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>;

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
    fn set_scissors<T>(&mut self, rects: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>;

    /// Sets the stencil reference value for comparison operations and store operations.
    /// Will be used on the LHS of stencil compare ops and as store value when the
    /// store op is Reference.
    fn set_stencil_reference(&mut self, front: pso::StencilValue, back: pso::StencilValue);

    /// Set the blend constant values dynamically.
    fn set_blend_constants(&mut self, pso::ColorValue);

    /// Just does some type conversions and calls `begin_render_pass_raw`.
    fn begin_render_pass<T>(
        &mut self,
        render_pass: &B::RenderPass,
        framebuffer: &B::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
        first_subpass: SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ClearValue>
    {
        let clear_values = clear_values
            .into_iter()
            .map(|cv| {
                match *cv.borrow() {
                    ClearValue::Color(ClearColor::Float(cv)) =>
                        ClearValueRaw { color: ClearColorRaw { float32: cv }},
                    ClearValue::Color(ClearColor::Int(cv)) =>
                        ClearValueRaw { color: ClearColorRaw { int32: cv }},
                    ClearValue::Color(ClearColor::Uint(cv)) =>
                        ClearValueRaw { color: ClearColorRaw { uint32: cv }},
                    ClearValue::DepthStencil(ClearDepthStencil(depth, stencil)) =>
                        ClearValueRaw { depth_stencil: ClearDepthStencilRaw { depth, stencil }},
                }
            });

        self.begin_render_pass_raw(
            render_pass,
            framebuffer,
            render_area,
            clear_values,
            first_subpass,
        )
    }

    /// Begins recording commands for a render pass on the given framebuffer.
    /// `render_area` is the section of the framebuffer to render,
    /// `clear_values` is an iterator of `ClearValue`'s to use to use for
    /// `clear_*` commands, one for each attachment of the render pass.
    /// `first_subpass` specifies, for the first subpass, whether the
    /// rendering commands are provided inline or whether the render
    /// pass is composed of subpasses.
    fn begin_render_pass_raw<T>(
        &mut self,
        render_pass: &B::RenderPass,
        framebuffer: &B::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
        first_subpass: SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ClearValueRaw>;

    /// Steps to the next subpass in the current render pass.
    fn next_subpass(&mut self, contents: SubpassContents);

    /// Finishes recording commands for the current a render pass.
    fn end_render_pass(&mut self);

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

    /// Takes an iterator of graphics `DescriptorSet`'s, and binds them to the command buffer.
    /// `first_set` is the index that the first descriptor is mapped to in the command buffer.
    fn bind_graphics_descriptor_sets<T>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<B::DescriptorSet>;

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

    /// Takes an iterator of compute `DescriptorSet`'s, and binds them to the command buffer,
    /// `first_set` is the index that the first descriptor is mapped to in the command buffer.
    fn bind_compute_descriptor_sets<T>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<B::DescriptorSet>;

    /// Execute a workgroup in the compute pipeline. `x`, `y` and `z` are the
    /// number of local workgroups to dispatch along each "axis"; a total of `x`*`y`*`z`
    /// local workgroups will be created.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Command buffer must be in recording state.
    /// - A compute pipeline must be bound using `bind_compute_pipeline`.
    /// - Only queues with compute capability support this function.
    /// - This function must be called outside of a render pass.
    /// - `count` must be less than or equal to `Limits::max_compute_group_count`
    ///
    /// TODO:
    fn dispatch(&mut self, count: WorkGroupCount);

    /// Works similarly to `dispatch()` but reads parameters from the given
    /// buffer during execution.
    fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: buffer::Offset);

    /// Adds a command to copy regions from the source to destination buffer.
    fn copy_buffer<T>(
        &mut self,
        src: &B::Buffer,
        dst: &B::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferCopy>;

    /// Copies regions from the source to the destination images, which
    /// have the given layouts.  No format conversion is done; the source and destination
    /// `Layout`'s **must** have the same sized image formats (such as `Rgba8Unorm` and
    /// `R32`, both of which are 32 bits).
    fn copy_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: Layout,
        dst: &B::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageCopy>;

    /// Copies regions from the source buffer to the destination image.
    fn copy_buffer_to_image<T>(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>;

    /// Copies regions from the source image to the destination buffer.
    fn copy_image_to_buffer<T>(
        &mut self,
        src: &B::Image,
        src_layout: Layout,
        dst: &B::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>;

    // TODO: This explanation needs improvement.
    /// Performs a non-indexed drawing operation, fetching vertex attributes
    /// from the currently bound vertex buffers.  It performs instanced
    /// drawing, drawing `instances.len()`
    /// times with an `instanceIndex` starting with the start of the range.
    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    );

    /// Performs indexed drawing, drawing the range of indices
    /// given by the current index buffer and any bound vertex buffers.
    /// `base_vertex` specifies the vertex offset corresponding to index 0.
    /// That is, the offset into the vertex buffer is `(current_index + base_vertex)`
    ///
    /// It also performs instanced drawing, identical to `draw()`.
    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    );

    /// Functions identically to `draw()`, except the parameters are read
    /// from the given buffer, starting at `offset` and increasing `stride`
    /// bytes with each successive draw.  Performs `draw_count` draws total.
    /// `draw_count` may be zero.
    ///
    /// Each draw command in the buffer is a series of 4 `u32` values specifying,
    /// in order, the number of vertices to draw, the number of instances to draw,
    /// the index of the first vertex to draw, and the instance ID of the first
    /// instance to draw.
    fn draw_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        draw_count: u32,
        stride: u32,
    );

    /// Like `draw_indirect()`, this does indexed drawing a la `draw_indexed()` but
    /// reads the draw parameters out of the given buffer.
    ///
    /// Each draw command in the buffer is a series of 5 values specifying,
    /// in order, the number of indices, the number of instances, the first index,
    /// the vertex offset, and the first instance.  All are `u32`'s except
    /// the vertex offset, which is an `i32`.
    fn draw_indexed_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        draw_count: u32,
        stride: u32,
    );

    /// Begins a query operation.  Queries count operations or record timestamps
    /// resulting from commands that occur between the beginning and end of the query,
    /// and save the results to the query pool.
    fn begin_query(&mut self, query: Query<B>, flags: QueryControl);

    /// End a query.
    fn end_query(&mut self, query: Query<B>);

    /// Reset/clear the values in the given range of the query pool.
    fn reset_query_pool(&mut self, pool: &B::QueryPool, queries: Range<QueryId>);

    /// Requests a timestamp to be written.
    fn write_timestamp(&mut self, pso::PipelineStage, Query<B>);

    /// Modify constant data in a graphics pipeline.
    /// Push constants are intended to modify data in a pipeline more
    /// quickly than a updating the values inside a descriptor set.
    fn push_graphics_constants(
        &mut self,
        layout: &B::PipelineLayout,
        stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    );

    /// Modify constant data in a compute pipeline.
    /// Push constants are intended to modify data in a pipeline more
    /// quickly than a updating the values inside a descriptor set.
    fn push_compute_constants(
        &mut self,
        layout: &B::PipelineLayout,
        offset: u32,
        constants: &[u32],
    );

    /// Execute the given secondary command buffers.
    fn execute_commands<I>(
        &mut self,
        buffers: I,
    ) where
        I: IntoIterator,
        I::Item: Borrow<B::CommandBuffer>;
}
