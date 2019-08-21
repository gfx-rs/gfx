//! Dummy backend implementation to test the code for compile errors
//! outside of the graphics development environment.

extern crate gfx_hal as hal;

use hal::range::RangeArg;
use hal::{
    adapter,
    buffer,
    command,
    device,
    error,
    format,
    image,
    mapping,
    memory,
    pass,
    pool,
    pso,
    query,
    queue,
    window,
};
use std::borrow::Borrow;
use std::ops::Range;

/// Dummy backend.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = CommandQueue;
    type CommandBuffer = CommandBuffer;

    type Memory = ();
    type CommandPool = CommandPool;

    type ShaderModule = ();
    type RenderPass = ();
    type Framebuffer = ();

    type Buffer = ();
    type BufferView = ();
    type Image = ();
    type ImageView = ();
    type Sampler = ();

    type ComputePipeline = ();
    type GraphicsPipeline = ();
    type PipelineCache = ();
    type PipelineLayout = ();
    type DescriptorSetLayout = ();
    type DescriptorPool = DescriptorPool;
    type DescriptorSet = ();

    type Fence = ();
    type Semaphore = ();
    type Event = ();
    type QueryPool = ();
}

/// Dummy physical device.
#[derive(Debug)]
pub struct PhysicalDevice;
impl adapter::PhysicalDevice<Backend> for PhysicalDevice {
    unsafe fn open(
        &self,
        _: &[(&QueueFamily, &[queue::QueuePriority])],
        _: hal::Features,
    ) -> Result<adapter::Gpu<Backend>, error::DeviceCreationError> {
        unimplemented!()
    }

    fn format_properties(&self, _: Option<format::Format>) -> format::Properties {
        unimplemented!()
    }

    fn image_format_properties(
        &self,
        _: format::Format,
        _dim: u8,
        _: image::Tiling,
        _: image::Usage,
        _: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        unimplemented!()
    }

    fn memory_properties(&self) -> adapter::MemoryProperties {
        unimplemented!()
    }

    fn features(&self) -> hal::Features {
        unimplemented!()
    }

    fn limits(&self) -> hal::Limits {
        unimplemented!()
    }
}

/// Dummy command queue doing nothing.
#[derive(Debug)]
pub struct CommandQueue;
impl queue::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        _: queue::Submission<Ic, Iw, Is>,
        _: Option<&()>,
    ) where
        T: 'a + Borrow<CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<()>,
        Iw: IntoIterator<Item = (&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        unimplemented!()
    }

    unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        _: Is,
        _: Iw,
    ) -> Result<Option<window::Suboptimal>, window::PresentError>
    where
        W: 'a + Borrow<Swapchain>,
        Is: IntoIterator<Item = (&'a W, window::SwapImageIndex)>,
        S: 'a + Borrow<()>,
        Iw: IntoIterator<Item = &'a S>,
    {
        unimplemented!()
    }

    unsafe fn present_surface(
        &mut self,
        _surface: &mut Surface,
        _image: (),
        _wait_semaphore: Option<&()>,
    ) -> Result<Option<window::Suboptimal>, window::PresentError> {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }
}

/// Dummy device doing nothing.
#[derive(Debug)]
pub struct Device;
impl device::Device<Backend> for Device {
    unsafe fn create_command_pool(
        &self,
        _: queue::QueueFamilyId,
        _: pool::CommandPoolCreateFlags,
    ) -> Result<CommandPool, device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn destroy_command_pool(&self, _: CommandPool) {
        unimplemented!()
    }

