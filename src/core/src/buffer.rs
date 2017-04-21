// Copyright 2016 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Memory buffers

use std::error::Error;
use std::{mem, fmt, cmp, hash};
use {memory, mapping};
use Resources;

/// Untyped buffer
#[derive(Debug)]
pub struct Raw<R: Resources> {
    resource: R::Buffer,
    info: Info,
    mapping: Option<mapping::Raw<R>>,
}

impl<R: Resources> Raw<R> {
    #[doc(hidden)]
    pub fn new(resource: R::Buffer,
               info: Info,
               mapping: Option<R::Mapping>) -> Self {
        Raw {
            resource: resource,
            info: info,
            mapping: mapping.map(|m| mapping::Raw::new(m)),
        }
    }

    #[doc(hidden)]
    pub fn resource(&self) -> &R::Buffer { &self.resource }

    /// Get buffer info
    pub fn get_info(&self) -> &Info { &self.info }

    /// Is this buffer mapped ?
    pub fn is_mapped(&self) -> bool {
        self.mapping.is_some()
    }

    #[doc(hidden)]
    pub fn mapping(&self) -> Option<&mapping::Raw<R>> {
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

impl<R: Resources + cmp::PartialEq> cmp::PartialEq for Raw<R> {
    fn eq(&self, other: &Self) -> bool {
        self.resource().eq(other.resource())
    }
}

impl<R: Resources + cmp::Eq> cmp::Eq for Raw<R> {}

impl<R: Resources + hash::Hash> hash::Hash for Raw<R> {
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
