//! Memory buffers.
//!
//! # Buffer
//!
//! Buffers interpret memory slices as linear contiguous data array.
//! They can be used as shader resources, vertex buffers, index buffers or for
//! specifying the action commands for indirect execution.

pub use bal::buffer::Offset;
use {format, Backend, IndexType};

/// Buffer state.
pub type State = Access;

/// Error creating a buffer.
#[derive(Fail, Debug, Clone, PartialEq, Eq)]
pub enum CreationError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Host memory allocation failed.")]
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Device memory allocation failed.")]
    OutOfDeviceMemory,
    /// Requested buffer usage is not supported.
    ///
    /// Older GL version don't support constant buffers or multiple usage flags.
    #[fail(display = "Buffer usage unsupported ({:?}).", usage)]
    UnsupportedUsage {
        /// Unsupported usage passed on buffer creation.
        usage: Usage,
    },
}

/// Error creating a buffer view.
#[derive(Fail, Debug, Clone, PartialEq, Eq)]
pub enum ViewCreationError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Host memory allocation failed.")]
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Device memory allocation failed.")]
    OutOfDeviceMemory,
    /// Buffer view format is not supported.
    #[fail(display = "Buffer view format unsupported ({:?}).", format)]
    UnsupportedFormat {
        /// Unsupported format passed on view creation.
        format: Option<format::Format>,
    },
}

bitflags!(
    /// Buffer usage flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Usage: u32 {
        ///
        const TRANSFER_SRC  = 0x1;
        ///
        const TRANSFER_DST = 0x2;
        ///
        const UNIFORM_TEXEL = 0x4;
        ///
        const STORAGE_TEXEL = 0x8;
        ///
        const UNIFORM = 0x10;
        ///
        const STORAGE = 0x20;
        ///
        const INDEX = 0x40;
        ///
        const VERTEX = 0x80;
        ///
        const INDIRECT = 0x100;
    }
);

impl Usage {
    /// Returns if the buffer can be used in transfer operations.
    pub fn can_transfer(&self) -> bool {
        self.intersects(Usage::TRANSFER_SRC | Usage::TRANSFER_DST)
    }
}

bitflags!(
    /// Buffer access flags.
    ///
    /// Access of buffers by the pipeline or shaders.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Access: u32 {
        /// Read commands instruction for indirect execution.
        const INDIRECT_COMMAND_READ = 0x1;
        /// Read index values for indexed draw commands.
        ///
        /// See [`draw_indexed`](../command/trait.RawCommandBuffer.html#tymethod.draw_indexed)
        /// and [`draw_indexed_indirect`](../command/trait.RawCommandBuffer.html#tymethod.draw_indexed_indirect).
        const INDEX_BUFFER_READ = 0x2;
        /// Read vertices from vertex buffer for draw commands in the [`VERTEX_INPUT`](
        /// ../pso/struct.PipelineStage.html#associatedconstant.VERTEX_INPUT) stage.
        const VERTEX_BUFFER_READ = 0x4;
        ///
        const CONSTANT_BUFFER_READ = 0x8;
        ///
        const SHADER_READ = 0x20;
        ///
        const SHADER_WRITE = 0x40;
        ///
        const TRANSFER_READ = 0x800;
        ///
        const TRANSFER_WRITE = 0x1000;
        ///
        const HOST_READ = 0x2000;
        ///
        const HOST_WRITE = 0x4000;
        ///
        const MEMORY_READ = 0x8000;
        ///
        const MEMORY_WRITE = 0x10000;
    }
);

/// Index buffer view for `bind_index_buffer`.
///
/// Defines a buffer slice used for acquiring the indices on draw commands.
/// Indices are used to lookup vertex indices in the vertex buffers.
pub struct IndexBufferView<'a, B: Backend> {
    /// The buffer to bind.
    pub buffer: &'a B::Buffer,
    /// The offset into the buffer to start at.
    pub offset: u64,
    /// The type of the table elements (`u16` or `u32`).
    pub index_type: IndexType,
}