    unsafe fn allocate_memory(
        &self,
        _: hal::MemoryTypeId,
        _: u64,
    ) -> Result<(), device::AllocationError> {
        unimplemented!()
    }

    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        _: IA,
        _: IS,
        _: ID,
    ) -> Result<(), device::OutOfMemory>
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        unimplemented!()
    }

    unsafe fn create_pipeline_layout<IS, IR>(&self, _: IS, _: IR) -> Result<(), device::OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<()>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        unimplemented!()
    }

    unsafe fn create_pipeline_cache(
        &self,
        _data: Option<&[u8]>,
    ) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn get_pipeline_cache_data(&self, _cache: &()) -> Result<Vec<u8>, device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn destroy_pipeline_cache(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn merge_pipeline_caches<I>(&self, _: &(), _: I) -> Result<(), device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
    {
        unimplemented!()
    }

    unsafe fn create_framebuffer<I>(
        &self,
        _: &(),
        _: I,
        _: image::Extent,
    ) -> Result<(), device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
    {
        unimplemented!()
    }

    unsafe fn create_shader_module(&self, _: &[u32]) -> Result<(), device::ShaderError> {
        unimplemented!()
    }

    unsafe fn create_sampler(&self, _: image::SamplerInfo) -> Result<(), device::AllocationError> {
        unimplemented!()
    }
    unsafe fn create_buffer(&self, _: u64, _: buffer::Usage) -> Result<(), buffer::CreationError> {
        unimplemented!()
    }

    unsafe fn get_buffer_requirements(&self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    unsafe fn bind_buffer_memory(
        &self,
        _: &(),
        _: u64,
        _: &mut (),
    ) -> Result<(), device::BindError> {
        unimplemented!()
    }

    unsafe fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        _: &(),
        _: Option<format::Format>,
        _: R,
    ) -> Result<(), buffer::ViewCreationError> {
        unimplemented!()
    }

    unsafe fn create_image(
        &self,
        _: image::Kind,
        _: image::Level,
        _: format::Format,
        _: image::Tiling,
        _: image::Usage,
        _: image::ViewCapabilities,
    ) -> Result<(), image::CreationError> {
        unimplemented!()
    }

    unsafe fn get_image_requirements(&self, _: &()) -> memory::Requirements {
        unimplemented!()
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        _: &(),
        _: image::Subresource,
    ) -> image::SubresourceFootprint {
        unimplemented!()
    }

    unsafe fn bind_image_memory(
        &self,
        _: &(),
        _: u64,
        _: &mut (),
    ) -> Result<(), device::BindError> {
        unimplemented!()
    }

    unsafe fn create_image_view(
        &self,
        _: &(),
        _: image::ViewKind,
        _: format::Format,
        _: format::Swizzle,
        _: image::SubresourceRange,
    ) -> Result<(), image::ViewError> {
        unimplemented!()
    }

    unsafe fn create_descriptor_pool<I>(
        &self,
        _: usize,
        _: I,
        _: pso::DescriptorPoolCreateFlags,
    ) -> Result<DescriptorPool, device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        unimplemented!()
    }

    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        _: I,
        _: J,
    ) -> Result<(), device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<()>,
    {
        unimplemented!()
    }

    unsafe fn write_descriptor_sets<'a, I, J>(&self, _: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        unimplemented!()
    }

    unsafe fn copy_descriptor_sets<'a, I>(&self, _: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        unimplemented!()
    }

    fn create_semaphore(&self) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    fn create_fence(&self, _: bool) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn get_fence_status(&self, _: &()) -> Result<bool, device::DeviceLost> {
        unimplemented!()
    }

    fn create_event(&self) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn get_event_status(&self, _: &()) -> Result<bool, device::OomOrDeviceLost> {
        unimplemented!()
    }

    unsafe fn set_event(&self, _: &()) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn reset_event(&self, _: &()) -> Result<(), device::OutOfMemory> {
        unimplemented!()
    }

    unsafe fn create_query_pool(&self, _: query::Type, _: u32) -> Result<(), query::CreationError> {
        unimplemented!()
    }

    unsafe fn destroy_query_pool(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn get_query_pool_results(
        &self,
        _: &(),
        _: Range<query::Id>,
        _: &mut [u8],
        _: buffer::Offset,
        _: query::ResultFlags,
    ) -> Result<bool, device::OomOrDeviceLost> {
        unimplemented!()
    }

    unsafe fn map_memory<R: RangeArg<u64>>(&self, _: &(), _: R) -> Result<*mut u8, mapping::Error> {
        unimplemented!()
    }

    unsafe fn unmap_memory(&self, _: &()) {
        unimplemented!()
    }

    unsafe fn flush_mapped_memory_ranges<'a, I, R>(&self, _: I) -> Result<(), device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a (), R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I, R>(
        &self,
        _: I,
    ) -> Result<(), device::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a (), R)>,
        R: RangeArg<u64>,
    {
        unimplemented!()
    }

    unsafe fn free_memory(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_shader_module(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_render_pass(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_pipeline_layout(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_graphics_pipeline(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_compute_pipeline(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_framebuffer(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_buffer(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_buffer_view(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_image(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_image_view(&self, _: ()) {
        unimplemented!()
    }
    unsafe fn destroy_sampler(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_descriptor_pool(&self, _: DescriptorPool) {
        unimplemented!()
    }

    unsafe fn destroy_descriptor_set_layout(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_fence(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_semaphore(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn destroy_event(&self, _: ()) {
        unimplemented!()
    }

    unsafe fn create_swapchain(
        &self,
        _: &mut Surface,
        _: window::SwapchainConfig,
        _: Option<Swapchain>,
    ) -> Result<(Swapchain, Vec<()>), hal::window::CreationError> {
        unimplemented!()
    }

    unsafe fn destroy_swapchain(&self, _: Swapchain) {
        unimplemented!()
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct QueueFamily;
impl queue::QueueFamily for QueueFamily {
    fn queue_type(&self) -> queue::QueueType {
        unimplemented!()
    }
    fn max_queues(&self) -> usize {
        unimplemented!()
    }
    fn id(&self) -> queue::QueueFamilyId {
        unimplemented!()
    }
}

/// Dummy raw command pool.
#[derive(Debug)]
pub struct CommandPool;
impl pool::CommandPool<Backend> for CommandPool {
    unsafe fn reset(&mut self, _: bool) {
        unimplemented!()
    }

    unsafe fn free<I>(&mut self, _: I)
    where
        I: IntoIterator<Item = CommandBuffer>,
    {
        unimplemented!()
    }
}

/// Dummy command buffer, which ignores all the calls.
#[derive(Debug)]
pub struct CommandBuffer;
impl command::CommandBuffer<Backend> for CommandBuffer {
    unsafe fn begin(
        &mut self,
        _: command::CommandBufferFlags,
        _: command::CommandBufferInheritanceInfo<Backend>,
    ) {
        unimplemented!()
    }

    unsafe fn finish(&mut self) {
        unimplemented!()
    }

    unsafe fn reset(&mut self, _: bool) {
        unimplemented!()
    }

    unsafe fn pipeline_barrier<'a, T>(
        &mut self,
        _: Range<pso::PipelineStage>,
        _: memory::Dependencies,
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        unimplemented!()
    }

    unsafe fn fill_buffer<R>(&mut self, _: &(), _: R, _: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        unimplemented!()
    }

    unsafe fn update_buffer(&mut self, _: &(), _: buffer::Offset, _: &[u8]) {
        unimplemented!()
    }

    unsafe fn clear_image<T>(&mut self, _: &(), _: image::Layout, _: command::ClearValue, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        unimplemented!()
    }

    unsafe fn clear_attachments<T, U>(&mut self, _: T, _: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        unimplemented!()
    }

    unsafe fn resolve_image<T>(&mut self, _: &(), _: image::Layout, _: &(), _: image::Layout, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageResolve>,
    {
        unimplemented!()
    }

    unsafe fn blit_image<T>(
        &mut self,
        _: &(),
        _: image::Layout,
        _: &(),
        _: image::Layout,
        _: image::Filter,
        _: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageBlit>,
    {
        unimplemented!()
    }

    unsafe fn bind_index_buffer(&mut self, _: buffer::IndexBufferView<Backend>) {
        unimplemented!()
    }

    unsafe fn bind_vertex_buffers<I, T>(&mut self, _: u32, _: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<()>,
    {
        unimplemented!()
    }

    unsafe fn set_viewports<T>(&mut self, _: u32, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        unimplemented!()
    }

    unsafe fn set_scissors<T>(&mut self, _: u32, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        unimplemented!()
    }

    unsafe fn set_stencil_reference(&mut self, _: pso::Face, _: pso::StencilValue) {
        unimplemented!()
    }

    unsafe fn set_stencil_read_mask(&mut self, _: pso::Face, _: pso::StencilValue) {
        unimplemented!()
    }

    unsafe fn set_stencil_write_mask(&mut self, _: pso::Face, _: pso::StencilValue) {
        unimplemented!()
    }

    unsafe fn set_blend_constants(&mut self, _: pso::ColorValue) {
        unimplemented!()
    }

    unsafe fn set_depth_bounds(&mut self, _: Range<f32>) {
        unimplemented!()
    }

    unsafe fn set_line_width(&mut self, _: f32) {
        unimplemented!()
    }

    unsafe fn set_depth_bias(&mut self, _: pso::DepthBias) {
        unimplemented!()
    }

    unsafe fn begin_render_pass<T>(
        &mut self,
        _: &(),
        _: &(),
        _: pso::Rect,
        _: T,
        _: command::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValue>,
    {
        unimplemented!()
    }

    unsafe fn next_subpass(&mut self, _: command::SubpassContents) {
        unimplemented!()
    }

    unsafe fn end_render_pass(&mut self) {
        unimplemented!()
    }

    unsafe fn bind_graphics_pipeline(&mut self, _: &()) {
        unimplemented!()
    }

    unsafe fn bind_graphics_descriptor_sets<I, J>(&mut self, _: &(), _: usize, _: I, _: J)
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
    {
        unimplemented!()
    }

    unsafe fn bind_compute_pipeline(&mut self, _: &()) {
        unimplemented!()
    }

    unsafe fn bind_compute_descriptor_sets<I, J>(&mut self, _: &(), _: usize, _: I, _: J)
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
    {
        unimplemented!()
    }

    unsafe fn dispatch(&mut self, _: hal::WorkGroupCount) {
        unimplemented!()
    }

    unsafe fn dispatch_indirect(&mut self, _: &(), _: buffer::Offset) {
        unimplemented!()
    }

    unsafe fn copy_buffer<T>(&mut self, _: &(), _: &(), _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferCopy>,
    {
        unimplemented!()
    }

    unsafe fn copy_image<T>(&mut self, _: &(), _: image::Layout, _: &(), _: image::Layout, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        unimplemented!()
    }

    unsafe fn copy_buffer_to_image<T>(&mut self, _: &(), _: &(), _: image::Layout, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    unsafe fn copy_image_to_buffer<T>(&mut self, _: &(), _: image::Layout, _: &(), _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        unimplemented!()
    }

    unsafe fn draw(&mut self, _: Range<hal::VertexCount>, _: Range<hal::InstanceCount>) {
        unimplemented!()
    }

    unsafe fn draw_indexed(
        &mut self,
        _: Range<hal::IndexCount>,
        _: hal::VertexOffset,
        _: Range<hal::InstanceCount>,
    ) {
        unimplemented!()
    }

    unsafe fn draw_indirect(&mut self, _: &(), _: buffer::Offset, _: hal::DrawCount, _: u32) {
        unimplemented!()
    }

    unsafe fn draw_indexed_indirect(
        &mut self,
        _: &(),
        _: buffer::Offset,
        _: hal::DrawCount,
        _: u32,
    ) {
        unimplemented!()
    }

    unsafe fn set_event(&mut self, _: &(), _: pso::PipelineStage) {
        unimplemented!()
    }

    unsafe fn reset_event(&mut self, _: &(), _: pso::PipelineStage) {
        unimplemented!()
    }

    unsafe fn wait_events<'a, I, J>(&mut self, _: I, _: Range<pso::PipelineStage>, _: J)
    where
        I: IntoIterator,
        I::Item: Borrow<()>,
        J: IntoIterator,
        J::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        unimplemented!()
    }

    unsafe fn begin_query(&mut self, _: query::Query<Backend>, _: query::ControlFlags) {
        unimplemented!()
    }

    unsafe fn end_query(&mut self, _: query::Query<Backend>) {
        unimplemented!()
    }

    unsafe fn reset_query_pool(&mut self, _: &(), _: Range<query::Id>) {
        unimplemented!()
    }

    unsafe fn copy_query_pool_results(
        &mut self,
        _: &(),
        _: Range<query::Id>,
        _: &(),
        _: buffer::Offset,
        _: buffer::Offset,
        _: query::ResultFlags,
    ) {
        unimplemented!()
    }

    unsafe fn write_timestamp(&mut self, _: pso::PipelineStage, _: query::Query<Backend>) {
        unimplemented!()
    }

    unsafe fn push_graphics_constants(
        &mut self,
        _: &(),
        _: pso::ShaderStageFlags,
        _: u32,
        _: &[u32],
    ) {
        unimplemented!()
    }

    unsafe fn push_compute_constants(&mut self, _: &(), _: u32, _: &[u32]) {
        unimplemented!()
    }

    unsafe fn execute_commands<'a, T, I>(&mut self, _: I)
    where
        T: 'a + Borrow<CommandBuffer>,
        I: IntoIterator<Item = &'a T>,
    {
        unimplemented!()
    }
}

// Dummy descriptor pool.
#[derive(Debug)]
pub struct DescriptorPool;
impl pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn free_sets<I>(&mut self, _descriptor_sets: I)
    where
        I: IntoIterator<Item = ()>,
    {
        unimplemented!()
    }

    unsafe fn reset(&mut self) {
        unimplemented!()
    }
}

/// Dummy surface.
#[derive(Debug)]
pub struct Surface;
impl window::Surface<Backend> for Surface {
    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        window::SurfaceCapabilities,
        Option<Vec<format::Format>>,
        Vec<window::PresentMode>,
    ) {
        unimplemented!()
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }
}
impl window::PresentationSurface<Backend> for Surface {
    type SwapchainImage = ();

    unsafe fn configure_swapchain(
        &mut self,
        _: &Device,
        _: window::SwapchainConfig,
    ) -> Result<(), window::CreationError> {
        unimplemented!()
    }

    unsafe fn unconfigure_swapchain(&mut self, _: &Device) {
        unimplemented!()
    }

    unsafe fn acquire_image(
        &mut self,
        _: u64,
    ) -> Result<((), Option<window::Suboptimal>), window::AcquireError> {
        unimplemented!()
    }
}

/// Dummy swapchain.
#[derive(Debug)]
pub struct Swapchain;
impl window::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _: u64,
        _: Option<&()>,
        _: Option<&()>,
    ) -> Result<(window::SwapImageIndex, Option<window::Suboptimal>), window::AcquireError> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Instance;

impl Instance {
    /// Create instance.
    pub fn create(_name: &str, _version: u32) -> Self {
        Instance
    }

    pub fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, hal::window::InitError> {
        unimplemented!()
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;
    fn enumerate_adapters(&self) -> Vec<adapter::Adapter<Backend>> {
        vec![]
    }
}
