use hal::window;
use crate::{Backend, PhysicalDevice, QueueFamily, native};

#[derive(Debug)]
pub struct Surface;

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

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) extent: window::Extent2D,
    pub(crate) fbos: Vec<native::RawFrameBuffer>,
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
