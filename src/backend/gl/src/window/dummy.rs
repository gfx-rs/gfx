use crate::{native, Backend, Device, PhysicalDevice, QueueFamily};
use arrayvec::ArrayVec;
use hal::window;

#[derive(Debug)]
pub struct Instance {}

#[cfg(dummy)]
impl hal::Instance<Backend> for Instance {
    fn create(_: &str, _: u32) -> Result<Self, hal::UnsupportedBackend> {
        Ok(Instance {})
    }
    fn enumerate_adapters(&self) -> Vec<hal::adapter::Adapter<Backend>> {
        Vec::new()
    }
    unsafe fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, hal::window::InitError> {
        unimplemented!()
    }
    unsafe fn destroy_surface(&self, _surface: Surface) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Surface {
    pub(crate) swapchain: Option<Swapchain>,
}

impl window::Surface<Backend> for Surface {
    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }

    fn capabilities(&self, _: &PhysicalDevice) -> window::SurfaceCapabilities {
        unimplemented!()
    }

    fn supported_formats(&self, _: &PhysicalDevice) -> Option<Vec<hal::format::Format>> {
        unimplemented!()
    }
}

impl window::PresentationSurface<Backend> for Surface {
    type SwapchainImage = native::SwapchainImage;

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
    ) -> Result<(Self::SwapchainImage, Option<window::Suboptimal>), window::AcquireError> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) extent: window::Extent2D,
    pub(crate) fbos: ArrayVec<[native::RawFrameBuffer; 0]>,
}
