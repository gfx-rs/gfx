//! Command pools

use {Backend};
use command::CommandBuffer;
use queue::CommandQueue;
use queue::capability::Supports;
use std::marker::PhantomData;

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
    unsafe fn from_queue(queue: &B::CommandQueue, capacity: usize) -> Self;

    #[doc(hidden)]
    unsafe fn acquire_command_buffer(&mut self) -> B::RawCommandBuffer;

    #[doc(hidden)]
    unsafe fn return_command_buffer(&mut self, B::RawCommandBuffer);
}

///
pub struct CommandPool<B: Backend, C>(
    B::RawCommandPool,
    PhantomData<C>,
);

impl<B: Backend, C> CommandPool<B, C> {
    /// Create a pool for a specific command queue
    pub fn from_queue<D: Supports<C>>(
        queue: &CommandQueue<B, D>,
        capacity: usize,
    ) -> Self
    {
        let raw = unsafe {
            B::RawCommandPool::from_queue(queue.raw(), capacity)
        };
        CommandPool(raw, PhantomData)
    }

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
    pub fn acquire_command_buffer(&mut self) -> CommandBuffer<B, C> {
        unsafe { CommandBuffer::new(&mut self.0) }
    }
}

///
pub trait SubpassCommandPool<B: Backend> { }
