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

use crate::{conv, native, Backend as B, Device, GlContainer, PhysicalDevice, QueueFamily, Starc};
use hal::{adapter::Adapter, format as f, image, window};

use std::ffi::c_void;
use std::os::raw::c_ulong;
use std::sync::Arc;

use arrayvec::ArrayVec;
use glow::HasContext;
use glutin::{self, platform::unix::RawContextExt};

use std::iter;

#[derive(Debug)]
pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) context: Starc<glutin::RawContext<glutin::PossiblyCurrent>>,
    // Extent because the window lies
    pub(crate) extent: window::Extent2D,
    ///
    pub(crate) fbos: ArrayVec<[native::RawFrameBuffer; 3]>,
}

impl window::Swapchain<B> for Swapchain {
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

#[derive(Debug)]
pub enum Instance {
    Headless(Headless),
    Surface(Surface),
}

impl Instance {
    pub fn create_surface_from_wayland(
        &self,
        display: *mut c_void,
        surface: *mut c_void,
    ) -> Surface {
        log::trace!("Creating GL surface from wayland");
        let context = unsafe {
            glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_raw_wayland_context(
                    display as _,
                    surface,
                    /*TODO: do something with these dimensions*/
                    400,
                    400,
                )
                .expect("TODO: handle this error")
        };
        let context = unsafe { context.make_current().expect("TODO: handle this error") };
        Surface::from_context(context)
    }

    pub fn create_surface_from_xlib(&self, window: c_ulong, display: *mut c_void) -> Surface {
        log::trace!("Creating GL surface from Xlib");
        let xconn = {
            // This is taken from `glutin::platform::unix::x11::XConnection::new except with tweaks
            // that allow us to create the connection with an existing display pointer
            use glutin::platform::unix::x11::{ffi, XConnection};
            // opening the libraries
            let xlib = ffi::Xlib::open().expect("TODO: Handle error");
            let xcursor = ffi::Xcursor::open().expect("TODO: Handle error");
            let xrandr = ffi::Xrandr_2_2_0::open().expect("TODO: Handle error");
            let xrandr_1_5 = ffi::Xrandr::open().ok();
            let xinput2 = ffi::XInput2::open().expect("TODO: Handle error");
            let xlib_xcb = ffi::Xlib_xcb::open().expect("TODO: Handle error");
            let xrender = ffi::Xrender::open().expect("TODO: Handle error");

            unsafe { (xlib.XInitThreads)() };
            // unsafe { (xlib.XSetErrorHandler)(error_handler) };

            // Get X11 socket file descriptor
            let fd = unsafe { (xlib.XConnectionNumber)(display as *mut ffi::_XDisplay) };

            XConnection {
                xlib,
                xrandr,
                xrandr_1_5,
                xcursor,
                xinput2,
                xlib_xcb,
                xrender,
                display: display as _,
                x11_fd: fd,
                latest_error: parking_lot::Mutex::new(None),
                cursor_cache: Default::default(),
            }
        };
        let xconn = Arc::new(xconn);

        let context = unsafe {
            glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_raw_x11_context(xconn, window)
                .expect("TODO: handle this error")
        };
        let context = unsafe { context.make_current().expect("TODO: handle this error") };
        Surface::from_context(context)
    }
}

impl hal::Instance<B> for Instance {
    fn create(name: &str, version: u32) -> Result<Instance, hal::UnsupportedBackend> {
        Headless::create(name, version).map(Instance::Headless)
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        match self {
            Instance::Headless(instance) => instance.enumerate_adapters(),
            Instance::Surface(instance) => instance.enumerate_adapters(),
        }
    }

    unsafe fn create_surface(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, hal::window::InitError> {
        use raw_window_handle::RawWindowHandle;

        match self {
            Instance::Headless(instance) => instance.create_surface(has_handle),
            Instance::Surface(instance) => instance.create_surface(has_handle),
        }
        .expect("TODO");

        match has_handle.raw_window_handle() {
            #[cfg(all(unix, not(android), not(macos)))]
            RawWindowHandle::Wayland(handle) => {
                Ok(self.create_surface_from_wayland(handle.display, handle.surface))
            }
            #[cfg(all(unix, not(android), not(macos)))]
            RawWindowHandle::Xlib(handle) => {
                Ok(self.create_surface_from_xlib(handle.display as *mut _, handle.window))
            }
            _ => Err(hal::window::InitError::UnsupportedWindowHandle),
        }
    }

    unsafe fn destroy_surface(&self, surface: Surface) {
        match self {
            Instance::Headless(instance) => instance.destroy_surface(surface),
            Instance::Surface(instance) => instance.destroy_surface(surface),
        }
    }
}

//TODO: if we make `Surface` a `WindowBuilder` instead of `RawContext`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
#[derive(Debug)]
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

impl window::PresentationSurface<B> for Surface {
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

impl window::Surface<B> for Surface {
    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        true
    }

