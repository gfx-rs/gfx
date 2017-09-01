use core::{self, pool};
use command::{self, Command, RawCommandBuffer, SubpassCommandBuffer};
use native as n;
use {Backend, CommandQueue, Share};
use gl;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

fn create_fbo_internal(gl: &gl::Gl) -> gl::types::GLuint {
    let mut name = 0 as n::FrameBuffer;
    unsafe {
        gl.GenFramebuffers(1, &mut name);
    }
    info!("\tCreated frame buffer {}", name);
    name
}

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
}

// Shared command buffer/pool memory.
pub type SharedBuffer = UnsafeCell<OwnedBuffer>;

// Storage of command buffer memory.
// Depends on the reset model chosen when creating the command pool.
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
    // Reseting the pool will free all data and commands recorded. Therefore it's
    // crucial that all submits have been finished **before** calling `reset`.
    Linear(Arc<SharedBuffer>),
    // Storing the memory for each command buffer separately to allow individual
    // command buffer resets.
    Individual {
        storage: HashMap<u64, Arc<SharedBuffer>>,
        next_buffer_id: u64,
    },
}

impl BufferMemory {

}

pub struct RawCommandPool {
    fbo: n::FrameBuffer,
    limits: command::Limits,
    memory: BufferMemory,
    // Indicate if the pool or the associated memory can be accessed.
    // Trying to prevent errors due to non-API conformant usage.
    accessible: Arc<AtomicBool>,
}

unsafe impl Send for RawCommandPool {}

impl RawCommandPool {
    fn take_access(&self) -> bool {
        self.accessible.swap(false, atomic::Ordering::SeqCst)
    }

    fn release_access(&self) {
        self.accessible.store(true, atomic::Ordering::SeqCst)
    }
}

impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        let clear = |shared: &SharedBuffer| {
            if self.take_access() {
                let mut buf = unsafe { &mut *shared.get() };
                buf.commands.clear();
                buf.data.clear();
                self.release_access();
            } else {
                error!("Trying to reset command pool while in access!");
            }
        };

        match self.memory {
            BufferMemory::Linear(ref shared) => {
                clear(shared);
            }
            BufferMemory::Individual { ref storage, .. } => {
                for (_, ref mut shared) in storage {
                    clear(shared);
                }
            }
        }
    }

    unsafe fn from_queue(
        mut queue: &CommandQueue,
        flags: pool::CommandPoolCreateFlags,
    ) -> Self {
        let fbo = create_fbo_internal(&queue.share.context);
        let limits = queue.share.limits.into();
        let memory = if flags.contains(pool::RESET_INDIVIDUAL) {
            BufferMemory::Individual {
                storage: HashMap::new(),
                next_buffer_id: 0,
            }
        } else {
            BufferMemory::Linear(Arc::new(UnsafeCell::new(OwnedBuffer::new())))
        };

        // Ignoring `TRANSIENT` hint, unsure how to make use of this.

        RawCommandPool {
            fbo,
            limits,
            memory,
            accessible: Arc::new(AtomicBool::new(true)),
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<RawCommandBuffer> {
        if self.take_access() {
            let buffers =
                (0..num).map(|_|
                        RawCommandBuffer::new(
                            self.fbo,
                            self.limits,
                            &mut self.memory,
                            self.accessible.clone()))
                        .collect();
            self.release_access();
            buffers
        } else {
            error!("Trying to allocate command buffers while in access!");
            Vec::new()
        }
    }

    unsafe fn free(&mut self, buffers: Vec<RawCommandBuffer>) {
        if self.take_access() {
            if let BufferMemory::Individual { ref mut storage, .. } = self.memory {
                // Expecting that the buffers actually are allocated from this pool.
                for buffer in buffers {
                    storage.remove(&buffer.id);
                }
            }
            // Linear: Freeing doesn't really matter here as everything is backed by
            //         only one Vec.

            self.release_access();
        } else {
            error!("Trying to free command buffers while in access!")
        }
    }
}

pub struct SubpassCommandPool {
    command_buffers: Vec<SubpassCommandBuffer>,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool { }
