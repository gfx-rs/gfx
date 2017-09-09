use core::{self as c, device as d, format, image, pass, pso, buffer, mapping};
use core::{Features, Limits, HeapType};
use core::memory::Requirements;
use std::ops::Range;
use std::sync::Arc;
use {native as n, Backend as B, Device};

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Mapping;

#[derive(Debug)]
pub struct UnboundBuffer;
#[derive(Debug)]
pub struct UnboundImage;

// TODO: dummy only
impl d::Device<B> for Device {
    fn get_features(&self) -> &Features { &self.features }
    fn get_limits(&self) -> &Limits { &self.limits }

    fn create_heap(&mut self, _heap_type: &HeapType, _resource_type: d::ResourceHeapType, _size: u64) -> Result<n::Heap, d::ResourceHeapError> {
        unimplemented!()
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> n::RenderPass
    {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(&mut self,
        descs: &[(&n::ShaderLib, &n::PipelineLayout, pass::Subpass<'a, B>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>>
    {
        unimplemented!()
    }

    fn create_compute_pipelines(&mut self,
        descs: &[(&n::ShaderLib, pso::EntryPoint, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>>
    {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self,
        renderpass: &n::RenderPass,
        color_attachments: &[&n::RenderTargetView],
        depth_stencil_attachments: &[&n::DepthStencilView],
        extent: d::Extent,
    ) -> n::FrameBuffer {
        unimplemented!()
    }

    fn create_sampler(&mut self, sampler_info: image::SamplerInfo) -> n::Sampler {
        unimplemented!()
    }

    fn create_buffer(&mut self, size: u64, _stride: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&mut self, heap: &n::Heap, offset: u64, buffer: UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        unimplemented!()
    }

    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError>
    {
        unimplemented!()
    }

    fn get_image_requirements(&mut self, image: &UnboundImage) -> Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&mut self, heap: &n::Heap, offset: u64, image: UnboundImage) -> Result<n::Image, image::CreationError> {
        unimplemented!()
    }

    fn view_buffer_as_constant(&mut self, buffer: &n::Buffer, range: Range<u64>) -> Result<n::ConstantBufferView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self,
        image: &n::Image,
        format: format::Format,
        range: image::SubresourceRange,
    ) -> Result<n::RenderTargetView, d::TargetViewError>
    {
        unimplemented!()
    }

    fn view_image_as_shader_resource(&mut self, image: &n::Image, format: format::Format) -> Result<n::ShaderResourceView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_unordered_access(&mut self, image: &n::Image, format: format::Format) -> Result<n::UnorderedAccessView, d::TargetViewError> {
        unimplemented!()
    }

    fn create_descriptor_pool(&mut self,
        max_sets: usize,
        descriptor_pools: &[pso::DescriptorRangeDesc],
    ) -> n::DescriptorPool
    {
        unimplemented!()
    }

    fn create_descriptor_set_layout(&mut self, bindings: &[pso::DescriptorSetLayoutBinding])-> n::DescriptorSetLayout {
        unimplemented!()
    }

    fn update_descriptor_sets(&mut self, writes: &[pso::DescriptorSetWrite<B>]) {
        unimplemented!()
    }

    fn read_mapping_raw(&mut self, buf: &n::Buffer, range: Range<u64>)
        -> Result<(*const u8, Mapping), mapping::Error>
    {
        unimplemented!()
    }

    fn write_mapping_raw(&mut self, buf: &n::Buffer, range: Range<u64>)
        -> Result<(*mut u8, Mapping), mapping::Error>
    {
        unimplemented!()
    }

    fn unmap_mapping_raw(&mut self, _mapping: Mapping) {
        unimplemented!()
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
        unimplemented!()
    }

    fn create_fence(&mut self, signaled: bool) -> n::Fence {
        unimplemented!()
    }

    fn reset_fences(&mut self, fences: &[&n::Fence]) {
        unimplemented!()
    }

    fn wait_for_fences(&mut self, fences: &[&n::Fence], wait: d::WaitFor, timeout_ms: u32) -> bool {
        unimplemented!()
    }

    fn destroy_heap(&mut self, heap: n::Heap) {
        unimplemented!()
    }

    fn destroy_shader_lib(&mut self, shader_lib: n::ShaderLib) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, rp: n::RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, pl: n::PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: n::GraphicsPipeline) {
        unimplemented!()
    }

    fn destroy_compute_pipeline(&mut self, pipeline: n::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, fb: n::FrameBuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&mut self, buffer: n::Buffer) {
        unimplemented!()
    }

    fn destroy_image(&mut self, image: n::Image) {
        unimplemented!()
    }

    fn destroy_render_target_view(&mut self, rtv: n::RenderTargetView) {
        unimplemented!()
    }

    fn destroy_depth_stencil_view(&mut self, dsv: n::DepthStencilView) {
        unimplemented!()
    }

    fn destroy_constant_buffer_view(&mut self, _: n::ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, srv: n::ShaderResourceView) {
        unimplemented!()
    }

    fn destroy_unordered_access_view(&mut self, uav: n::UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, sampler: n::Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&mut self, pool: n::DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, layout: n::DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, fence: n::Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, semaphore: n::Semaphore) {
        unimplemented!()
    }
}
