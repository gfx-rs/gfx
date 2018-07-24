use {Backend, QueueFamily};
use device::{Device, PhysicalDevice};
use internal::Channel;
use native;

use std::sync::Arc;

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use cocoa::foundation::{NSRect};
use foreign_types::{ForeignType, ForeignTypeRef};
use parking_lot::{Mutex, MutexGuard};
use metal;
use objc::rc::autoreleasepool;
use objc::runtime::Object;


pub type CAMetalLayer = *mut Object;

pub struct Surface {
    pub(crate) inner: Arc<SurfaceInner>,
    pub(crate) apply_pixel_scale: bool,
    pub(crate) has_swapchain: bool
}

#[derive(Debug)]
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

impl SurfaceInner {
    fn next_frame<'a>(&self, frames: &'a [Frame]) -> (usize, MutexGuard<'a, FrameInner>) {
        let layer_ref = self.render_layer.lock();
        autoreleasepool(|| { // for the drawable
            let (drawable, texture_temp): (&metal::DrawableRef, &metal::TextureRef) = unsafe {
                let drawable = msg_send![*layer_ref, nextDrawable];
                (drawable, msg_send![drawable, texture])
            };

            trace!("looking for {:?}", texture_temp);
            let index = frames
                .iter()
                .position(|f| f.texture.as_ptr() == texture_temp.as_ptr())
                .expect("Surface lost?");

            let mut frame = frames[index].inner.lock();
            assert!(frame.drawable.is_none());
            frame.drawable = Some(drawable.to_owned());

            debug!("next is frame[{}]", index);
            (index, frame)
        })
    }
}

#[derive(Debug)]
struct FrameInner {
    drawable: Option<metal::Drawable>,
    /// If there is a `drawable`, availability indicates if it's free for grabs.
    /// If there is `None`, `available == false` means that the frame has already
    /// been acquired and the `drawable` will appear at some point.
    available: bool,
    last_frame: usize,
}

#[derive(Debug)]
struct Frame {
    inner: Mutex<FrameInner>,
    texture: metal::Texture,
}

unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Drop for Frame {
    fn drop(&mut self) {
        info!("dropping Frame");
    }
}

pub struct Swapchain {
    frames: Arc<Vec<Frame>>,
    surface: Arc<SurfaceInner>,
    size_pixels: (image::Size, image::Size),
    last_frame: usize,
    image_ready_callbacks: Vec<Arc<Mutex<Option<SwapchainImage>>>>,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        info!("dropping Swapchain");
        for ir in self.image_ready_callbacks.drain(..) {
            if ir.lock().take().is_some() {
                debug!("\twith a callback");
            }
        }
    }
}

impl Swapchain {
    /// Returns the drawable for the specified swapchain image index,
    /// marks the index as free for future use.
    pub(crate) fn take_drawable(&self, index: hal::SwapImageIndex) -> metal::Drawable {
        let mut frame = self
            .frames[index as usize]
            .inner
            .lock();
        assert!(!frame.available);
        frame.available = true;
        frame.drawable
            .take()
            .expect("Drawable has not been acquired!")
    }

    fn signal_sync(&self, sync: hal::FrameSync<Backend>) {
        match sync {
            hal::FrameSync::Semaphore(semaphore) => {
                if let Some(ref system) = semaphore.system {
                    system.signal();
                }
            }
            hal::FrameSync::Fence(fence) => {
                *fence.0.borrow_mut() = native::FenceInner::Idle { signaled: true };
            }
        }
    }
}

#[derive(Debug)]
pub struct SwapchainImage {
    frames: Arc<Vec<Frame>>,
    surface: Arc<SurfaceInner>,
    index: hal::SwapImageIndex,
}

impl SwapchainImage {
    /// Waits until the specified swapchain index is available for rendering.
    /// Returns the number of frames it had to wait.
    pub fn wait_until_ready(&self) -> usize {
        // check the target frame first
        {
            let frame = self.frames[self.index as usize].inner.lock();
            assert!(!frame.available);
            if frame.drawable.is_some() {
                return 0;
            }
        }
        // wait for new frames to come until we meet the chosen one
        let mut count = 1;
        while self.surface.next_frame(&self.frames).0 != self.index as usize {
            count += 1;
        }
        debug!("Swapchain image is ready after {} frames", count);
        count
    }
}


impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let (width, height) = self.inner.pixel_dimensions();

        image::Kind::D2(width, height, 1, 1)
    }

    fn compatibility(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>, Vec<hal::PresentMode>) {
        let caps = hal::SurfaceCapabilities {
            //Note: this is hardcoded in `CAMetalLayer` documentation
            image_count: 2 .. 4,
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

impl SurfaceInner {
    fn pixel_dimensions(&self) -> (image::Size, image::Size) {
        unsafe {
            // NSView bounds are measured in DIPs
            let bounds: NSRect = msg_send![self.nsview, bounds];
            let bounds_pixel: NSRect = msg_send![self.nsview, convertRectToBacking:bounds];
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
        info!("build_swapchain {:?}", config);

        let mtl_format = self.private_caps
            .map_format(config.color_format)
            .expect("unsupported backbuffer format");

        let render_layer_borrow = surface.inner.render_layer.lock();
        let render_layer = *render_layer_borrow;
        let nsview = surface.inner.nsview;
        let format_desc = config.color_format.surface_desc();
        let framebuffer_only = config.image_usage == image::Usage::COLOR_ATTACHMENT;
        let display_sync = match config.present_mode {
            hal::PresentMode::Immediate => false,
            _ => true,
        };
        let device = self.shared.device.lock();
        let device_raw: &metal::DeviceRef = &*device;

        let (view_size, scale_factor) = unsafe {
            msg_send![render_layer, setDevice: device_raw];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];
            msg_send![render_layer, setMaximumDrawableCount: config.image_count as u64];
            //TODO: only set it where supported
            msg_send![render_layer, setDisplaySyncEnabled: display_sync];

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

        let pixel_width = (view_size.width * scale_factor) as image::Size;
        let pixel_height = (view_size.height * scale_factor) as image::Size;

        let frames = (0 .. config.image_count)
            .map(|index| autoreleasepool(|| { // for the drawable & texture
                let (drawable, texture) = unsafe {
                    let drawable: &metal::DrawableRef = msg_send![render_layer, nextDrawable];
                    assert!(!drawable.as_ptr().is_null());
                    let texture: &metal::TextureRef = msg_send![drawable, texture];
                    (drawable, texture)
                };
                trace!("\tframe[{}] = {:?}", index, texture);

                let drawable = if index == 0 && surface.has_swapchain {
                    // when resizing, this trick frees up the currently shown frame
                    // HACK: the has_swapchain is unfortunate, and might not be
                    //       correct in all cases.
                    drawable.present();
                    None
                } else {
                    Some(drawable.to_owned())
                };
                Frame {
                    inner: Mutex::new(FrameInner {
                        drawable,
                        available: true,
                        last_frame: 0,
                    }),
                    texture: texture.to_owned(),
                }
            }))
            .collect::<Vec<_>>();

        let images = frames
            .iter()
            .map(|frame| native::Image {
                raw: frame.texture.clone(),
                kind: image::Kind::D2(pixel_width, pixel_height, 1, 1),
                format_desc,
                shader_channel: Channel::Float,
                mtl_format,
                mtl_type: metal::MTLTextureType::D2,
            })
            .collect();

        surface.has_swapchain = true;

        let swapchain = Swapchain {
            frames: Arc::new(frames),
            surface: surface.inner.clone(),
            size_pixels: (pixel_width, pixel_height),
            last_frame: 0,
            image_ready_callbacks: Vec::new(),
        };


        (swapchain, Backbuffer::Images(images))
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_image(&mut self, sync: hal::FrameSync<Backend>) -> Result<hal::SwapImageIndex, ()> {
        self.last_frame += 1;

        //TODO: figure out a proper story of HiDPI
        if false && self.surface.pixel_dimensions() != self.size_pixels {
            return Err(())
        }

        let mut oldest_index = 0;
        let mut oldest_frame = self.last_frame;

        for (index, frame_arc) in self.frames.iter().enumerate() {
            let mut frame = frame_arc.inner.lock();
            if !frame.available {
                continue
            }
            if frame.drawable.is_some() {
                frame.available = false;
                frame.last_frame = self.last_frame;
                self.signal_sync(sync);
                return Ok(index as _);
            }
            if frame.last_frame < oldest_frame {
                oldest_frame = frame.last_frame;
                oldest_index = index;
            }
        }

        let blocking = false;

        let (index, mut frame) = if blocking {
            self.surface.next_frame(&self.frames)
        } else {
            self.image_ready_callbacks.retain(|ir| ir.lock().is_some());
            match sync {
                hal::FrameSync::Semaphore(semaphore) => {
                    self.image_ready_callbacks.push(Arc::clone(&semaphore.image_ready));
                    let mut sw_image = semaphore.image_ready.lock();
                    assert!(sw_image.is_none());
                    *sw_image = Some(SwapchainImage {
                        frames: self.frames.clone(),
                        surface: self.surface.clone(),
                        index: oldest_index as _,
                    });
                }
                hal::FrameSync::Fence(_fence) => {
                    //TODO: need presentation handlers always created and setting a bool
                    unimplemented!()
                }
            }

            let frame = self.frames[oldest_index].inner.lock();
            (oldest_index, frame)
        };

        assert!(frame.available);
        frame.last_frame = self.last_frame;
        frame.available = false;

        Ok(index as _)
    }
}
