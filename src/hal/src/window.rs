//! Windowing system interoperability
//!
//! Screen presentation (fullscreen or window) of images requires two objects:
//!
//! * [`Surface`] is the host abstraction of the native screen
//! * [`Swapchain`] is the device abstraction for a surface, containing multiple presentable images
//!
//! ## Window
//!
//! // DOC TODO
//!
//! ## Surface
//!
//! // DOC TODO
//!
//! ## Swapchain
//!
//! The most interesting part of a swapchain are the contained presentable images/backbuffers.
//! Presentable images are specialized images, which can be presented on the screen. They are
//! 2D color images with optionally associated depth-stencil images.
//!
//! The common steps for presentation of a frame are acquisition and presentation:
//!
//! ```no_run
//! # extern crate gfx_backend_empty as empty;
//! # extern crate gfx_hal;
//! # fn main() {
//! # use gfx_hal::prelude::*;
//!
//! # let mut swapchain: empty::Swapchain = return;
//! # let device: empty::Device = return;
//! # let mut present_queue: empty::CommandQueue = return;
//! # unsafe {
//! let acquisition_semaphore = device.create_semaphore().unwrap();
//! let render_semaphore = device.create_semaphore().unwrap();
//!
//! let (frame, suboptimal) = swapchain.acquire_image(!0, Some(&acquisition_semaphore), None).unwrap();
//! // render the scene..
//! // `render_semaphore` will be signalled once rendering has been finished
//! swapchain.present(&mut present_queue, 0, &[render_semaphore]);
//! # }}
//! ```
//!
//! Queues need to synchronize with the presentation engine, usually done via signalling a semaphore
//! once a frame is available for rendering and waiting on a separate semaphore until scene rendering
//! has finished.
//!
//! ### Recreation
//!
//! DOC TODO

use crate::device;
use crate::format::Format;
use crate::image;
use crate::queue::CommandQueue;
use crate::Backend;

use std::any::Any;
use std::borrow::Borrow;
use std::cmp::{max, min};
use std::fmt;
use std::iter;
use std::ops::RangeInclusive;

/// Error occurred during swapchain creation.
#[derive(Clone, Copy, Debug, Fail, PartialEq, Eq)]
pub enum CreationError {
    /// Out of either host or device memory.
    #[fail(display = "{}", _0)]
    OutOfMemory(device::OutOfMemory),
    /// Device is lost
    #[fail(display = "{}", _0)]
    DeviceLost(device::DeviceLost),
    /// Surface is lost
    #[fail(display = "{}", _0)]
    SurfaceLost(device::SurfaceLost),
    /// Window in use
    #[fail(display = "{}", _0)]
    WindowInUse(device::WindowInUse),
}

impl From<device::OutOfMemory> for CreationError {
    fn from(error: device::OutOfMemory) -> Self {
        CreationError::OutOfMemory(error)
    }
}

impl From<device::DeviceLost> for CreationError {
    fn from(error: device::DeviceLost) -> Self {
        CreationError::DeviceLost(error)
    }
}

impl From<device::SurfaceLost> for CreationError {
    fn from(error: device::SurfaceLost) -> Self {
        CreationError::SurfaceLost(error)
    }
}

impl From<device::WindowInUse> for CreationError {
    fn from(error: device::WindowInUse) -> Self {
        CreationError::WindowInUse(error)
    }
}

/// An extent describes the size of a rectangle, such as
/// a window or texture. It is not used for referring to a
/// sub-rectangle; for that see `command::Rect`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Extent2D {
    /// Width
    pub width: image::Size,
    /// Height
    pub height: image::Size,
}

impl From<image::Extent> for Extent2D {
    fn from(ex: image::Extent) -> Self {
        Extent2D {
            width: ex.width,
            height: ex.height,
        }
    }
}

impl Extent2D {
    /// Convert into a regular image extent.
    pub fn to_extent(&self) -> image::Extent {
        image::Extent {
            width: self.width,
            height: self.height,
            depth: 1,
        }
    }
}

