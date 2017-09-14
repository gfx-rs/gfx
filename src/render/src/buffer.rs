use memory::Memory;
use mapping;

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
    /// Mapping informations
    pub mapping: Option<mapping::Info>,
}
