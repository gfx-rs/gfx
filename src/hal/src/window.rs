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
//! use gfx_hal::{Device, FrameSync};
//! # use gfx_hal::{CommandQueue, Graphics, Swapchain};
//!
//! # let mut swapchain: empty::Swapchain = return;
//! # let device: empty::Device = return;
//! # let mut present_queue: CommandQueue<empty::Backend, Graphics> = return;
//! let acquisition_semaphore = device.create_semaphore();
//! let render_semaphore = device.create_semaphore();
//!
//! let frame = swapchain.acquire_image(!0, FrameSync::Semaphore(&acquisition_semaphore));
//! // render the scene..
//! // `render_semaphore` will be signalled once rendering has been finished
//! swapchain.present(&mut present_queue, 0, &[render_semaphore]);
//! # }
//! ```
//!
//! Queues need to synchronize with the presentation engine, usually done via signalling a semaphore
//! once a frame is available for rendering and waiting on a separate semaphore until scene rendering
//! has finished.
//!
//! ### Recreation
//!
//! DOC TODO

use Backend;
use image;
use format::Format;
use queue::CommandQueue;

use std::any::Any;
use std::borrow::Borrow;
use std::ops::Range;


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
/// Fetch this with `surface.capabilities_and_formats(device)`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SurfaceCapabilities {
    /// Number of presentable images supported by the adapter for a swapchain
    /// created from this surface.
    ///
    /// - `image_count.start` must be at least 1.
    /// - `image_count.end` must be larger of equal to `image_count.start`.
    pub image_count: Range<SwapImageIndex>,

    /// Current extent of the surface.
    ///
    /// `None` if the surface has no explicit size, depending on the swapchain extent.
    pub current_extent: Option<Extent2D>,

    /// Range of supported extents.
    ///
    /// `current_extent` must be inside this range.
    pub extents: Range<Extent2D>,

    /// Maximum number of layers supported for presentable images.
    ///
    /// Must be at least 1.
    pub max_image_layers: image::Layer,

    /// Supported image usage flags.
    pub usage: image::Usage,
}

/// A `Surface` abstracts the surface of a native window, which will be presented
/// on the display.
pub trait Surface<B: Backend>: Any + Send + Sync {
    /// Retrieve the surface image kind.
    fn kind(&self) -> image::Kind;

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
    /// If formats is `None` than the surface has no preferred format and the
    /// application may use any desired format.
    fn compatibility(
        &self, physical_device: &B::PhysicalDevice
    ) -> (SurfaceCapabilities, Option<Vec<Format>>, Vec<PresentMode>);
}

/// Index of an image in the swapchain.
///
/// The swapchain is a series of one or more images, usually
/// with one being drawn on while the other is displayed by
/// the GPU (aka double-buffering). A `SwapImageIndex` refers
/// to a particular image in the swapchain.
pub type SwapImageIndex = u32;

/// Synchronization primitives which will be signalled once a frame got retrieved.
///
/// The semaphore or fence _must_ be unsignalled.
pub enum FrameSync<'a, B: Backend> {
    /// Semaphore used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Semaphore(&'a B::Semaphore),

    /// Fence used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Fence(&'a B::Fence),
}

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
/// # use gfx_hal::{SwapchainConfig};
/// # use gfx_hal::format::Format;
/// let config = SwapchainConfig::new(100, 100, Format::Bgra8Unorm, 2);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    /// Presentation mode.
    pub present_mode: PresentMode,
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
            format,
            extent: Extent2D { width, height },
            image_count,
            image_layers: 1,
            image_usage: image::Usage::COLOR_ATTACHMENT,
        }
    }

    /// Create a swapchain configuration based on the capabilities
    /// returned from a physical device query.
    pub fn from_caps(caps: &SurfaceCapabilities, format: Format) -> Self {
        SwapchainConfig {
            present_mode: PresentMode::Fifo,
            format,
            extent: caps.current_extent.unwrap_or(caps.extents.start),
            image_count: caps.image_count.start,
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

/// Swapchain backbuffer type
#[derive(Debug)]
pub enum Backbuffer<B: Backend> {
    /// Color image chain
    Images(Vec<B::Image>),
    /// A single opaque framebuffer
    Framebuffer(B::Framebuffer),
}


/// Error on acquiring the next image from a swapchain.
#[derive(Debug)]
pub enum AcquireError {
    /// No image was ready after the specified timeout expired.
    NotReady,
    /// The swapchain is no longer in sync with the surface, needs to be re-created.
    OutOfDate,
    /// The surface was lost, and the swapchain is no longer usable.
    SurfaceLost,
}

/// The `Swapchain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait Swapchain<B: Backend>: Any + Send + Sync {
    /// Acquire a new swapchain image for rendering. This needs to be called before presenting.
    ///
    /// May fail according to one of the reasons indicated in `AcquireError` enum.
    ///
    /// # Synchronization
    ///
    /// The acquired image will not be immediately available when the function returns.
    /// Once available the underlying primitive of `sync` will be signaled.
    /// This can either be a [`Semaphore`](../trait.Resources.html#associatedtype.Semaphore)
    /// or a [`Fence`](../trait.Resources.html#associatedtype.Fence).
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn acquire_image(
        &mut self, timeout_ns: u64, sync: FrameSync<B>
    ) -> Result<SwapImageIndex, AcquireError>;

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
    fn present<'a, C, IW>(
        &'a self,
        present_queue: &mut CommandQueue<B, C>,
        image_index: SwapImageIndex,
        wait_semaphores: IW,
    ) -> Result<(), ()>
    where
        &'a Self: Borrow<B::Swapchain>,
        Self: Sized + 'a,
        IW: IntoIterator,
        IW::Item: Borrow<B::Semaphore>,
    {
        present_queue.present(Some((self, image_index)), wait_semaphores)
    }
}
