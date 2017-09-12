use std::sync::mpsc;
use std::collections::HashMap;

use core::{Device as CoreDevice, HeapType};
use core::device::ResourceHeapType;
use core::memory::Requirements;
use memory::{self, Allocator, Memory, ReleaseFn, DropDelayed, DropDelayer};
use {Backend, Device};

pub struct StackAllocator<B: Backend>(DropDelayed<InnerStackAllocator<B>>);

pub struct InnerStackAllocator<B: Backend> {
    device: B::Device,
    heap_chunks: HashMap<memory::Usage, HeapChunks<B>>,
    chunk_size: u64,
}

impl<B: Backend> Drop for InnerStackAllocator<B> {
    fn drop(&mut self) {
        self.shrink();
    }
}

struct HeapChunks<B: Backend> {
    // TODO: Any support ?
    buffers: ChunkStack<B>,
    images: ChunkStack<B>,
    targets: ChunkStack<B>,
}

impl<B: Backend> HeapChunks<B> {
    fn new(heap_type: HeapType) -> Self {
        HeapChunks {
            buffers: ChunkStack::new(heap_type, ResourceHeapType::Buffers),
            images: ChunkStack::new(heap_type, ResourceHeapType::Images),
            targets: ChunkStack::new(heap_type, ResourceHeapType::Targets),
        }
    }
}

impl<B: Backend> StackAllocator<B> {
    pub fn new(device: &Device<B>) -> Self {
        let mega = 1 << 20;
        Self::with_chunk_size(device, 128 * mega)
    }

    pub fn with_chunk_size(device: &Device<B>, chunk_size: u64) -> Self {
        let mut heap_chunks = HashMap::new();
        if let Some(data_heap) = device.find_data_heap() {
            heap_chunks.insert(memory::Usage::Data, HeapChunks::new(data_heap));
        }
        if let Some(upload_heap) = device.find_upload_heap() {
            heap_chunks.insert(memory::Usage::Upload, HeapChunks::new(upload_heap));
        }
        if let Some(download_heap) = device.find_download_heap() {
            heap_chunks.insert(memory::Usage::Download, HeapChunks::new(download_heap));
        }
        StackAllocator(DropDelayed::new(InnerStackAllocator {
            device: (*device.ref_raw()).clone(),
            heap_chunks,
            chunk_size,
        }))
    }

    pub fn shrink(&mut self) {
        self.0.shrink();
    }
}

impl<B: Backend> InnerStackAllocator<B> {
    fn shrink(&mut self) {
        let device = &mut self.device;
        for (_, chunks) in &mut self.heap_chunks {
            chunks.buffers.shrink(device);
            chunks.images.shrink(device);
            chunks.targets.shrink(device);
        }
    }
}

impl<B: Backend> Allocator<B> for StackAllocator<B> {
    fn allocate_buffer(&mut self,
        _: &mut Device<B>,
        usage: memory::Usage,
        bind: memory::Bind,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory) {
        let drop_delayer = self.0.drop_delayer();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let device = &mut inner.device;
        let requirements = device.get_buffer_requirements(&buffer);
        let stack = &mut inner.heap_chunks.get_mut(&usage).unwrap().buffers;
        let (heap, offset, release) = stack.allocate(
            device,
            inner.chunk_size,
            requirements,
            drop_delayer,
        );
        println!("bind buffer memory to {:?}, offset {:?}", heap, offset);
        let buffer = device.bind_buffer_memory(heap, offset, buffer)
            .unwrap();
        (buffer, Memory::new(release, usage, bind))
    }
    
    fn allocate_image(&mut self,
        _: &mut Device<B>,
        usage: memory::Usage,
        bind: memory::Bind,
        image: B::UnboundImage
    ) -> (B::Image, Memory) {
        let drop_delayer = self.0.drop_delayer();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let device = &mut inner.device;
        let requirements = device.get_image_requirements(&image);
        let chunks = inner.heap_chunks.get_mut(&usage).unwrap();
        let stack = if bind.is_target() {
            &mut chunks.targets
        } else {
            &mut chunks.images
        };
        let (heap, offset, release) = stack.allocate(
            device,
            inner.chunk_size,
            requirements,
            drop_delayer,
        );
        println!("bind image memory to {:?}, offset {:?}", heap, offset);
        let image = device.bind_image_memory(heap, offset, image)
            .unwrap();
        (image, Memory::new(release, usage, bind))
    }
}

struct ChunkStack<B: Backend> {
    heap_type: HeapType,
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
    fn new(heap_type: HeapType, resource_type: ResourceHeapType) -> Self {
        let (sender, receiver) = mpsc::channel();

        ChunkStack {
            heap_type,
            resource_type,
            chunks: Vec::new(),
            allocs: Vec::new(),
            receiver,
            sender,
        }
    }

    fn allocate(&mut self,
        device: &mut B::Device,
        chunk_size: u64,
        req: Requirements,
        drop_delayer: DropDelayer<InnerStackAllocator<B>>,
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
            self.grow(device, chunk_size);
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
            let _ = drop_delayer;
            sender.send(alloc_index).unwrap_or_else(|_| {
                error!("could not release StackAllocator's memory")
            });
        }))
    }

    fn grow(&mut self, device: &mut B::Device, chunk_size: u64) {
        println!("create chunk of {} bytes on {:?} ({:?})", chunk_size, self.heap_type, self.resource_type);
        let heap = device.create_heap(&self.heap_type, self.resource_type, chunk_size)
            .unwrap();
        self.chunks.push(heap);
    }

    fn shrink(&mut self, device: &mut B::Device) {
        self.update_allocs();

        let drain_beg = self.allocs.last()
            .map(|a| a.chunk_index + 1)
            .unwrap_or(0);

        for heap in self.chunks.drain(drain_beg..) {
            println!("destroy chunk on {:?} ({:?})", self.heap_type, self.resource_type);
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
