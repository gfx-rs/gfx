use ::Resources;
use ::native::*;

use std::path::Path;

use core::{self, image, pass, format, mapping, memory, buffer, pso, shade};
use core::factory::*;

pub struct Factory {
}

impl Factory {
    pub fn create_shader_library_from_file<P>(
        &mut self,
        path: P,
    ) -> Result<ShaderLib, shade::CreateShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &mut self,
        source: S,
    ) -> Result<ShaderLib, shade::CreateShaderError> where S: AsRef<str> {
        unimplemented!()
    }
}

impl core::Factory<Resources> for Factory {
    fn create_heap(&mut self, heap_type: &core::HeapType, size: u64) -> Heap {
        unimplemented!()
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> RenderPass {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, sets: &[&DescriptorSetLayout]) -> PipelineLayout {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(&mut self, params: &[(&ShaderLib, &PipelineLayout, core::SubPass<'a, Resources>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<GraphicsPipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_compute_pipelines(&mut self, params: &[(&ShaderLib, pso::EntryPoint, &PipelineLayout)]) -> Vec<Result<ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &RenderPass,
        color_attachments: &[&RenderTargetView], depth_stencil_attachments: &[&DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> FrameBuffer {
        unimplemented!()
    }

    fn create_sampler(&mut self, info: image::SamplerInfo) -> Sampler {
        unimplemented!()
    }

    fn create_buffer(&mut self, size: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> memory::MemoryRequirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&mut self, heap: &Heap, offset: u64, buffer: UnboundBuffer) -> Result<Buffer, buffer::CreationError> {
        unimplemented!()
    }

    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError> {
        unimplemented!()
    }

    fn get_image_requirements(&mut self, image: &UnboundImage) -> memory::MemoryRequirements {
        unimplemented!()
    }

    fn bind_image_memory(&mut self, heap: &Heap, offset: u64, image: UnboundImage) -> Result<Image, image::CreationError> {
        unimplemented!()
    }

    fn view_buffer_as_constant(&mut self, buffer: &Buffer, offset: usize, size: usize) -> Result<ConstantBufferView, TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &Image, format: format::Format) -> Result<RenderTargetView, TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_shader_resource(&mut self, image: &Image, format: format::Format) -> Result<ShaderResourceView, TargetViewError> {
        unimplemented!()
    }

    fn view_image_as_unordered_access(&mut self, image: &Image, format: format::Format) -> Result<UnorderedAccessView, TargetViewError> {
        unimplemented!()
    }

    fn create_descriptor_heap(&mut self, ty: DescriptorHeapType, size: usize) -> DescriptorHeap {
        unimplemented!()
    }

    fn create_descriptor_set_pool(&mut self, heap: &DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[DescriptorPoolDesc]) -> DescriptorSetPool {
        unimplemented!()
    }

    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> DescriptorSetLayout {
        unimplemented!()
    }

    fn create_descriptor_sets(&mut self, set_pool: &mut DescriptorSetPool, layout: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        unimplemented!()
    }

    fn reset_descriptor_set_pool(&mut self, pool: &mut DescriptorSetPool) {
        unimplemented!()
    }

    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<Resources>]) {
        unimplemented!()
    }

    fn read_mapping<'a, T>(&self, buf: &'a Buffer, offset: u64, size: u64)
                               -> Result<mapping::Reader<'a, Resources, T>,
                                         mapping::Error>
        where T: Copy {
        unimplemented!()
    }

    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a Buffer, offset: u64, size: u64)
                                -> Result<mapping::Writer<'a, Resources, T>,
                                          mapping::Error>
        where T: Copy {
        unimplemented!()
    }

    fn create_semaphore(&mut self) -> Semaphore {
        unimplemented!()
    }

    fn create_fence(&mut self, signaled: bool) -> Fence {
        unimplemented!()
    }

    fn reset_fences(&mut self, fences: &[&Fence]) {
        unimplemented!()
    }

    fn destroy_heap(&mut self, heap: Heap) {
        unimplemented!()
    }

    fn destroy_shader_lib(&mut self, lib: ShaderLib) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, pass: RenderPass) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, pipeline_layout: PipelineLayout) {
        unimplemented!()
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: GraphicsPipeline) {
        unimplemented!()
    }

    fn destroy_compute_pipeline(&mut self, pipeline: ComputePipeline) {
        unimplemented!()
    }

    fn destroy_framebuffer(&mut self, buffer: FrameBuffer) {
        unimplemented!()
    }

    fn destroy_buffer(&mut self, buffer: Buffer) {
        unimplemented!()
    }

    fn destroy_image(&mut self, image: Image) {
        unimplemented!()
    }

    fn destroy_render_target_view(&mut self, view: RenderTargetView) {
        unimplemented!()
    }

    fn destroy_constant_buffer_view(&mut self, view: ConstantBufferView) {
        unimplemented!()
    }

    fn destroy_shader_resource_view(&mut self, view: ShaderResourceView) {
        unimplemented!()
    }

    fn destroy_unordered_access_view(&mut self, view: UnorderedAccessView) {
        unimplemented!()
    }

    fn destroy_sampler(&mut self, sampler: Sampler) {
        unimplemented!()
    }

    fn destroy_descriptor_heap(&mut self, heap: DescriptorHeap) {
        unimplemented!()
    }

    fn destroy_descriptor_set_pool(&mut self, pool: DescriptorSetPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, layout: DescriptorSetLayout) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, fence: Fence) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, semaphore: Semaphore) {
        unimplemented!()
    }
}
