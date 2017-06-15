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
use core::command::{GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, Encoder};
use core::queue::{Compatible,
    GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use command::{self, RawCommandBuffer, SubpassCommandBuffer};
use {Backend, CommandQueue, Resources};
use core::CommandPool;

macro_rules! impl_pool {
    ($pool:ident, $queue:ident, $buffer:ident) => (
        pub struct $pool {
        }

        impl core::CommandPool<Backend> for $pool {
            fn reset(&mut self) {
                unimplemented!()
            }

            fn reserve(&mut self, additional: usize) {
                unimplemented!()
            }
        }

        impl pool::$pool<Backend> for $pool {
            fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
                where Q: Compatible<$queue<Backend>> + AsRef<CommandQueue>
            {
                unimplemented!()
            }

            fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, $buffer<Backend>> {
                unimplemented!()
            }
        }
    )
}

impl_pool!{ GeneralCommandPool, GeneralQueue, GeneralCommandBuffer }
impl_pool!{ GraphicsCommandPool, GraphicsQueue, GraphicsCommandBuffer }
impl_pool!{ ComputeCommandPool, ComputeQueue, ComputeCommandBuffer }
impl_pool!{ TransferCommandPool, TransferQueue, TransferCommandBuffer }

pub struct SubpassCommandPool {
}

impl core::CommandPool<Backend> for SubpassCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {
    fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
        where Q: Compatible<GraphicsQueue<Backend>> + AsRef<CommandQueue>
    {
        unimplemented!()
    }

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, SubpassCommandBuffer> {
        unimplemented!()
    }
}