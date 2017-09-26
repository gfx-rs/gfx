use std::marker::PhantomData;

use core::{Device as CoreDevice};
use memory::{self, Allocator, Memory};
use {buffer, image};
use {Backend, Device};

pub struct BoxedAllocator<B: Backend> {
    usage: memory::Usage,
    phantom: PhantomData<B>
}

impl<B: Backend> BoxedAllocator<B> {
    pub fn new(usage: memory::Usage, _: &Device<B>) -> Self {
        BoxedAllocator {
            usage,
            phantom: PhantomData
        }
    }

    fn make_memory(&self, mut device: B::Device, memory: B::Memory) -> Memory {
        let mut memory = Some(memory);
        let release = Box::new(move || device.free_memory(memory.take().unwrap()));
        Memory::new(release, self.usage)
    }
}

impl<B: Backend> Allocator<B> for BoxedAllocator<B> {
    fn allocate_buffer(&mut self,
        device: &mut Device<B>,
        _: buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory) {
        let requirements = device.mut_raw().get_buffer_requirements(&buffer);
        let mem_type = device.find_usage_memory(self.usage, requirements.type_mask).unwrap();
        let mut device = device.ref_raw().clone();
        let memory = device.allocate_memory(&mem_type, requirements.size)
            .unwrap();
        let buffer = device.bind_buffer_memory(&memory, 0, buffer)
            .unwrap();

        (buffer, self.make_memory(device, memory))
    }

    fn allocate_image(&mut self,
        device: &mut Device<B>,
        usage: image::Usage,
        image: B::UnboundImage
    ) -> (B::Image, Memory) {
        let requirements = device.mut_raw().get_image_requirements(&image);
        let mem_type = device.find_usage_memory(self.usage, requirements.type_mask).unwrap();
        let mut device = device.ref_raw().clone();
        let memory = device.allocate_memory(&mem_type, requirements.size)
            .unwrap();
        let image = device.bind_image_memory(&memory, 0, image)
            .unwrap();

        (image, self.make_memory(device, memory))
    }
}
