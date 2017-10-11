//! Dummy backend implementation to test the code for compile errors
//! outside of the graphics development environment.

extern crate gfx_core as core;

use std::ops::Range;
use core::{buffer, command, device, format, image, target, mapping, memory, pass, pool, pso};

/// Dummy backend.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Backend { }
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type CommandQueue = CommandQueue;
    type CommandBuffer = RawCommandBuffer;
    type SubpassCommandBuffer = SubpassCommandBuffer;
    type QueueFamily = QueueFamily;

    type Memory = ();
    type CommandPool = RawCommandPool;
    type SubpassCommandPool = SubpassCommandPool;

    type ShaderModule = ();
    type RenderPass = ();
    type Framebuffer = ();

    type UnboundBuffer = ();
    type Buffer = ();
    type BufferView = ();
    type UnboundImage = ();
    type Image = ();
    type ImageView = ();
    type Sampler = ();

    type ComputePipeline = ();
    type GraphicsPipeline = ();
    type PipelineLayout = ();
    type DescriptorSetLayout = ();
    type DescriptorPool = DescriptorPool;
    type DescriptorSet = ();

    type Fence = ();
    type Semaphore = ();
}

/// Dummy adapter.
pub struct Adapter;
impl core::Adapter<Backend> for Adapter {
    fn open(&self, _: &[(&QueueFamily, core::QueueType, u32)]) -> core::Gpu<Backend> {
        unimplemented!()
    }

    fn get_info(&self) -> &core::AdapterInfo {
        unimplemented!()
    }

    fn get_queue_families(&self) -> &[(QueueFamily, core::QueueType)] {
        unimplemented!()
    }
}

/// Dummy command queue doing nothing.
pub struct CommandQueue;
impl core::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(&mut self, _: core::RawSubmission<Backend>, _: Option<&()>) {
        unimplemented!()
    }
}

/// Dummy device doing nothing.
#[derive(Clone)]
pub struct Device;
impl core::Device<Backend> for Device {
    fn get_features(&self) -> &core::Features {
        unimplemented!()
    }

    fn get_limits(&self) -> &core::Limits {
        unimplemented!()
    }

    fn allocate_memory(&mut self, _: &core::MemoryType, _: u64) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    fn create_renderpass(&mut self, _: &[pass::Attachment], _: &[pass::SubpassDesc], _: &[pass::SubpassDependency]) -> () {
        unimplemented!()
    }

