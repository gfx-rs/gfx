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
use hal::format::AsFormat;

use std::sync::Weak;

use {
    Backend as B, Device, PhysicalDevice, QueueFamily, Starc, Wstarc,
    Instance, Contexts, native, gl
};

use glutin::{self, GlContext};

#[cfg(all(unix, not(target_os = "android")))]
use glutin::wplatform::{XNotSupported, XConnection};
#[cfg(all(unix, not(target_os = "android")))]
use glutin::os::unix::{EventsLoopExt, WindowExt};

use std::sync::{Arc, Mutex};

pub(crate) fn physical_to_extent(ex: &glutin::dpi::PhysicalSize) -> hal::window::Extent2D {
    hal::window::Extent2D {
        width: ex.width as _,
        height: ex.height as _,
    }
}

pub struct Swapchain {
    // The fbo on the window's context
    pub(crate) fbo: u32,

    pub(crate) surface_context: Arc<SurfaceContext>,
    pub(crate) window: Arc<glutin::Window>,
    pub(crate) device: Device,

    pub(crate) images: Vec<SwapchainImage>,

    // The images' extents
    pub(crate) extent: hal::window::Extent2D,
}

pub struct SwapchainImage {
    // The image we render too.
    pub(crate) image: native::Texture,

    pub(crate) currently_acquired: Mutex<bool>,
}

impl hal::Swapchain<B> for Swapchain {
    fn acquire_image(
        &mut self, timeout_ns: u64, _sync: hal::FrameSync<B>
    ) -> Result<hal::SwapImageIndex, hal::AcquireError> {
        // NOTE: If the extents don't match, it is *only* suboptimal. HAL doesn't
        // currently distinguish between suboptimal and out of date, so we just
        // have to return out of date. This is a note to whoever adds the ability to
        // distinquish between the two.
        let extent = physical_to_extent(
            &self
            .window
            .get_inner_size()
            .unwrap()
            .to_physical(self.window.get_hidpi_factor())
        );

        if extent != self.extent {
            return Err(hal::AcquireError::OutOfDate);
        }

        use std::time::Instant;
        let start = Instant::now();
        let mut current = Instant::now();
        while timeout_ns == u64::max_value() || current.duration_since(start).as_nanos() <= timeout_ns as u128 {
            for (i, img) in self.images.iter().enumerate() {
                let mut currently_acquired = img.currently_acquired.lock().unwrap();
                if !*currently_acquired {
                    *currently_acquired = true;
                    return Ok(i as _);
                }
            }

            // NOTE:
            // "If timeout is zero, then vkAcquireNextImageKHR does not wait,
            // and will either successfully acquire an image, or fail and return
            // VK_NOT_READY if no image is available."
            //      -- Vulkan Spec, §30.8, pg. 1015.
            if timeout_ns == 0 { return Err(hal::AcquireError::NotReady); }

            current = Instant::now();
        }

        // NOTE:
        // "An Image will eventually be acquired if the number of images that
        // the application has currently acquired (bot not yet presented) is
        // less than or equal to the difference between the number of images in
        // swapchain and the value of VkSurfaceCapabilitiesKHR::minImageCount.
        // If the number of currently acquired images is greater than this,
        // vkAcquireNextImage should not be called; if it iz, timeout must not
        // be UINT64_MAX."
        //      -- Vulkan Spec, §30.8, pg. 1015.
        Err(hal::AcquireError::NotReady)
    }
}

#[cfg(all(unix, not(target_os = "android")))]
pub struct Surface {
    events_loop: Starc<glutin::EventsLoop>,
    window: Arc<glutin::Window>,
    surface_context: Weak<SurfaceContext>,
}

pub struct SurfaceContext {
    pub(crate) context: glutin::GlSeparatedContext,
}

impl Surface {
    // TODO: expose proper formats
    fn swapchain_formats(_context: &glutin::GlSeparatedContext) -> Vec<f::Format> {
        /*let pixel_format = context.get_pixel_format();
        let color_bits = pixel_format.color_bits;
        let alpha_bits = pixel_format.alpha_bits;
        let srgb = pixel_format.srgb;

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
        }*/

        vec![
            f::Format::Rgba8Srgb,
            f::Format::Bgra8Srgb,
            f::Format::Rgba8Unorm,
            f::Format::Bgra8Unorm,
        ]
    }
}

