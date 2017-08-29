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

use std::ptr;
use std::sync::Arc;
use ash::vk;
use ash::version::DeviceV1_0;
use smallvec::SmallVec;

use command::{CommandBuffer, SubpassCommandBuffer};
use core::pool;
use {Backend, CommandQueue, RawDevice};


pub struct RawCommandPool {
    pool: vk::CommandPool,
    device: Arc<RawDevice>,
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unsafe {
            self.device.0.fp_v1_0().reset_command_pool(
                self.device.0.handle(),
                self.pool,
                vk::CommandPoolResetFlags::empty()
            );
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<CommandBuffer> {
        let info = vk::CommandBufferAllocateInfo {
            s_type: vk::StructureType::CommandBufferAllocateInfo,
            p_next: ptr::null(),
            command_pool: self.pool,
            level: vk::CommandBufferLevel::Primary,
            command_buffer_count: num as u32,
        };

        let device = &self.device;
        let cbufs_raw = unsafe {
            device.0.allocate_command_buffers(&info)
        }.expect("Error on command buffer allocation");

        cbufs_raw
            .into_iter()
            .map(|buffer| {
                CommandBuffer {
                    raw: buffer,
                    device: device.clone(),
                }
            }).collect()
    }

    unsafe fn free(&mut self, cbufs: Vec<CommandBuffer>) {
        let buffers: SmallVec<[vk::CommandBuffer; 16]> =
            cbufs.into_iter()
                 .map(|buffer| buffer.raw)
                 .collect();
        self.device.0.free_command_buffers(self.pool, &buffers);
    }

    unsafe fn from_queue(queue: &CommandQueue) -> RawCommandPool {
        // Create command pool
        let info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::CommandPoolCreateInfo,
            p_next: ptr::null(),
            flags: vk::CommandPoolCreateFlags::empty(),
            queue_family_index: queue.family_index,
        };

        let command_pool_raw = queue.device.0
            .create_command_pool(&info, None)
            .expect("Error on command pool creation"); // TODO: better error handling

        RawCommandPool {
            pool: command_pool_raw,
            device: queue.device.clone(),
        }
    }
}

pub struct SubpassCommandPool {
    _pool: vk::CommandPool,
    _command_buffers: Vec<SubpassCommandBuffer>,
    _next_buffer: usize,
    _device: Arc<RawDevice>,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {
    /*
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

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, SubpassCommandBuffer> {
        unimplemented!()
    }

    fn from_queue<Q>(mut queue: Q, capacity: usize) -> SubpassCommandPool
        where Q: Compatible<GraphicsQueue<Backend>> + AsRef<CommandQueue>
    {
        let queue = queue.as_ref();

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
            level: vk::CommandBufferLevel::Secondary,
            command_buffer_count: capacity as u32,
        };

        let command_buffers = unsafe {
            queue.device.0.allocate_command_buffers(&info)
                          .expect("Error on command buffer allocation") // TODO: better error handling
        };
        let command_buffers = command_buffers.into_iter().map(|buffer| {
            SubpassCommandBuffer(
                CommandBuffer {
                    raw: buffer,
                    device: queue.device.clone(),
                }
            )
        }).collect();

        SubpassCommandPool {
            pool: command_pool,
            command_buffers: command_buffers,
            next_buffer: 0,
            device: queue.device.clone(),
        }
    }
    */
}
