use std::sync::atomic::{self, AtomicBool, AtomicUsize};

use memory::Memory;

pub use core::buffer::{CreationError};
pub use core::buffer::{Usage,
    TRANSFER_SRC, TRANSFER_DST, CONSTANT, INDEX, INDIRECT, VERTEX
};

/// An information block that is immutable and associated to each buffer.
#[derive(Debug)]
pub struct Info {
    /// Usage
    pub usage: Usage,
    /// Memory
    pub memory: Memory,
    /// Size in bytes
    pub size: u64,
    /// Stride of a single element, in bytes. Only used for structured buffers
    /// that you use via shader resource / unordered access views.
    pub stride: u64,
    /// Exclusive access
    pub(crate) access: Access,
}

impl Info {
    pub(crate) fn new(usage: Usage, memory: Memory, size: u64, stride: u64)
        -> Self
    {
        let access = Access {
            cpu: AtomicBool::new(false),
            gpu: AtomicUsize::new(0),
        };
        Info { usage, memory, size, stride, access }
    }
}

#[derive(Debug)]
pub(crate) struct Access {
    cpu: AtomicBool,
    gpu: AtomicUsize,
}

impl Access {
    pub(crate) fn acquire_exclusive(&self) -> bool {
        if self.acquire_cpu() {
            if self.gpu.load(atomic::Ordering::Relaxed) == 0 {
                true
            } else {
                // Release before notifying of failure
                self.release_cpu();
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn release_exclusive(&self) {
        self.release_cpu()
    }

    pub(crate) fn acquire_cpu(&self) -> bool {
        !self.cpu.swap(true, atomic::Ordering::Acquire)
    }

    pub(crate) fn release_cpu(&self) {
        if cfg!(debug) {
            assert!(self.cpu.swap(false, atomic::Ordering::Release));
        } else {
            self.cpu.store(false, atomic::Ordering::Release);
        }
    }

    pub(crate) fn gpu_start(&self) {
        self.gpu.fetch_add(1, atomic::Ordering::Relaxed);
    }

    pub(crate) fn gpu_end(&self) {
        self.gpu.fetch_sub(1, atomic::Ordering::Relaxed);
    }
}
