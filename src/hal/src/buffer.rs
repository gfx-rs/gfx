//! Memory buffers

use std::error::Error;
use std::fmt;

use {IndexType, Backend};


/// An offset inside a buffer, in bytes.
pub type Offset = u64;

/// Error creating a buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CreationError {
    /// Required `Usage` is not supported.
    Usage(Usage),
    /// Some other problem.
    Other,
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            CreationError::Usage(usage) => write!(f, "{}: {:?}", description, usage),
            _ => write!(f, "{}", description)
        }
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::Usage(_) =>
                "Required `Usage` is not supported",
            CreationError::Other =>
                "Some other problem",
        }
    }
}

/// Error creating a `BufferView`.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewError {
    /// The required usage flag is not present in the image.
    Usage(Usage),
    /// The backend refused for some reason.
    Unsupported,
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            ViewError::Usage(usage) => write!(f, "{}: {:?}", description, usage),
            _ => write!(f, "{}", description)
        }
    }
}

impl Error for ViewError {
    fn description(&self) -> &str {
        match *self {
            ViewError::Usage(_) =>
                "The required usage flag is not present in the image",
            ViewError::Unsupported =>
                "The backend refused for some reason",
        }
    }
}

bitflags!(
    /// Buffer usage flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Usage: u16 {
        ///
        const TRANSFER_SRC  = 0x1;
        ///
        const TRANSFER_DST = 0x2;
        ///
        const UNIFORM = 0x4;
        ///
        const STORAGE = 0x8;
        ///
        const UNIFORM_TEXEL = 0x10;
        ///
        const STORAGE_TEXEL = 0x20;
        ///
        const INDEX = 0x40;
        ///
        const INDIRECT = 0x80;
        ///
        const VERTEX = 0x100;
    }
);

impl Usage {
    /// Can this buffer be used in transfer operations ?
    pub fn can_transfer(&self) -> bool {
        self.intersects(Usage::TRANSFER_SRC | Usage::TRANSFER_DST)
    }
}

bitflags!(
    /// Buffer state flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Access: u16 {
        ///
        const TRANSFER_READ          = 0x01;
        ///
        const TRANSFER_WRITE         = 0x02;
        ///
        const INDEX_BUFFER_READ      = 0x10;
        ///
        const VERTEX_BUFFER_READ     = 0x20;
        ///
        const CONSTANT_BUFFER_READ   = 0x40;
        ///
        const INDIRECT_COMMAND_READ  = 0x80;
        ///
        const SHADER_READ = 0x100;
        ///
        const SHADER_WRITE = 0x200;
        ///
        const HOST_READ = 0x400;
        ///
        const HOST_WRITE = 0x800;
        ///
        const MEMORY_READ = 0x1000;
        ///
        const MEMORY_WRITE = 0x2000;
    }
);

/// Buffer state
pub type State = Access;

/// Index buffer view for `bind_index_buffer`, slightly
/// analogous to an index table into an array.
pub struct IndexBufferView<'a, B: Backend> {
    /// The buffer to bind.
    pub buffer: &'a B::Buffer,
    /// The offset into the buffer to start at.
    pub offset: u64,
    /// The type of the table elements (`u16` or `u32`).
    pub index_type: IndexType,
}
