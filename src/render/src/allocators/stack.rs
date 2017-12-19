use std::sync::{mpsc, Arc};
use std::collections::HashMap;

use hal::{self, MemoryType, Device as Device_, Limits};
use hal::memory::Requirements;
use memory::{self, Allocator, Memory, ReleaseFn, Provider, Dependency};
use {buffer, image};
use {Backend, Device};

/// Retrieve the complete memory requirements for this buffer,
/// taking usage and device limits into account
pub fn complete_requirements<B: Backend>(
    device: &B::Device,
    buffer: &B::UnboundBuffer,
    limits: &Limits,
    usage: buffer::Usage,
) -> Requirements {
    use std::cmp::max;

    let mut requirements = device.get_buffer_requirements(buffer);
    if usage.can_transfer() {
        requirements.alignment = max(
            limits.min_buffer_copy_offset_alignment as u64,
            requirements.alignment);
    }
    requirements
}


pub struct StackAllocator<B: Backend>(Provider<InnerStackAllocator<B>>);

pub struct InnerStackAllocator<B: Backend> {
    device: Arc<B::Device>,
    usage: memory::Usage,
    limits: Limits, // TODO: only store relevant data
    // stacks by memory type
    // TODO: VecMap ?
    stacks: HashMap<usize, ChunkStack<B>>,
    chunk_size: u64,
}

impl<B: Backend> Drop for InnerStackAllocator<B> {
    fn drop(&mut self) {
        self.shrink();
    }
}

impl<B: Backend> StackAllocator<B> {
    pub fn new(usage: memory::Usage, device: &Device<B>, limits: Limits) -> Self {
        let mega = 1 << 20;
        Self::with_chunk_size(usage, device, limits, 128 * mega)
    }

    pub fn with_chunk_size(
        usage: memory::Usage,
        device: &Device<B>,
        limits: Limits,
        chunk_size: u64
    ) -> Self {
        StackAllocator(Provider::new(InnerStackAllocator {
            device: Arc::clone(&device.raw),
            usage,
            limits,
            stacks: HashMap::new(),
            chunk_size,
        }))
    }

    pub fn shrink(&mut self) {
        self.0.shrink();
    }
}

impl<B: Backend> InnerStackAllocator<B> {
    fn shrink(&mut self) {
        for (_, stack) in &mut self.stacks {
            stack.shrink(&self.device);
        }
    }
}

impl<B: Backend> Allocator<B> for StackAllocator<B> {
    fn allocate_buffer(&mut self,
        device: &Device<B>,
        usage: buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory) {
        let dependency = self.0.dependency();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let requirements = complete_requirements::<B>(
            &device.raw,
            &buffer,
            &inner.limits,
            usage,
        );
        let memory_type = device.find_usage_memory(inner.usage, requirements.type_mask)
            .expect("could not find suitable memory");
        let stack = inner.stacks.entry(memory_type.id)
            .or_insert_with(|| ChunkStack::new(memory_type));
        let (memory, offset, release) = stack.allocate(
            device,
            inner.chunk_size,
            requirements,
            dependency,
        );
        let buffer = device.raw
            .bind_buffer_memory(memory, offset, buffer)
            .unwrap();
        (buffer, Memory::new(release, inner.usage))
    }

    fn allocate_image(&mut self,
        device: &mut Device<B>,
        _: image::Usage,
        image: B::UnboundImage
    ) -> (B::Image, Memory) {
        let dependency = self.0.dependency();
        let inner: &mut InnerStackAllocator<B> = &mut self.0;
        let requirements = device.raw.get_image_requirements(&image);
        let memory_type = device.find_usage_memory(inner.usage, requirements.type_mask)
            .expect("could not find suitable memory");
        let stack = inner.stacks.entry(memory_type.id)
            .or_insert_with(|| ChunkStack::new(memory_type));
        let (memory, offset, release) = stack.allocate(
            device,
            inner.chunk_size,
            requirements,
            dependency,
        );
        let image = device.raw
            .bind_image_memory(memory, offset, image)
            .unwrap();
        (image, Memory::new(release, inner.usage))
    }
}

struct ChunkStack<B: Backend> {
    memory_type: MemoryType,
    chunks: Vec<B::Memory>,
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
    fn new(memory_type: MemoryType) -> Self {
        let (sender, receiver) = mpsc::channel();

        ChunkStack {
            memory_type,
            chunks: Vec::new(),
            allocs: Vec::new(),
            receiver,
            sender,
        }
    }

    fn allocate(&mut self,
        device: &Device<B>,
        chunk_size: u64,
        req: Requirements,
        dependency: Dependency<InnerStackAllocator<B>>,
    ) -> (&B::Memory, u64, ReleaseFn)
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

        let sender = self.sender.clone();
        (&self.chunks[chunk_index], beg, Box::new(move || {
            let _ = dependency;
            sender.send(alloc_index).unwrap_or_else(|_| {
                error!("could not release StackAllocator's memory")
            });
        }))
    }

    fn grow(&mut self,
        device: &Device<B>,
        chunk_size: u64,
    ) {
        let memory = device.raw
            .allocate_memory(&self.memory_type, chunk_size)
            .unwrap();
        self.chunks.push(memory);
    }

    fn shrink(&mut self, device: &B::Device) {
        self.update_allocs();

        let drain_beg = self.allocs.last()
            .map(|a| a.chunk_index + 1)
            .unwrap_or(0);

        for memory in self.chunks.drain(drain_beg..) {
            device.free_memory(memory);
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
