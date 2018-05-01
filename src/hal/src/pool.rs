//! Command pools

use {pass};
use {Backend};
use command::{
    CommandBuffer, RawCommandBuffer, SecondaryCommandBuffer, 
    SubpassCommandBuffer, CommandBufferFlags, Shot, RawLevel,
    CommandBufferInheritanceInfo
};
use queue::capability::{Supports, Graphics};

use std::any::Any;
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
pub trait RawCommandPool<B: Backend>: Any + Send + Sync {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    fn reset(&mut self);

    /// Allocate new command buffers from the pool.
    fn allocate(&mut self, num: usize, level: RawLevel) -> Vec<B::CommandBuffer>;

    /// Free command buffers which are allocated from this pool.
    unsafe fn free(&mut self, buffers: Vec<B::CommandBuffer>);
}

/// Strong-typed command pool.
///
/// This a safer wrapper around `RawCommandPool` which ensures that only **one**
/// command buffer is recorded at the same time from the current queue.
/// Command buffers are stored internally and can only be obtained via a strong-typed
/// `CommandBuffer` wrapper for encoding.
pub struct CommandPool<B: Backend, C> {
    buffers: Vec<B::CommandBuffer>,
    secondary_buffers: Vec<B::CommandBuffer>,
    raw: B::CommandPool,
    next_buffer: usize,
    next_secondary_buffer: usize,
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
            buffers: Vec::new(),
            secondary_buffers: Vec::new(),
            raw: raw,
            next_buffer: 0,
            next_secondary_buffer: 0,
            _capability: PhantomData,
        }
    }

    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) {
        self.raw.reset();
        self.next_buffer = 0;
        self.next_secondary_buffer = 0;
    }

    /// Reserve an additional amount of primary command buffers.
    pub fn reserve(&mut self, additional: usize) {
        let available = self.buffers.len() - self.next_buffer;
        if additional > available {
            let buffers = self.raw.allocate(additional - available, RawLevel::Primary);
            self.buffers.extend(buffers);
        }
    }

    /// Reserve an additional amount of secondary command buffers.
    pub fn reserve_secondary(&mut self, additional: usize) {
        let available = self.secondary_buffers.len() - self.next_secondary_buffer;
        if additional > available {
            let buffers = self.raw.allocate(additional - available, RawLevel::Secondary);
            self.secondary_buffers.extend(buffers);
        }
    }

    /// Get a primary command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer<S: Shot>(
        &mut self, allow_pending_resubmit: bool
    ) -> CommandBuffer<B, C, S> {
        self.reserve(1);

        let buffer = &mut self.buffers[self.next_buffer];
        let mut flags = S::FLAGS;
        if allow_pending_resubmit {
            flags |= CommandBufferFlags::SIMULTANEOUS_USE;
        }
        buffer.begin(flags, CommandBufferInheritanceInfo::default());
        self.next_buffer += 1;
        unsafe {
            CommandBuffer::new(buffer)
        }
    }

    /// Get a secondary command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_secondary_command_buffer<'a, S: Shot>(
        &mut self,
        allow_pending_resubmit: bool,
        subpass: Option<pass::Subpass<'a, B>>,
        framebuffer: Option<&'a B::Framebuffer>,
    ) -> SecondaryCommandBuffer<B, C, S> {
        self.reserve_secondary(1);

        let buffer = &mut self.secondary_buffers[self.next_secondary_buffer];
        let mut flags = S::FLAGS;
        if allow_pending_resubmit {
            flags |= CommandBufferFlags::SIMULTANEOUS_USE;
        }
        let inheritance_info = CommandBufferInheritanceInfo {
            subpass,
            framebuffer,
            ..CommandBufferInheritanceInfo::default()
        };
        buffer.begin(flags, inheritance_info);
        self.next_secondary_buffer += 1;
        unsafe {
            SecondaryCommandBuffer::new(buffer)
        }
    }

    /// Downgrade a typed command pool to untyped one, free up the allocated command buffers.
    pub fn into_raw(mut self) -> B::CommandPool {
        unsafe {
            self.raw.free(self.buffers.drain(..).collect::<Vec<_>>());
            self.raw.free(self.secondary_buffers.drain(..).collect::<Vec<_>>());
        }
        self.raw
    }
}

impl<B: Backend, C: Supports<Graphics>> CommandPool<B, C> {
    /// Get a subpass command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_subpass_command_buffer<'a, S: Shot>(
        &mut self,
        allow_pending_resubmit: bool,
        subpass: pass::Subpass<'a, B>,
        framebuffer: Option<&'a B::Framebuffer>,
    ) -> SubpassCommandBuffer<B, S> {
        self.reserve_secondary(1);

        let buffer = &mut self.secondary_buffers[self.next_secondary_buffer];
        let mut flags = S::FLAGS;
        if allow_pending_resubmit {
            flags |= CommandBufferFlags::SIMULTANEOUS_USE;
        }
        let inheritance_info = CommandBufferInheritanceInfo {
            subpass: Some(subpass),
            framebuffer,
            ..CommandBufferInheritanceInfo::default()
        };
        buffer.begin(flags, inheritance_info);
        self.next_secondary_buffer += 1;
        unsafe {
            SubpassCommandBuffer::new(buffer)
        }
    }
}
