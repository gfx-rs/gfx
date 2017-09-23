//! Memory buffers
use memory;
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
        const CONSTANT = 0x4,
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
        ///
        const SHADER_READ = 0x100,
        ///
        const SHADER_WRITE = 0x200,
        ///
        const HOST_READ = 0x400,
        ///
        const HOST_WRITE = 0x800,
        ///
        const MEMORY_READ = 0x1000,
        ///
        const MEMORY_WRITE = 0x2000,
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

/// Retrieve the complete memory requirements for this buffer,
/// taking usage and device limits into account
pub fn complete_requirements<B: Backend>(
    device: &mut B::Device,
    buffer: &B::UnboundBuffer,
    usage: Usage,
) -> memory::Requirements {
    use std::cmp::max;
    use device::Device;

    let mut requirements = device.get_buffer_requirements(buffer);
    if usage.can_transfer() {
        let limits = device.get_limits();
        requirements.alignment = max(
            limits.min_buffer_copy_offset_alignment as u64,
            requirements.alignment);
    }
    requirements
}
