use std::ptr;
use std::sync::Arc;
use ash::vk;
use ash::version::DeviceV1_0;

use core::pool;
use command::{CommandBuffer, SubpassCommandBuffer};
//use core::command::{GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer};
use {Backend, CommandQueue, RawDevice};


pub struct RawCommandPool {
    pool: vk::CommandPool,
    command_buffers: Vec<CommandBuffer>,
    next_buffer: usize,
    device: Arc<RawDevice>,
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
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

    unsafe fn acquire_command_buffer<'a>(&'a mut self) -> &mut CommandBuffer {
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

        self.device.0.begin_command_buffer(buffer.raw, &info); // TODO: error handling
        buffer
    }

    unsafe fn from_queue<Q>(mut queue: Q, capacity: usize) -> RawCommandPool
        where Q: AsRef<CommandQueue>
    {
        let queue = queue.as_ref();

        // Create command pool
        let info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::CommandPoolCreateInfo,
            p_next: ptr::null(),
            flags: vk::CommandPoolCreateFlags::empty(),
            queue_family_index: queue.family_index,
        };

        let command_pool = queue.device.0
            .create_command_pool(&info, None)
            .expect("Error on command pool creation"); // TODO: better error handling

        // Allocate initial command buffers
        let info = vk::CommandBufferAllocateInfo {
            s_type: vk::StructureType::CommandBufferAllocateInfo,
            p_next: ptr::null(),
            command_pool: command_pool,
            level: vk::CommandBufferLevel::Primary,
            command_buffer_count: capacity as u32,
        };

        let command_buffers = queue.device.0
            .allocate_command_buffers(&info)
            .expect("Error on command buffer allocation"); // TODO: better error handling

        let command_buffers = command_buffers.into_iter().map(|buffer| {
            CommandBuffer {
                raw: buffer,
                device: queue.device.clone(),
            }
        }).collect();

        RawCommandPool {
            pool: command_pool,
            command_buffers: command_buffers,
            next_buffer: 0,
            device: queue.device.clone(),
        }
    }
}

pub struct SubpassCommandPool {
    pool: vk::CommandPool,
    command_buffers: Vec<SubpassCommandBuffer>,
    next_buffer: usize,
    device: Arc<RawDevice>,
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
