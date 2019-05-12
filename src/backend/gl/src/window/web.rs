use crate::hal::window::Extent2D;
use crate::hal::{self, format as f, image, memory, CompositeAlpha};
use crate::{native, Backend as B, Device, PhysicalDevice, QueueFamily};

use glow::Context;

fn get_window_extent(window: &Window) -> image::Extent {
    image::Extent {
        width: 640 as image::Size,
        height: 480 as image::Size,
        depth: 1,
    }
}

struct PixelFormat {
    color_bits: u32,
    alpha_bits: u32,
    srgb: bool,
    double_buffer: bool,
    multisampling: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Window;

impl Window {
    fn get_pixel_format(&self) -> PixelFormat {
        PixelFormat {
            color_bits: 24,
            alpha_bits: 8,
            srgb: false,
            double_buffer: true,
            multisampling: None,
        }
    }

    pub fn get_hidpi_factor(&self) -> i32 {
        1
    }

    pub fn resize<T>(&self, parameter: T) {}
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) window: Window,
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

#[derive(Debug)]
pub struct Surface {
    window: Window,
}

impl Surface {
    pub fn from_window(window: Window) -> Self {
        Surface { window: Window }
    }

    pub fn get_window(&self) -> &Window {
        &self.window
    }

    pub fn window(&self) -> &Window {
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
    // TODO: Share most of this implementation with `glutin`
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
        let adapter = PhysicalDevice::new_adapter(|s| 0 as *const _, Some("canvas")); // TODO: Move to `self` like native/window
        vec![adapter]
    }
}
