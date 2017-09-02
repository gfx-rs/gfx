use core::memory;

/// An information block that is immutable and associated to each buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Info {
    /// Role
    pub role: Role,
    /// Usage
    pub usage: memory::Usage,
    /// Bind flags
    pub bind: memory::Bind,
    /// Size in bytes
    pub size: usize,
    // /// Stride of a single element, in bytes. Only used for structured buffers
    // /// that you use via shader resource / unordered access views.
    // pub stride: usize,
    // TODO: mapping stuff
}

/// Role of the memory buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Role {
    /// Generic vertex buffer
    Vertex,
    /// Index buffer
    Index,
    /// Constant buffer
    Constant,
    /// Staging buffer
    Staging,
}
