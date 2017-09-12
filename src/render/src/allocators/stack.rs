use std::sync::mpsc;

use core::{self, Device as CoreDevice};
use core::device::ResourceHeapType;
use core::memory::Requirements;
use memory::{self, Allocator, Memory, ReleaseFn, Provider, Dependency};
use {buffer, image};
use {Backend, Device};

pub struct StackAllocator<B: Backend>(Provider<InnerStackAllocator<B>>);

pub struct InnerStackAllocator<B: Backend> {
    device: B::Device,
    usage: memory::Usage,
    // TODO: Any support ?
    buffers: ChunkStack<B>,
    images: ChunkStack<B>,
    targets: ChunkStack<B>,
    chunk_size: u64,
}

impl<B: Backend> Drop for InnerStackAllocator<B> {
    fn drop(&mut self) {
        self.shrink();
    }
}

impl<B: Backend> StackAllocator<B> {
    pub fn new(usage: memory::Usage, device: &Device<B>) -> Self {
        let mega = 1 << 20;
        Self::with_chunk_size(usage, device, 128 * mega)
    }

    pub fn with_chunk_size(
        usage: memory::Usage,
        device: &Device<B>,
        chunk_size: u64
    ) -> Self {
        StackAllocator(Provider::new(InnerStackAllocator {
            device: (*device.ref_raw()).clone(),
            usage,
            buffers: ChunkStack::new(ResourceHeapType::Buffers),
            images: ChunkStack::new(ResourceHeapType::Images),
            targets: ChunkStack::new(ResourceHeapType::Targets),
            chunk_size,
        }))
    }

    pub fn shrink(&mut self) {
        self.0.shrink();
    }
}

impl<B: Backend> InnerStackAllocator<B> {
    fn shrink(&mut self) {
        self.buffers.shrink(&mut self.device);
        self.images.shrink(&mut self.device);
        self.targets.shrink(&mut self.device);
    }
}

impl<B: Backend> Allocator<B> for StackAllocator<B> {
    fn allocate_buffer(&mut self,
        device: &mut Device<B>,
        usage: buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory) {
        let dependency = self.0.dependency();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let requirements = core::buffer::complete_requirements::<B>(
            device.mut_raw(), &buffer, usage);
        let (heap, offset, release) = inner.buffers.allocate(
            device,
            inner.usage,
            inner.chunk_size,
            requirements,
            dependency,
        );
        println!("bind buffer memory to {:?}, offset {:?}", heap, offset);
        let buffer = device.mut_raw().bind_buffer_memory(heap, offset, buffer)
            .unwrap();
        (buffer, Memory::new(release, inner.usage))
    }
    
    fn allocate_image(&mut self,
        device: &mut Device<B>,
        usage: image::Usage,
        image: B::UnboundImage
    ) -> (B::Image, Memory) {
        let dependency = self.0.dependency();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let requirements = device.mut_raw().get_image_requirements(&image);
        let stack = if usage.can_target() {
            &mut inner.targets
        } else {
            &mut inner.images
        };
        let (heap, offset, release) = stack.allocate(
            device,
            inner.usage,
            inner.chunk_size,
            requirements,
            dependency,
        );
        println!("bind image memory to {:?}, offset {:?}", heap, offset);
        let image = device.mut_raw().bind_image_memory(heap, offset, image)
            .unwrap();
        (image, Memory::new(release, inner.usage))
    }
}

struct ChunkStack<B: Backend> {
    resource_type: ResourceHeapType,
    chunks: Vec<B::Heap>,
    allocs: Vec<StackAlloc>,
    receiver: mpsc::Receiver<usize>,
    sender: mpsc::Sender<usize>,
}

struct StackAlloc {
    chunk_index: usize,
    end: u64,
    released: bool,
}

impl<B: Backend> ChunkStack<B> {
    fn new(resource_type: ResourceHeapType) -> Self {
        let (sender, receiver) = mpsc::channel();

        ChunkStack {
            resource_type,
            chunks: Vec::new(),
            allocs: Vec::new(),
            receiver,
            sender,
        }
    }

    fn allocate(&mut self,
        device: &mut Device<B>,
        usage: memory::Usage,
        chunk_size: u64,
        req: Requirements,
        dependency: Dependency<InnerStackAllocator<B>>,
    ) -> (&B::Heap, u64, ReleaseFn)
    {
        self.update_allocs();
        assert!(req.size <= chunk_size);

        let (chunk_index, beg, end) =
            if let Some(tail) = self.allocs.last() {
                let rem = tail.end % req.alignment;
                let beg = if rem == 0 {
                    tail.end
                } else {
                    tail.end - rem + req.alignment
                };
                let end = beg + req.size;
                if end <= chunk_size {
                    (tail.chunk_index, beg, end)
                } else {
                    (tail.chunk_index + 1, 0, req.size)
                }
            } else {
                (0, 0, req.size)
            };

        if chunk_index == self.chunks.len() {
            self.grow(device, usage, chunk_size);
        }

        let alloc_index = self.allocs.len();
        self.allocs.push(StackAlloc {
            chunk_index,
            end,
            released: false,
        });

        println!("allocated #{:?} {:?}..{:?}) from chunk {:?}", alloc_index, beg, end, chunk_index);
        let sender = self.sender.clone();
        (&self.chunks[chunk_index], beg, Box::new(move || {
            let _ = dependency;
            sender.send(alloc_index).unwrap_or_else(|_| {
                error!("could not release StackAllocator's memory")
            });
        }))
    }

    fn grow(&mut self,
        device: &mut Device<B>,
        usage: memory::Usage,
        chunk_size: u64
    ) {
        let heap_type = device.find_usage_heap(usage).unwrap();
        println!("create chunk of {} bytes on {:?} ({:?})", chunk_size, heap_type, self.resource_type);
        let heap = device.mut_raw()
            .create_heap(&heap_type, self.resource_type, chunk_size)
            .unwrap();
        self.chunks.push(heap);
    }

    fn shrink(&mut self, device: &mut B::Device) {
        self.update_allocs();

        let drain_beg = self.allocs.last()
            .map(|a| a.chunk_index + 1)
            .unwrap_or(0);

        for heap in self.chunks.drain(drain_beg..) {
            println!("destroy chunk of {:?}", self.resource_type);
            device.destroy_heap(heap);
        }
    }
    
    fn update_allocs(&mut self) {
        for alloc_index in self.receiver.try_iter() {
            self.allocs[alloc_index].released = true;
        }
        while self.allocs.last().map(|a| a.released).unwrap_or(false) {
            self.allocs.pop();
        }
    }
}
