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

use std::ops::DerefMut;

use core::{self, pool};
use core::command::Encoder;
use core::{CommandPool, GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use native::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use CommandQueue;

macro_rules! impl_pool {
    ($pool:ident, $queue:ident, $buffer:ident) => (
        pub struct $pool;

        impl core::CommandPool for $pool {
            type Queue = CommandQueue;
            type PoolBuffer = $buffer;

            fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, $buffer> {
                unimplemented!()
            }

            fn reset(&mut self) {
                unimplemented!()
            }

            fn reserve(&mut self, additional: usize) {
                unimplemented!()
            }
        }

        impl pool::$pool for $pool {
            fn from_queue<Q>(queue: &mut Q, capacity: usize) -> $pool
                where Q: Into<$queue<CommandQueue>> + DerefMut<Target=CommandQueue>
            {
                unimplemented!()
            }
        }
    )
}

impl_pool!{ GeneralCommandPool, GeneralQueue, GeneralCommandBuffer }
impl_pool!{ GraphicsCommandPool, GraphicsQueue, GraphicsCommandBuffer }
impl_pool!{ ComputeCommandPool, ComputeQueue, ComputeCommandBuffer }
impl_pool!{ TransferCommandPool, TransferQueue, TransferCommandBuffer }
impl_pool!{ SubpassCommandPool, GraphicsQueue, SubpassCommandBuffer }
