// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
//! use gfx_core::{Device, FrameSync};
//! # use gfx_core::{GraphicsQueue, Swapchain};
//! # use gfx_core::dummy::{DummyBackend, DummyDevice, DummyResources, DummySwapchain};
//!
//! # let mut swapchain: DummySwapchain = return;
//! # let mut device: DummyDevice = return;
//! # let mut present_queue: GraphicsQueue<DummyBackend> = return;
//! let acquisition_semaphore = device.create_semaphore();
//! let render_semaphore = device.create_semaphore();
//!
//! let frame = swapchain.acquire_frame(FrameSync::Semaphore(&acquisition_semaphore));
//! // render the scene..
//! // `render_semaphore` will be signalled once rendering has been finished
//! swapchain.present(&mut present_queue, &[&render_semaphore]);
//! ```
//!
//! Queues need to synchronize with the presentation engine, usually done via signalling a semaphore
//! once a frame is available for rendering and waiting on a separate semaphore until scene rendering
//! has finished.
//!
//! ### Recreation
//!
//! // TODO
//!
//! # Examples
//!
//! Initializing a swapchain and device from a window:
//!
//! ```no_run
//! use gfx_core::{Adapter, Surface, WindowExt};
//! # use gfx_core::dummy::DummyWindow;
//!
//! # let mut window: DummyWindow = return;
//! let (mut surface, adapters) = window.get_surface_and_adapters();
//! # // TODO:
//! ```
//!
//! > *Note*: `WindowExt`, `Surface` and `Swapchain` are _not_ part of the `Backend`
//! > to allow support for different window libraries.
//!
//! [`Surface`]: trait.Surface.html
//! [`Swapchain`]: trait.Swapchain.html
//!

use {Adapter, Backend, Resources};
use {format, handle};
use format::Formatted;

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface<B: Backend> {
    ///
    type Swapchain: Swapchain<B>;

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
    /// use gfx_core::{Surface, SwapchainConfig};
    /// use gfx_core::format::Srgba8;
    /// # use gfx_core::GraphicsQueue;
    /// # use gfx_core::dummy::{DummyBackend, DummySurface};
    ///
    /// # let mut surface: DummySurface = return;
    /// # let queue: GraphicsQueue<DummyBackend> = return;
    /// let swapchain_config = SwapchainConfig::new()
    ///                             .with_color::<Srgba8>();
    /// surface.build_swapchain(swapchain_config, &queue);
    /// ```
    fn build_swapchain<Q>(&mut self, config: SwapchainConfig, present_queue: &Q) -> Self::Swapchain
    where
        Q: AsRef<B::CommandQueue>;
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
pub enum FrameSync<'a, R: Resources> {
    /// Semaphore used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Semaphore(&'a handle::Semaphore<R>),

    /// Fence used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Fence(&'a handle::Fence<R>),
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

/// Swapchain backbuffer type (color image, depth-stencil image).
pub type Backbuffer<B: Backend> = (handle::RawTexture<B::Resources>,
                                   Option<handle::RawTexture<B::Resources>>);

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
    fn acquire_frame(&mut self, sync: FrameSync<B::Resources>) -> Frame;

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
    fn present<Q: AsMut<B::CommandQueue>>(
        &mut self,
        present_queue: &mut Q,
        wait_semaphores: &[&handle::Semaphore<B::Resources>],
    );
}

/// Extension for windows.
/// Main entry point for backend initialization from a window.
pub trait WindowExt<B: Backend> {
    /// Associated `Surface` type.
    type Surface: Surface<B>;
    /// Associated `Adapter` type.
    type Adapter: Adapter<B>;

    /// Create window surface and enumerate all available adapters.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// ```
    fn get_surface_and_adapters(&mut self) -> (Self::Surface, Vec<Self::Adapter>);
}
