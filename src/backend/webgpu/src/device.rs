use std::{
    borrow::{Borrow, BorrowMut},
    ops::Range,
};

use hal::{
    buffer,
    device::{
        AllocationError, BindError, DeviceLost, MapError, OutOfMemory, ShaderError, WaitError,
    },
    format, image,
    memory::{Requirements, Segment},
    pass,
    pool::CommandPoolCreateFlags,
    pso, query,
    queue::QueueFamilyId,
    MemoryTypeId,
};

use crate::Backend;

#[derive(Debug)]
pub struct Device;
impl hal::device::Device<Backend> for Device {
    unsafe fn allocate_memory(
        &self,
        _memory_type: MemoryTypeId,
        _size: u64,
    ) -> Result<<Backend as hal::Backend>::Memory, AllocationError> {
        todo!()
    }

    unsafe fn free_memory(&self, _memory: <Backend as hal::Backend>::Memory) {
        todo!()
    }

    unsafe fn create_command_pool(
        &self,
        _family: QueueFamilyId,
        _create_flags: CommandPoolCreateFlags,
    ) -> Result<crate::CommandPool, OutOfMemory> {
        todo!()
    }

    unsafe fn destroy_command_pool(&self, _pool: crate::CommandPool) {
        todo!()
    }

    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        _attachments: IA,
        _subpasses: IS,
        _dependencies: ID,
    ) -> Result<<Backend as hal::Backend>::RenderPass, OutOfMemory>
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        todo!()
    }

    unsafe fn destroy_render_pass(&self, _rp: <Backend as hal::Backend>::RenderPass) {
        todo!()
    }

    unsafe fn create_pipeline_layout<IS, IR>(
        &self,
        _set_layouts: IS,
        _push_constant: IR,
    ) -> Result<<Backend as hal::Backend>::PipelineLayout, OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<<Backend as hal::Backend>::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        todo!()
    }

    unsafe fn destroy_pipeline_layout(&self, _layout: <Backend as hal::Backend>::PipelineLayout) {
        todo!()
    }

    unsafe fn create_pipeline_cache(
        &self,
        _data: Option<&[u8]>,
    ) -> Result<<Backend as hal::Backend>::PipelineCache, OutOfMemory> {
        todo!()
    }

    unsafe fn get_pipeline_cache_data(
        &self,
        _cache: &<Backend as hal::Backend>::PipelineCache,
    ) -> Result<Vec<u8>, OutOfMemory> {
        todo!()
    }

    unsafe fn merge_pipeline_caches<I>(
        &self,
        _target: &<Backend as hal::Backend>::PipelineCache,
        _sources: I,
    ) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::PipelineCache>,
    {
        todo!()
    }

    unsafe fn destroy_pipeline_cache(&self, _cache: <Backend as hal::Backend>::PipelineCache) {
        todo!()
    }

    unsafe fn create_graphics_pipeline<'a>(
        &self,
        _desc: &pso::GraphicsPipelineDesc<'a, Backend>,
        _cache: Option<&<Backend as hal::Backend>::PipelineCache>,
    ) -> Result<<Backend as hal::Backend>::GraphicsPipeline, pso::CreationError> {
        todo!()
    }

    unsafe fn create_graphics_pipelines<'a, I>(
        &self,
        _descs: I,
        _cache: Option<&<Backend as hal::Backend>::PipelineCache>,
    ) -> Vec<Result<<Backend as hal::Backend>::GraphicsPipeline, hal::pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<hal::pso::GraphicsPipelineDesc<'a, Backend>>,
    {
        todo!()
    }

    unsafe fn destroy_graphics_pipeline(
        &self,
        _pipeline: <Backend as hal::Backend>::GraphicsPipeline,
    ) {
        todo!()
    }

    unsafe fn create_compute_pipeline<'a>(
        &self,
        _desc: &hal::pso::ComputePipelineDesc<'a, Backend>,
        _cache: Option<&<Backend as hal::Backend>::PipelineCache>,
    ) -> Result<<Backend as hal::Backend>::ComputePipeline, pso::CreationError> {
        todo!()
    }

    unsafe fn create_compute_pipelines<'a, I>(
        &self,
        _descs: I,
        _cache: Option<&<Backend as hal::Backend>::PipelineCache>,
    ) -> Vec<Result<<Backend as hal::Backend>::ComputePipeline, hal::pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<hal::pso::ComputePipelineDesc<'a, Backend>>,
    {
        todo!()
    }

    unsafe fn destroy_compute_pipeline(
        &self,
        _pipeline: <Backend as hal::Backend>::ComputePipeline,
    ) {
        todo!()
    }

    unsafe fn create_framebuffer<I>(
        &self,
        _pass: &<Backend as hal::Backend>::RenderPass,
        _attachments: I,
        _extent: hal::image::Extent,
    ) -> Result<<Backend as hal::Backend>::Framebuffer, OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::ImageView>,
    {
        todo!()
    }

    unsafe fn destroy_framebuffer(&self, _buf: <Backend as hal::Backend>::Framebuffer) {
        todo!()
    }

    unsafe fn create_shader_module(
        &self,
        _spirv_data: &[u32],
    ) -> Result<<Backend as hal::Backend>::ShaderModule, ShaderError> {
        todo!()
    }

    unsafe fn destroy_shader_module(&self, _shader: <Backend as hal::Backend>::ShaderModule) {
        todo!()
    }

    unsafe fn create_buffer(
        &self,
        _size: u64,
        _usage: buffer::Usage,
    ) -> Result<<Backend as hal::Backend>::Buffer, buffer::CreationError> {
        todo!()
    }

    unsafe fn get_buffer_requirements(
        &self,
        _buf: &<Backend as hal::Backend>::Buffer,
    ) -> Requirements {
        todo!()
    }

    unsafe fn bind_buffer_memory(
        &self,
        _memory: &<Backend as hal::Backend>::Memory,
        _offset: u64,
        _buf: &mut <Backend as hal::Backend>::Buffer,
    ) -> Result<(), BindError> {
        todo!()
    }

    unsafe fn destroy_buffer(&self, _buffer: <Backend as hal::Backend>::Buffer) {
        todo!()
    }

    unsafe fn create_buffer_view(
        &self,
        _buf: &<Backend as hal::Backend>::Buffer,
        _fmt: Option<format::Format>,
        _range: buffer::SubRange,
    ) -> Result<<Backend as hal::Backend>::BufferView, buffer::ViewCreationError> {
        todo!()
    }

    unsafe fn destroy_buffer_view(&self, _view: <Backend as hal::Backend>::BufferView) {
        todo!()
    }

    unsafe fn create_image(
        &self,
        _kind: image::Kind,
        _mip_levels: image::Level,
        _format: format::Format,
        _tiling: image::Tiling,
        _usage: image::Usage,
        _view_caps: image::ViewCapabilities,
    ) -> Result<<Backend as hal::Backend>::Image, image::CreationError> {
        todo!()
    }

    unsafe fn get_image_requirements(
        &self,
        _image: &<Backend as hal::Backend>::Image,
    ) -> Requirements {
        todo!()
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        _image: &<Backend as hal::Backend>::Image,
        _subresource: image::Subresource,
    ) -> image::SubresourceFootprint {
        todo!()
    }

    unsafe fn bind_image_memory(
        &self,
        _memory: &<Backend as hal::Backend>::Memory,
        _offset: u64,
        _image: &mut <Backend as hal::Backend>::Image,
    ) -> Result<(), BindError> {
        todo!()
    }

    unsafe fn destroy_image(&self, _image: <Backend as hal::Backend>::Image) {
        todo!()
    }

    unsafe fn create_image_view(
        &self,
        _image: &<Backend as hal::Backend>::Image,
        _view_kind: image::ViewKind,
        _format: format::Format,
        _swizzle: format::Swizzle,
        _range: image::SubresourceRange,
    ) -> Result<<Backend as hal::Backend>::ImageView, image::ViewCreationError> {
        todo!()
    }

    unsafe fn destroy_image_view(&self, _view: <Backend as hal::Backend>::ImageView) {
        todo!()
    }

    unsafe fn create_sampler(
        &self,
        _desc: &image::SamplerDesc,
    ) -> Result<<Backend as hal::Backend>::Sampler, AllocationError> {
        todo!()
    }

    unsafe fn destroy_sampler(&self, _sampler: <Backend as hal::Backend>::Sampler) {
        todo!()
    }

    unsafe fn create_descriptor_pool<I>(
        &self,
        _max_sets: usize,
        _descriptor_ranges: I,
        _flags: hal::pso::DescriptorPoolCreateFlags,
    ) -> Result<<Backend as hal::Backend>::DescriptorPool, OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        todo!()
    }

    unsafe fn destroy_descriptor_pool(&self, _pool: <Backend as hal::Backend>::DescriptorPool) {
        todo!()
    }

    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        _bindings: I,
        _immutable_samplers: J,
    ) -> Result<<Backend as hal::Backend>::DescriptorSetLayout, OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<<Backend as hal::Backend>::Sampler>,
    {
        todo!()
    }

    unsafe fn destroy_descriptor_set_layout(
        &self,
        _layout: <Backend as hal::Backend>::DescriptorSetLayout,
    ) {
        todo!()
    }

    unsafe fn write_descriptor_set<'a, I>(&self, _op: pso::DescriptorSetWrite<'a, Backend, I>)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        todo!()
    }

    unsafe fn copy_descriptor_set<'a>(&self, _op: pso::DescriptorSetCopy<'a, Backend>) {
        todo!()
    }

    unsafe fn map_memory(
        &self,
        _memory: &mut <Backend as hal::Backend>::Memory,
        _segment: Segment,
    ) -> Result<*mut u8, MapError> {
        todo!()
    }

    unsafe fn flush_mapped_memory_ranges<'a, I>(&self, _ranges: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a <Backend as hal::Backend>::Memory, Segment)>,
    {
        todo!()
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I>(&self, _ranges: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a <Backend as hal::Backend>::Memory, Segment)>,
    {
        todo!()
    }

    unsafe fn unmap_memory(&self, _memory: &mut <Backend as hal::Backend>::Memory) {
        todo!()
    }

    fn create_semaphore(&self) -> Result<<Backend as hal::Backend>::Semaphore, OutOfMemory> {
        todo!()
    }

    unsafe fn destroy_semaphore(&self, _semaphore: <Backend as hal::Backend>::Semaphore) {
        todo!()
    }

    fn create_fence(
        &self,
        _signaled: bool,
    ) -> Result<<Backend as hal::Backend>::Fence, OutOfMemory> {
        todo!()
    }

    unsafe fn reset_fence(
        &self,
        _fence: &mut <Backend as hal::Backend>::Fence,
    ) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn reset_fences<I>(&self, _fences: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: BorrowMut<<Backend as hal::Backend>::Fence>,
    {
        todo!()
    }

    unsafe fn wait_for_fence(
        &self,
        _fence: &<Backend as hal::Backend>::Fence,
        _timeout_ns: u64,
    ) -> Result<bool, WaitError> {
        todo!()
    }

    unsafe fn wait_for_fences<I>(
        &self,
        _fences: I,
        _wait: hal::device::WaitFor,
        _timeout_ns: u64,
    ) -> Result<bool, WaitError>
    where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::Fence>,
    {
        todo!()
    }

    unsafe fn get_fence_status(
        &self,
        _fence: &<Backend as hal::Backend>::Fence,
    ) -> Result<bool, DeviceLost> {
        todo!()
    }

    unsafe fn destroy_fence(&self, _fence: <Backend as hal::Backend>::Fence) {
        todo!()
    }

    fn create_event(&self) -> Result<<Backend as hal::Backend>::Event, OutOfMemory> {
        todo!()
    }

    unsafe fn destroy_event(&self, _event: <Backend as hal::Backend>::Event) {
        todo!()
    }

    unsafe fn get_event_status(
        &self,
        _event: &<Backend as hal::Backend>::Event,
    ) -> Result<bool, WaitError> {
        todo!()
    }

    unsafe fn set_event(
        &self,
        _event: &mut <Backend as hal::Backend>::Event,
    ) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn reset_event(
        &self,
        _event: &mut <Backend as hal::Backend>::Event,
    ) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn create_query_pool(
        &self,
        _ty: query::Type,
        _count: query::Id,
    ) -> Result<<Backend as hal::Backend>::QueryPool, query::CreationError> {
        todo!()
    }

    unsafe fn destroy_query_pool(&self, _pool: <Backend as hal::Backend>::QueryPool) {
        todo!()
    }

    unsafe fn get_query_pool_results(
        &self,
        _pool: &<Backend as hal::Backend>::QueryPool,
        _queries: Range<query::Id>,
        _data: &mut [u8],
        _stride: buffer::Stride,
        _flags: query::ResultFlags,
    ) -> Result<bool, WaitError> {
        todo!()
    }

    fn wait_idle(&self) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn set_image_name(&self, _image: &mut <Backend as hal::Backend>::Image, _name: &str) {
        todo!()
    }

    unsafe fn set_buffer_name(&self, _buffer: &mut <Backend as hal::Backend>::Buffer, _name: &str) {
        todo!()
    }

    unsafe fn set_command_buffer_name(
        &self,
        _command_buffer: &mut <Backend as hal::Backend>::CommandBuffer,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_semaphore_name(
        &self,
        _semaphore: &mut <Backend as hal::Backend>::Semaphore,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_fence_name(&self, _fence: &mut <Backend as hal::Backend>::Fence, _name: &str) {
        todo!()
    }

    unsafe fn set_framebuffer_name(
        &self,
        _framebuffer: &mut <Backend as hal::Backend>::Framebuffer,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_render_pass_name(
        &self,
        _render_pass: &mut <Backend as hal::Backend>::RenderPass,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_descriptor_set_name(
        &self,
        _descriptor_set: &mut <Backend as hal::Backend>::DescriptorSet,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_descriptor_set_layout_name(
        &self,
        _descriptor_set_layout: &mut <Backend as hal::Backend>::DescriptorSetLayout,
        _name: &str,
    ) {
        todo!()
    }

    unsafe fn set_pipeline_layout_name(
        &self,
        _pipeline_layout: &mut <Backend as hal::Backend>::PipelineLayout,
        _name: &str,
    ) {
        // TODO
    }
}
