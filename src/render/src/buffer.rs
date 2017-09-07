use std::fmt;
use std::error::Error;

use memory;

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
    /// Stride of a single element, in bytes. Only used for structured buffers
    /// that you use via shader resource / unordered access views.
    pub stride: usize,
    // TODO: do we need things from buffer::Usage ?
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