/// Describes information about what a `Surface`'s properties are.
/// Fetch this with `surface.compatibility(device)`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SurfaceCapabilities {
    /// Number of presentable images supported by the adapter for a swapchain
    /// created from this surface.
    ///
    /// - `image_count.start` must be at least 1.
    /// - `image_count.end` must be larger or equal to `image_count.start`.
    pub image_count: RangeInclusive<SwapImageIndex>,

    /// Current extent of the surface.
    ///
    /// `None` if the surface has no explicit size, depending on the swapchain extent.
    pub current_extent: Option<Extent2D>,

    /// Range of supported extents.
    ///
    /// `current_extent` must be inside this range.
    pub extents: RangeInclusive<Extent2D>,

    /// Maximum number of layers supported for presentable images.
    ///
    /// Must be at least 1.
    pub max_image_layers: image::Layer,

    /// Supported image usage flags.
    pub usage: image::Usage,

    /// A bitmask of supported alpha composition modes.
    pub composite_alpha: CompositeAlpha,
}

impl SurfaceCapabilities {
    fn clamped_extent(&self, default_extent: Extent2D) -> Extent2D {
        match self.current_extent {
            Some(current) => current,
            None => {
                let (min_width, max_width) = (self.extents.start().width, self.extents.end().width);
                let (min_height, max_height) =
                    (self.extents.start().height, self.extents.end().height);

                // clamp the default_extent to within the allowed surface sizes
                let width = min(max_width, max(default_extent.width, min_width));
                let height = min(max_height, max(default_extent.height, min_height));

                Extent2D { width, height }
            }
        }
    }
}

/// A `Surface` abstracts the surface of a native window.
pub trait Surface<B: Backend>: fmt::Debug + Any + Send + Sync {
    /// Check if the queue family supports presentation to this surface.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn supports_queue_family(&self, family: &B::QueueFamily) -> bool;

    /// Query surface capabilities, formats, and present modes for this physical device.
    ///
    /// Use this function for configuring swapchain creation.
    ///
    /// Returns a tuple of surface capabilities and formats.
    /// If formats are `None` then the surface has no preferred format and the
    /// application may use any desired format.
    fn compatibility(
        &self,
        physical_device: &B::PhysicalDevice,
    ) -> (SurfaceCapabilities, Option<Vec<Format>>, Vec<PresentMode>);
}

/// A surface trait that exposes the ability to present images on the
/// associtated swap chain.
pub trait PresentationSurface<B: Backend>: Surface<B> {
    /// An opaque type wrapping the swapchain image.
    type SwapchainImage: Borrow<B::ImageView> + fmt::Debug + Send + Sync;

    /// Set up the swapchain associated with the surface to have the given format.
    unsafe fn configure_swapchain(
        &mut self,
        device: &B::Device,
        config: SwapchainConfig,
    ) -> Result<(), CreationError>;

    /// Remove the associated swapchain from this surface.
    ///
    /// This has to be done before the surface is dropped.
    unsafe fn unconfigure_swapchain(&mut self, device: &B::Device);

    /// Acquire a new swapchain image for rendering.
    ///
    /// May fail according to one of the reasons indicated in `AcquireError` enum.
    ///
    /// # Synchronization
    ///
    /// The acquired image is available to render. No synchronization is required.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    unsafe fn acquire_image(
        &mut self,
        timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<Suboptimal>), AcquireError>;
}

/// Index of an image in the swapchain.
///
/// The swapchain is a series of one or more images, usually
/// with one being drawn on while the other is displayed by
/// the GPU (aka double-buffering). A `SwapImageIndex` refers
/// to a particular image in the swapchain.
pub type SwapImageIndex = u32;

/// Specifies the mode regulating how a swapchain presents frames.
#[repr(C)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum PresentMode {
    /// Don't ever wait for v-sync.
    Immediate = 0,
    /// Wait for v-sync, overwrite the last rendered frame.
    Mailbox = 1,
    /// Present frames in the same order they are rendered.
    Fifo = 2,
    /// Don't wait for the next v-sync if we just missed it.
    Relaxed = 3,
}

