use {Backend, CommandQueue};
use command::RawCommandBuffer;
use core::pool;

pub struct RawCommandPool {
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        unimplemented!()
    }

    fn reserve(&mut self, additional: usize) {
        unimplemented!()
    }

    unsafe fn from_queue<Q>(mut queue: Q, capacity: usize) -> RawCommandPool
    where Q: AsRef<CommandQueue>
    {
        unimplemented!()
    }

    unsafe fn acquire_command_buffer(&mut self) -> &mut RawCommandBuffer {
        unimplemented!()
    }
}

pub struct SubpassCommandPool;
impl pool::SubpassCommandPool<Backend> for SubpassCommandPool {

}