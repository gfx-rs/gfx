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
//!     let cb = ContextBuilder::new().with_vsync(true);
//!     let glutin_window = WindowedContext::new_windowed(wb, cb, &events_loop).unwrap();
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
//! use glutin::{Context, ContextBuilder, EventsLoop};
//!
//! fn main() {
//!     let events_loop = EventsLoop::new();
//!     let context = Context::new_headless(&events_loop, ContextBuilder::new(), glutin::dpi::PhysicalSize::new(0.0, 0.0))
//!         .expect("Failed to build headless context");
//!     let headless = Headless(context);
//!     let _adapters = headless.enumerate_adapters();
//! }
//! ```

use crate::hal::window::Extent2D;
use crate::hal::{self, format as f, image, memory, CompositeAlpha};
use crate::{native, Backend as B, Device, PhysicalDevice, QueueFamily, Starc};

use glow::Context;

use glutin::{self, ContextTrait};

fn get_window_extent(window: &glutin::WindowedContext) -> image::Extent {
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
    pub(crate) window: Starc<glutin::WindowedContext>,
    // Extent because the window lies
    pub(crate) extent: Extent2D,
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

//TODO: if we make `Surface` a `WindowBuilder` instead of `WindowedContext`,
// we could spawn window + GL context when a swapchain is requested
// and actually respect the swapchain configuration provided by the user.
#[derive(Debug)]
pub struct Surface {
    window: Starc<glutin::WindowedContext>,
}

impl Surface {
    pub fn from_window(window: glutin::WindowedContext) -> Self {
        Surface {
            window: Starc::new(window),
        }
    }

    pub fn get_window(&self) -> &glutin::WindowedContext {
        &*self.window
    }

    pub fn window(&self) -> &glutin::WindowedContext {
        &self.window
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        let pixel_format = self.window.get_pixel_format();
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
    fn kind(&self) -> hal::image::Kind {
        let ex = get_window_extent(&self.window);
        let samples = self.window.get_pixel_format().multisampling.unwrap_or(1);
        hal::image::Kind::D2(ex.width, ex.height, 1, samples as _)
    }

    fn compatibility(
        &self,
        _: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<f::Format>>,
        Vec<hal::PresentMode>,
    ) {
        let ex = get_window_extent(&self.window);
        let extent = hal::window::Extent2D::from(ex);

        let caps = hal::SurfaceCapabilities {
            image_count: if self.window.get_pixel_format().double_buffer {
                2..3
            } else {
                1..2
            },
            current_extent: Some(extent),
            extents: extent..hal::window::Extent2D {
                width: ex.width + 1,
                height: ex.height + 1,
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

impl Device {
    pub(crate) fn create_swapchain_impl(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
    ) -> (Swapchain, Vec<native::Image>) {
        let swapchain = Swapchain {
            extent: config.extent,
            window: surface.window.clone(),
        };

        let gl = &self.share.context;

        let (int_format, iformat, itype) = match config.format {
            f::Format::Rgba8Unorm => (glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE),
            f::Format::Rgba8Srgb => (glow::SRGB8_ALPHA8, glow::RGBA, glow::UNSIGNED_BYTE),
            _ => unimplemented!(),
        };

        let channel = config.format.base_format().1;

        let images = (0..config.image_count)
            .map(|_| unsafe {
                let image = if config.image_layers > 1
                    || config.image_usage.contains(image::Usage::STORAGE)
                    || config.image_usage.contains(image::Usage::SAMPLED)
                {
                    let name = gl.create_texture().unwrap();
                    match config.extent {
                        Extent2D {
                            width: w,
                            height: h,
                        } => {
                            gl.bind_texture(glow::TEXTURE_2D, Some(name));
                            if self.share.private_caps.image_storage {
                                gl.tex_storage_2d(
                                    glow::TEXTURE_2D,
                                    config.image_layers as _,
                                    int_format,
                                    w as _,
                                    h as _,
                                );
                            } else {
                                gl.tex_parameter_i32(
                                    glow::TEXTURE_2D,
                                    glow::TEXTURE_MAX_LEVEL,
                                    (config.image_layers - 1) as _,
                                );
                                let mut w = w;
                                let mut h = h;
                                for i in 0..config.image_layers {
                                    gl.tex_image_2d(
                                        glow::TEXTURE_2D,
                                        i as _,
                                        int_format as _,
                                        w as _,
                                        h as _,
                                        0,
                                        iformat,
                                        itype,
                                        None,
                                    );
                                    w = std::cmp::max(w / 2, 1);
                                    h = std::cmp::max(h / 2, 1);
                                }
                            }
                        }
                    };
                    native::ImageKind::Texture(name)
                } else {
                    let name = gl.create_renderbuffer().unwrap();
                    match config.extent {
                        Extent2D {
                            width: w,
                            height: h,
                        } => {
                            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(name));
                            gl.renderbuffer_storage(glow::RENDERBUFFER, int_format, w as _, h as _);
                        }
                    };
                    native::ImageKind::Surface(name)
                };

                let surface_desc = config.format.base_format().0.desc();
                let bytes_per_texel = surface_desc.bits / 8;
                let ext = config.extent;
                let size = (ext.width * ext.height) as u64 * bytes_per_texel as u64;

                if let Err(err) = self.share.check() {
                    panic!(
                        "Error creating swapchain image: {:?} with {:?} format",
                        err, config.format
                    );
                }

                native::Image {
                    kind: image,
                    channel,
                    requirements: memory::Requirements {
                        size,
                        alignment: 1,
                        type_mask: 0x7,
                    },
                }
            })
            .collect::<Vec<_>>();

        (swapchain, images)
    }
}

impl hal::Instance for Surface {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        unsafe { self.window.make_current().unwrap() };
        let adapter =
            PhysicalDevice::new_adapter(|s| self.window.get_proc_address(s) as *const _, None);
        vec![adapter]
    }
}

pub fn config_context(
    builder: glutin::ContextBuilder,
    color_format: f::Format,
    ds_format: Option<f::Format>,
) -> glutin::ContextBuilder {
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

pub struct Headless(pub glutin::Context);

unsafe impl Send for Headless {}
unsafe impl Sync for Headless {}

impl hal::Instance for Headless {
    type Backend = B;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<B>> {
        unsafe { self.0.make_current().unwrap() };
        let adapter = PhysicalDevice::new_adapter(|s| self.0.get_proc_address(s) as *const _, None);
        vec![adapter]
    }
}
