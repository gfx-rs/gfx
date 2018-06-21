use {AutoreleasePool, Backend, QueueFamily};
use internal::Channel;
use native;
use device::{Device, PhysicalDevice};

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use metal;
use objc::runtime::{Object};
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use cocoa::foundation::{NSRect};
use foreign_types::{ForeignType, ForeignTypeRef};


pub type CAMetalLayer = *mut Object;
pub type CADrawable = *mut Object;

pub struct Surface {
    pub(crate) inner: Arc<SurfaceInner>,
    pub(crate) apply_pixel_scale: bool,
}

//TODO: double-check who needs it shared
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

#[derive(Debug)]
struct Frame {
    drawable: Cell<Option<CADrawable>>,
    texture: metal::Texture,
}

impl Drop for Frame {
    fn drop(&mut self) {
        if let Some(drawable) = self.drawable.get() {
            unsafe {
                msg_send![drawable, release];
            }
        }
    }
}

pub struct Swapchain {
    frames: Vec<Frame>,
    surface: Arc<SurfaceInner>,
    _size_pixels: (u64, u64),
}

unsafe impl Send for Swapchain {}
unsafe impl Sync for Swapchain {}

impl Swapchain {
    pub(crate) fn present(&self, index: hal::SwapImageIndex) {
        let drawable = self.frames[index as usize].drawable
            .replace(None)
            .unwrap();
        unsafe {
            msg_send![drawable, present];
            //TODO: delay the actual release
            msg_send![drawable, release];
        }
    }
}


impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let (width, height) = self.pixel_dimensions();

        image::Kind::D2(width, height, 1, 1)
    }

    fn compatibility(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>, Vec<hal::PresentMode>) {
        let caps = hal::SurfaceCapabilities {
            //Note: this is hardcoded in `CAMetalLayer` documentation
            image_count: 2 .. 3,
            current_extent: None,
            extents: Extent2D { width: 4, height: 4} .. Extent2D { width: 4096, height: 4096 },
            max_image_layers: 1,
        };

        let formats = vec![
            format::Format::Bgra8Unorm,
            format::Format::Bgra8Srgb,
            format::Format::Rgba16Float,
        ];
        let present_modes = vec![
            hal::PresentMode::Fifo,
            hal::PresentMode::Immediate,
        ];

        (caps, Some(formats), present_modes)
    }

    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool {
        // we only expose one family atm, so it's compatible
        true
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
        let _ap = AutoreleasePool::new(); // for the drawable

        let mtl_format = self.private_caps
            .map_format(config.color_format)
            .expect("unsupported backbuffer format");

        let render_layer_borrow = surface.inner.render_layer.lock().unwrap();
        let render_layer = *render_layer_borrow;
        let nsview = surface.inner.nsview;
        let format_desc = config.color_format.surface_desc();
        let framebuffer_only = config.image_usage == image::Usage::COLOR_ATTACHMENT;
        let display_sync = match config.present_mode {
            hal::PresentMode::Immediate => false,
            _ => true,
        };
        let device = self.shared.device.lock().unwrap();
        let device_raw: &metal::DeviceRef = &*device;

        let (view_size, scale_factor) = unsafe {
            msg_send![render_layer, setDevice: device_raw];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];
            msg_send![render_layer, setMaximumDrawableCount: config.image_count as u64];
            //TODO: only set it where supported
            msg_send![render_layer, setDisplaySyncEnabled: display_sync];
            //msg_send![render_layer, setPresentsWithTransaction: true];

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

        let frames = (0 .. config.image_count)
            .map(|_| unsafe {
                let drawable: *mut Object = msg_send![render_layer, nextDrawable];
                assert!(!drawable.is_null());
                let texture: metal::Texture = msg_send![drawable, texture];
                //HACK: not retaining the texture here
                Frame {
                    drawable: Cell::new(None), //Note: careful!
                    texture,
                }
            })
            .collect::<Vec<_>>();

        let images = frames
            .iter()
            .map(|frame| native::Image {
                raw: frame.texture.clone(), //Note: careful!
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

        let swapchain = Swapchain {
            frames,
            surface: surface.inner.clone(),
            _size_pixels: (pixel_width, pixel_height),
        };

        (swapchain, Backbuffer::Images(images))
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_image(&mut self, sync: hal::FrameSync<Backend>) -> Result<hal::SwapImageIndex, ()> {
        let _ap = AutoreleasePool::new(); // for the drawable

        unsafe {
            match sync {
                hal::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                hal::FrameSync::Fence(_fence) => unimplemented!(),
            }
        }

        let layer_ref = self.surface.render_layer.lock().unwrap();
        let drawable: CADrawable = unsafe {
            msg_send![*layer_ref, nextDrawable]
        };
        let texture_temp: &metal::TextureRef = unsafe {
            msg_send![drawable, retain];
            msg_send![drawable, texture]
        };

        let index = self.frames
            .iter()
            .position(|f| f.texture.as_ptr() == texture_temp.as_ptr())
            .expect("Surface lost?");
        let old = self.frames[index].drawable.replace(Some(drawable));
        assert_eq!(old, None);

        Ok(index as _)
    }
}
