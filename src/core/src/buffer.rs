//! Memory buffers

use std::error::Error;
use std::{mem, fmt, cmp, hash};
use {memory, mapping};
use {IndexType, Backend};

/// Untyped buffer
#[derive(Debug)]
pub struct Raw<B: Backend> {
    resource: B::Buffer,
    info: Info,
    mapping: Option<mapping::Raw<B>>,
}

impl<B: Backend> Raw<B> {
    #[doc(hidden)]
    pub fn new(resource: B::Buffer,
               info: Info,
               mapping: Option<B::Mapping>) -> Self {
        Raw {
            resource: resource,
            info: info,
            mapping: mapping.map(|m| mapping::Raw::new(m)),
        }
    }

    #[doc(hidden)]
    pub fn resource(&self) -> &B::Buffer { &self.resource }

    /// Get buffer info
    pub fn get_info(&self) -> &Info { &self.info }

    /// Is this buffer mapped ?
    pub fn is_mapped(&self) -> bool {
        self.mapping.is_some()
    }

    /// Set the mapping
    pub fn map(&mut self, m: B::Mapping) {
        assert!(!self.is_mapped());
        self.mapping = Some(mapping::Raw::new(m))
    }

    #[doc(hidden)]
    pub fn mapping(&self) -> Option<&mapping::Raw<B>> {
        self.mapping.as_ref()
    }

    /// Get the number of elements in the buffer.
    ///
    /// Fails if `T` is zero-sized.
    #[doc(hidden)]
    pub unsafe fn len<T>(&self) -> usize {
        assert!(mem::size_of::<T>() != 0, "Cannot determine the length of zero-sized buffers.");
        self.get_info().size / mem::size_of::<T>()
    }
}

impl<B: Backend + cmp::PartialEq> cmp::PartialEq for Raw<B> {
    fn eq(&self, other: &Self) -> bool {
        self.resource().eq(other.resource())
    }
}

impl<B: Backend + cmp::Eq> cmp::Eq for Raw<B> {}

impl<B: Backend + hash::Hash> hash::Hash for Raw<B> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.resource().hash(state);
    }
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

/// An information block that is immutable and associated to each buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Info {
    /// Role
    pub role: Role,
    /// Usage hint
    pub usage: memory::Usage,
    /// Bind flags
    pub bind: memory::Bind,
    /// Size in bytes
    pub size: usize,
    /// Stride of a single element, in bytes. Only used for structured buffers
    /// that you use via shader resource / unordered access views.
    pub stride: usize,
}

/// Error creating a buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CreationError {
    /// Some of the bind flags are not supported.
    UnsupportedBind(memory::Bind),
    /// Unknown other error.
    Other,
    /// Usage mode is not supported
    UnsupportedUsage(memory::Usage),
    // TODO: unsupported role
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreationError::UnsupportedBind(ref bind) => write!(f, "{}: {:?}", self.description(), bind),
            CreationError::UnsupportedUsage(usage) => write!(f, "{}: {:?}", self.description(), usage),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::UnsupportedBind(_) => "Bind flags are not supported",
            CreationError::Other => "An unknown error occurred",
            CreationError::UnsupportedUsage(_) => "Requested memory usage mode is not supported",
        }
    }
}

/// Index buffer view for `bind_index_buffer`.
pub struct IndexBufferView<'a, B: Backend> {
    ///
    pub buffer: &'a B::Buffer,
    ///
    pub offset: u64,
    ///
    pub index_type: IndexType,
}

bitflags!(
    /// Buffer usage flags.
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
