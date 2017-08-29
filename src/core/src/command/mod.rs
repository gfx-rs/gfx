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

//!

use Backend;
use std::marker::PhantomData;

mod compute;
mod graphics;
mod raw;
mod renderpass;
mod transfer;

pub use self::graphics::*;
pub use self::raw::RawCommandBuffer;
pub use self::renderpass::*;
pub use self::transfer::*;

/// Thread-safe finished command buffer for submission.
pub struct Submit<B: Backend, C>(pub(crate) B::CommandBuffer, PhantomData<C>);
unsafe impl<B: Backend, C> Send for Submit<B, C> {}

impl<B: Backend, C> Submit<B, C> {
    ///
    pub unsafe fn new(buffer: CommandBuffer<B, C>) -> Self {
        Submit(buffer.raw.clone(), PhantomData)
    }
}

/// Command buffer with compute, graphics and transfer functionality.
pub struct CommandBuffer<'a, B: Backend, C> {
    pub(crate) raw: &'a mut B::CommandBuffer,
    _capability: PhantomData<C>,
}

impl<'a, B: Backend, C> CommandBuffer<'a, B, C> {
    /// Create a new typed command buffer from a raw command pool.
    pub unsafe fn new(raw: &'a mut B::CommandBuffer) -> Self {
        CommandBuffer {
            raw,
            _capability: PhantomData,
        }
    }

    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, C> {
        self.raw.finish();
        unsafe {
            Submit::new(self)
        }
    }
}
