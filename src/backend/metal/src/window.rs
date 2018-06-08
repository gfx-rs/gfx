use {AutoreleasePool, Backend, QueueFamily};
use internal::Channel;
use native;
use device::{Device, PhysicalDevice};

use std::{fmt, ops};
use std::sync::{Arc, Mutex, RwLock};

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use metal;
use objc::runtime::{Object};
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use cocoa::foundation::{NSRect};


pub type CAMetalLayer = *mut Object;
pub type CADrawable = *mut Object;

pub struct Surface {
    pub(crate) inner: Arc<SurfaceInner>,
    pub(crate) apply_pixel_scale: bool,
}

pub(crate) struct SurfaceInner {
    pub(crate) nsview: *mut Object,
    pub(crate) render_layer: Mutex<CAMetalLayer>,
}

unsafe impl Send for SurfaceInner {}
unsafe impl Sync for SurfaceInner {}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

pub struct SwapchainInner {
    frames: Vec<Option<(CADrawable, metal::Texture)>>,
}

impl ops::Index<hal::FrameImage> for SwapchainInner {
    type Output = metal::TextureRef;
    fn index(&self, index: hal::FrameImage) -> &Self::Output {
        self.frames[index as usize]
            .as_ref()
            .map(|&(_, ref tex)| tex)
            .expect("Frame texture is not resident!")
    }
}

unsafe impl Send for SwapchainInner {}
unsafe impl Sync for SwapchainInner {}

impl fmt::Debug for SwapchainInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Swapchain with {} image", self.frames.len())
    }
}

impl Drop for SwapchainInner {
    fn drop(&mut self) {
        for maybe in self.frames.drain(..) {
            if let Some((drawable, _)) = maybe {
                unsafe {
                    msg_send![drawable, release];
                }
            }
        }
    }
}

pub struct Swapchain {
    inner: Arc<RwLock<SwapchainInner>>,
    surface: Arc<SurfaceInner>,
    _size_pixels: (u64, u64),
}

unsafe impl Send for Swapchain {}
unsafe impl Sync for Swapchain {}

impl Swapchain {
    pub(crate) fn matches(&self, other: &Arc<RwLock<SwapchainInner>>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }

    pub(crate) fn present(&self, index: hal::FrameImage) {
        let render_layer_borrow = self.surface.render_layer.lock().unwrap();
        let (drawable, _) = self.inner
            .write()
            .unwrap()
            .frames[index as usize]
            .take()
            .expect("Frame is not ready to present!");
        unsafe {
            let render_layer = *render_layer_borrow;
            msg_send![render_layer, presentDrawable:drawable];
            msg_send![drawable, release];
        }
    }
}


impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let (width, height) = self.pixel_dimensions();

        image::Kind::D2(width, height, 1, 1)
    }

    fn capabilities_and_formats(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>) {
        let caps = hal::SurfaceCapabilities {
            image_count: 1..8,
            current_extent: None,
            extents: Extent2D { width: 4, height: 4} .. Extent2D { width: 4096, height: 4096 },
            max_image_layers: 1,
        };
        let formats = Some(vec![format::Format::Rgba8Srgb]);
        (caps, formats)
    }

    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool {
        true // TODO: Not sure this is the case, don't know associativity of IOSurface
    }
}

impl Surface {
    fn pixel_dimensions(&self) -> (image::Size, image::Size) {
        unsafe {
            // NSView bounds are measured in DIPs
            let bounds: NSRect = msg_send![self.inner.nsview, bounds];
            let bounds_pixel: NSRect = msg_send![self.inner.nsview, convertRectToBacking:bounds];
            (bounds_pixel.size.width as _, bounds_pixel.size.height as _)
        }
    }
}

impl Device {
    pub(crate) fn build_swapchain(
        &self,
        surface: &mut Surface,
        config: SwapchainConfig,
    ) -> (Swapchain, Backbuffer<Backend>) {
        let mtl_format = self.private_caps
            .map_format(config.color_format)
            .expect("unsupported backbuffer format");

        let render_layer_borrow = surface.inner.render_layer.lock().unwrap();
        let render_layer = *render_layer_borrow;
        let nsview = surface.inner.nsview;
        let format_desc = config.color_format.surface_desc();
        let framebuffer_only = config.image_usage == image::Usage::COLOR_ATTACHMENT;
        let device = self.shared.device.lock().unwrap();

        let (view_size, scale_factor) = unsafe {
            msg_send![render_layer, setDevice: &device];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];

            // Update render layer size
            let view_points_size: CGRect = msg_send![nsview, bounds];
            msg_send![render_layer, setBounds: view_points_size];

            let view_window: *mut Object = msg_send![nsview, window];
            if view_window.is_null() {
                panic!("surface is not attached to a window");
            }
            let scale_factor: CGFloat = if surface.apply_pixel_scale {
                msg_send![view_window, backingScaleFactor]
            } else {
                1.0
            };
            msg_send![render_layer, setContentsScale: scale_factor];
            info!("view points size {:?} scale factor {:?}", view_points_size, scale_factor);
            (view_points_size.size, scale_factor)
        };

        let pixel_width = (view_size.width * scale_factor) as u64;
        let pixel_height = (view_size.height * scale_factor) as u64;

        let inner = SwapchainInner {
            frames: (0 .. config.image_count).map(|_| None).collect(),
        };

        let swapchain = Swapchain {
            inner: Arc::new(RwLock::new(inner)),
            surface: surface.inner.clone(),
            _size_pixels: (pixel_width, pixel_height),
        };

        let images = (0 .. config.image_count)
            .map(|index| native::Image {
                root: native::ImageRoot::Frame(native::Frame {
                    swapchain: swapchain.inner.clone(),
                    index,
                }),
                extent: image::Extent {
                    width: pixel_width as _,
                    height: pixel_height as _,
                    depth: 1,
                },
                num_layers: None,
                format_desc,
                shader_channel: Channel::Float,
                mtl_format,
                mtl_type: metal::MTLTextureType::D2,
            })
            .collect();

        (swapchain, Backbuffer::Images(images))
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, sync: hal::FrameSync<Backend>) -> Result<hal::FrameImage, ()> {
        unsafe {
            match sync {
                hal::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                hal::FrameSync::Fence(_fence) => unimplemented!(),
            }
        }

        let layer = self.surface.render_layer.lock().unwrap();
        let mut inner = self.inner.write().unwrap();

        let index = inner.frames
            .iter_mut()
            .position(|d| d.is_some())
            .expect("No frames available to acquire!");

        let _ap = AutoreleasePool::new(); // for the drawable
        inner.frames[index] = Some(unsafe {
            let drawable: *mut Object = msg_send![*layer, nextDrawable];
            let texture: metal::Texture = msg_send![drawable, getTexture];
            msg_send![drawable, retain];
            msg_send![texture, retain];
            (drawable, texture)
        });

            let frame = self.frame_index % self.io_surfaces.len();
            self.frame_index += 1;
            Ok(frame as _)
        }
    }
}
