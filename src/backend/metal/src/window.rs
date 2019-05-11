use crate::{
    native,
    Backend, QueueFamily,
    device::{Device, PhysicalDevice},
    internal::Channel,
};

use hal::{
    format, image,
    SwapchainConfig, CompositeAlpha,
    window::{Extent2D, Suboptimal},
};

use core_graphics::base::CGFloat;
use core_graphics::geometry::{CGRect, CGSize};
use foreign_types::{ForeignType, ForeignTypeRef};
use metal;
use objc::rc::autoreleasepool;
use objc::runtime::Object;
use parking_lot::{Mutex, MutexGuard};

use std::ptr::NonNull;
use std::sync::Arc;
use std::thread;


//TODO: make it a weak pointer, so that we know which
// frames can be replaced if we receive an unknown
// texture pointer by an acquired drawable.
pub type CAMetalLayer = *mut Object;

/// This is a random ID to be used for signposts associated with swapchain events.
const SIGNPOST_ID: u32 = 0x100;

#[derive(Debug)]
pub struct Surface {
    inner: Arc<SurfaceInner>,
    main_thread_id: thread::ThreadId,
}

#[derive(Debug)]
pub(crate) struct SurfaceInner {
    view: Option<NonNull<Object>>,
    render_layer: Mutex<CAMetalLayer>,
    /// Place start/end signposts for the duration of frames held
    enable_signposts: bool,
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
            enable_signposts: false,
        }
    }

    pub fn into_surface(mut self, enable_signposts: bool) -> Surface {
        self.enable_signposts = enable_signposts;
        Surface {
            inner: Arc::new(self),
            main_thread_id: thread::current().id(),
        }
    }

    fn next_frame<'a>(
        &self,
        frames: &'a [Frame],
    ) -> Result<(usize, MutexGuard<'a, FrameInner>), FrameNotFound> {
        let layer_ref = self.render_layer.lock();
        autoreleasepool(|| {
            // for the drawable
            let _signpost = if self.enable_signposts {
                Some(native::Signpost::new(SIGNPOST_ID, [0, 0, 0, 0]))
            } else {
                None
            };
            let (drawable, texture_temp): (&metal::DrawableRef, &metal::TextureRef) = unsafe {
                let drawable = msg_send![*layer_ref, nextDrawable];
                (drawable, msg_send![drawable, texture])
            };

            trace!("looking for {:?}", texture_temp);
            match frames
                .iter()
                .position(|f| f.texture.as_ptr() == texture_temp.as_ptr())
            {
                Some(index) => {
                    let mut frame = frames[index].inner.lock();
                    assert!(frame.drawable.is_none());
                    frame.iteration += 1;
                    frame.drawable = Some(drawable.to_owned());
                    if self.enable_signposts && false {
                        //Note: could encode the `iteration` here if we need it
                        frame.signpost = Some(native::Signpost::new(SIGNPOST_ID, [1, index as usize, 0, 0]));
                    }

                    debug!("Next is frame[{}]", index);
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
        let (size, scale): (CGSize, CGFloat) = match self.view {
            Some(view) if !cfg!(target_os = "macos") => unsafe {
                let bounds: CGRect = msg_send![view.as_ptr(), bounds];
                let window: Option<NonNull<Object>> = msg_send![view.as_ptr(), window];
                let screen = window.and_then(|window| -> Option<NonNull<Object>> {
                    msg_send![window.as_ptr(), screen]
                });
                match screen {
                    Some(screen) => {
                        let screen_space: *mut Object = msg_send![screen.as_ptr(), coordinateSpace];
                        let rect: CGRect = msg_send![view.as_ptr(), convertRect:bounds toCoordinateSpace:screen_space];
                        let scale_factor: CGFloat = msg_send![screen.as_ptr(), nativeScale];
                        (rect.size, scale_factor)
                    }
                    None => (bounds.size, 1.0),
                }
            },
            _ => unsafe {
                let render_layer = *self.render_layer.lock();
                let bounds: CGRect = msg_send![render_layer, bounds];
                let contents_scale: CGFloat = msg_send![render_layer, contentsScale];
                (bounds.size, contents_scale)
            },
        };
        Extent2D {
            width: (size.width * scale) as u32,
            height: (size.height * scale) as u32,
        }
    }
}

#[derive(Debug)]
struct FrameInner {
    drawable: Option<metal::Drawable>,
    signpost: Option<native::Signpost>,
    /// If there is a `drawable`, availability indicates if it's free for grabs.
    /// If there is `None`, `available == false` means that the frame has already
    /// been acquired and the `drawable` will appear at some point.
    available: bool,
    /// Stays true for as long as the drawable is circulating through the
    /// CAMetalLayer's frame queue.
    linked: bool,
    iteration: usize,
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

#[derive(Debug)]
pub struct Swapchain {
    frames: Arc<Vec<Frame>>,
    surface: Arc<SurfaceInner>,
    extent: Extent2D,
    last_frame: usize,
    pub acquire_mode: AcquireMode,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        info!("dropping Swapchain");
    }
}

impl Swapchain {
    fn clear_drawables(&self) {
        for frame in self.frames.iter() {
            let mut inner = frame.inner.lock();
            inner.drawable = None;
            inner.signpost = None;
        }
    }

    /// Returns the drawable for the specified swapchain image index,
    /// marks the index as free for future use.
    pub(crate) fn take_drawable(&self, index: hal::SwapImageIndex) -> Result<metal::Drawable, ()> {
        let mut frame = self.frames[index as usize].inner.lock();
        assert!(!frame.available && frame.linked);
        frame.signpost = None;

        match frame.drawable.take() {
            Some(drawable) => {
                debug!("Making frame {} available again", index);
                frame.available = true;
                Ok(drawable)
            }
            None => {
                warn!("Failed to get the drawable of frame {}", index);
                frame.linked = false;
                Err(())
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
    /// Returns the associated frame iteration.
    pub fn iteration(&self) -> usize {
        self.frames[self.index as usize].inner.lock().iteration
    }

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
                    debug!("Swapchain image {} is ready after {} frames", self.index, count);
                    break;
                }
                Ok(_) => {
                    count += 1;
                }
                Err(_e) => {
                    warn!("Swapchain drawables are changed, unable to wait for {}", self.index);
                    break;
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
        &self,
        device: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<format::Format>>,
        Vec<hal::PresentMode>,
    ) {
        let current_extent = if self.main_thread_id == thread::current().id() {
            Some(self.inner.dimensions())
        } else {
            warn!("Unable to get the current view dimensions on a non-main thread");
            None
        };

        let device_caps = &device.shared.private_caps;

        let can_set_maximum_drawables_count =
            device_caps.os_is_mac || device_caps.has_version_at_least(11, 2);

        let image_count = if can_set_maximum_drawables_count {
            2..4
        } else {
            // 3 is the default in `CAMetalLayer` documentation
            // iOS 10.3 was tested to use 3 on iphone5s
            3..4
        };

        let caps = hal::SurfaceCapabilities {
            //Note: this is hardcoded in `CAMetalLayer` documentation
            image_count,
            current_extent,
            extents: Extent2D {
                width: 4,
                height: 4,
            }..Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT
                | image::Usage::SAMPLED
                | image::Usage::TRANSFER_SRC
                | image::Usage::TRANSFER_DST,
            composite_alpha: CompositeAlpha::OPAQUE, //TODO
        };

        let formats = vec![
            format::Format::Bgra8Unorm,
            format::Format::Bgra8Srgb,
            format::Format::Rgba16Sfloat,
        ];

        let can_set_display_sync =
            device_caps.os_is_mac && device_caps.has_version_at_least(10, 13);

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
    ) -> (Swapchain, Vec<native::Image>) {
        info!("build_swapchain {:?}", config);
        if let Some(ref sc) = old_swapchain {
            sc.clear_drawables();
        }

        let caps = &self.shared.private_caps;
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
        let drawable_size = CGSize::new(config.extent.width as f64, config.extent.height as f64);

        let cmd_queue = self.shared.queue.lock();

        unsafe {
            let device_raw = self.shared.device.lock().as_ptr();
            msg_send![render_layer, setDevice: device_raw];
            msg_send![render_layer, setPixelFormat: mtl_format];
            msg_send![render_layer, setFramebufferOnly: framebuffer_only];

            // this gets ignored on iOS for certain OS/device combinations (iphone5s iOS 10.3)
            msg_send![render_layer, setMaximumDrawableCount: config.image_count as u64];

            msg_send![render_layer, setDrawableSize: drawable_size];
            if can_set_next_drawable_timeout {
                msg_send![render_layer, setAllowsNextDrawableTimeout:false];
            }
            if can_set_display_sync {
                msg_send![render_layer, setDisplaySyncEnabled: display_sync];
            }
        };

        let frames = (0..config.image_count)
            .map(|index| {
                autoreleasepool(|| {
                    // for the drawable & texture
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
                                    &self.shared.private_caps,
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
                            signpost: if index != 0 && surface.inner.enable_signposts {
                                Some(native::Signpost::new(SIGNPOST_ID, [1, index as usize, 0, 0]))
                            } else {
                                None
                            },
                            available: true,
                            linked: true,
                            iteration: 0,
                            last_frame: 0,
                        }),
                        texture: texture.to_owned(),
                    }
                })
            })
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
            acquire_mode: AcquireMode::Oldest,
        };

        (swapchain, images)
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
        semaphore: Option<&native::Semaphore>,
        fence: Option<&native::Fence>,
    ) -> Result<(hal::SwapImageIndex, Option<Suboptimal>), hal::AcquireError> {
        self.last_frame += 1;

        //TODO: figure out a proper story of HiDPI
        if false && self.surface.dimensions() != self.extent {
            // mark the method as used
            native::Signpost::place(SIGNPOST_ID, [0, 0, 0, 0]);
            unimplemented!()
        }

        let mut oldest_index = self.frames.len();
        let mut oldest_frame = self.last_frame;

        for (index, frame_arc) in self.frames.iter().enumerate() {
            let mut frame = frame_arc.inner.lock();
            if !frame.available {
                continue;
            }
            if frame.drawable.is_some() {
                debug!("Found drawable of frame {}, acquiring", index);
                frame.available = false;
                frame.last_frame = self.last_frame;
                if self.surface.enable_signposts {
                    //Note: could encode the `iteration` here if we need it
                    frame.signpost = Some(native::Signpost::new(SIGNPOST_ID, [1, index, 0, 0]));
                }
                if let Some(semaphore) = semaphore {
                    if let Some(ref system) = semaphore.system {
                        system.signal();
                    }
                }
                if let Some(fence) = fence {
                    fence.0.replace(native::FenceInner::Idle { signaled: true });
                }
                return Ok((index as _, None));
            }
            if frame.last_frame < oldest_frame {
                oldest_frame = frame.last_frame;
                oldest_index = index;
            }
        }

        let (index, mut frame) = match self.acquire_mode {
            AcquireMode::Wait => {
                let pair = self
                    .surface
                    .next_frame(&self.frames)
                    .map_err(|_| hal::AcquireError::OutOfDate)?;

                if let Some(fence) = fence {
                    fence.0.replace(native::FenceInner::Idle { signaled: true });
                }
                pair
            },
            AcquireMode::Oldest => {
                let frame = match self.frames.get(oldest_index) {
                    Some(frame) => frame.inner.lock(),
                    None => {
                        warn!("No frame is available");
                        return Err(hal::AcquireError::OutOfDate);
                    }
                };
                if !frame.linked {
                    return Err(hal::AcquireError::OutOfDate);
                }

                if let Some(semaphore) = semaphore {
                    let mut sw_image = semaphore.image_ready.lock();
                    if let Some(ref swi) = *sw_image {
                        warn!("frame {} hasn't been waited upon", swi.index);
                    }
                    *sw_image = Some(SwapchainImage {
                        frames: Arc::clone(&self.frames),
                        surface: Arc::clone(&self.surface),
                        index: oldest_index as _,
                    });
                }
                if let Some(fence) = fence {
                    fence.0.replace(native::FenceInner::AcquireFrame {
                        swapchain_image: SwapchainImage {
                            frames: Arc::clone(&self.frames),
                            surface: Arc::clone(&self.surface),
                            index: oldest_index as _,
                        },
                        iteration: frame.iteration,
                    });
                }

                (oldest_index, frame)
            }
        };

        debug!("Acquiring frame {}", index);
        assert!(frame.available);
        frame.last_frame = self.last_frame;
        frame.available = false;
        if self.surface.enable_signposts {
            //Note: could encode the `iteration` here if we need it
            frame.signpost = Some(native::Signpost::new(SIGNPOST_ID, [1, index, 0, 0]));
        }

        Ok((index as _, None))
    }
}
