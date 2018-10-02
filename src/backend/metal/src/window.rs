use {Backend, QueueFamily};
use device::{Device, PhysicalDevice};
use internal::Channel;
use native;

use std::ptr::NonNull;
use std::sync::Arc;
use std::thread;

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use core_graphics::geometry::{CGRect, CGSize};
use foreign_types::{ForeignType, ForeignTypeRef};
use parking_lot::{Mutex, MutexGuard};
use metal;
use objc::rc::autoreleasepool;
use objc::runtime::Object;


//TODO: make it a weak pointer, so that we know which
// frames can be replaced if we receive an unknown
// texture pointer by an acquired drawable.
pub type CAMetalLayer = *mut Object;

pub struct Surface {
    inner: Arc<SurfaceInner>,
    main_thread_id: thread::ThreadId,
}

#[derive(Debug)]
pub struct SurfaceInner {
    view: Option<NonNull<Object>>,
    render_layer: Mutex<CAMetalLayer>,
}

unsafe impl Send for SurfaceInner {}
unsafe impl Sync for SurfaceInner {}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        let object = match self.view {
            Some(view) => view.as_ptr(),
            None => *self.render_layer.lock(),
        };
        unsafe {
            msg_send![object, release];
        }
    }
}

#[derive(Debug)]
struct FrameNotFound {
    drawable: metal::Drawable,
    texture: metal::Texture,
}


impl SurfaceInner {
    pub fn new(view: Option<NonNull<Object>>, layer: CAMetalLayer) -> Self {
        SurfaceInner {
            view,
            render_layer: Mutex::new(layer),
        }
    }

    pub fn into_surface(self) -> Surface {
        Surface {
            inner: Arc::new(self),
            main_thread_id: thread::current().id(),
        }
    }

    fn next_frame<'a>(&self, frames: &'a [Frame]) -> Result<(usize, MutexGuard<'a, FrameInner>), FrameNotFound> {
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
                None => Err(FrameNotFound {
                    drawable: drawable.to_owned(),
                    texture: texture_temp.to_owned(),
                }),
            }
        })
    }

    fn dimensions(&self) -> Extent2D {
        let size = match self.view {
            Some(view) => unsafe {
                let bounds: CGRect = msg_send![view.as_ptr(), bounds];
                bounds.size
            },
            None => unsafe {
                msg_send![*self.render_layer.lock(), drawableSize]
            },
        };
        Extent2D {
            width: size.width as _,
            height: size.height as _,
        }
    }
}


#[derive(Debug)]
struct FrameInner {
    drawable: Option<metal::Drawable>,
    /// If there is a `drawable`, availability indicates if it's free for grabs.
    /// If there is `None`, `available == false` means that the frame has already
    /// been acquired and the `drawable` will appear at some point.
    available: bool,
    /// Stays true for as long as the drawable is circulating through the
    /// CAMetalLayer's frame queue.
    linked: bool,
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
    pub(crate) fn take_drawable(&self, index: hal::SwapImageIndex) -> Result<metal::Drawable, ()> {
        let mut frame = self
            .frames[index as usize]
            .inner
            .lock();
        assert!(!frame.available && frame.linked);

        match frame.drawable.take() {
            Some(drawable) => {
                frame.available = true;
                Ok(drawable)
            }
            None => {
                frame.linked = false;
                Err(())
            }
        }
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
        loop {
            match self.surface.next_frame(&self.frames) {
                Ok((index, _)) if index == self.index as usize => {
                    debug!("Swapchain image is ready after {} frames", count);
                    break
                }
                Ok(_) => {
                    count += 1;
                }
                Err(_e) => {
                    debug!("Swapchain drawables are changed");
                    break
                }
            }
        }        
        count
    }
}


impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let ex = self.inner.dimensions();
        image::Kind::D2(ex.width, ex.height, 1, 1)
    }

    fn compatibility(
        &self, device: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>, Vec<hal::PresentMode>) {
        let current_extent = if self.main_thread_id == thread::current().id() {
            Some(self.inner.dimensions())
        } else {
            warn!("Unable to get the current view dimensions on a non-main thread");
            None
        };

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

        let device_caps = &device.private_caps;
        let can_set_display_sync = device_caps.os_is_mac && device_caps.has_version_at_least(10, 13);

        let present_modes = if can_set_display_sync {
            vec![hal::PresentMode::Fifo, hal::PresentMode::Immediate]
        } else {
            vec![hal::PresentMode::Fifo]
        };

        (caps, Some(formats), present_modes)
    }

    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool {
        // we only expose one family atm, so it's compatible
        true
    }
}

impl Device {
    pub(crate) fn build_swapchain(
        &self,
        surface: &mut Surface,
        config: SwapchainConfig,
        old_swapchain: Option<Swapchain>,
    ) -> (Swapchain, Backbuffer<Backend>) {
        info!("build_swapchain {:?}", config);

        let caps = &self.private_caps;
        let mtl_format = caps
            .map_format(config.format)
            .expect("unsupported backbuffer format");

        let render_layer_borrow = surface.inner.render_layer.lock();
        let render_layer = *render_layer_borrow;
        let format_desc = config.format.surface_desc();
        let framebuffer_only = config.image_usage == image::Usage::COLOR_ATTACHMENT;
        let display_sync = config.present_mode != hal::PresentMode::Immediate;
        let is_mac = caps.os_is_mac;
        let can_set_next_drawable_timeout = if is_mac {
            caps.has_version_at_least(10, 13)
        } else {
            caps.has_version_at_least(11, 0)
        };
        let can_set_display_sync = is_mac && caps.has_version_at_least(10, 13);

        let cmd_queue = self.shared.queue.lock();

        unsafe {
            let device_raw = self.shared.device.lock().as_ptr();
            msg_send![render_layer, setDevice: device_raw];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];
            msg_send![render_layer, setMaximumDrawableCount: config.image_count as u64];
            msg_send![render_layer, setDrawableSize: CGSize::new(config.extent.width as f64, config.extent.height as f64)];
            if can_set_next_drawable_timeout {
                msg_send![render_layer, setAllowsNextDrawableTimeout:false];
            }
            if can_set_display_sync {
                msg_send![render_layer, setDisplaySyncEnabled: display_sync];
            }
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

                let drawable = if index == 0 {
                    // when resizing, this trick frees up the currently shown frame
                    match old_swapchain {
                        Some(ref old) => {
                            let cmd_buffer = cmd_queue.spawn_temp();
                            self.shared.service_pipes.simple_blit(
                                &self.shared.device,
                                cmd_buffer,
                                &old.frames[0].texture,
                                texture,
                            );
                            cmd_buffer.present_drawable(drawable);
                            cmd_buffer.set_label("build_swapchain");
                            cmd_buffer.commit();
                            cmd_buffer.wait_until_completed();
                        }
                        None => {
                            // this will look as a black frame
                            drawable.present();
                        }
                    }
                    None
                } else {
                    Some(drawable.to_owned())
                };
                Frame {
                    inner: Mutex::new(FrameInner {
                        drawable,
                        available: true,
                        linked: true,
                        last_frame: 0,
                    }),
                    texture: texture.to_owned(),
                }
            }))
            .collect::<Vec<_>>();

        let images = frames
            .iter()
            .map(|frame| native::Image {
                like: native::ImageLike::Texture(frame.texture.clone()),
                kind: image::Kind::D2(config.extent.width, config.extent.height, 1, 1),
                format_desc,
                shader_channel: Channel::Float,
                mtl_format,
                mtl_type: metal::MTLTextureType::D2,
            })
            .collect();

        let swapchain = Swapchain {
            frames: Arc::new(frames),
            surface: surface.inner.clone(),
            extent: config.extent,
            last_frame: 0,
            image_ready_callbacks: Vec::new(),
            acquire_mode: AcquireMode::Oldest,
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
                        if let Some(ref swi) = *sw_image {
                            warn!("frame {} hasn't been waited upon", swi.index);
                        }
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
                if !frame.linked {
                    return Err(hal::AcquireError::OutOfDate);
                }
                (oldest_index, frame)
            }
        };

        assert!(frame.available);
        frame.last_frame = self.last_frame;
        frame.available = false;

        Ok(index as _)
    }
}
