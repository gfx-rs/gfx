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
//!     use glutin::{EventsLoop, WindowBuilder, ContextBuilder, WindowedContext};
//!
//!     // First create a window using glutin.
//!     let mut events_loop = EventsLoop::new();
//!     let wb = WindowBuilder::new();
//!     let glutin_window = ContextBuilder::new().with_vsync(true).build_windowed(wb, &events_loop).unwrap();
//!     let (glutin_context, glutin_window) = unsafe { glutin_window.make_current().expect("Failed to make the context current").split() };
//!
//!     // Then use the glutin window to create a gfx surface.
//!     let surface = Surface::from_context(glutin_context);
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
//! use glutin::{Context, ContextBuilder, EventsLoop};
//!
//! fn main() {
//!     let events_loop = EventsLoop::new();
//!     let context = ContextBuilder::new().build_headless(&events_loop, glutin::dpi::PhysicalSize::new(0.0, 0.0))
//!         .expect("Failed to build headless context");
//!     let context = unsafe { context.make_current() }.expect("Failed to make the context current");
//!     let headless = Headless::from_context(context);
//!     let _adapters = headless.enumerate_adapters();
//! }
//! ```

use crate::hal::window::Extent2D;
use crate::hal::{self, format as f, image, memory, CompositeAlpha};
use crate::{native, Backend as B, Device, GlContainer, PhysicalDevice, QueueFamily, Starc};

use glow::Context;

use glutin;

fn get_window_extent(window: &glutin::Window) -> image::Extent {
    let px = window
        .get_inner_size()
        .unwrap()
        .to_physical(window.get_hidpi_factor());
    image::Extent {
        width: px.width as image::Size,
        height: px.height as image::Size,
        depth: 1,
    }
}

#[derive(Debug)]
pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) context: Starc<glutin::RawContext<glutin::PossiblyCurrent>>,
    // Extent because the window lies
    pub(crate) extent: Extent2D,
    ///
    pub(crate) fbos: Vec<native::RawFrameBuffer>,
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

//TODO: if we make `Surface` a `WindowBuilder` instead of `RawContext`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
#[derive(Clone, Debug)]
pub struct Surface {
    pub(crate) context: Starc<glutin::RawContext<glutin::PossiblyCurrent>>,
}

impl Surface {
    pub fn from_context(context: glutin::RawContext<glutin::PossiblyCurrent>) -> Self {
        Surface {
            context: Starc::new(context),
        }
    }

    pub fn get_context(&self) -> &glutin::RawContext<glutin::PossiblyCurrent> {
        &*self.context
    }

    pub fn context(&self) -> &glutin::RawContext<glutin::PossiblyCurrent> {
        &self.context
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        let pixel_format = self.context.get_pixel_format();
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
    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<f::Format>>,
        Vec<hal::PresentMode>,
    ) {
        let caps = hal::SurfaceCapabilities {
            image_count: if self.context.get_pixel_format().double_buffer {
                2 ..= 2
            } else {
                1 ..= 1
            },
            current_extent: None,
            extents: hal::window::Extent2D {
                width: 4,
                height: 4,
            } ..= hal::window::Extent2D {
                width: 4096,
                height: 4096,
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
        let adapter = PhysicalDevice::new_adapter(
            (),
            GlContainer::from_fn_proc(|s| self.context.get_proc_address(s) as *const _),
        );
        vec![adapter]
    }
}

pub fn config_context<C>(
    builder: glutin::ContextBuilder<C>,
    color_format: f::Format,
    ds_format: Option<f::Format>,
) -> glutin::ContextBuilder<C>
where
    C: glutin::ContextCurrentState,
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

#[derive(Debug)]
pub struct Headless(pub Starc<glutin::Context<glutin::PossiblyCurrent>>);

impl Headless {
    pub fn from_context(context: glutin::Context<glutin::PossiblyCurrent>) -> Headless {
        Headless(Starc::new(context))
    }
}

impl hal::Instance for Headless {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        let adapter = PhysicalDevice::new_adapter(
            (),
            GlContainer::from_fn_proc(|s| self.0.get_proc_address(s) as *const _),
        );
        vec![adapter]
    }
}
