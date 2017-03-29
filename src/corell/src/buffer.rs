// Copyright 2017 The Gfx-rs Developers.
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

use std::error::Error;
use std::fmt;

use {IndexType, Resources};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CreationError { }

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        "Could not create buffer on device."
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
