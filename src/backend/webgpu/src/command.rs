use std::{borrow::Borrow, ops::Range};

use hal::{
    buffer,
    command::{
        AttachmentClear, BufferCopy, BufferImageCopy, ClearValue, CommandBufferFlags,
        CommandBufferInheritanceInfo, DescriptorSetOffset, ImageBlit, ImageCopy, ImageResolve,
        Level, SubpassContents,
    },
    device::OutOfMemory,
    image::{Filter, Layout, SubresourceRange},
    memory::{Barrier, Dependencies},
    pso, query,
    queue::Submission,
    window::{PresentError, PresentationSurface, Suboptimal, SwapImageIndex},
    DrawCount, IndexCount, IndexType, InstanceCount, TaskCount, VertexCount, VertexOffset,
    WorkGroupCount,
};

use crate::Backend;

#[derive(Debug)]
pub struct CommandQueue;

impl hal::queue::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        _submission: Submission<Ic, Iw, Is>,
        _fence: Option<&<Backend as hal::Backend>::Fence>,
    ) where
        T: 'a + Borrow<<Backend as hal::Backend>::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<<Backend as hal::Backend>::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        todo!()
    }

    unsafe fn submit_without_semaphores<'a, T, Ic>(
        &mut self,
        _command_buffers: Ic,
        _fence: Option<&<Backend as hal::Backend>::Fence>,
    ) where
        T: 'a + Borrow<<Backend as hal::Backend>::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
    {
        todo!()
    }

    unsafe fn present(
        &mut self,
        _surface: &mut <Backend as hal::Backend>::Surface,
        _image: <<Backend as hal::Backend>::Surface as PresentationSurface<Backend>>::SwapchainImage,
        _wait_semaphore: Option<&<Backend as hal::Backend>::Semaphore>,
    ) -> Result<Option<Suboptimal>, PresentError> {
        todo!()
    }

    fn wait_idle(&self) -> Result<(), OutOfMemory> {
        todo!()
    }
}

#[derive(Debug)]
pub struct CommandPool;
impl hal::pool::CommandPool<Backend> for CommandPool {
    unsafe fn reset(&mut self, _release_resources: bool) {
        todo!()
    }

    unsafe fn allocate_one(&mut self, _level: Level) -> CommandBuffer {
        todo!()
    }

    unsafe fn allocate<E>(&mut self, _num: usize, _level: Level, _list: &mut E)
    where
        E: Extend<CommandBuffer>,
    {
        todo!()
    }

    unsafe fn free<I>(&mut self, _buffers: I)
    where
        I: IntoIterator<Item = CommandBuffer>,
    {
        todo!()
    }
}

#[derive(Debug)]
pub struct CommandBuffer;
impl hal::command::CommandBuffer<Backend> for CommandBuffer {
    unsafe fn begin(
        &mut self,
        _flags: CommandBufferFlags,
        _inheritance_info: CommandBufferInheritanceInfo<Backend>,
    ) {
        todo!()
    }

    unsafe fn begin_primary(&mut self, _flags: CommandBufferFlags) {
        todo!()
    }

    unsafe fn finish(&mut self) {
        todo!()
    }

    unsafe fn reset(&mut self, _release_resources: bool) {
        todo!()
    }

    unsafe fn pipeline_barrier<'a, T>(
        &mut self,
        _stages: Range<pso::PipelineStage>,
        _dependencies: Dependencies,
        _barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'a, Backend>>,
    {
        todo!()
    }

    unsafe fn fill_buffer(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _range: buffer::SubRange,
        _data: u32,
    ) {
        todo!()
    }

    unsafe fn update_buffer(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _data: &[u8],
    ) {
        todo!()
    }

