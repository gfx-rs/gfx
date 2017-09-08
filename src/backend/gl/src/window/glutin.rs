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
//! extern crate gfx_core as core;
//!
//! use core::Instance;
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

use core::{self, format, image};

use {native as n, Adapter, Backend as B, QueueFamily};

use glutin::{self, GlContext};
use std::rc::Rc;

fn get_window_dimensions(window: &glutin::GlWindow) -> image::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as image::NumSamples;
    ((width as f32 * window.hidpi_factor()) as image::Size, (height as f32 * window.hidpi_factor()) as image::Size, 1, aa.into())
}

/*
/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf, Df>(window: &glutin::GlWindow, color_view: &mut handle::RenderTargetView<R, Cf>,
                    ds_view: &mut handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    let dim = color_view.get_dimensions();
    assert_eq!(dim, ds_view.get_dimensions());
    if let Some((cv, dv)) = update_views_raw(window, dim, Cf::get_format(), Df::get_format()) {
        *color_view = Typed::new(cv);
        *ds_view = Typed::new(dv);
    }
}

/// Return new main target views if the window resolution has changed from the old dimensions.
pub fn update_views_raw(window: &glutin::GlWindow, old_dimensions: image::Dimensions,
                        color_format: format::Format, ds_format: format::Format)
                        -> Option<(handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)>
{
    let dim = get_window_dimensions(window);
    if dim != old_dimensions {
        Some(device_gl::create_main_targets_raw(dim, color_format.0, ds_format.0))
    }else {
        None
    }
}
*/

pub struct Swapchain {
    // Underlying window, required for presentation
    window: Rc<glutin::GlWindow>,
    // Single element backbuffer
    backbuffer: [core::Backbuffer<B>; 1],
}

impl core::Swapchain<B> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<B>] {
        &self.backbuffer
    }

    fn acquire_frame(&mut self, _sync: core::FrameSync<B>) -> core::Frame {
        // TODO: sync
        core::Frame::new(0)
    }

    fn present<C>(&mut self, _: &mut core::CommandQueue<B, C>, _: &[&n::Semaphore]) {
        self.window.swap_buffers().unwrap();
    }
}

pub struct Surface {
    window: Rc<glutin::GlWindow>,
}

impl Surface {
    pub fn from_window(window: glutin::GlWindow) -> Self {
        Surface {
            window: Rc::new(window)
        }
    }
}

impl core::Surface<B> for Surface {
    fn supports_queue(&self, _: &QueueFamily) -> bool { true }

    fn build_swapchain<C>(
        &mut self,
        _config: core::SwapchainConfig,
        _: &core::CommandQueue<B, C>,
    ) -> Swapchain {
        let backbuffer = core::Backbuffer {
            color: n::Image::Surface(0),
            depth_stencil: Some(n::Image::Surface(0)),
        };

        Swapchain {
            window: self.window.clone(),
            backbuffer: [backbuffer; 1],
        }
    }
}

impl core::Instance<B> for Surface {
    fn enumerate_adapters(&self) -> Vec<Adapter> {
        unsafe { self.window.make_current().unwrap() };
        let adapter = Adapter::new(|s| self.window.get_proc_address(s) as *const _);
        vec![adapter]
    }
}

pub fn config_context(
    builder: glutin::ContextBuilder,
    color_format: format::Format, ds_format: format::Format) -> glutin::ContextBuilder
{
    let color_total_bits = color_format.0.get_total_bits();
    let alpha_bits = color_format.0.get_alpha_stencil_bits();
    let depth_total_bits = ds_format.0.get_total_bits();
    let stencil_bits = ds_format.0.get_alpha_stencil_bits();
    builder
        .with_depth_buffer(depth_total_bits - stencil_bits)
        .with_stencil_buffer(stencil_bits)
        .with_pixel_format(color_total_bits - alpha_bits, alpha_bits)
        .with_srgb(color_format.1 == format::ChannelType::Srgb)
}


pub struct Headless(pub glutin::HeadlessContext);

impl core::Instance<B> for Headless {
    fn enumerate_adapters(&self) -> Vec<Adapter> {
        unsafe { self.0.make_current().unwrap() };
        let adapter = Adapter::new(|s| self.0.get_proc_address(s) as *const _);
        vec![adapter]
    }
}