    fn create_pipeline_layout(&mut self, _: &[&()]) -> () {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(
        &mut self,
        _: &[(pso::GraphicsShaderSet<'a, Backend>, &(), pass::Subpass<'a, Backend>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<(), pso::CreationError>> {
        unimplemented!()
    }

    fn create_compute_pipelines<'a>(
        &mut self,
        _: &[(pso::EntryPoint<'a, Backend>, &())],
    ) -> Vec<Result<(), pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &mut self, _: &(),
        _: &[&()],
        _: device::Extent,
    ) -> Result<(), device::FramebufferError> {
        unimplemented!()
    }

    fn create_shader_module(&mut self, _: &[u8]) -> Result<(), device::ShaderError> {
        unimplemented!()
    }

    fn create_sampler(&mut self, _: image::SamplerInfo) -> () {
        unimplemented!()
    }
    fn create_buffer(&mut self, _: u64, _: u64, _: buffer::Usage) -> Result<(), buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&mut self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&mut self, _: &(), _: u64, _: ()) -> Result<(), device::BindError> {
        unimplemented!()
    }

    fn create_buffer_view(&mut self, _: &(), _: Range<u64>) -> Result<(), buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(&mut self, _: image::Kind, _: image::Level, _: format::Format, _: image::Usage)
         -> Result<(), image::CreationError>
    {
        unimplemented!()
    }

    fn get_image_requirements(&mut self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&mut self, _: &(), _: u64, _: ()) -> Result<(), device::BindError> {
        unimplemented!()
    }

    fn create_image_view(&
        mut self,
        _: &(),
        _: format::Format,
        _: format::Swizzle,
        _: image::SubresourceRange,
    ) -> Result<(), image::ViewError> {
        unimplemented!()
    }

    fn create_descriptor_pool(&mut self, _: usize, _: &[pso::DescriptorRangeDesc]) -> DescriptorPool {
        unimplemented!()
    }
    fn create_descriptor_set_layout(&mut self, _: &[pso::DescriptorSetLayoutBinding]) -> () {
        unimplemented!()
    }

    fn update_descriptor_sets(&mut self, _: &[pso::DescriptorSetWrite<Backend>]) {
        unimplemented!()
    }

    fn acquire_mapping_raw(&mut self, _: &(), _: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>
    {
        unimplemented!()
    }

    fn release_mapping_raw(&mut self, _: &(), _: Option<Range<u64>>) {
        unimplemented!()
    }

    fn create_semaphore(&mut self) -> () {
        unimplemented!()
    }

    fn create_fence(&mut self, _: bool) -> () {
        unimplemented!()
    }

    fn reset_fences(&mut self, _: &[&()]) {
        unimplemented!()
    }
    fn wait_for_fences(&mut self, _: &[&()], _: device::WaitFor, _: u32) -> bool {
        unimplemented!()
    }

    fn free_memory(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_shader_module(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_renderpass(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_graphics_pipeline(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_compute_pipeline(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_framebuffer(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_buffer(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_buffer_view(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_image(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_image_view(&mut self, _: ()) {
        unimplemented!()
    }
    fn destroy_sampler(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&mut self, _: DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_fence(&mut self, _: ()) {
        unimplemented!()
    }

    fn destroy_semaphore(&mut self, _: ()) {
        unimplemented!()
    }
}

/// Dummy queue family;
pub struct QueueFamily;
impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 {
        unimplemented!()
    }
}

/// Dummy subpass command buffer.
pub struct SubpassCommandBuffer;

/// Dummy raw command pool.
pub struct RawCommandPool;
impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    unsafe fn from_queue(_: &CommandQueue, _: pool::CommandPoolCreateFlags) -> Self {
        unimplemented!()
    }

    fn allocate(&mut self, _: usize) -> Vec<RawCommandBuffer> {
        unimplemented!()
    }

    unsafe fn free(&mut self, _: Vec<RawCommandBuffer>) {
        unimplemented!()
    }
}

/// Dummy subpass command pool.
pub struct SubpassCommandPool;
impl core::SubpassCommandPool<Backend> for SubpassCommandPool {

}

/// Dummy command buffer, which ignores all the calls.
#[derive(Clone)]
pub struct RawCommandBuffer;
impl core::RawCommandBuffer<Backend> for RawCommandBuffer {
    fn begin(&mut self) {
        unimplemented!()
    }

    fn finish(&mut self) {
        unimplemented!()
    }

    fn reset(&mut self, _: bool) {
        unimplemented!()
    }

    fn pipeline_barrier(
        &mut self,
        _: Range<pso::PipelineStage>,
        _: &[memory::Barrier<Backend>],
    ) {
        unimplemented!()
    }

    fn fill_buffer(&mut self, _: &(), _: Range<u64>, _: u32) {
        unimplemented!()
    }

    fn update_buffer(&mut self, _: &(), _: u64, _: &[u8]) {
        unimplemented!()
    }

    fn clear_color_image(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: image::SubresourceRange,
        _: command::ClearColor,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil_image(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: image::SubresourceRange,
        _: command::ClearDepthStencil,
    ) {
        unimplemented!()
    }

    fn clear_attachments(&mut self, _: &[command::AttachmentClear], _: &[target::Rect]) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: image::ImageLayout,
        _: &[command::ImageResolve],
    ) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, _: buffer::IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Backend>) {
        unimplemented!()
    }

    fn set_viewports(&mut self, _: &[core::Viewport]) {

    }

    fn set_scissors(&mut self, _: &[target::Rect]) {
        unimplemented!()
    }


    fn set_stencil_reference(&mut self, _: target::Stencil, _: target::Stencil) {
        unimplemented!()
    }


    fn set_blend_constants(&mut self, _: target::ColorValue) {
        unimplemented!()
    }


    fn begin_renderpass(
        &mut self,
        _: &(),
        _: &(),
        _: target::Rect,
        _: &[command::ClearValue],
        _: command::SubpassContents,
    ) {
        unimplemented!()
    }

    fn next_subpass(&mut self, _: command::SubpassContents) {
        unimplemented!()
    }

    fn end_renderpass(&mut self) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, _: &()) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        _: &(),
        _: usize,
        _: &[&()],
    ) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, _: &()) {
        unimplemented!()
    }

    fn bind_compute_descriptor_sets(
        &mut self,
        _: &(),
        _: usize,
        _: &[&()],
    ) {
        unimplemented!()
    }

    fn dispatch(&mut self, _: u32, _: u32, _: u32) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, _: &(), _: u64) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, _: &(), _: &(), _: &[command::BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: image::ImageLayout,
        _: &[command::ImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_buffer_to_image(
        &mut self,
        _: &(),
        _: &(),
        _: image::ImageLayout,
        _: &[command::BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_image_to_buffer(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: &[command::BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(&mut self,
        _: Range<core::VertexCount>,
        _: Range<core::InstanceCount>,
    ) {
        unimplemented!()
    }

    fn draw_indexed(
        &mut self,
        _: Range<core::IndexCount>,
        _: core::VertexOffset,
        _: Range<core::InstanceCount>,
    ) {
        unimplemented!()
    }

    fn draw_indirect(&mut self, _: &(), _: u64, _: u32, _: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _: &(),
        _: u64,
        _: u32,
        _: u32,
    ) {
        unimplemented!()
    }
}

// Dummy descriptor pool.
#[derive(Debug)]
pub struct DescriptorPool;
impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, _: &[&()]) -> Vec<()> {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

/// Dummy surface.
pub struct Surface;
impl core::Surface<Backend> for Surface {
    fn get_kind(&self) -> core::image::Kind {
        unimplemented!()
    }

    fn surface_capabilities(&self, _: &Adapter) -> core::SurfaceCapabilities {
        unimplemented!()
    }

    fn supports_queue(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }

    fn build_swapchain<C>(&mut self,
        _: core::SwapchainConfig,
        _: &core::CommandQueue<Backend, C>
    ) -> (Swapchain, core::Backbuffer<Backend>) {
        unimplemented!()
    }
}

/// Dummy swapchain.
pub struct Swapchain;
impl core::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _: core::FrameSync<Backend>) -> core::Frame {
        unimplemented!()
    }

    fn present<C>(
        &mut self,
        _: &mut core::CommandQueue<Backend, C>,
        _: &[&()],
    ) {
        unimplemented!()
    }
}

pub struct Instance;
impl core::Instance<Backend> for Instance {
    fn enumerate_adapters(&self) -> Vec<Adapter> {
        Vec::new()
    }
}
