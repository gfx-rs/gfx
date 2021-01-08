use crate::{
    command::{self, Command, CommandBuffer},
    info, native as n, Backend,
};

use auxil::FastHashMap;
use parking_lot::Mutex;
use std::sync::Arc;

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
pub struct CommandPool {
    pub(crate) fbo: Option<n::RawFramebuffer>,
    pub(crate) limits: command::Limits,
    pub(crate) memory: Arc<Mutex<BufferMemory>>,
    pub(crate) legacy_features: info::LegacyFeatures,
}

impl hal::pool::CommandPool<Backend> for CommandPool {
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

    unsafe fn allocate_one(&mut self, _level: hal::command::Level) -> CommandBuffer {
        // TODO: Implement secondary buffers
        CommandBuffer::new(
            self.fbo,
            self.limits,
            self.memory.clone(),
            self.legacy_features,
        )
    }

    unsafe fn free<I>(&mut self, buffers: I)
    where
        I: IntoIterator<Item = CommandBuffer>,
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
                storage.remove(&buffer.data.id);
            }
        }
        // Linear: Freeing doesn't really matter here as everything is backed by
        //         only one Vec.
    }
}
