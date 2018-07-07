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

use hal::{self, format as f, image, memory as m};
use hal::format::AsFormat;

use {
    Backend as B, Device, PhysicalDevice, QueueFamily, Starc, gl, native, Wstarc,
    Contexts, Instance
};

use glutin::{self, GlContext};

use std::sync::{Arc, Mutex};

fn physical_to_extent(ex: &glutin::dpi::PhysicalSize) -> hal::window::Extent2D {
    hal::window::Extent2D {
        width: ex.width as _,
        height: ex.height as _,
    }
}

pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) window: Arc<Mutex<Window>>,
}

impl hal::Swapchain<B> for Swapchain {
    fn acquire_image(&mut self, _sync: hal::FrameSync<B>) -> Result<hal::SwapImageIndex, ()> {
        // TODO: sync
        // Insure the extent hasn't changed.
        let window = self.window.lock().unwrap();
        let extent = window.swapchain.as_ref().unwrap().extent;
        let new_extent = window
            .get_inner_size()
            .unwrap();
        if extent.width != new_extent.width as _ || extent.height != new_extent.height as _ {
            return Err(())
        }
        else {
            Ok(0)
        }
    }
}

pub(crate) struct WindowState {
    pub(crate) window: Starc<glutin::GlWindow>,
    pub(crate) device: Arc<Device>,
    pub(crate) fbo: u32,
}

pub(crate) struct SwapchainState {
    pub(crate) image: native::Image,
    pub(crate) image_mem: native::Memory,
    pub(crate) extent: hal::window::Extent2D,
}

pub struct Window {
    wb: Option<Starc<glutin::WindowBuilder>>,
    pub(crate) window: Option<WindowState>,
    pub(crate) swapchain: Option<SwapchainState>,
}

impl Drop for Window {
    fn drop(&mut self) {
        use hal::Device;
        if let Some(mut ws) = self.window.take() {
            unsafe {
                let gl = &ws.device.share.context;
                gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl.DeleteFramebuffers(1, &mut ws.fbo);
            }

            if let Some(scs) = self.swapchain.take() {
                ws.device.destroy_image(scs.image);
                ws.device.free_memory(scs.image_mem);
            }
        }
    }
}

impl Window {
    pub fn new(wb: glutin::WindowBuilder) -> Arc<Mutex<Window>> {
        Arc::new(Mutex::new(Window {
            wb: Some(Starc::new(wb)),
            window: None,
            swapchain: None,
        }))
    }

