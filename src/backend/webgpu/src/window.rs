use hal::{
    format::Format,
    window::{
        AcquireError,
        CreationError,
        Suboptimal,
        SurfaceCapabilities,
        SwapImageIndex,
        SwapchainConfig,
    },
};

use crate::Backend;

#[derive(Debug)]
pub struct Surface;
impl hal::window::Surface<Backend> for Surface {
    fn supports_queue_family(&self, _family: &crate::QueueFamily) -> bool {
        todo!()
    }

    fn capabilities(&self, _physical_device: &crate::PhysicalDevice) -> SurfaceCapabilities {
        todo!()
    }

    fn supported_formats(&self, _physical_device: &crate::PhysicalDevice) -> Option<Vec<Format>> {
        todo!()
    }
}

impl hal::window::PresentationSurface<Backend> for Surface {
    type SwapchainImage = ();

    unsafe fn configure_swapchain(
        &mut self,
        _device: &crate::Device,
        _config: SwapchainConfig,
    ) -> Result<(), CreationError> {
        todo!()
    }

    unsafe fn unconfigure_swapchain(&mut self, _device: &crate::Device) {
        todo!()
    }

    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<Suboptimal>), AcquireError> {
        todo!()
    }
}
