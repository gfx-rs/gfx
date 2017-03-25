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
use std::ptr;
use std::sync::Arc;
use ash::vk;
use ash::version::DeviceV1_0;

use core::{self, pool};
use core::command::{Encoder};
use core::{CommandPool, GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use command::CommandBuffer;
use native::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use {CommandQueue, DeviceInner};

macro_rules! impl_pool {
    ($pool:ident, $queue:ident, $buffer:ident) => (
        pub struct $pool {
            pool: vk::CommandPool,
            command_buffers: Vec<$buffer>,
            next_buffer: usize,
            device: Arc<DeviceInner>,
        }

        impl core::CommandPool for $pool {
            type Queue = CommandQueue;
            type PoolBuffer = $buffer;

            fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, $buffer> {
                let available_buffers = self.command_buffers.len() as isize - self.next_buffer as isize;
                if available_buffers <= 0 {
                    self.reserve((-available_buffers) as usize + 1);
                }

                let buffer = &mut self.command_buffers[self.next_buffer];
                self.next_buffer += 1;

                let info = vk::CommandBufferBeginInfo {
                    s_type: vk::StructureType::CommandBufferBeginInfo,
                    p_next: ptr::null(),
                    flags: vk::COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
                    p_inheritance_info: ptr::null(),
                };

                unsafe {
                    self.device.0.begin_command_buffer(buffer.0.inner, &info); // TODO: error handling
                    Encoder::new(buffer)
                }
            }

            fn reset(&mut self) {
                self.next_buffer = 0;
                unsafe {
                    self.device.0.fp_v1_0().reset_command_pool(
                        self.device.0.handle(),
                        self.pool,
                        vk::CommandPoolResetFlags::empty()
                    );
                }
            }

            fn reserve(&mut self, additional: usize) {
                unimplemented!()
            }
        }

        impl pool::$pool for $pool {
            fn from_queue<Q>(queue: &mut Q, capacity: usize) -> $pool
                where Q: Into<$queue<CommandQueue>> + DerefMut<Target=CommandQueue>
            {
                // Create command pool
                let info = vk::CommandPoolCreateInfo {
                    s_type: vk::StructureType::CommandPoolCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::CommandPoolCreateFlags::empty(),
                    queue_family_index: queue.family_index,
                };

                let command_pool = unsafe {
                    queue.device.0.create_command_pool(&info, None)
                                .expect("Error on command pool creation") // TODO: better error handling
                };

                // Allocate initial command buffers
                let info = vk::CommandBufferAllocateInfo {
                    s_type: vk::StructureType::CommandBufferAllocateInfo,
                    p_next: ptr::null(),
                    command_pool: command_pool,
                    level: vk::CommandBufferLevel::Primary,
                    command_buffer_count: capacity as u32,
                };

                let command_buffers = unsafe {
                    queue.device.0.allocate_command_buffers(&info)
                                  .expect("Error on command buffer allocation") // TODO: better error handling
                };
                let command_buffers = command_buffers.into_iter().map(|buffer| {
                    $buffer(
                        CommandBuffer {
                            inner: buffer,
                            device: queue.device.clone(),
                        }
                    )
                }).collect::<Vec<_>>();

                $pool {
                    pool: command_pool,
                    command_buffers: command_buffers,
                    next_buffer: 0,
                    device: queue.device.clone(),
                }
            }
        }
    )
}

impl_pool!{ GeneralCommandPool, GeneralQueue, GeneralCommandBuffer }
impl_pool!{ GraphicsCommandPool, GraphicsQueue, GraphicsCommandBuffer }
impl_pool!{ ComputeCommandPool, ComputeQueue, ComputeCommandBuffer }
impl_pool!{ TransferCommandPool, TransferQueue, TransferCommandBuffer }
impl_pool!{ SubpassCommandPool, GraphicsQueue, SubpassCommandBuffer }
