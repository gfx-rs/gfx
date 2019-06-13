use crate::command::{self, Command, RawCommandBuffer};
use crate::hal::backend::FastHashMap;
use crate::hal::{self, pool};
use crate::native as n;
use crate::Backend;

use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct OwnedBuffer {
    pub(crate) commands: Vec<Command>,
    pub(crate) data: Vec<u8>,
}

impl OwnedBuffer {
    pub fn new() -> Self {
        OwnedBuffer {
            commands: Vec::new(),
            data: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.commands.clear();
        self.data.clear();
    }
}

// Storage of command buffer memory.
// Depends on the reset model chosen when creating the command pool.
#[derive(Debug)]
pub enum BufferMemory {
    // Storing all recorded commands and data in the pool in a linear
    // piece of memory shared by all associated command buffers.
    //
    // # Safety!
    //
    // This implementation heavily relays on the fact that the user **must**
    // ensure that only **one** associated command buffer from each pool
    // is recorded at the same time. Additionally, we only allow to reset the
    // whole command pool. This allows us to avoid fragmentation of the memory
    // and saves us additional bookkeeping overhead for keeping track of all
    // allocated buffers.
    //
    // Resetting the pool will free all data and commands recorded. Therefore it's
    // crucial that all submits have been finished **before** calling `reset`.
    Linear(OwnedBuffer),
    // Storing the memory for each command buffer separately to allow individual
    // command buffer resets.
    Individual {
        storage: FastHashMap<u64, OwnedBuffer>,
        next_buffer_id: u64,
    },
}

#[derive(Debug)]
pub struct RawCommandPool {
    pub(crate) fbo: Option<n::FrameBuffer>,
    pub(crate) limits: command::Limits,
    pub(crate) memory: Arc<Mutex<BufferMemory>>,
}

impl pool::RawCommandPool<Backend> for RawCommandPool {
    unsafe fn reset(&mut self, _release_resources: bool) {
        let mut memory = self
            .memory
            .try_lock()
            .expect("Trying to reset command pool, while memory is still in-use.");

        match *memory {
            BufferMemory::Linear(ref mut buffer) => {
                buffer.clear();
            }
            BufferMemory::Individual {
                ref mut storage, ..
            } => {
                for (_, ref mut buffer) in storage {
                    buffer.clear();
                }
            }
        }
    }

    fn allocate_one(&mut self, _level: hal::command::RawLevel) -> RawCommandBuffer {
        // TODO: Implement secondary buffers
        RawCommandBuffer::new(self.fbo, self.limits, self.memory.clone())
    }

    unsafe fn free<I>(&mut self, buffers: I)
    where
        I: IntoIterator<Item = RawCommandBuffer>,
    {
        let mut memory = self
            .memory
            .try_lock()
            .expect("Trying to free command buffers, while memory is still in-use.");

        if let BufferMemory::Individual {
            ref mut storage, ..
        } = *memory
        {
            // Expecting that the buffers actually are allocated from this pool.
            for buffer in buffers {
                storage.remove(&buffer.id);
            }
        }
        // Linear: Freeing doesn't really matter here as everything is backed by
        //         only one Vec.
    }
}