    fn capabilities(&self, _physical_device: &PhysicalDevice) -> window::SurfaceCapabilities {
        window::SurfaceCapabilities {
            present_modes: window::PresentMode::FIFO, //TODO
            composite_alpha_modes: window::CompositeAlphaMode::OPAQUE, //TODO
            image_count: if self.context.get_pixel_format().double_buffer {
                2..=2
            } else {
                1..=1
            },
            current_extent: None,
            extents: window::Extent2D {
                width: 4,
                height: 4,
            }..=window::Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
        }
    }

    fn supported_formats(&self, _physical_device: &PhysicalDevice) -> Option<Vec<f::Format>> {
        Some(self.swapchain_formats())
    }
}

impl hal::Instance<B> for Surface {
    fn create(_: &str, _: u32) -> Result<Self, hal::UnsupportedBackend> {
        panic!("Unable to create a surface")
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        let adapter = PhysicalDevice::new_adapter(
            (),
            GlContainer::from_fn_proc(|s| self.context.get_proc_address(s) as *const _),
        );
        vec![adapter]
    }

    unsafe fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, window::InitError> {
        unimplemented!()
    }

    unsafe fn destroy_surface(&self, _surface: Surface) {
        // TODO: Implement Surface cleanup
    }
}

// This isn't used anymore according to the linter. Keeping it commented just in case.
//
// pub fn config_context<C>(
//     builder: glutin::ContextBuilder<C>,
//     color_format: f::Format,
//     ds_format: Option<f::Format>,
// ) -> glutin::ContextBuilder<C>
// where
//     C: glutin::ContextCurrentState,
// {
//     let color_base = color_format.base_format();
//     let color_bits = color_base.0.describe_bits();
//     let depth_bits = match ds_format {
//         Some(fm) => fm.base_format().0.describe_bits(),
//         None => f::BITS_ZERO,
//     };
//     builder
//         .with_depth_buffer(depth_bits.depth)
//         .with_stencil_buffer(depth_bits.stencil)
//         .with_pixel_format(color_bits.color, color_bits.alpha)
//         .with_srgb(color_base.1 == f::ChannelType::Srgb)
// }

#[derive(Debug)]
pub struct Headless(pub Starc<glutin::Context<glutin::PossiblyCurrent>>);

impl Headless {
    pub fn from_context(context: glutin::Context<glutin::PossiblyCurrent>) -> Headless {
        Headless(Starc::new(context))
    }
}

impl hal::Instance<B> for Headless {
    fn create(_: &str, _: u32) -> Result<Self, hal::UnsupportedBackend> {
        let context: glutin::Context<glutin::NotCurrent>;
        #[cfg(linux)]
        {
            /// TODO: Update portability to make this more flexible
            use glutin::platform::unix::HeadlessContextExt;
            let size = glutin::dpi::PhysicalSize::from((800, 600));
            let builder = glutin::ContextBuilder::new().with_hardware_acceleration(Some(false));
            context = HeadlessContextExt::build_osmesa(builder, size).map_err(|e| {
                info!("Headless context error {:?}", e);
                hal::UnsupportedBackend
            })?;
        }
        #[cfg(not(linux))]
        {
            context = unimplemented!();
        }
        let context = unsafe { context.make_current() }.expect("failed to make context current");
        Ok(Headless::from_context(context))
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        let adapter = PhysicalDevice::new_adapter(
            (),
            GlContainer::from_fn_proc(|s| self.0.get_proc_address(s) as *const _),
        );
        vec![adapter]
    }

    unsafe fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, window::InitError> {
        unimplemented!()
    }

    unsafe fn destroy_surface(&self, _surface: Surface) {
        // TODO: Implement Surface cleanup
    }
}
