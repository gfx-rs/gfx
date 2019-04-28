//! Command pools

use crate::command::{
    CommandBuffer, IntoRawCommandBuffer, RawLevel, SecondaryCommandBuffer, Shot,
    SubpassCommandBuffer,
};
use crate::queue::capability::{Graphics, Supports};
use crate::Backend;

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

bitflags!(
    /// Command pool creation flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct CommandPoolCreateFlags: u8 {
        /// Indicates short-lived command buffers.
        /// Memory optimization hint for implementations.
        const TRANSIENT = 0x1;
        /// Allow command buffers to be reset individually.
        const RESET_INDIVIDUAL = 0x2;
    }
);

/// The allocated command buffers are associated with the creating command queue.
pub trait RawCommandPool<B: Backend>: fmt::Debug + Any + Send + Sync {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    unsafe fn reset(&mut self);

    /// Allocate a single command buffers from the pool.
    fn allocate_one(&mut self, level: RawLevel) -> B::CommandBuffer {
        self.allocate_vec(1, level).pop().unwrap()
    }

    /// Allocate new command buffers from the pool.
    fn allocate_vec(&mut self, num: usize, level: RawLevel) -> Vec<B::CommandBuffer> {
        (0..num).map(|_| self.allocate_one(level)).collect()
    }

    /// Free command buffers which are allocated from this pool.
    unsafe fn free<I>(&mut self, buffers: I)
    where
        I: IntoIterator<Item = B::CommandBuffer>;
}

/// Strong-typed command pool.
///
/// This a safer wrapper around `RawCommandPool` which ensures that only **one**
/// command buffer is recorded at the same time from the current queue.
/// Command buffers are stored internally and can only be obtained via a strong-typed
/// `CommandBuffer` wrapper for encoding.
#[derive(Debug)]
pub struct CommandPool<B: Backend, C> {
    raw: B::CommandPool,
    _capability: PhantomData<C>,
}

impl<B: Backend, C> CommandPool<B, C> {
    /// Create typed command pool from raw.
    ///
    /// # Safety
    ///
    /// `<C as Capability>::supported_by(queue_type)` must return true
    /// for `queue_type` being the type of queues from family this `raw` pool is associated with.
    ///
    pub unsafe fn new(raw: B::CommandPool) -> Self {
        CommandPool {
            raw,
            _capability: PhantomData,
        }
    }

    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub unsafe fn reset(&mut self) {
        self.raw.reset();
    }

    /// Allocates a new primary command buffer from the pool.
    pub fn acquire_command_buffer<S: Shot>(&mut self) -> CommandBuffer<B, C, S> {
        let buffer = self.raw.allocate_one(RawLevel::Primary);
        unsafe { CommandBuffer::new(buffer) }
    }

    /// Allocates a new secondary command buffer from the pool.
    pub fn acquire_secondary_command_buffer<S: Shot>(&mut self) -> SecondaryCommandBuffer<B, C, S> {
        let buffer = self.raw.allocate_one(RawLevel::Secondary);
        unsafe { SecondaryCommandBuffer::new(buffer) }
    }

    /// Free the given iterator of command buffers from the pool.
    pub unsafe fn free<I>(&mut self, cmd_buffers: I)
    where
        I: IntoIterator,
        I::Item: IntoRawCommandBuffer<B, C>,
    {
        self.raw
            .free(cmd_buffers.into_iter().map(|cmb| cmb.into_raw()))
    }

    /// Downgrade a typed command pool to untyped one.
    pub fn into_raw(self) -> B::CommandPool {
        self.raw
    }
}

impl<B: Backend, C: Supports<Graphics>> CommandPool<B, C> {
    /// Allocates a new subpass command buffer from the pool.
    pub fn acquire_subpass_command_buffer<S: Shot>(&mut self) -> SubpassCommandBuffer<B, S> {
        let buffer = self.raw.allocate_one(RawLevel::Secondary);
        unsafe { SubpassCommandBuffer::new(buffer) }
    }
}
