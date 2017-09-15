use std::sync::atomic::{self, AtomicBool};

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
    pub(crate) access: AtomicBool,
}

impl Info {
    pub(crate) fn new(usage: Usage, memory: Memory, size: u64, stride: u64)
        -> Self
    {
        let access = AtomicBool::new(false);
        Info { usage, memory, size, stride, access }
    }

    pub(crate) fn acquire_access(&self) -> bool {
        !self.access.swap(true, atomic::Ordering::Acquire)
    }

    pub(crate) fn release_access(&self) {
        if cfg!(debug) {
            assert!(self.access.swap(false, atomic::Ordering::Release));
        } else {
            self.access.store(false, atomic::Ordering::Release);
        }
    }
}
