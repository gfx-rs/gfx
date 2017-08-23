// Copyright 2015 The Gfx-rs Developers.
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

#![deny(missing_docs, missing_copy_implementations)]

//! Memory mapping

use std::{fmt, ops};
use std::error::Error as StdError;
use Backend;
use memory;


/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
    /// The requested mapping access overlaps with another.
    AccessOverlap,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            InvalidAccess(ref access, ref usage) => {
                write!(f, "{}: access = {:?}, usage = {:?}", self.description(), access, usage)
            }
            AccessOverlap => write!(f, "{}", self.description())
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            InvalidAccess(..) => "The requested mapping access did not match the expected usage",
            AccessOverlap => "The requested mapping access overlaps with another"
        }
    }
}

/// Mapping reader
pub struct Reader<'a, B: Backend, T: 'a + Copy> {
    slice: &'a [T],
    _mapping: B::Mapping,
}

impl<'a, B: Backend, T: 'a + Copy> Reader<'a, B, T> {
    /// Create a new mapping reader
    pub unsafe fn new(slice: &'a [T], mapping: B::Mapping) -> Self {
        Reader {
            slice,
            _mapping: mapping,
        }
    }
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Reader<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
pub struct Writer<'a, B: Backend, T: 'a + Copy> {
    slice: &'a mut [T],
    _mapping: B::Mapping,
}

impl<'a, B: Backend, T: 'a + Copy> Writer<'a, B, T> {
    /// Create a new mapping reader
    pub unsafe fn new(slice: &'a mut [T], mapping: B::Mapping) -> Self {
        Writer {
            slice,
            _mapping: mapping,
        }
    }
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Writer<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

impl<'a, B: Backend, T: 'a + Copy> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