    unsafe fn clear_image<T>(
        &mut self,
        _image: &<Backend as hal::Backend>::Image,
        _layout: Layout,
        _value: ClearValue,
        _subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<SubresourceRange>,
    {
        todo!()
    }

    unsafe fn clear_attachments<T, U>(&mut self, _clears: T, _rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        todo!()
    }

    unsafe fn resolve_image<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Image,
        _src_layout: Layout,
        _dst: &<Backend as hal::Backend>::Image,
        _dst_layout: Layout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageResolve>,
    {
        todo!()
    }

    unsafe fn blit_image<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Image,
        _src_layout: Layout,
        _dst: &<Backend as hal::Backend>::Image,
        _dst_layout: Layout,
        _filter: Filter,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageBlit>,
    {
        todo!()
    }

    unsafe fn bind_index_buffer(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _sub: buffer::SubRange,
        _ty: IndexType,
    ) {
        todo!()
    }

    unsafe fn bind_vertex_buffers<I, T>(&mut self, _first_binding: pso::BufferIndex, _buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::SubRange)>,
        T: Borrow<<Backend as hal::Backend>::Buffer>,
    {
        todo!()
    }

    unsafe fn set_viewports<T>(&mut self, _first_viewport: u32, _viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        todo!()
    }

    unsafe fn set_scissors<T>(&mut self, _first_scissor: u32, _rects: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        todo!()
    }

    unsafe fn set_stencil_reference(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        todo!()
    }

    unsafe fn set_stencil_read_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        todo!()
    }

    unsafe fn set_stencil_write_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        todo!()
    }

    unsafe fn set_blend_constants(&mut self, _color: pso::ColorValue) {
        todo!()
    }

    unsafe fn set_depth_bounds(&mut self, _bounds: Range<f32>) {
        todo!()
    }

    unsafe fn set_line_width(&mut self, _width: f32) {
        todo!()
    }

    unsafe fn set_depth_bias(&mut self, _depth_bias: pso::DepthBias) {
        todo!()
    }

    unsafe fn begin_render_pass<T>(
        &mut self,
        _render_pass: &<Backend as hal::Backend>::RenderPass,
        _framebuffer: &<Backend as hal::Backend>::Framebuffer,
        _render_area: pso::Rect,
        _clear_values: T,
        _first_subpass: SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        todo!()
    }

    unsafe fn next_subpass(&mut self, _contents: SubpassContents) {
        todo!()
    }

    unsafe fn end_render_pass(&mut self) {
        todo!()
    }

    unsafe fn bind_graphics_pipeline(
        &mut self,
        _pipeline: &<Backend as hal::Backend>::GraphicsPipeline,
    ) {
        todo!()
    }

    unsafe fn bind_graphics_descriptor_sets<I, J>(
        &mut self,
        _layout: &<Backend as hal::Backend>::PipelineLayout,
        _first_set: usize,
        _sets: I,
        _offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<DescriptorSetOffset>,
    {
        todo!()
    }

    unsafe fn bind_compute_pipeline(
        &mut self,
        _pipeline: &<Backend as hal::Backend>::ComputePipeline,
    ) {
        todo!()
    }

    unsafe fn bind_compute_descriptor_sets<I, J>(
        &mut self,
        _layout: &<Backend as hal::Backend>::PipelineLayout,
        _first_set: usize,
        _sets: I,
        _offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<DescriptorSetOffset>,
    {
        todo!()
    }

    unsafe fn dispatch(&mut self, _count: WorkGroupCount) {
        todo!()
    }

    unsafe fn dispatch_indirect(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
    ) {
        todo!()
    }

    unsafe fn copy_buffer<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Buffer,
        _dst: &<Backend as hal::Backend>::Buffer,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferCopy>,
    {
        todo!()
    }

    unsafe fn copy_image<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Image,
        _src_layout: Layout,
        _dst: &<Backend as hal::Backend>::Image,
        _dst_layout: Layout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageCopy>,
    {
        todo!()
    }

    unsafe fn copy_buffer_to_image<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Buffer,
        _dst: &<Backend as hal::Backend>::Image,
        _dst_layout: Layout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        todo!()
    }

    unsafe fn copy_image_to_buffer<T>(
        &mut self,
        _src: &<Backend as hal::Backend>::Image,
        _src_layout: Layout,
        _dst: &<Backend as hal::Backend>::Buffer,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        todo!()
    }

    unsafe fn draw(&mut self, _vertices: Range<VertexCount>, _instances: Range<InstanceCount>) {
        todo!()
    }

    unsafe fn draw_indexed(
        &mut self,
        _indices: Range<IndexCount>,
        _base_vertex: VertexOffset,
        _instances: Range<InstanceCount>,
    ) {
        todo!()
    }

    unsafe fn draw_indirect(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _draw_count: DrawCount,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn draw_indexed_indirect(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _draw_count: DrawCount,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn draw_indirect_count(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _count_buffer: &<Backend as hal::Backend>::Buffer,
        _count_buffer_offset: buffer::Offset,
        _max_draw_count: u32,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn draw_indexed_indirect_count(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _count_buffer: &<Backend as hal::Backend>::Buffer,
        _count_buffer_offset: buffer::Offset,
        _max_draw_count: u32,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn draw_mesh_tasks(&mut self, _task_count: TaskCount, _first_task: TaskCount) {
        todo!()
    }

    unsafe fn draw_mesh_tasks_indirect(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _draw_count: DrawCount,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn draw_mesh_tasks_indirect_count(
        &mut self,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _count_buffer: &<Backend as hal::Backend>::Buffer,
        _count_buffer_offset: buffer::Offset,
        _max_draw_count: DrawCount,
        _stride: buffer::Stride,
    ) {
        todo!()
    }

    unsafe fn set_event(
        &mut self,
        _event: &<Backend as hal::Backend>::Event,
        _stages: pso::PipelineStage,
    ) {
        todo!()
    }

    unsafe fn reset_event(
        &mut self,
        _event: &<Backend as hal::Backend>::Event,
        _stages: pso::PipelineStage,
    ) {
        todo!()
    }

    unsafe fn wait_events<'a, I, J>(
        &mut self,
        _events: I,
        _stages: Range<pso::PipelineStage>,
        _barriers: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::Event>,
        J: IntoIterator,
        J::Item: Borrow<Barrier<'a, Backend>>,
    {
        todo!()
    }

    unsafe fn begin_query(&mut self, _query: query::Query<Backend>, _flags: query::ControlFlags) {
        todo!()
    }

    unsafe fn end_query(&mut self, _query: query::Query<Backend>) {
        todo!()
    }

    unsafe fn reset_query_pool(
        &mut self,
        _pool: &<Backend as hal::Backend>::QueryPool,
        _queries: Range<query::Id>,
    ) {
        todo!()
    }

    unsafe fn copy_query_pool_results(
        &mut self,
        _pool: &<Backend as hal::Backend>::QueryPool,
        _queries: Range<query::Id>,
        _buffer: &<Backend as hal::Backend>::Buffer,
        _offset: buffer::Offset,
        _stride: buffer::Offset,
        _flags: query::ResultFlags,
    ) {
        todo!()
    }

    unsafe fn write_timestamp(
        &mut self,
        _stage: pso::PipelineStage,
        _query: query::Query<Backend>,
    ) {
        todo!()
    }

    unsafe fn push_graphics_constants(
        &mut self,
        _layout: &<Backend as hal::Backend>::PipelineLayout,
        _stages: pso::ShaderStageFlags,
        _offset: u32,
        _constants: &[u32],
    ) {
        todo!()
    }

    unsafe fn push_compute_constants(
        &mut self,
        _layout: &<Backend as hal::Backend>::PipelineLayout,
        _offset: u32,
        _constants: &[u32],
    ) {
        todo!()
    }

    unsafe fn execute_commands<'a, T, I>(&mut self, _cmd_buffers: I)
    where
        T: 'a + Borrow<<Backend as hal::Backend>::CommandBuffer>,
        I: IntoIterator<Item = &'a T>,
    {
        todo!()
    }

    unsafe fn insert_debug_marker(&mut self, _name: &str, _color: u32) {
        todo!()
    }

    unsafe fn begin_debug_marker(&mut self, _name: &str, _color: u32) {
        todo!()
    }

    unsafe fn end_debug_marker(&mut self) {
        todo!()
    }
}