bitflags!(
    /// Specifies how the alpha channel of the images should be handled during
    /// compositing.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct CompositeAlpha: u32 {
        /// The alpha channel, if it exists, of the images is ignored in the
        /// compositing process. Instead, the image is treated as if it has a
        /// constant alpha of 1.0.
        const OPAQUE = 0x1;
        /// The alpha channel, if it exists, of the images is respected in the
        /// compositing process. The non-alpha channels of the image are
        /// expected to already be multiplied by the alpha channel by the
        /// application.
        const PREMULTIPLIED = 0x2;
        /// The alpha channel, if it exists, of the images is respected in the
        /// compositing process. The non-alpha channels of the image are not
        /// expected to already be multiplied by the alpha channel by the
        /// application; instead, the compositor will multiply the non-alpha
        /// channels of the image by the alpha channel during compositing.
        const POSTMULTIPLIED = 0x4;
        /// The way in which the presentation engine treats the alpha channel in
        /// the images is unknown to gfx-hal. Instead, the application is
        /// responsible for setting the composite alpha blending mode using
        /// native window system commands. If the application does not set the
        /// blending mode using native window system commands, then a
        /// platform-specific default will be used.
        const INHERIT = 0x8;
    }
);

/// Contains all the data necessary to create a new `Swapchain`:
/// color, depth, and number of images.
///
/// # Examples
///
/// This type implements the builder pattern, method calls can be
/// easily chained.
///
/// ```no_run
/// # extern crate gfx_hal;
/// # fn main() {
/// # use gfx_hal::window::SwapchainConfig;
/// # use gfx_hal::format::Format;
/// let config = SwapchainConfig::new(100, 100, Format::Bgra8Unorm, 2);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    /// Presentation mode.
    pub present_mode: PresentMode,
    /// Alpha composition mode.
    pub composite_alpha: CompositeAlpha,
    /// Format of the backbuffer images.
    pub format: Format,
    /// Requested image extent. Must be in
    /// `SurfaceCapabilities::extents` range.
    pub extent: Extent2D,
    /// Number of images in the swapchain. Must be in
    /// `SurfaceCapabilities::image_count` range.
    pub image_count: SwapImageIndex,
    /// Number of image layers. Must be lower or equal to
    /// `SurfaceCapabilities::max_image_layers`.
    pub image_layers: image::Layer,
    /// Image usage of the backbuffer images.
    pub image_usage: image::Usage,
}

impl SwapchainConfig {
    /// Create a new default configuration (color images only).
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn new(width: u32, height: u32, format: Format, image_count: SwapImageIndex) -> Self {
        SwapchainConfig {
            present_mode: PresentMode::Fifo,
            composite_alpha: CompositeAlpha::OPAQUE,
            format,
            extent: Extent2D { width, height },
            image_count,
            image_layers: 1,
            image_usage: image::Usage::COLOR_ATTACHMENT,
        }
    }

    /// Create a swapchain configuration based on the capabilities
    /// returned from a physical device query. If the surface does not
    /// specify a current size, default_extent is clamped and used instead.
    pub fn from_caps(caps: &SurfaceCapabilities, format: Format, default_extent: Extent2D) -> Self {
        let composite_alpha = if caps.composite_alpha.contains(CompositeAlpha::INHERIT) {
            CompositeAlpha::INHERIT
        } else if caps.composite_alpha.contains(CompositeAlpha::OPAQUE) {
            CompositeAlpha::OPAQUE
        } else {
            unreachable!("neither INHERIT or OPAQUE CompositeAlpha modes are supported")
        };

        SwapchainConfig {
            present_mode: PresentMode::Fifo,
            composite_alpha,
            format,
            extent: caps.clamped_extent(default_extent),
            image_count: *caps.image_count.start(),
            image_layers: 1,
            image_usage: image::Usage::COLOR_ATTACHMENT,
        }
    }

    /// Specify the presentation mode.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn with_mode(mut self, mode: PresentMode) -> Self {
        self.present_mode = mode;
        self
    }

    /// Specify the usage of backbuffer images.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn with_image_usage(mut self, usage: image::Usage) -> Self {
        self.image_usage = usage;
        self
    }

    // TODO: depth-only, stencil-only, swapchain size, present modes, etc.
}

