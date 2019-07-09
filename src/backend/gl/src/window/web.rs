use crate::hal::window::Extent2D;
use crate::hal::{self, format as f, image, CompositeAlpha};
use crate::{native, Backend as B, GlContainer, PhysicalDevice, QueueFamily};

fn get_window_extent(window: &Window) -> image::Extent {
    image::Extent {
        width: 640 as image::Size,
        height: 480 as image::Size,
        depth: 1,
    }
}

struct PixelFormat {
    color_bits: u32,
    alpha_bits: u32,
    srgb: bool,
    double_buffer: bool,
    multisampling: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Window;

impl Window {
    fn get_pixel_format(&self) -> PixelFormat {
        PixelFormat {
            color_bits: 24,
            alpha_bits: 8,
            srgb: false,
            double_buffer: true,
            multisampling: None,
        }
    }

    pub fn get_hidpi_factor(&self) -> i32 {
        1
    }

    pub fn resize<T>(&self, parameter: T) {}
}

#[derive(Clone, Debug)]
pub struct Swapchain {
    pub(crate) window: Window,
    pub(crate) extent: Extent2D,
    pub(crate) fbos: Vec<native::FrameBuffer>,
}

impl hal::Swapchain<B> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
        _semaphore: Option<&native::Semaphore>,
        _fence: Option<&native::Fence>,
    ) -> Result<(hal::SwapImageIndex, Option<hal::window::Suboptimal>), hal::AcquireError> {
        // TODO: sync
        Ok((0, None))
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Surface {
    window: Window,
}

impl Surface {
    pub fn from_window(window: Window) -> Self {
        Surface { window: Window }
    }

    pub fn get_window(&self) -> &Window {
        &self.window
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        let pixel_format = self.window.get_pixel_format();
        let color_bits = pixel_format.color_bits;
        let alpha_bits = pixel_format.alpha_bits;
        let srgb = pixel_format.srgb;

        // TODO: expose more formats
        match (color_bits, alpha_bits, srgb) {
            (24, 8, true) => vec![f::Format::Rgba8Srgb, f::Format::Bgra8Srgb],
            (24, 8, false) => vec![f::Format::Rgba8Unorm, f::Format::Bgra8Unorm],
            _ => vec![],
        }
    }
}

impl hal::Surface<B> for Surface {
    fn kind(&self) -> hal::image::Kind {
        let ex = get_window_extent(&self.window);
        let samples = self.window.get_pixel_format().multisampling.unwrap_or(1);
        hal::image::Kind::D2(ex.width, ex.height, 1, samples as _)
    }

    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<f::Format>>,
        Vec<hal::PresentMode>,
    ) {
        let ex = get_window_extent(&self.window);
        let extent = hal::window::Extent2D::from(ex);

        let caps = hal::SurfaceCapabilities {
            image_count: if self.window.get_pixel_format().double_buffer {
                2 .. 3
            } else {
                1 .. 2
            },
            current_extent: Some(extent),
            extents: extent .. hal::window::Extent2D {
                width: ex.width + 1,
                height: ex.height + 1,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
            composite_alpha: CompositeAlpha::OPAQUE, //TODO
        };
        let present_modes = vec![
            hal::PresentMode::Fifo, //TODO
        ];

        (caps, Some(self.swapchain_formats()), present_modes)
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        true
    }
}

impl hal::Instance for Surface {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        let adapter = PhysicalDevice::new_adapter((), GlContainer::from_new_canvas()); // TODO: Move to `self` like native/window
        vec![adapter]
    }
}
