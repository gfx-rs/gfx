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
//!     use glutin::{ContextBuilder, WindowedContext};
//!     use glutin::window::WindowBuilder;
//!     use glutin::event_loop::EventLoop;
//!
//!     // First create a window using glutin.
//!     let mut events_loop = EventLoop::new();
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
//! use glutin::{Context, ContextBuilder};
//! use glutin::event_loop::EventLoop;
//!
//! fn main() {
//!     let events_loop = EventLoop::new();
//!     let context = ContextBuilder::new().build_headless(&events_loop, glutin::dpi::PhysicalSize::new(0.0, 0.0))
//!         .expect("Failed to build headless context");
//!     let context = unsafe { context.make_current() }.expect("Failed to make the context current");
//!     let headless = Headless::from_context(context);
//!     let _adapters = headless.enumerate_adapters();
//! }
//! ```

use crate::{conv, native, Instance, Device, GlContainer, PhysicalDevice, QueueFamily, Starc};
use hal::{adapter::Adapter, format as f, image, window};

use arrayvec::ArrayVec;
use glow::Context as _;
use glutin;

use std::iter;


#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) context: Starc<glutin::RawContext<glutin::PossiblyCurrent>>,
    // Extent because the window lies
    pub(crate) extent: window::Extent2D,
    ///
    pub(crate) fbos: ArrayVec<[native::RawFrameBuffer; 3]>,
}

impl window::Swapchain<Instance> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
        _semaphore: Option<&native::Semaphore>,
        _fence: Option<&native::Fence>,
    ) -> Result<(window::SwapImageIndex, Option<window::Suboptimal>), window::AcquireError> {
        // TODO: sync
        Ok((0, None))
    }
}

//TODO: if we make `Surface` a `WindowBuilder` instead of `RawContext`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Surface {
    pub(crate) context: Starc<glutin::RawContext<glutin::PossiblyCurrent>>,
    pub(crate) swapchain: Option<Swapchain>,
    renderbuffer: Option<native::Renderbuffer>,
}

impl Surface {
    pub fn from_context(context: glutin::RawContext<glutin::PossiblyCurrent>) -> Self {
        Surface {
            renderbuffer: None,
            swapchain: None,
            context: Starc::new(context),
        }
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

impl window::PresentationSurface<Instance> for Surface {
    type SwapchainImage = native::ImageView;

    unsafe fn configure_swapchain(
        &mut self,
        device: &Device,
        config: window::SwapchainConfig,
    ) -> Result<(), window::CreationError> {
        let gl = &device.share.context;

        if let Some(old) = self.swapchain.take() {
            for fbo in old.fbos {
                gl.delete_framebuffer(fbo);
            }
        }

        if self.renderbuffer.is_none() {
            self.renderbuffer = Some(gl.create_renderbuffer().unwrap());
        }

        let desc = conv::describe_format(config.format).unwrap();
        gl.bind_renderbuffer(glow::RENDERBUFFER, self.renderbuffer);
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            desc.tex_internal,
            config.extent.width as i32,
            config.extent.height as i32,
        );

        let fbo = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
        gl.framebuffer_renderbuffer(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            self.renderbuffer,
        );
        self.swapchain = Some(Swapchain {
            context: self.context.clone(),
            extent: config.extent,
            fbos: iter::once(fbo).collect(),
        });

        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, device: &Device) {
        let gl = &device.share.context;
        if let Some(old) = self.swapchain.take() {
            for fbo in old.fbos {
                gl.delete_framebuffer(fbo);
            }
        }
        if let Some(rbo) = self.renderbuffer.take() {
            gl.delete_renderbuffer(rbo);
        }
    }

    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<window::Suboptimal>), window::AcquireError> {
        let image = native::ImageView::Renderbuffer(self.renderbuffer.unwrap());
        Ok((image, None))
    }
}

impl window::Surface<Instance> for Surface {
    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        window::SurfaceCapabilities,
        Option<Vec<f::Format>>,
        Vec<window::PresentMode>,
    ) {
        let caps = window::SurfaceCapabilities {
            image_count: if self.context.get_pixel_format().double_buffer {
                2 ..= 2
            } else {
                1 ..= 1
            },
            current_extent: None,
            extents: window::Extent2D {
                width: 4,
                height: 4,
            } ..= window::Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
            composite_alpha: window::CompositeAlpha::OPAQUE, //TODO
        };
        let present_modes = vec![
            window::PresentMode::Fifo, //TODO
        ];

        (caps, Some(self.swapchain_formats()), present_modes)
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        true
    }
}

impl hal::Instance for Surface {
    type Backend = Instance;
    fn enumerate_adapters(&self) -> Vec<Adapter<Instance>> {
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Headless(pub Starc<glutin::Context<glutin::PossiblyCurrent>>);

impl Headless {
    pub fn from_context(context: glutin::Context<glutin::PossiblyCurrent>) -> Headless {
        Headless(Starc::new(context))
    }
}

impl hal::Instance for Headless {
    type Backend = Instance;
    fn enumerate_adapters(&self) -> Vec<Adapter<Instance>> {
        let adapter = PhysicalDevice::new_adapter(
            (),
            GlContainer::from_fn_proc(|s| self.0.get_proc_address(s) as *const _),
        );
        vec![adapter]
    }
}
