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

use {Backend as B, Device, PhysicalDevice, QueueFamily};

use glutin::{self, GlContext};
use std::rc::Rc;

fn get_window_dimensions(window: &glutin::GlWindow) -> image::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as image::NumSamples;
    ((width as f32 * window.hidpi_factor()) as image::Size, (height as f32 * window.hidpi_factor()) as image::Size, 1, aa.into())
}

pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) window: Rc<glutin::GlWindow>,
}

impl hal::Swapchain<B> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<B>) -> hal::Frame {
        // TODO: sync
        hal::Frame::new(0)
    }
}

//TODO: if we make `Surface` a `WindowBuilder` instead of `GlWindow`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
pub struct Surface {
    window: Rc<glutin::GlWindow>,
}

impl Surface {
    pub fn from_window(window: glutin::GlWindow) -> Self {
        Surface {
            window: Rc::new(window)
        }
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
        let (w, h, _, a) = get_window_dimensions(&self.window);
        hal::image::Kind::D2(w, h, a)
    }

    fn capabilities_and_formats(&self, _: &PhysicalDevice) -> (hal::SurfaceCapabilities, Option<Vec<f::Format>>) {
        let dim = get_window_dimensions(&self.window);
        let extent = hal::window::Extent2D {
            width: dim.0 as u32,
            height: dim.1 as u32,
        };

        (hal::SurfaceCapabilities {
            image_count: if self.window.get_pixel_format().double_buffer { 2..3 } else { 1..2 },
            current_extent: Some(extent),
            extents: extent..hal::window::Extent2D {
                width: dim.0 as u32 + 1,
                height: dim.1 as u32 + 1
            },
            max_image_layers: 1,
        }, Some(self.swapchain_formats()))
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

impl hal::Instance for Headless {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        unsafe { self.0.make_current().unwrap() };
        let adapter = PhysicalDevice::new_adapter(|s| self.0.get_proc_address(s) as *const _);
        vec![adapter]
    }
}
