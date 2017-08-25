use core::{self, pool};
use core::command::{Buffer, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, Encoder};
use core::queue::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
use command::{self, RawCommandBuffer, SubpassCommandBuffer};
use {Backend, CommandQueue, CommandList, Resources};

pub struct RawCommandPool {
    command_buffers: Vec<RawCommandBuffer<CommandList>>,
    next_buffer: usize,
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        self.next_buffer = 0;
        for cb in &mut self.command_buffers {
            cb.reset();
        }
    }

    fn reserve(&mut self, additional: usize) {
        for _ in 0..additional {
            self.command_buffers.push(CommandList::new().into());
        }
    }

    unsafe fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
        where Q: AsRef<CommandQueue>
    {
        let buffers = (0..capacity).map(|_| CommandList::new().into())
                                    .collect();
        RawCommandPool {
            command_buffers: buffers,
            next_buffer: 0,
        }
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut RawCommandBuffer<CommandList> {
        let available_buffers = self.command_buffers.len() as isize - self.next_buffer as isize;
        if available_buffers <= 0 {
            self.reserve((-available_buffers) as usize + 1);
        }

        let buffer = &mut self.command_buffers[self.next_buffer];
        self.next_buffer += 1;
        buffer
    }
}

pub struct SubpassCommandPool {
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {
    /*
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }

    fn from_queue<'a, Q>(mut _queue: Q, capacity: usize) -> Self
        where Q: Compatible<GraphicsQueue<Backend>> + AsRef<CommandQueue>
    {
        unimplemented!()
    }

    fn acquire_command_buffer<'a>(&'a mut self) -> Encoder<'a, Backend, SubpassCommandBuffer<CommandList>> {
        unimplemented!()
    }
    */
}
