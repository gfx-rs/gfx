use std::borrow::Borrow;

use hal::{
    adapter::{Gpu, MemoryProperties},
    device::CreationError,
    format,
    image,
    queue::{QueueFamilyId, QueuePriority, QueueType},
    Features,
};

mod command;
mod device;
mod window;

pub use crate::command::{CommandBuffer, CommandPool, CommandQueue};
pub use crate::device::Device;
pub use crate::window::{Surface, Swapchain};

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}

impl hal::Backend for Backend {
    type Instance = Instance;
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = command::CommandQueue;
    type CommandBuffer = command::CommandBuffer;

    type Memory = ();
    type CommandPool = command::CommandPool;

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

#[derive(Debug)]
pub struct Instance;

impl hal::Instance<Backend> for Instance {
    fn create(_name: &str, _version: u32) -> Result<Self, hal::UnsupportedBackend> {
        todo!()
    }

    fn enumerate_adapters(&self) -> Vec<hal::adapter::Adapter<Backend>> {
        todo!()
    }

    unsafe fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, hal::window::InitError> {
        todo!()
    }

    unsafe fn destroy_surface(&self, _surface: Surface) {
        todo!()
    }
}


#[derive(Debug)]
pub struct PhysicalDevice;

impl hal::adapter::PhysicalDevice<Backend> for PhysicalDevice {
    unsafe fn open(
        &self,
        _families: &[(&<Backend as hal::Backend>::QueueFamily, &[QueuePriority])],
        _requested_features: Features,
    ) -> Result<Gpu<Backend>, CreationError> {
        todo!()
    }

    fn format_properties(&self, _format: Option<hal::format::Format>) -> hal::format::Properties {
        todo!()
    }

    fn image_format_properties(
        &self,
        _format: format::Format,
        _dimensions: u8,
        _tiling: image::Tiling,
        _usage: image::Usage,
        _view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        todo!()
    }

    fn memory_properties(&self) -> MemoryProperties {
        todo!()
    }

    fn features(&self) -> hal::Features {
        todo!()
    }

    fn hints(&self) -> hal::Hints {
        todo!()
    }

    fn limits(&self) -> hal::Limits {
        todo!()
    }
}

#[derive(Debug)]
pub struct QueueFamily;

impl hal::queue::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType {
        todo!()
    }

    fn max_queues(&self) -> usize {
        todo!()
    }

    fn id(&self) -> QueueFamilyId {
        todo!()
    }
}

use hal::pso::AllocationError;

#[derive(Debug)]
pub struct DescriptorPool;

impl hal::pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn allocate_set(
        &mut self,
        _layout: &<Backend as hal::Backend>::DescriptorSetLayout,
    ) -> Result<<Backend as hal::Backend>::DescriptorSet, AllocationError> {
        todo!()
    }

    unsafe fn allocate<I, E>(&mut self, _layouts: I, _list: &mut E) -> Result<(), AllocationError>
    where
        I: IntoIterator,
        I::Item: Borrow<<Backend as hal::Backend>::DescriptorSetLayout>,
        E: Extend<<Backend as hal::Backend>::DescriptorSet>,
    {
        todo!()
    }

    unsafe fn free<I>(&mut self, _descriptor_sets: I)
    where
        I: IntoIterator<Item = <Backend as hal::Backend>::DescriptorSet>,
    {
        todo!()
    }

    unsafe fn reset(&mut self) {
        todo!()
    }
}
