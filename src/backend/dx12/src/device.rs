use core::{buffer, device as d, format, image, mapping, pass, pso};
use core::{Features, HeapType, Limits};
use core::memory::Requirements;
use std::ops::Range;
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

    fn create_renderpass(
        &mut self,
        _attachments: &[pass::Attachment],
        _subpasses: &[pass::SubpassDesc],
        _dependencies: &[pass::SubpassDependency],
    ) -> n::RenderPass {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, _sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(
        &mut self,
        _descs: &[(&n::ShaderLib, &n::PipelineLayout, pass::Subpass<'a, B>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_compute_pipelines(
        &mut self,
        _descs: &[(&n::ShaderLib, pso::EntryPoint, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self,
        _renderpass: &n::RenderPass,
        _color_attachments: &[&n::RenderTargetView],
        _depth_stencil_attachments: &[&n::DepthStencilView],
        _extent: d::Extent,
    ) -> n::FrameBuffer {
        unimplemented!()
    }

    fn create_sampler(&mut self, _sampler_info: image::SamplerInfo) -> n::Sampler {
        unimplemented!()
    }

    fn create_buffer(&mut self, _size: u64, _stride: u64, _usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&mut self, _buffer: &UnboundBuffer) -> Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&mut self, _heap: &n::Heap, _offset: u64, _buffer: UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        unimplemented!()
    }

    fn create_image(
        &mut self,
        _kind: image::Kind,
        _mip_levels: image::Level,
        _format: format::Format,
        _usage: image::Usage,
    ) -> Result<UnboundImage, image::CreationError> {
        unimplemented!()
    }

    fn get_image_requirements(&mut self, _image: &UnboundImage) -> Requirements {
        unimplemented!()
    }

    fn bind_image_memory(
        &mut self,
        _heap: &n::Heap,
        _offset: u64,
        _image: UnboundImage,
    ) -> Result<n::Image, image::CreationError> {
        unimplemented!()
    }

    fn view_buffer_as_constant(
        &mut self,
        _buffer: &n::Buffer,
        _range: Range<u64>,
    ) -> Result<n::ConstantBufferView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self,
        _image: &n::Image,
        _format: format::Format,
        _range: image::SubresourceRange,
    ) -> Result<n::RenderTargetView, d::TargetViewError>
    {
        unimplemented!()
    }

    fn view_image_as_shader_resource(
        &mut self,
        _image: &n::Image,
        _format: format::Format,
    ) -> Result<n::ShaderResourceView, d::TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_unordered_access(
        &mut self,
        _image: &n::Image,
        _format: format::Format,
    ) -> Result<n::UnorderedAccessView, d::TargetViewError> {
        unimplemented!()
    }

    fn create_descriptor_pool(&mut self,
        _max_sets: usize,
        _descriptor_pools: &[pso::DescriptorRangeDesc],
    ) -> n::DescriptorPool
    {
        unimplemented!()
    }

    fn create_descriptor_set_layout(
        &mut self,
        _bindings: &[pso::DescriptorSetLayoutBinding],
    )-> n::DescriptorSetLayout {
        unimplemented!()
    }

    fn update_descriptor_sets(&mut self, _writes: &[pso::DescriptorSetWrite<B>]) {
        unimplemented!()
    }

    fn read_mapping_raw(
        &mut self,
        _buf: &n::Buffer,
        _range: Range<u64>,
    ) -> Result<(*const u8, Mapping), mapping::Error> {
        unimplemented!()
    }

    fn write_mapping_raw(
        &mut self,
        _buf: &n::Buffer,
        _range: Range<u64>,
    ) -> Result<(*mut u8, Mapping), mapping::Error> {
        unimplemented!()
    }

    fn unmap_mapping_raw(&mut self, _mapping: Mapping) {
        unimplemented!()
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
        unimplemented!()
    }

    fn create_fence(&mut self, _signaled: bool) -> n::Fence {
        unimplemented!()
    }

    fn reset_fences(&mut self, _fences: &[&n::Fence]) {
        unimplemented!()
    }

    fn wait_for_fences(&mut self, _fences: &[&n::Fence], _wait: d::WaitFor, _timeout_ms: u32) -> bool {
        unimplemented!()
    }

    fn destroy_heap(&mut self, _heap: n::Heap) {
        unimplemented!()
    }

    fn destroy_shader_lib(&mut self, _shader_lib: n::ShaderLib) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, _rp: n::RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, _pl: n::PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&mut self, _pipeline: n::GraphicsPipeline) {
        unimplemented!()
    }

    fn destroy_compute_pipeline(&mut self, _pipeline: n::ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, _fb: n::FrameBuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&mut self, _buffer: n::Buffer) {
        unimplemented!()
    }

    fn destroy_image(&mut self, _image: n::Image) {
        unimplemented!()
    }

    fn destroy_render_target_view(&mut self, _rtv: n::RenderTargetView) {
        unimplemented!()
    }

    fn destroy_depth_stencil_view(&mut self, _dsv: n::DepthStencilView) {
        unimplemented!()
    }

    fn destroy_constant_buffer_view(&mut self, _: n::ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, _srv: n::ShaderResourceView) {
        unimplemented!()
    }

    fn destroy_unordered_access_view(&mut self, _uav: n::UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, _sampler: n::Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&mut self, _pool: n::DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, _layout: n::DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, _fence: n::Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, _semaphore: n::Semaphore) {
        unimplemented!()
    }
}
