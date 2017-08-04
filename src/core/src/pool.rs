//! Command pools

use {Backend, CommandQueue};
use command::{self, ComputeCommandBuffer, GeneralCommandBuffer,
    GraphicsCommandBuffer, TransferCommandBuffer};
pub use queue::{ComputeQueue, GeneralQueue, GraphicsQueue, TransferQueue};
use std::ops::DerefMut;

/// `CommandPool` can allocate command buffers of a specific type only.
/// The allocated command buffers are associated with the creating command queue.
pub trait RawCommandPool<B: Backend>: Send {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    fn reset(&mut self);

    /// Reserve an additional amount of command buffers.
    fn reserve(&mut self, additional: usize);

    #[doc(hidden)]
    unsafe fn from_queue<Q>(queue: Q, capacity: usize) -> Self
    where Q: AsRef<B::CommandQueue>;

    #[doc(hidden)]
    unsafe fn acquire_command_buffer(&mut self)
        -> &mut B::RawCommandBuffer;
}

///
pub struct GeneralCommandPool<B: Backend>(pub(crate) B::RawCommandPool);
impl<B: Backend> GeneralCommandPool<B> {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) { self.0.reset() }

    /// Reserve an additional amount of command buffers.
    pub fn reserve(&mut self, additional: usize) { self.0.reserve(additional) }

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer(&mut self) -> GeneralCommandBuffer<B> {
        GeneralCommandBuffer(self.0.acquire_command_buffer())
    }
}
///
pub struct GraphicsCommandPool<B: Backend>(pub(crate) B::RawCommandPool);
impl<B: Backend> GraphicsCommandPool<B> {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) { self.0.reset() }

    /// Reserve an additional amount of command buffers.
    pub fn reserve(&mut self, additional: usize) { self.0.reserve(additional) }

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer(&mut self) -> GraphicsCommandBuffer<B> {
        GraphicsCommandBuffer(self.0.acquire_command_buffer())
    }
}
///
pub struct ComputeCommandPool<B: Backend>(pub(crate) B::RawCommandPool);
impl<B: Backend> ComputeCommandPool<B> {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) { self.0.reset() }

    /// Reserve an additional amount of command buffers.
    pub fn reserve(&mut self, additional: usize) { self.0.reserve(additional) }

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer(&mut self) -> ComputeCommandBuffer<B> {
        ComputeCommandBuffer(self.0.acquire_command_buffer())
    }
}
///
pub struct TransferCommandPool<B: Backend>(pub(crate) B::RawCommandPool);
impl<B: Backend> TransferCommandPool<B> {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) { self.0.reset() }

    /// Reserve an additional amount of command buffers.
    pub fn reserve(&mut self, additional: usize) { self.0.reserve(additional) }

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer(&mut self) -> TransferCommandBuffer<B> {
        TransferCommandBuffer(self.0.acquire_command_buffer())
    }
}

///
pub trait SubpassCommandPool<B: Backend> { }
