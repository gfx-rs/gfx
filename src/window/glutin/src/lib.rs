//! Window creation using glutin for gfx.
//!
//! # Examples
//! The following code creates a `gfx::WindowExt` using glutin.
//!
//! ```no_run
//! extern crate glutin;
//! extern crate gfx_device_gl;
//! extern crate gfx_window_glutin;
//! 
//! fn main() {
//!     use gfx_window_glutin::Window;
//!     use glutin::{EventsLoop, WindowBuilder, ContextBuilder, GlWindow};
//! 
//!     // First create a window using glutin.
//!     let mut events_loop = EventsLoop::new();
//!     let wb = WindowBuilder::new();
//!     let cb = ContextBuilder::new().with_vsync(true);
//!     let glutin_window = GlWindow::new(wb, cb, &events_loop).unwrap();
//! 
//!     // Then use the glutin window to create a gfx window.
//!     let window = Window::new(glutin_window);
//! }
//! ```
#[deny(missing_docs)]

extern crate gfx_core as core;
extern crate gfx_device_gl as device_gl;
extern crate glutin;

#[cfg(feature = "headless")]
pub use headless::Headless;

use core::{format, handle, texture};
use core::memory;
use device_gl::{native as n, Backend as B};
use glutin::GlContext;
use std::rc::Rc;

#[cfg(feature = "headless")]
mod headless;

fn get_window_dimensions(window: &glutin::GlWindow) -> texture::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as texture::NumSamples;
    ((width as f32 * window.hidpi_factor()) as texture::Size, (height as f32 * window.hidpi_factor()) as texture::Size, 1, aa.into())
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
pub fn update_views_raw(window: &glutin::GlWindow, old_dimensions: texture::Dimensions,
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

impl core::Swapchain<B> for SwapChain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<B>] {
        &self.backbuffer
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<B>) -> core::Frame {
        // TODO: sync
        core::Frame::new(0)
    }

    fn present<Q>(&mut self, _: &mut Q, _: &[&handle::Semaphore<B>])
        where Q: AsMut<device_gl::CommandQueue>
    {
        self.window.swap_buffers();
    }
}

pub struct Surface {
    window: Rc<glutin::GlWindow>,
    manager: handle::Manager<B>,
}

impl core::Surface<B> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, _: &device_gl::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Swapchain
        where Q: AsRef<device_gl::CommandQueue>
    {
        use core::handle::Producer;
        let dim = get_window_dimensions(&self.window);
        let color = self.manager.make_image(
            n::Image::Surface(0),
            texture::Info {
                levels: 1,
                kind: texture::Kind::D2(dim.0, dim.1, dim.3),
                format: config.color_format.0,
                bind: memory::RENDER_TARGET | memory::TRANSFER_SRC,
                usage: memory::Usage::Data,
            },
        );

        let ds = config.depth_stencil_format.map(|ds_format| {
            self.manager.make_image(
                n::Image::Surface(0),
                texture::Info {
                    levels: 1,
                    kind: texture::Kind::D2(dim.0, dim.1, dim.3),
                    format: ds_format.0,
                    bind: memory::DEPTH_STENCIL | memory::TRANSFER_SRC,
                    usage: memory::Usage::Data,
                },
            )
        });

        Swapchain {
            window: self.window.clone(),
            backbuffer: [(color, ds); 1],
        }
    }
}

pub struct Window(Rc<glutin::GlWindow>);

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

impl Window {
    /// Create a new window.
    pub fn new(window: glutin::GlWindow) -> Self {
        Window(Rc::new(window))
    }

    /// Get the internal glutin window.
    pub fn raw(&self) -> &glutin::GlWindow {
        &self.0
    }
}
impl core::WindowExt<B> for Window {
    type Surface = Surface;
    type Adapter = device_gl::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_gl::Adapter>) {
        unsafe { self.0.make_current().unwrap() };
        let adapter = device_gl::Adapter::new(|s| self.0.get_proc_address(s) as *const std::os::raw::c_void);
        let surface = Surface {
            window: self.0.clone(),
            manager: handle::Manager::new(),
        };

        (surface, vec![adapter])
    }
}
