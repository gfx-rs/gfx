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

use std::borrow::BorrowMut;
use std::ops::{DerefMut};
use {command, Backend, CommandPool, CommandQueue};
pub use queue::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};

/// General command pool can allocate general command buffers.
pub trait GeneralCommandPool<B: Backend>: CommandPool<B> {
    ///
    fn from_queue<Q>(queue: Q, capacity: usize) -> Self
        where Q: Into<GeneralQueue<B>> +
                 BorrowMut<B::CommandQueue>;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, B, B::GeneralCommandBuffer>;
}

/// Graphics command pool can allocate graphics command buffers.
pub trait GraphicsCommandPool<B: Backend>: CommandPool<B> {
    ///
    fn from_queue<Q>(queue: Q, capacity: usize) -> Self
        where Q: Into<GraphicsQueue<B>> +
                 BorrowMut<B::CommandQueue>;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, B, B::GraphicsCommandBuffer>;
}

/// Compute command pool can allocate compute command buffers.
pub trait ComputeCommandPool<B: Backend>: CommandPool<B> {
    ///
    fn from_queue<Q>(queue: Q, capacity: usize) -> Self
        where Q: Into<ComputeQueue<B>> +
                 BorrowMut<B::CommandQueue>;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, B, B::ComputeCommandBuffer>;
}

/// Transfer command pool can allocate transfer command buffers.
pub trait TransferCommandPool<B: Backend>: CommandPool<B> {
    ///
    fn from_queue<Q>(queue: Q, capacity: usize) -> Self
        where Q: Into<TransferQueue<B>> +
                 BorrowMut<B::CommandQueue>;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, B, B::TransferCommandBuffer>;
}

/// Subpass command pool can allocate subpass command buffers.
pub trait SubpassCommandPool<B: Backend>: CommandPool<B> {
    ///
    fn from_queue<Q>(queue: Q, capacity: usize) -> Self
        where Q: Into<GraphicsQueue<B>> +
                 BorrowMut<B::CommandQueue>;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, B, B::SubpassCommandBuffer>;
}
