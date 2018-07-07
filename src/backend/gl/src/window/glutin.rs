//! Window creation using glutin for gfx.
//!
//! # Examples
//!
//! The following code creates a `gfx::Surface` using glutin.
//!
//! ```no_run
//! extern crate glutin;
//! extern crate gfx_backend_gl;
//!
//! fn main() {
//!     use gfx_backend_gl::Surface;
//!     use glutin::{EventsLoop, WindowBuilder, ContextBuilder, GlWindow};
//!
//!     // First create a window using glutin.
//!     let mut events_loop = EventsLoop::new();
//!     let wb = WindowBuilder::new();
//!     let cb = ContextBuilder::new().with_vsync(true);
//!     let glutin_window = GlWindow::new(wb, cb, &events_loop).unwrap();
//!
//!     // Then use the glutin window to create a gfx surface.
//!     let surface = Surface::from_window(glutin_window);
//! }
//! ```
//!
//! Headless initialization without a window.
//!
//! ```no_run
//! extern crate glutin;
//! extern crate gfx_backend_gl;
//! extern crate gfx_hal;
//!
//! use gfx_hal::Instance;
//! use gfx_backend_gl::Headless;
//! use glutin::{HeadlessRendererBuilder};
//!
//! fn main() {
//!     let context = HeadlessRendererBuilder::new(256, 256)
//!         .build()
//!         .expect("Failed to build headless context");
//!     let headless = Headless(context);
//!     let _adapters = headless.enumerate_adapters();
//! }
//! ```

use hal::{self, format as f, image};

use {Backend as B, Device, PhysicalDevice, QueueFamily, Starc};

use glutin::{self, GlContext};


fn get_window_extent(window: &glutin::GlWindow) -> image::Extent {
    let px = window.get_inner_size().unwrap().to_physical(window.get_hidpi_factor());
    image::Extent {
        width: px.width as image::Size,
        height: px.height as image::Size,
        depth: 1,
    }
}

pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) window: Starc<glutin::GlWindow>,
}

impl hal::Swapchain<B> for Swapchain {
    fn acquire_image(&mut self, _sync: hal::FrameSync<B>) -> Result<hal::SwapImageIndex, ()> {
        // TODO: sync
        Ok(0)
    }
}

//TODO: if we make `Surface` a `WindowBuilder` instead of `GlWindow`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
pub struct Surface {
    window: Starc<glutin::GlWindow>,
}

impl Surface {
    pub fn from_window(window: glutin::GlWindow) -> Self {
        Surface {
            window: Starc::new(window)
        }
    }

    pub fn get_window(&self) -> &glutin::GlWindow {
        &*self.window
    }

    pub fn window(&self) -> &glutin::GlWindow {
        &self.window
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        let pixel_format = self.window.get_pixel_format();
        let color_bits = pixel_format.color_bits;
        let alpha_bits = pixel_format.alpha_bits;
        let srgb = pixel_format.srgb;

        // TODO: expose more formats
        match (color_bits, alpha_bits, srgb) {
            (24, 8, true) => vec![
                f::Format::Rgba8Srgb,
                f::Format::Bgra8Srgb,
            ],
            (24, 8, false) => vec![
                f::Format::Rgba8Unorm,
                f::Format::Bgra8Unorm,
            ],
            _ => vec![],
        }
    }
}

impl hal::Surface<B> for Surface {
    fn kind(&self) -> hal::image::Kind {
        let ex = get_window_extent(&self.window);
        let samples = self.window
            .get_pixel_format()
            .multisampling
            .unwrap_or(1);
        hal::image::Kind::D2(ex.width, ex.height, 1, samples as _)
    }

    fn compatibility(
        &self, _: &PhysicalDevice
    ) -> (hal::SurfaceCapabilities, Option<Vec<f::Format>>, Vec<hal::PresentMode>) {
        let ex = get_window_extent(&self.window);
        let extent = hal::window::Extent2D::from(ex);

        let caps = hal::SurfaceCapabilities {
            image_count: if self.window.get_pixel_format().double_buffer { 2..3 } else { 1..2 },
            current_extent: Some(extent),
            extents: extent .. hal::window::Extent2D {
                width: ex.width + 1,
                height: ex.height + 1,
            },
            max_image_layers: 1,
        };
        let present_modes = vec![hal::PresentMode::Fifo]; //TODO

        (caps, Some(self.swapchain_formats()), present_modes)
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool { true }
}

impl Device {
    pub(crate) fn create_swapchain_impl(
        &self,
        surface: &mut Surface,
        _config: hal::SwapchainConfig,
    ) -> (Swapchain, hal::Backbuffer<B>) {
        let swapchain = Swapchain {
            window: surface.window.clone(),
        };
        let backbuffer = hal::Backbuffer::Framebuffer(0);
        (swapchain, backbuffer)
    }
}

impl hal::Instance for Surface {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        unsafe { self.window.make_current().unwrap() };
        let adapter = PhysicalDevice::new_adapter(|s| self.window.get_proc_address(s) as *const _);
        vec![adapter]
    }
}

pub fn config_context(
    builder: glutin::ContextBuilder,
    color_format: f::Format,
    ds_format: Option<f::Format>,
) -> glutin::ContextBuilder
{
    let color_base = color_format.base_format();
    let color_bits = color_base.0.describe_bits();
    let depth_bits = match ds_format {
        Some(fm) => fm.base_format().0.describe_bits(),
        None => f::BITS_ZERO,
    };
    builder
        .with_depth_buffer(depth_bits.depth)
        .with_stencil_buffer(depth_bits.stencil)
        .with_pixel_format(color_bits.color, color_bits.alpha)
        .with_srgb(color_base.1 == f::ChannelType::Srgb)
}


pub struct Headless(pub glutin::HeadlessContext);

unsafe impl Send for Headless {}
unsafe impl Sync for Headless {}

impl hal::Instance for Headless {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        unsafe { self.0.make_current().unwrap() };
        let adapter = PhysicalDevice::new_adapter(|s| self.0.get_proc_address(s) as *const _);
        vec![adapter]
    }
}
