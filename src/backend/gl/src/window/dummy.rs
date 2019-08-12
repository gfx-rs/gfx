use crate::{native, Device, Backend, PhysicalDevice, QueueFamily};
use arrayvec::ArrayVec;
use hal::window;

#[derive(Debug)]
pub struct Surface {
    pub(crate) swapchain: Option<Swapchain>,
}

impl window::Surface<Backend> for Surface {
    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        window::SurfaceCapabilities,
        Option<Vec<hal::format::Format>>,
        Vec<window::PresentMode>,
    ) {
        unimplemented!()
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }
}

impl window::PresentationSurface<Backend> for Surface {
    type SwapchainImage = native::ImageView;

    unsafe fn configure_swapchain(
        &mut self, _: &Device, _: window::SwapchainConfig
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

impl window::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _: u64,
        _: Option<&native::Semaphore>,
        _: Option<&native::Fence>,
    ) -> Result<(window::SwapImageIndex, Option<window::Suboptimal>), window::AcquireError> {
        unimplemented!()
    }
}
