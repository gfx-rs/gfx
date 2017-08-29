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

//! Command pools

use {Backend};
use command::{CommandBuffer, RawCommandBuffer};
use queue::CommandQueue;
use queue::capability::Supports;
use std::marker::PhantomData;

/// The allocated command buffers are associated with the creating command queue.
pub trait RawCommandPool<B: Backend>: Send {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    fn reset(&mut self);

    #[doc(hidden)]
    unsafe fn from_queue(queue: &B::CommandQueue) -> Self;

    /// Allocate new command buffers from the pool.
    fn allocate(&mut self, num: usize) -> Vec<B::CommandBuffer>;

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
    pool: B::CommandPool,
    next_buffer: usize,
    _capability: PhantomData<C>,
}

impl<B: Backend, C> CommandPool<B, C> {
    /// Create a pool for a specific command queue
    pub fn from_queue<D: Supports<C>>(
        queue: &CommandQueue<B, D>,
        capacity: usize,
    ) -> Self
    {
        let raw = unsafe {
            B::CommandPool::from_queue(queue.as_raw())
        };
        let mut pool = CommandPool {
            buffers: Vec::new(),
            pool: raw,
            next_buffer: 0,
            _capability: PhantomData,
        };
        pool.reserve(capacity);
        pool
    }

    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    pub fn reset(&mut self) {
        self.pool.reset();
        self.next_buffer = 0;
    }

    /// Reserve an additional amount of command buffers.
    pub fn reserve(&mut self, additional: usize) {
        let buffers = self.pool.allocate(additional);
        self.buffers.extend(buffers);
    }

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    pub fn acquire_command_buffer<'a>(&'a mut self) -> CommandBuffer<'a, B, C> {
        if self.buffers.len() <= self.next_buffer {
            self.reserve(1);
        }

        let buffer = &mut self.buffers[self.next_buffer];
        self.next_buffer += 1;
        unsafe {
            buffer.begin();
            CommandBuffer::new(buffer)
        }
    }
}

impl<B: Backend, C> Drop for CommandPool<B, C> {
    fn drop(&mut self) {
        let free_list = self.buffers.drain(..).collect::<Vec<_>>();
        unsafe { self.pool.free(free_list); }
    }
}

///
pub trait SubpassCommandPool<B: Backend> { }
