//! Dummy backend implementation to test the code for compile errors
//! outside of the graphics development environment.

extern crate gfx_hal as hal;

use std::borrow::Borrow;
use std::ops::Range;
use hal::{buffer, command, device, format, image, mapping, memory, pass, pool, pso, query, queue};

/// Dummy backend.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Backend { }
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = RawCommandQueue;
    type CommandBuffer = RawCommandBuffer;
    type SubpassCommandBuffer = SubpassCommandBuffer;

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
    type QueryPool = ();
}

/// Dummy physical device.
pub struct PhysicalDevice;
impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(self, _: Vec<(QueueFamily, Vec<hal::QueuePriority>)>) -> hal::Gpu<Backend> {
        unimplemented!()
    }

    fn format_properties(&self, _: Option<format::Format>) -> format::Properties {
        unimplemented!()
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        unimplemented!()
    }

    fn get_features(&self) -> hal::Features {
        unimplemented!()
    }

    fn get_limits(&self) -> hal::Limits {
        unimplemented!()
    }
}

/// Dummy command queue doing nothing.
pub struct RawCommandQueue;
impl queue::RawCommandQueue<Backend> for RawCommandQueue {
    unsafe fn submit_raw(&mut self, _: queue::RawSubmission<Backend>, _: Option<&()>) {
        unimplemented!()
    }
}

/// Dummy device doing nothing.
pub struct Device;
impl hal::Device<Backend> for Device {
    fn create_command_pool(&self, _: &QueueFamily, _: pool::CommandPoolCreateFlags) -> RawCommandPool {
        unimplemented!()
    }

    fn destroy_command_pool(&self, _: RawCommandPool) {
        unimplemented!()
    }

    fn allocate_memory(&self, _: hal::MemoryTypeId, _: u64) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    fn create_render_pass(&self, _: &[pass::Attachment], _: &[pass::SubpassDesc], _: &[pass::SubpassDependency]) -> () {
        unimplemented!()
    }

    fn create_pipeline_layout(&self, _: &[&()], _: &[(pso::ShaderStageFlags, Range<u32>)]) -> () {
        unimplemented!()
    }

    fn create_graphics_pipelines<'a>(
        &self,
        _: &[pso::GraphicsPipelineDesc<'a, Backend>],
    ) -> Vec<Result<(), pso::CreationError>> {
        unimplemented!()
    }

    fn create_compute_pipelines<'a>(
        &self,
        _: &[pso::ComputePipelineDesc<'a, Backend>],
    ) -> Vec<Result<(), pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(
        &self, _: &(),
        _: &[&()],
        _: device::Extent,
    ) -> Result<(), device::FramebufferError> {
        unimplemented!()
    }

    fn create_shader_module(&self, _: &[u8]) -> Result<(), device::ShaderError> {
        unimplemented!()
    }

    fn create_sampler(&self, _: image::SamplerInfo) -> () {
        unimplemented!()
    }
    fn create_buffer(&self, _: u64, _: buffer::Usage) -> Result<(), buffer::CreationError> {
        unimplemented!()
    }

