// Copyright 2014 The Gfx-rs Developers.
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

//! Traits for working with blobs of data.

use std::fmt;
use std::mem;

/// A trait that slice-like types implement.
pub trait Blob<T> {
    /// Get the address to the data this `Blob` stores.
    fn get_address(&self) -> *const T;
    /// Get the number of bytes in this blob.
    fn get_size(&self) -> uint;
}

/// Helper trait for casting &Blob
pub trait RefBlobCast<'a> {
    /// Cast the type the blob references
    fn cast<U>(self) -> &'a Blob<U>+'a;
}

/// Helper trait for casting Box<Blob>
pub trait BoxBlobCast {
    /// Cast the type the blob references
    fn cast<U>(self) -> Box<Blob<U> + Send>;
}

impl<'a, T> RefBlobCast<'a> for &'a Blob<T>+'a {
    fn cast<U>(self) -> &'a Blob<U>+'a {
        unsafe { mem::transmute(self) }
    }
}

impl<T> BoxBlobCast for Box<Blob<T> + Send> {
    fn cast<U>(self) -> Box<Blob<U> + Send> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: Send> Blob<T> for Vec<T> {
    fn get_address(&self) -> *const T {
        self.as_ptr()
    }

    fn get_size(&self) -> uint {
        self.len() * mem::size_of::<T>()
    }
}

impl<'a, T> Blob<T> for &'a [T] {
    fn get_address(&self) -> *const T {
        self.as_ptr()
    }

    fn get_size(&self) -> uint {
        self.len() * mem::size_of::<T>()
    }
}

impl<T> fmt::Show for Box<Blob<T> + Send> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blob({:#p}, {})", self.get_address(), self.get_size())
    }
}
