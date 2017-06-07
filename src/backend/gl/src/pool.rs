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

use core::{self, pool};
use core::command::{Encoder};
use core::queue::{Compatible,
    GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue,
    GeneralQueueRef, GraphicsQueueRef, ComputeQueueRef, TransferQueueRef};
use command::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use {Backend, CommandQueue, Resources};
use core::CommandPool;

macro_rules! impl_pool {
    ($pool:ident, $queue:ident, $queue_ref:ident, $buffer:ident) => (
        pub struct $pool {
            command_buffers: Vec<$buffer>,
            next_buffer: usize,
        }

        impl core::CommandPool<Backend> for $pool {
            fn reset(&mut self) {
                self.next_buffer = 0;
            }

            fn reserve(&mut self, additional: usize) {
                for _ in 0..additional {
                    self.command_buffers.push($buffer::new(0));
                }
            }
        }

        impl pool::$pool<Backend> for $pool {
            fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
                where Q: Compatible<$queue<Backend>> + AsRef<CommandQueue>
            {
                let buffers = (0..capacity).map(|_| $buffer::new(0))
                                           .collect();
                $pool {
                    command_buffers: buffers,
                    next_buffer: 0,
                }
            }

            fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, $buffer> {
                let available_buffers = self.command_buffers.len() as isize - self.next_buffer as isize;
                if available_buffers <= 0 {
                    self.reserve((-available_buffers) as usize + 1);
                }

                let buffer = &mut self.command_buffers[self.next_buffer];
                self.next_buffer += 1;

                unsafe { Encoder::new(buffer) }
            }
        }
    )
}

impl_pool!{ GeneralCommandPool, GeneralQueue, GeneralQueueRef, GeneralCommandBuffer }
impl_pool!{ GraphicsCommandPool, GraphicsQueue, GraphicsQueueRef, GraphicsCommandBuffer }
impl_pool!{ ComputeCommandPool, ComputeQueue, ComputeQueueRef, ComputeCommandBuffer }
impl_pool!{ TransferCommandPool, TransferQueue, TransferQueueRef, TransferCommandBuffer }
impl_pool!{ SubpassCommandPool, GraphicsQueue, GraphicsQueueRef, SubpassCommandBuffer }
