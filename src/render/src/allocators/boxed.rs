use std::marker::PhantomData;

use core::{Device as CoreDevice};
use core::device::ResourceHeapType;
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

    fn make_memory(&self, mut device: B::Device, heap: B::Heap) -> Memory {
        let mut heap = Some(heap);
        let release = Box::new(move || device.destroy_heap(heap.take().unwrap()));
        Memory::new(release, self.usage)
    }
}

impl<B: Backend> Allocator<B> for BoxedAllocator<B> {
    fn allocate_buffer(&mut self,
        device: &mut Device<B>,
        _: buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory) {
        let heap_type = device.find_usage_heap(self.usage).unwrap();
        let mut device = device.ref_raw().clone();
        let requirements = device.get_buffer_requirements(&buffer);
        let resource_type = ResourceHeapType::Buffers;
        let heap = device.create_heap(&heap_type, resource_type, requirements.size)
            .unwrap();
        let buffer = device.bind_buffer_memory(&heap, 0, buffer)
            .unwrap();
        
        (buffer, self.make_memory(device, heap))
    }
    
    fn allocate_image(&mut self,
        device: &mut Device<B>,
        usage: image::Usage,
        image: B::UnboundImage
    ) -> (B::Image, Memory) {
        let heap_type = device.find_usage_heap(self.usage).unwrap();
        let mut device = device.ref_raw().clone();
        let requirements = device.get_image_requirements(&image);
        let resource_type = if usage.can_target() {
            ResourceHeapType::Targets
        } else {
            ResourceHeapType::Images
        };
        let heap = device.create_heap(&heap_type, resource_type, requirements.size)
            .unwrap();
        let image = device.bind_image_memory(&heap, 0, image)
            .unwrap();
        
        (image, self.make_memory(device, heap))
    }
}