impl hal::Surface<B> for Surface {
    fn kind(&self) -> hal::image::Kind {
        match self.surface_context.upgrade() {
            Some(surface_context) => {
                let extent = physical_to_extent(
                    &self
                    .window
                    .get_inner_size()
                    .unwrap()
                    .to_physical(self.window.get_hidpi_factor())
                );
                let samples = surface_context
                    .context
                    .get_pixel_format()
                    .multisampling
                    .unwrap_or(1);
                hal::image::Kind::D2(extent.width, extent.height, 1, samples as _)
            }
            _ => unimplemented!(),
        }
    }

    // TODO return proper requirments, currently doesn't matter because we just
    // ignore them
    fn compatibility(
        &self, _: &PhysicalDevice
    ) -> (hal::SurfaceCapabilities, Option<Vec<f::Format>>, Vec<hal::PresentMode>) {
        let present_modes = vec![hal::PresentMode::Fifo];

        let (image_count, formats) = match self.surface_context.upgrade() {
            None => {
                let formats = vec![
                    f::Format::Rgba8Srgb,
                    f::Format::Bgra8Srgb,
                    f::Format::Rgba8Unorm,
                    f::Format::Bgra8Unorm,
                ];
                (2..3, formats)
            }
            Some(surface_context) => {
                // NOTE: Once swapchained, we cannot change a surface's
                // properties. That's why we return the surface's properties,
                // so those pesky applications don't try to change it.
                //
                // NOTE: What we are currently returning *might* actually be
                // invalid and I just haven't noticed, as, like mentioned above,
                // we actually currently ignore the format they pass in.
                // Hopefully this code ends up correct, but, let's be honest
                // here, it probably isn't.
                let image_count = if surface_context
                    .context
                    .get_pixel_format()
                    .double_buffer { 2..3 } else { 1..2 };
                let formats = Self::swapchain_formats(&surface_context.context);
                (image_count, formats)
            }
        };

        let extent = physical_to_extent(
            &self
            .window
            .get_inner_size()
            .unwrap()
            .to_physical(self.window.get_hidpi_factor())
        );
        let extents = extent .. hal::window::Extent2D {
            width: extent.width + 1,
            height: extent.height + 1,
        };

        let caps = hal::SurfaceCapabilities {
            image_count,
            current_extent: Some(extent),
            extents,
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
        };
        (caps, Some(formats), present_modes)
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool { true }
}

impl Device {
    pub(crate) fn destroy_swapchain_impl(&self, swapchain: Swapchain) {
        let gl = &swapchain.device.share.context;
        for image in swapchain.images {
            unsafe { gl.DeleteTextures(1, &image.image) }
        }
    }

