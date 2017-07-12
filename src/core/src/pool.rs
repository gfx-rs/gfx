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

use {Backend, CommandQueue};
use command::{self, ComputeCommandBuffer, Encoder, GeneralCommandBuffer, GraphicsCommandBuffer,
              TransferCommandBuffer};
pub use queue::{ComputeQueue, GeneralQueue, GraphicsQueue, TransferQueue};
use std::ops::DerefMut;

/// `CommandPool` can allocate command buffers of a specific type only.
/// The allocated command buffers are associated with the creating command queue.
pub trait RawCommandPool<B: Backend> {
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
    pub fn acquire_command_buffer(&mut self) -> Encoder<B, GeneralCommandBuffer<B>> {
        unsafe { Encoder::new(GeneralCommandBuffer(self.0.acquire_command_buffer())) }
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
    pub fn acquire_command_buffer(&mut self) -> Encoder<B, GraphicsCommandBuffer<B>> {
        unsafe { Encoder::new(GraphicsCommandBuffer(self.0.acquire_command_buffer())) }
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
    pub fn acquire_command_buffer(&mut self) -> Encoder<B, ComputeCommandBuffer<B>> {
        unsafe { Encoder::new(ComputeCommandBuffer(self.0.acquire_command_buffer())) }
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
    pub fn acquire_command_buffer(&mut self) -> Encoder<B, TransferCommandBuffer<B>> {
        unsafe { Encoder::new(TransferCommandBuffer(self.0.acquire_command_buffer())) }
    }
}

///
pub trait SubpassCommandPool<B: Backend> { }
