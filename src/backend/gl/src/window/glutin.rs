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

use {native as n, Backend as B, PhysicalDevice, QueueFamily};

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
    window: Rc<glutin::GlWindow>,
}

impl hal::Swapchain<B> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<B>) -> hal::Frame {
        // TODO: sync
        hal::Frame::new(0)
    }

    fn present<C>(&mut self, _: &mut hal::CommandQueue<B, C>, _: &[&n::Semaphore]) {
        self.window.swap_buffers().unwrap();
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
        use hal::format::ChannelType::*;
        use hal::format::SurfaceType::*;

        let pixel_format = self.window.get_pixel_format();
        let color_bits = pixel_format.color_bits;
        let alpha_bits = pixel_format.alpha_bits;
        let srgb = pixel_format.srgb;

        // TODO: expose more formats
        match (color_bits, alpha_bits, srgb) {
            (24, 8, true) => vec![
                f::Format(R8_G8_B8_A8, Srgb),
                f::Format(B8_G8_R8_A8, Srgb),
            ],
            (24, 8, false) => vec![
                f::Format(R8_G8_B8_A8, Unorm),
                f::Format(B8_G8_R8_A8, Unorm),
            ],
            _ => vec![],
        }
    }
}

impl hal::Surface<B> for Surface {
    fn get_kind(&self) -> hal::image::Kind {
        let (w, h, _, a) = get_window_dimensions(&self.window);
        hal::image::Kind::D2(w, h, a)
    }

    fn capabilities_and_formats(&self, _: &PhysicalDevice) -> (hal::SurfaceCapabilities, Vec<f::Format>) {
        let _formats = self.swapchain_formats();
        unimplemented!()
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool { true }

    fn build_swapchain<C>(
        &mut self,
        _config: hal::SwapchainConfig,
        _: &hal::CommandQueue<B, C>,
    ) -> (Swapchain, hal::Backbuffer<B>) {
        let swapchain = Swapchain {
            window: self.window.clone(),
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
    let color_bits = color_format.0.describe_bits();
    let depth_bits = match ds_format {
        Some(fm) => fm.0.describe_bits(),
        None => f::BITS_ZERO,
    };
    builder
        .with_depth_buffer(depth_bits.depth)
        .with_stencil_buffer(depth_bits.stencil)
        .with_pixel_format(color_bits.color, color_bits.alpha)
        .with_srgb(color_format.1 == f::ChannelType::Srgb)
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
