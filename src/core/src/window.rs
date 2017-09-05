//! Windowing system interoperability
//!
//! Screen presentation (fullscreen or window) of images requires two objects:
//!
//! * [`Surface`] is the host abstraction of the native screen
//! * [`Swapchain`] is the device abstraction for a surface, containing multiple presentable images
//!
//! ## Window
//!
//! // TODO
//!
//! ## Surface
//!
//! // TODO
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
//! # extern crate gfx_core;
//! # fn main() {
//! use gfx_core::{Device, FrameSync};
//! # use gfx_core::{CommandQueue, Graphics, Swapchain};
//!
//! # let mut swapchain: empty::Swapchain = return;
//! # let mut device: empty::Device = return;
//! # let mut present_queue: CommandQueue<empty::Backend, Graphics> = return;
//! let acquisition_semaphore = device.create_semaphore();
//! let render_semaphore = device.create_semaphore();
//!
//! let frame = swapchain.acquire_frame(FrameSync::Semaphore(&acquisition_semaphore));
//! // render the scene..
//! // `render_semaphore` will be signalled once rendering has been finished
//! swapchain.present(&mut present_queue, &[&render_semaphore]);
//! # }
//! ```
//!
//! Queues need to synchronize with the presentation engine, usually done via signalling a semaphore
//! once a frame is available for rendering and waiting on a separate semaphore until scene rendering
//! has finished.
//!
//! ### Recreation
//!
//! //TODO

use Backend;
use format::{self, Formatted};
use queue::CommandQueue;

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface<B: Backend> {

    /// Check if the queue family supports presentation for this surface.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn supports_queue(&self, queue_family: &B::QueueFamily) -> bool;

    /// Create a new swapchain from a surface and a queue.
    ///
    /// # Safety
    ///
    /// The queue family of the passed `present_queue` _must_ support surface presentation.
    /// This can be checked by calling [`supports_queue`](trait.Surface.html#tymethod.supports_queue)
    /// on this surface.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_core;
    /// # fn main() {
    /// use gfx_core::{Surface, SwapchainConfig};
    /// use gfx_core::format::Srgba8;
    /// # use gfx_core::{CommandQueue, Graphics};
    ///
    /// # let mut surface: empty::Surface = return;
    /// # let queue: CommandQueue<empty::Backend, Graphics> = return;
    /// let swapchain_config = SwapchainConfig::new()
    ///                             .with_color::<Srgba8>();
    /// surface.build_swapchain(swapchain_config, &queue);
    /// # }
    /// ```
    fn build_swapchain<C>(&mut self,
        config: SwapchainConfig,
        present_queue: &CommandQueue<B, C>,
    ) -> B::Swapchain;
}

/// Handle to a backbuffer of the swapchain.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Frame(usize);

impl Frame {
    #[doc(hidden)]
    pub fn new(id: usize) -> Self {
        Frame(id)
    }

    /// Retrieve frame id.
    ///
    /// The can be used to fetch the currently used backbuffer from
    /// [`get_backbuffers`](trait.Swapchain.html#tymethod.get_backbuffers)
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn id(&self) -> usize {
        self.0
    }
}

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

/// Allows you to configure a `Swapchain` for creation.
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    /// Color format of the backbuffer images.
    pub color_format: format::Format,
    /// Depth stencil format of the backbuffer images (optional).
    pub depth_stencil_format: Option<format::Format>,
}

impl SwapchainConfig {
    /// Create a new default configuration (color images only).
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn new() -> Self {
        SwapchainConfig {
            color_format: format::Rgba8::get_format(), // TODO: try to find best default format
            depth_stencil_format: None,
        }
    }

    /// Specify the color format for the backbuffer images.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn with_color<Cf: format::RenderFormat>(mut self) -> Self {
        self.color_format = Cf::get_format();
        self
    }

    /// Specify the depth stencil format for the backbuffer images.
    ///
    /// The Swapchain will create additional depth-stencil images for each backbuffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    pub fn with_depth_stencil<Dsf: format::DepthStencilFormat>(mut self) -> Self {
        self.depth_stencil_format = Some(Dsf::get_format());
        self
    }

    // TODO: depth-only, stencil-only, swapchain size, present modes, etc.
}

/// Swapchain backbuffer type
pub struct Backbuffer<B: Backend> {
    /// Back buffer color
    pub color: B::Image,
    /// Back buffer depth/stencil
    pub depth_stencil: Option<B::Image>,
}

/// The `Swapchain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait Swapchain<B: Backend> {
    /// Access the backbuffer color and depth-stencil images.
    ///
    /// *Note*: The number of exposed backbuffers might differ from number of internally used buffers.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn get_backbuffers(&mut self) -> &[Backbuffer<B>];

    /// Acquire a new frame for rendering. This needs to be called before presenting.
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
    fn acquire_frame(&mut self, sync: FrameSync<B>) -> Frame;

    /// Present one acquired frame in FIFO order.
    ///
    /// # Safety
    ///
    /// The passed queue _must_ be the **same** queue as used for creation.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn present<C>(
        &mut self,
        present_queue: &mut CommandQueue<B, C>,
        wait_semaphores: &[&B::Semaphore],
    );
}