    fn config_context(
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

    pub fn get_inner_size(&self) -> Option<glutin::dpi::PhysicalSize> {
        match self.window {
            Some(ref w) => {
                let ret = w.window
                    .get_inner_size()
                    .map(|s| s.to_physical(w.window.get_hidpi_factor()));
                ret
            }
            None => None,
        }
    }
}

pub struct Surface {
    pub(crate) window: Arc<Mutex<Window>>,
}

impl Surface {
    fn swapchain_formats(&self, w: &Starc<glutin::GlWindow>) -> Vec<f::Format> {
        let pixel_format = w.get_pixel_format();
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

impl hal::Surface<B> for Arc<Surface> {
    fn kind(&self) -> hal::image::Kind {
        let windowi = self.window.lock().unwrap();
        let w = windowi.window.as_ref().unwrap();

        let ex = windowi
            .swapchain
            .as_ref()
            .map(|s| s.extent)
            .unwrap_or_else(|| {
                physical_to_extent(
                    &w
                    .window
                    .get_inner_size()
                    .unwrap()
                    .to_physical(w.window.get_hidpi_factor())
                )
            });

        let samples = w.window.get_pixel_format().multisampling.unwrap_or(1);
        hal::image::Kind::D2(ex.width, ex.height, 1, samples as _)
    }

    fn compatibility(
        &self, _: &PhysicalDevice
    ) -> (hal::SurfaceCapabilities, Option<Vec<f::Format>>, Vec<hal::PresentMode>) {
        let present_modes = vec![hal::PresentMode::Fifo]; //TODO

        let window = self.window.lock().unwrap();
        let window = window.window.as_ref();
        let (extent, extents, image_count, formats) = match window {
            Some(w) => {
                let extent = physical_to_extent(
                    &w
                    .window
                    .get_inner_size()
                    .unwrap()
                    .to_physical(w.window.get_hidpi_factor())
                );
                let extents = extent .. hal::window::Extent2D {
                    width: extent.width + 1,
                    height: extent.height + 1,
                };

                let image_count = if w.window.get_pixel_format().double_buffer { 2..3 } else { 1..2 };
                let formats = self.swapchain_formats(&w.window);

                (Some(extent), extents, image_count, formats)
            }
            None => {
                // TODO return proper requirments, currently doesn't matter
                // because we just ignore them
                let extent = hal::window::Extent2D {
                    width: 0xFFFFFFFF,
                    height: 0xFFFFFFFF,
                };
                let extents = hal::window::Extent2D {
                    width: 1,
                    height: 1,
                } .. extent;

                let formats = vec![
                    f::Format::Rgba8Srgb,
                    f::Format::Bgra8Srgb,
                    f::Format::Rgba8Unorm,
                    f::Format::Bgra8Unorm,
                ];
                (None, extents, 2..3, formats)
            }
        };

        let caps = hal::SurfaceCapabilities {
            image_count,
            current_extent: extent,
            extents,
            max_image_layers: 1,
        };

        (caps, Some(formats), present_modes)
    }

    fn supports_queue_family(&self, _: &QueueFamily) -> bool { true }
}

// We can't use `impl Arc<Device>` but we can use `impl GlGlutinDevice for Arc<Device>`
// so we got to use this cheat.
pub(crate) trait GlGlutinDevice {
    fn create_swapchain_impl(
        &self,
        surface: &mut Arc<Surface>,
        _config: hal::SwapchainConfig,
        extent: hal::window::Extent2D,
    ) -> (Swapchain, hal::Backbuffer<B>);
}

impl GlGlutinDevice for Arc<Device> {
    fn create_swapchain_impl(
        &self,
        surface: &mut Arc<Surface>,
        _config: hal::SwapchainConfig,
        extent: hal::window::Extent2D,
    ) -> (Swapchain, hal::Backbuffer<B>) {
        // TODO: actually respect the swapchain configuration provided by the user.
        // Be sure to fix the options returned by `Compatibility`
        // NOTE: If the window's already been made, we can't change the config. if
        // we try to keep regenerating it we will (rightfully?) get an error
        // from either the events_loop or X11 itself (after a couple tries that is)
        // (at least on my platform, others may vary)
        let mut swindow = surface.window.lock().unwrap();
        {
            let ws = if let Some(ws) = swindow.window.take() {
                ws
            } else {
                let window = {
                    let instance = match *self.share.context.instance.lock().unwrap() {
                        Contexts::Instance(ref i) => Wstarc::upgrade(i).unwrap(),
                        _ => panic!(),
                    };

                    let cb =
                        Window::config_context(
                            glutin::ContextBuilder::new(),
                            f::Rgba8Srgb::SELF,
                            None,
                        )
                        .with_vsync(true)
                        .with_shared_lists(&instance.instance_context.context);

                    let wb = swindow.wb.take().unwrap().try_unwrap().unwrap()
                        .with_dimensions(glutin::dpi::LogicalSize {
                            width: extent.width as _,
                            height: extent.height as _,
                        });

                    let el = &*instance.instance_context.el.lock().unwrap();

                    Starc::new(glutin::GlWindow::new(
                        wb,
                        cb,
                        el,
                    ).unwrap())
                };

                // There is a possiblity that the function pointers from one context
                // aren't valid with the other. We ere on the side of caution and just
                // open a new device (and therefore a new Share)
                // NOTE: We discard the name and VAO made by new/new_device cause
                // we don't need them
                let (_, pd) = PhysicalDevice::new(
                    |s| window.get_proc_address(s) as *const _,
                    Contexts::Window(Starc::downgrade(&window))
                );

                let (_, device) = Instance::new_device(&pd).unwrap();

                let mut fbo = 0;
                unsafe {
                    let gl = &device.share.context;
                    gl.GenFramebuffers(1, &mut fbo);
                }

                WindowState {
                    device,
                    window,
                    fbo,
                }
            };

            // We recreate the image we render to every time in case it's size
            // changes.
            use hal::Device;
            let kind = image::Kind::D2(extent.width, extent.height, 1, 1);
            let uimage = self.create_image(
                kind,
                1,
                f::Rgba8Srgb::SELF,
                image::Tiling::Optimal, // ignored
                image::Usage::SAMPLED, // FIXME: Is this the correct one? I'm trying to get a texture, not a surface, and this appears to be the way.
                image::StorageFlags::empty(),
            ).unwrap();

            let image_req = self.get_image_requirements(&uimage);
            let device_type = PhysicalDevice::memory_properties(&self.share.private_caps)
                .memory_types
                .iter()
                .enumerate()
                .position(|(id, memory_type)| {
                    image_req.type_mask & (1 << id) != 0
                        && memory_type.properties.contains(m::Properties::DEVICE_LOCAL)
                })
                .unwrap()
                .into();
            let image_mem = self.allocate_memory(device_type, image_req.size).unwrap();

            let image = self
                .bind_image_memory(&image_mem, 0, uimage)
                .unwrap();

            unsafe {
                let gl = &ws.device.share.context;
                let fbo = ws.fbo;
                let image = match image.kind {
                    native::ImageKind::Texture(t) => t,
                    _ => panic!(),
                };

                gl.BindFramebuffer(gl::FRAMEBUFFER, fbo);
                gl.FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    image,
                    0,
                );

                if let Err(err) = self.share.check() {
                    panic!("Error creating FBO for swapchain: {:?}", err);
                }
            }

            let scs = SwapchainState {
                image,
                image_mem,
                extent,
            };

            swindow.swapchain = Some(scs);
            swindow.window = Some(ws);
        }

        let swapchain = Swapchain {
            window: Arc::clone(&surface.window),
        };

        let backbuffer = hal::Backbuffer::Images(vec![swindow.swapchain.as_ref().unwrap().image]);
        (swapchain, backbuffer)
    }
}
