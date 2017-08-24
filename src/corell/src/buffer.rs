use std::error::Error;
use std::fmt;

use {IndexType, Resources};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CreationError {
    OutOfHeap,
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::OutOfHeap => "Not enough space in the heap.",
        }
    }
}

bitflags!(
    /// Buffer usage flags.
    pub flags Usage: u16 {
        const TRANSFER_SRC  = 0x1,
        const TRANSFER_DST = 0x2,
        const CONSTANT    = 0x4,
        const INDEX = 0x8,
        const INDIRECT = 0x10,
        const VERTEX = 0x20,
    }
);

/// Index buffer view for `bind_index_buffer`.
pub struct IndexBufferView<'a, R: Resources> {
    pub buffer: &'a R::Buffer,
    pub offset: u64,
    pub index_type: IndexType,
}
