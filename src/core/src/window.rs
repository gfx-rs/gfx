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
//! Screen presentation (fullscreen or window) of images requires two objects
//!
//! * [`Surface`] is the host abstraction of the native screen
//! * [`SwapChain`] is the device abstraction for a surface, containing multiple presentable images
//!
//! [`Surface`]: trait.Surface.html
//! [`SwapChain`]: trait.SwapChain.html
//!

use {Adapter, Backend, Resources};
use {format, handle};
use format::Formatted;

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface<B: Backend> {
    ///
    type SwapChain: SwapChain<B>;

    /// Check if the queue family supports presentation for this surface.
    fn supports_queue(&self, queue_family: &B::QueueFamily) -> bool;

    /// Create a new swapchain from a surface and a queue.
    ///
    /// # Safety
    /// The queue family of the `present_queue` _must_ support surface presentation.
    /// This can be checked by calling [`supports_queue`](trait.Surface.html#tymethod.supports_queue).
    fn build_swapchain<Q>(&mut self, config: SwapchainConfig, present_queue: &Q) -> Self::SwapChain
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
    /// [`get_backbuffers`](trait.SwapChain.html#tymethod.get_backbuffers)
    pub fn id(&self) -> usize {
        self.0
    }
}

/// Synchronization primitives which will be signaled once a frame got retrieved.
///
/// The semaphore or fence _must_ be unsignaled.
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

/// Allows you to configure a `SwapChain` for creation.
#[derive(Debug, Clone)]
pub struct SwapchainConfig {
    /// Color format of the backbuffer images.
    pub color_format: format::Format,
    /// Depth stencil format of the backbuffer images (optional).
    pub depth_stencil_format: Option<format::Format>,
}

impl SwapchainConfig {
    /// Create a new default configuration (color images only).
    pub fn new() -> Self {
        SwapchainConfig {
            color_format: format::Rgba8::get_format(), // TODO: try to find best default format
            depth_stencil_format: None,
        }
    }

    /// Specify the color format for the backbuffer images.
    pub fn with_color<Cf: format::RenderFormat>(mut self) -> Self {
        self.color_format = Cf::get_format();
        self
    }

    /// Specify the depth stencil format for the backbuffer images.
    ///
    /// The SwapChain will create additional depth-stencil images for each backbuffer.
    pub fn with_depth_stencil<Dsf: format::DepthStencilFormat>(mut self) -> Self {
        self.depth_stencil_format = Some(Dsf::get_format());
        self
    }

    // TODO: depth-only, stencil-only, swapchain size, present modes, etc.
}

/// SwapChain backbuffer type (color image, depth-stencil image).
pub type Backbuffer<B: Backend> = (handle::RawTexture<B::Resources>,
                                   Option<handle::RawTexture<B::Resources>>);

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain<B: Backend> {
    /// Access the backbuffer color and depth-stencil images.
    ///
    /// *Note*: The number of exposed backbuffers might differ from number of internally used buffers.
    fn get_backbuffers(&mut self) -> &[Backbuffer<B>];

    /// Acquire a new frame for rendering. This needs to be called before presenting.
    ///
    /// # Synchronization
    /// The acquired image will not be immediately available when the function returns.
    /// Once available the underlying primitive of `sync` will be signaled.
    /// This can either be a [`Semaphore`](../trait.Resources.html#associatedtype.Semaphore)
    /// or a [`Fence`](../trait.Resources.html#associatedtype.Fence).
    fn acquire_frame(&mut self, sync: FrameSync<B::Resources>) -> Frame;

    /// Present one acquired frame in FIFO order.
    ///
    /// # Safety
    /// The passed queue _must_ be the **same** queue as used for creation.
    fn present<Q: AsMut<B::CommandQueue>>(&mut self, present_queue: &mut Q);
}

/// Extension for windows.
/// Main entry point for backend initialization from a window.
pub trait WindowExt<B: Backend> {
    /// Associated `Surface` type.
    type Surface: Surface<B>;
    /// Associated `Adapter` type.
    type Adapter: Adapter<B>;

    /// Create window surface and enumerate all available adapters.
    fn get_surface_and_adapters(&mut self) -> (Self::Surface, Vec<Self::Adapter>);
}