    pub(crate) fn create_swapchain_impl(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
        _old_swapchain: Option<Swapchain>,
    ) -> (Swapchain, hal::Backbuffer<B>) {
        // TODO: actually respect the swapchain configuration provided by the user.
        // Be sure to fix the options returned by `Compatibility`
        let ctxts = self.share.context.instance.lock().unwrap();
        #[cfg(all(unix, not(target_os = "android")))]
        let context = match *ctxts {
            Contexts::InstanceContext(ref ic) => {
                let ic = Wstarc::upgrade(ic).unwrap();
                let ctxt = &ic.context;

                // TODO: The surface format and shit should be setup right now.
                let cb = glutin::ContextBuilder::new()
                    .with_shared_lists(ctxt);

                unsafe {
                    glutin::GlSeparatedContext::new_shared(&surface.window, cb, &surface.events_loop).unwrap()
                }
            }
            Contexts::SurfaceContext(_) => unreachable!(),
        };

        let surface_context = Arc::new(SurfaceContext {
            context,
        });

        // There is a possiblity that the function pointers from one context
        // aren't valid with the other. We ere on the side of caution and
        // just open a new device (and therefore a new Share).
        // NOTE: We discard the name and VAO made by new/to_device cause we
        // don't need them.

        let adapter = PhysicalDevice::new_adapter(
            |s| surface_context.context.get_proc_address(s) as *const _,
            Contexts::SurfaceContext(Arc::downgrade(&surface_context)),
        );
        let (_, device) = adapter.physical_device.to_device(false).unwrap();

        surface.surface_context = Arc::downgrade(&surface_context);

        surface_context.context.resize(glutin::dpi::PhysicalSize {
            width: config.extent.width as _,
            height: config.extent.height as _,
        });

        let mut images = vec![];
        for _ in 0..config.image_count {

            let mut image = 0;
            unsafe {
                let gl = &device.share.context;
                gl.GenTextures(1, &mut image);
                gl.BindTexture(gl::TEXTURE_2D, image);
                gl.TexStorage2D(
                    gl::TEXTURE_2D,
                    1,
                    gl::SRGB8_ALPHA8,
                    config.extent.width as _,
                    config.extent.height as _
                );
            }

            let simage = SwapchainImage {
                image,
                currently_acquired: Mutex::new(false),
            };

            images.push(simage);
        }

        if let Err(err) = device.share.check() {
            panic!("Error creating swapchain: {:?}", err);
        }

        let mut fbo = 0;
        unsafe {
            let gl = &device.share.context;
            gl.GenFramebuffers(1, &mut fbo);
        }

        let swapchain = Swapchain {
            fbo,
            surface_context,
            device,
            window: Arc::clone(&surface.window),
            images,
            extent: config.extent,
        };

        let backbuffer = hal::Backbuffer::Images(
            swapchain.images.iter().map(|i| native::Image {
                kind: native::ImageKind::Texture(i.image),
                channel: f::Rgba8Srgb::SELF.base_format().1,
            }).collect()
        );
        (swapchain, backbuffer)
    }
}

pub struct ContextMaker;

#[derive(Debug)]
pub enum ContextCreationError {
    GlutinCreationError(glutin::CreationError),
    WinitCreationError(glutin::WindowCreationError),
    #[cfg(all(unix, not(target_os = "android")))]
    GlutinXNotSupported(XNotSupported),
}

pub enum RawDisplay {}
pub enum RawWindow {}

impl ContextMaker {
    #[cfg(all(unix, not(target_os = "android")))]
    pub fn new_empty() -> Result<(glutin::Context, glutin::EventsLoop), ContextCreationError> {
        let evlp = glutin::EventsLoop::new_x11()
            .map_err(|err| ContextCreationError::GlutinXNotSupported(err))?;
        glutin::Context::new(&evlp, glutin::ContextBuilder::new(), true)
            .map(|ctxt| (ctxt, evlp))
            .map_err(|err| ContextCreationError::GlutinCreationError(err))
    }

    #[cfg(all(unix, not(target_os = "android")))]
    pub fn new_xlib_window(
        display: *mut RawDisplay,
        window: *mut RawWindow,
    ) -> Result<(glutin::Window, glutin::EventsLoop), ContextCreationError> {
        let xconn = XConnection::new_from_display(
            XConnection::new_xlib_ptrs()
                .map_err(|err| ContextCreationError::GlutinXNotSupported(err))?,
            display as *mut _,
        ).map_err(|err| ContextCreationError::GlutinXNotSupported(err))?;

        let evlp_parts = glutin::wplatform::RawEventsLoopParts::X(glutin::wplatform::x11::RawEventsLoopParts {
            xconn: Arc::new(xconn),
        });
        let evlp = unsafe {
            glutin::EventsLoop::new_from_raw_parts(&evlp_parts)
        };

        let win_parts = glutin::wplatform::RawWindowParts::X(glutin::wplatform::x11::RawWindowParts {
            xwindow: window as _,
        });
        let win = unsafe {
            glutin::Window::new_from_raw_parts(&evlp, &win_parts)
                .map_err(|err| ContextCreationError::WinitCreationError(err))?
        };

        Ok((win, evlp))
    }
}

impl Instance {
    #[cfg(all(unix, not(target_os = "android")))]
    pub fn create_surface(&self, window: &glutin::Window) -> Surface {
        if let Some(display) = window.get_xlib_display() {
            let window = window.get_xlib_window().unwrap() as *mut _;
            let display = display as *mut _;

            let (window, events_loop) = ContextMaker::new_xlib_window(
                display,
                window,
            ).unwrap();

            let events_loop = Starc::new(events_loop);
            let window = Arc::new(window);

            Surface {
                window,
                events_loop,
                surface_context: Weak::new(),
            }
        } else {
            panic!("Winit window wasn't a xlib window.")
        }
    }
}
