use std::borrow::Borrow;

use hal::{
    adapter::{Adapter, AdapterInfo, DeviceType, Gpu, MemoryProperties},
    device::CreationError,
    format, image,
    queue::{QueueFamilyId, QueuePriority, QueueType},
    Features,
};

use wasm_bindgen_futures::JsFuture;

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
pub struct Instance(web_sys::Gpu);

impl hal::Instance<Backend> for Instance {
    fn create(_name: &str, _version: u32) -> Result<Self, hal::UnsupportedBackend> {
        // TODO: is there any way to check for WebGPU support
        // before accessing the `gpu` object?
        let gpu = web_sys::window().unwrap().navigator().gpu();

        Ok(Instance(gpu))
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<Backend>> {
        unimplemented!("Please use `enumerate_adapters_async` on WASM")
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

impl Instance {
    /// Enumerates the adapters available from this backend.
    pub async fn enumerate_adapters_async(&self) -> Vec<Adapter<Backend>> {
        let mut options = web_sys::GpuRequestAdapterOptions::new();

        // Request the high-performance dedicated GPU
        options.power_preference(web_sys::GpuPowerPreference::HighPerformance);
        let high_performance_adapter_promise = self.0.request_adapter_with_options(&options);

        // Request the low-power integrated GPU
        options.power_preference(web_sys::GpuPowerPreference::LowPower);
        let low_power_adapter_promise = self.0.request_adapter_with_options(&options);

        let high_performance_adapter = JsFuture::from(high_performance_adapter_promise).await;
        let low_power_adapter = JsFuture::from(low_power_adapter_promise).await;

        let high_performance_adapter = future_request_adapter(high_performance_adapter);
        let low_power_adapter = future_request_adapter(low_power_adapter);

        // If the system has at least two **different** graphics adapters,
        // we can return them and let the application choose.
        if high_performance_adapter != low_power_adapter {
            high_performance_adapter
                .into_iter()
                .chain(low_power_adapter)
                .map(map_wgpu_adapter_to_hal_adapter)
                .collect()
        } else {
            high_performance_adapter
                .into_iter()
                .map(map_wgpu_adapter_to_hal_adapter)
                .collect()
        }
    }
}

type JsFutureResult = Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

// This function was taken from `wgpu-rs`
fn future_request_adapter(result: JsFutureResult) -> Option<web_sys::GpuAdapter> {
    match result {
        Ok(js_value) => Some(web_sys::GpuAdapter::from(js_value)),
        Err(_) => None,
    }
}

fn map_wgpu_adapter_to_hal_adapter(adapter: web_sys::GpuAdapter) -> Adapter<Backend> {
    let info = AdapterInfo {
        name: adapter.name(),
        // WebGPU doesn't provide us with information about the adapter type
        vendor: 0,
        device: 0,
        device_type: DeviceType::Other,
    };
    let physical_device = PhysicalDevice(adapter);
    let queue_family = QueueFamily {};

    Adapter {
        info,
        physical_device,
        queue_families: vec![queue_family],
    }
}

// WASM doesn't have threads yet
unsafe impl std::marker::Send for Instance {}
unsafe impl std::marker::Sync for Instance {}

#[derive(Debug)]
pub struct PhysicalDevice(web_sys::GpuAdapter);

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

    fn capabilities(&self) -> hal::Capabilities {
        todo!()
    }

    fn limits(&self) -> hal::Limits {
        todo!()
    }
}

unsafe impl std::marker::Send for PhysicalDevice {}
unsafe impl std::marker::Sync for PhysicalDevice {}

#[derive(Debug)]
pub struct QueueFamily;

const WEBGPU_QUEUE_FAMILY_ID: QueueFamilyId = QueueFamilyId(1);

impl hal::queue::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType {
        QueueType::General
    }

    fn max_queues(&self) -> usize {
        1
    }

    fn id(&self) -> QueueFamilyId {
        WEBGPU_QUEUE_FAMILY_ID
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