    fn get_buffer_requirements(&self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_buffer_memory(&self, _: &(), _: u64, _: ()) -> Result<(), device::BindError> {
        unimplemented!()
    }

    fn create_buffer_view(&self, _: &(), _: Option<format::Format>, _: Range<u64>) -> Result<(), buffer::ViewError> {
        unimplemented!()
    }

    fn create_image(&self, _: image::Kind, _: image::Level, _: format::Format, _: image::Usage)
         -> Result<(), image::CreationError>
    {
        unimplemented!()
    }

    fn get_image_requirements(&self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    fn bind_image_memory(&self, _: &(), _: u64, _: ()) -> Result<(), device::BindError> {
        unimplemented!()
    }

    fn create_image_view(
        &self,
        _: &(),
        _: format::Format,
        _: format::Swizzle,
        _: image::SubresourceRange,
    ) -> Result<(), image::ViewError> {
        unimplemented!()
    }

    fn create_descriptor_pool(&self, _: usize, _: &[pso::DescriptorRangeDesc]) -> DescriptorPool {
        unimplemented!()
    }
    fn create_descriptor_set_layout(&self, _: &[pso::DescriptorSetLayoutBinding]) -> () {
        unimplemented!()
    }

    fn update_descriptor_sets(&self, _: &[pso::DescriptorSetWrite<Backend>]) {
        unimplemented!()
    }

    fn acquire_mapping_raw(&self, _: &(), _: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>
    {
        unimplemented!()
    }

    fn release_mapping_raw(&self, _: &(), _: Option<Range<u64>>) {
        unimplemented!()
    }

    fn create_semaphore(&self) -> () {
        unimplemented!()
    }

    fn create_fence(&self, _: bool) -> () {
        unimplemented!()
    }

    fn reset_fences(&self, _: &[&()]) {
        unimplemented!()
    }

    fn wait_for_fences(&self, _: &[&()], _: device::WaitFor, _: u32) -> bool {
        unimplemented!()
    }
    fn get_fence_status(&self, _: &()) -> bool {
        unimplemented!()
    }

    fn create_query_pool(&self, _: query::QueryType, _: u32) -> () {
        unimplemented!()
    }

    fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    fn free_memory(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_shader_module(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_renderpass(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_pipeline_layout(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_graphics_pipeline(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_compute_pipeline(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_framebuffer(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_buffer(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_buffer_view(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_image(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_image_view(&self, _: ()) {
        unimplemented!()
    }
    fn destroy_sampler(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_descriptor_pool(&self, _: DescriptorPool) {
        unimplemented!()
    }

    fn destroy_descriptor_set_layout(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_fence(&self, _: ()) {
        unimplemented!()
    }

    fn destroy_semaphore(&self, _: ()) {
        unimplemented!()
    }

    fn create_swapchain(
        &self,
        _: &mut Surface,
        _: hal::SwapchainConfig,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct QueueFamily;
impl queue::QueueFamily for QueueFamily {
    fn queue_type(&self) -> hal::QueueType {
        unimplemented!()
    }
    fn max_queues(&self) -> usize {
        unimplemented!()
    }
}

/// Dummy subpass command buffer.
pub struct SubpassCommandBuffer;

/// Dummy raw command pool.
pub struct RawCommandPool;
impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
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
impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {

}

/// Dummy command buffer, which ignores all the calls.
#[derive(Clone)]
pub struct RawCommandBuffer;
impl command::RawCommandBuffer<Backend> for RawCommandBuffer {
    fn begin(&mut self) {
        unimplemented!()
    }

    fn finish(&mut self) {
        unimplemented!()
    }

    fn reset(&mut self, _: bool) {
        unimplemented!()
    }

    fn pipeline_barrier<'a, T>(&mut self, _: Range<pso::PipelineStage>, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
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

    fn clear_attachments<T, U>(&mut self, _: T, _: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<command::Rect>,
    {
        unimplemented!()
    }

    fn resolve_image<T>(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: image::ImageLayout,
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageResolve>,
    {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, _: buffer::IndexBufferView<Backend>) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Backend>) {
        unimplemented!()
    }

    fn set_viewports<T>(&mut self, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::Viewport>,
    {
        unimplemented!()
    }

    fn set_scissors<T>(&mut self, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::Rect>,
    {
        unimplemented!()
    }


    fn set_stencil_reference(&mut self, _: command::StencilValue, _: command::StencilValue) {
        unimplemented!()
    }


    fn set_blend_constants(&mut self, _: command::ColorValue) {
        unimplemented!()
    }


    fn begin_renderpass<T>(
        &mut self,
        _: &(),
        _: &(),
        _: command::Rect,
        _: T,
        _: command::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValue>,
    {
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

    fn bind_graphics_descriptor_sets<'a, T>(&mut self, _: &(), _: usize, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<()>,
    {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, _: &()) {
        unimplemented!()
    }

    fn bind_compute_descriptor_sets<'a, T>(&mut self, _: &(), _: usize, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<()>,
    {
        unimplemented!()
    }

    fn dispatch(&mut self, _: u32, _: u32, _: u32) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self, _: &(), _: u64) {
        unimplemented!()
    }

    fn copy_buffer<T>(&mut self, _: &(), _: &(), _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferCopy>,
    {
        unimplemented!()
    }

    fn copy_image<T>(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: image::ImageLayout,
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        unimplemented!()
    }

    fn copy_buffer_to_image<T>(
        &mut self,
        _: &(),
        _: &(),
        _: image::ImageLayout,
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn copy_image_to_buffer<T>(
        &mut self,
        _: &(),
        _: image::ImageLayout,
        _: &(),
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    fn draw(&mut self,
        _: Range<hal::VertexCount>,
        _: Range<hal::InstanceCount>,
    ) {
        unimplemented!()
    }

    fn draw_indexed(
        &mut self,
        _: Range<hal::IndexCount>,
        _: hal::VertexOffset,
        _: Range<hal::InstanceCount>,
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

    fn begin_query(
        &mut self,
        _: query::Query<Backend>,
        _: query::QueryControl,
    ) {
        unimplemented!()
    }

    fn end_query(
        &mut self,
        _: query::Query<Backend>,
    ) {
        unimplemented!()
    }

    fn reset_query_pool(
        &mut self,
        _: &(),
        _: Range<query::QueryId>,
    ) {
        unimplemented!()
    }

    fn write_timestamp(
        &mut self,
        _: pso::PipelineStage,
        _: query::Query<Backend>,
    ) {
        unimplemented!()
    }

    fn push_graphics_constants(
        &mut self,
        _: &(),
        _: pso::ShaderStageFlags,
        _: u32,
        _: &[u32],
    ) {
        unimplemented!()
    }

    fn push_compute_constants(
        &mut self,
        _: &(),
        _: u32,
        _: &[u32],
    ) {
        unimplemented!()
    }
}

// Dummy descriptor pool.
#[derive(Debug)]
pub struct DescriptorPool;
impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, _: &[&()]) -> Vec<()> {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

/// Dummy surface.
pub struct Surface;
impl hal::Surface<Backend> for Surface {
    fn get_kind(&self) -> hal::image::Kind {
        unimplemented!()
    }

    fn capabilities_and_formats(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>) {
        unimplemented!()
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }
}

/// Dummy swapchain.
pub struct Swapchain;
impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _: hal::FrameSync<Backend>) -> hal::Frame {
        unimplemented!()
    }

    fn present<C>(
        &mut self,
        _: &mut hal::CommandQueue<Backend, C>,
        _: &[&()],
    ) {
        unimplemented!()
    }
}

pub struct Instance;
impl hal::Instance for Instance {
    type Backend = Backend;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        unimplemented!()
    }
}
