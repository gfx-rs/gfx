use crate::{Backend, PhysicalDevice, QueueFamily, native};

#[derive(Debug)]
pub struct Surface;

impl hal::Surface<Backend> for Surface {
    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<hal::format::Format>>,
        Vec<hal::PresentMode>,
    ) {
        unimplemented!()
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) extent: hal::window::Extent2D,
    pub(crate) fbos: Vec<native::RawFrameBuffer>,
}

impl hal::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _: u64,
        _: Option<&native::Semaphore>,
        _: Option<&native::Fence>,
    ) -> Result<(hal::SwapImageIndex, Option<hal::window::Suboptimal>), hal::AcquireError> {
        unimplemented!()
    }
}
