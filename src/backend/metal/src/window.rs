use {Backend, QueueFamily};
use device::{Device, PhysicalDevice};
use internal::Channel;
use native;

use std::sync::Arc;

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use core_graphics::geometry::{CGRect, CGSize};
use foreign_types::{ForeignType, ForeignTypeRef};
use parking_lot::{Mutex, MutexGuard};
use metal;
use objc::rc::autoreleasepool;
use objc::runtime::Object;


pub type CAMetalLayer = *mut Object;

pub struct Surface {
    pub(crate) inner: Arc<SurfaceInner>,
    pub(crate) has_swapchain: bool
}

#[derive(Debug)]
pub(crate) struct SurfaceInner {
    pub(crate) view: *mut Object,
    pub(crate) render_layer: Mutex<CAMetalLayer>,
}

unsafe impl Send for SurfaceInner {}
unsafe impl Sync for SurfaceInner {}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.view, release]; }
    }
}

impl SurfaceInner {
    fn next_frame<'a>(&self, frames: &'a [Frame]) -> Result<(usize, MutexGuard<'a, FrameInner>), ()> {
        let layer_ref = self.render_layer.lock();
        autoreleasepool(|| { // for the drawable
            let (drawable, texture_temp): (&metal::DrawableRef, &metal::TextureRef) = unsafe {
                let drawable = msg_send![*layer_ref, nextDrawable];
                (drawable, msg_send![drawable, texture])
            };

            trace!("looking for {:?}", texture_temp);
            match frames.iter().position(|f| f.texture.as_ptr() == texture_temp.as_ptr()) {
                Some(index) => {
                    let mut frame = frames[index].inner.lock();
                    assert!(frame.drawable.is_none());
                    frame.drawable = Some(drawable.to_owned());

                    debug!("next is frame[{}]", index);
                    Ok((index, frame))
                }
                None => Err(()),
            }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcquireMode {
    Wait,
    Oldest,
}

pub struct Swapchain {
    frames: Arc<Vec<Frame>>,
    surface: Arc<SurfaceInner>,
    extent: Extent2D,
    last_frame: usize,
    image_ready_callbacks: Vec<Arc<Mutex<Option<SwapchainImage>>>>,
    pub acquire_mode: AcquireMode,
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
        while self.surface.next_frame(&self.frames).unwrap().0 != self.index as usize {
            count += 1;
        }
        debug!("Swapchain image is ready after {} frames", count);
        count
    }
}


impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let ex = self.inner.dimensions();
        image::Kind::D2(ex.width, ex.height, 1, 1)
    }

    fn compatibility(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>, Vec<hal::PresentMode>) {
        let current_extent = Some(self.inner.dimensions());

        let caps = hal::SurfaceCapabilities {
            //Note: this is hardcoded in `CAMetalLayer` documentation
            image_count: 2 .. 4,
            current_extent,
            extents: Extent2D { width: 4, height: 4} .. Extent2D { width: 4096, height: 4096 },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::SAMPLED |
                image::Usage::TRANSFER_SRC | image::Usage::TRANSFER_DST,
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
    fn dimensions(&self) -> Extent2D {
        unsafe {
            // NSView/UIView bounds are measured in DIPs
            let bounds: CGRect = msg_send![self.view, bounds];
            //let bounds_pixel: NSRect = msg_send![self.nsview, convertRectToBacking:bounds];
            Extent2D {
                width: bounds.size.width as _,
                height: bounds.size.height as _,
            }
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
            .map_format(config.format)
            .expect("unsupported backbuffer format");

        let render_layer_borrow = surface.inner.render_layer.lock();
        let render_layer = *render_layer_borrow;
        let format_desc = config.format.surface_desc();
        let framebuffer_only = config.image_usage == image::Usage::COLOR_ATTACHMENT;
        let display_sync = match config.present_mode {
            hal::PresentMode::Immediate => false,
            _ => true,
        };
        let device = self.shared.device.lock();
        let device_raw: &metal::DeviceRef = &*device;

        unsafe {
            msg_send![render_layer, setDevice: device_raw];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];
            msg_send![render_layer, setMaximumDrawableCount: config.image_count as u64];
            msg_send![render_layer, setDrawableSize: CGSize::new(config.extent.width as f64, config.extent.height as f64)];
            //TODO: only set it where supported
            msg_send![render_layer, setAllowsNextDrawableTimeout:false];
            msg_send![render_layer, setDisplaySyncEnabled: display_sync];
        };

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
                kind: image::Kind::D2(config.extent.width, config.extent.height, 1, 1),
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
            extent: config.extent,
            last_frame: 0,
            image_ready_callbacks: Vec::new(),
            acquire_mode: AcquireMode::Wait,
        };


        (swapchain, Backbuffer::Images(images))
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_image(
        &mut self, _timeout_ns: u64, sync: hal::FrameSync<Backend>
    ) -> Result<hal::SwapImageIndex, hal::AcquireError> {
        self.last_frame += 1;

        //TODO: figure out a proper story of HiDPI
        if false && self.surface.dimensions() != self.extent {
            unimplemented!()
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

        let (index, mut frame) = match self.acquire_mode {
            AcquireMode::Wait => {
                self.surface.next_frame(&self.frames)
                    .map_err(|_| hal::AcquireError::OutOfDate)?
            }
            AcquireMode::Oldest => {
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
            }
        };

        assert!(frame.available);
        frame.last_frame = self.last_frame;
        frame.available = false;

        Ok(index as _)
    }
}