/// Marker value returned if the swapchain no longer matches the surface properties exactly,
/// but can still be used to present to the surface successfully.
#[derive(Debug)]
pub struct Suboptimal;

/// Error on acquiring the next image from a swapchain.
#[derive(Clone, Copy, Debug, Fail, PartialEq, Eq)]
pub enum AcquireError {
    /// Out of either host or device memory.
    #[fail(display = "{}", _0)]
    OutOfMemory(device::OutOfMemory),
    /// No image was ready and no timeout was specified.
    #[fail(display = "No images ready")]
    NotReady,
    /// No image was ready after the specified timeout expired.
    #[fail(display = "No images ready after the specified timeout expired")]
    Timeout,
    /// The swapchain is no longer in sync with the surface, needs to be re-created.
    #[fail(display = "Swapchain is out of date")]
    OutOfDate,
    /// The surface was lost, and the swapchain is no longer usable.
    #[fail(display = "{}", _0)]
    SurfaceLost(device::SurfaceLost),
    /// Device is lost
    #[fail(display = "{}", _0)]
    DeviceLost(device::DeviceLost),
}

/// Error on acquiring the next image from a swapchain.
#[derive(Clone, Copy, Debug, Fail, PartialEq, Eq)]
pub enum PresentError {
    /// Out of either host or device memory.
    #[fail(display = "{}", _0)]
    OutOfMemory(device::OutOfMemory),
    /// The swapchain is no longer in sync with the surface, needs to be re-created.
    #[fail(display = "Swapchain is out of date")]
    OutOfDate,
    /// The surface was lost, and the swapchain is no longer usable.
    #[fail(display = "{}", _0)]
    SurfaceLost(device::SurfaceLost),
    /// Device is lost
    #[fail(display = "{}", _0)]
    DeviceLost(device::DeviceLost),
}

/// The `Swapchain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait Swapchain<B: Backend>: fmt::Debug + Any + Send + Sync {
    /// Acquire a new swapchain image for rendering. This needs to be called before presenting.
    ///
    /// May fail according to one of the reasons indicated in `AcquireError` enum.
    ///
    /// # Synchronization
    ///
    /// The acquired image will not be immediately available when the function returns.
    /// Once available the provided [`Semaphore`](../trait.Resources.html#associatedtype.Semaphore)
    /// and [`Fence`](../trait.Resources.html#associatedtype.Fence) will be signaled.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    unsafe fn acquire_image(
        &mut self,
        timeout_ns: u64,
        semaphore: Option<&B::Semaphore>,
        fence: Option<&B::Fence>,
    ) -> Result<(SwapImageIndex, Option<Suboptimal>), AcquireError>;

    /// Present one acquired image.
    ///
    /// # Safety
    ///
    /// The passed queue _must_ support presentation on the surface, which is
    /// used for creating this swapchain.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    unsafe fn present<'a, S, Iw>(
        &'a self,
        present_queue: &mut B::CommandQueue,
        image_index: SwapImageIndex,
        wait_semaphores: Iw,
    ) -> Result<Option<Suboptimal>, PresentError>
    where
        Self: 'a + Sized + Borrow<B::Swapchain>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        present_queue.present(iter::once((self, image_index)), wait_semaphores)
    }

    /// Present one acquired image without any semaphore synchronization.
    unsafe fn present_without_semaphores<'a>(
        &'a self,
        present_queue: &mut B::CommandQueue,
        image_index: SwapImageIndex,
    ) -> Result<Option<Suboptimal>, PresentError>
    where
        Self: 'a + Sized + Borrow<B::Swapchain>,
    {
        self.present::<B::Semaphore, _>(present_queue, image_index, iter::empty())
    }
}

/// Error occurred during surface creation.
#[derive(Clone, Copy, Debug, Fail, PartialEq, Eq)]
pub enum InitError {
    /// Window handle is not supported by the backend.
    #[fail(display = "Backend does not support creating surfaces for this type of window handle")]
    UnsupportedWindowHandle,
}
