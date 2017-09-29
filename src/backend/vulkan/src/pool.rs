use std::ptr;
use ash::vk;
use ash::version::DeviceV1_0;
use smallvec::SmallVec;

use command::{CommandBuffer, SubpassCommandBuffer};
use core::pool;
use {Backend, CommandQueue, DeviceRef};


pub struct RawCommandPool {
    pool: vk::CommandPool,
    device: DeviceRef,
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

    unsafe fn from_queue(queue: &CommandQueue, create_flags: pool::CommandPoolCreateFlags) -> RawCommandPool {
        let mut flags = vk::CommandPoolCreateFlags::empty();
        if create_flags.contains(pool::TRANSIENT) {
            flags |= vk::COMMAND_POOL_CREATE_TRANSIENT_BIT;
        }
        if create_flags.contains(pool::RESET_INDIVIDUAL) {
            flags |= vk::COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
        }

        let info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::CommandPoolCreateInfo,
            p_next: ptr::null(),
            flags,
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
    _device: DeviceRef,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool { }
