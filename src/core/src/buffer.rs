//! Memory buffers

use {IndexType, Backend};

// TODO
/// Error creating a buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CreationError;

bitflags!(
    /// Buffer usage flags.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags Usage: u16 {
        ///
        const TRANSFER_SRC  = 0x1,
        ///
        const TRANSFER_DST = 0x2,
        ///
        const CONSTANT    = 0x4,
        ///
        const INDEX = 0x8,
        ///
        const INDIRECT = 0x10,
        ///
        const VERTEX = 0x20,
    }
);

impl Usage {
    /// Can this buffer be used in transfer operations ?
    pub fn can_transfer(&self) -> bool {
        self.intersects(TRANSFER_SRC | TRANSFER_DST)
    }
}

bitflags!(
    /// Buffer state flags.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags Access: u16 {
        ///
        const TRANSFER_READ          = 0x01,
        ///
        const TRANSFER_WRITE         = 0x02,
        ///
        const INDEX_BUFFER_READ      = 0x10,
        ///
        const VERTEX_BUFFER_READ     = 0x20,
        ///
        const CONSTANT_BUFFER_READ   = 0x40,
        ///
        const INDIRECT_COMMAND_READ  = 0x80,
    }
);

/// Buffer state
pub type State = Access;

/// Index buffer view for `bind_index_buffer`.
pub struct IndexBufferView<'a, B: Backend> {
    ///
    pub buffer: &'a B::Buffer,
    ///
    pub offset: u64,
    ///
    pub index_type: IndexType,
}
