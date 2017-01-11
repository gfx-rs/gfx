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

use std::ops::{Deref, DerefMut};
use std::sync::MutexGuard;
use Resources;
use {memory, buffer, handle};

/// Unsafe, backend-provided operations for a buffer mapping
#[doc(hidden)]
pub trait Gate<R: Resources> {
    /// Set the element at `index` to `val`. Not bounds-checked.
    unsafe fn set<T>(&self, index: usize, val: T);
    /// Returns a slice of the specified length.
    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T];
    /// Returns a mutable slice of the specified length.
    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T];
}

/// Error accessing a mapping.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
}

fn valid_access(access: memory::Access, usage: memory::Usage) -> Result<(), Error> {
    use memory::Usage::*;
    match usage {
        Upload if access == memory::WRITE => Ok(()),
        Download if access == memory::READ => Ok(()),
        _ => Err(Error::InvalidAccess(access, usage)),
    }
}

#[doc(hidden)]
pub unsafe fn read<R, T, S>(buffer: &buffer::Raw<R>, sync: S)
                            -> Result<Reader<R, T>, Error>
    where R: Resources, T: Copy, S: FnOnce(&mut R::Mapping)
{
    try!(valid_access(memory::READ, buffer.get_info().usage));
    let mut mapping = buffer.lock_mapping().unwrap();
    sync(&mut mapping);

    Ok(Reader {
        slice: mapping.slice(buffer.len::<T>()),
        mapping: mapping,
    })
}

#[doc(hidden)]
pub unsafe fn write<R, T, S>(buffer: &buffer::Raw<R>, sync: S)
                             -> Result<Writer<R, T>, Error>
    where R: Resources, T: Copy, S: FnOnce(&mut R::Mapping)
{
    try!(valid_access(memory::WRITE, buffer.get_info().usage));
    let mut mapping = buffer.lock_mapping().unwrap();
    sync(&mut mapping);

    Ok(Writer {
        slice: mapping.mut_slice(buffer.len::<T>()),
        mapping: mapping,
    })
}

/// Mapping reader
pub struct Reader<'a, R: Resources, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)] mapping: MutexGuard<'a, R::Mapping>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Reader<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
pub struct Writer<'a, R: Resources, T: 'a + Copy> {
    slice: &'a mut [T],
    #[allow(dead_code)] mapping: MutexGuard<'a, R::Mapping>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Writer<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &*self.slice }
}

impl<'a, R: Resources, T: 'a + Copy> DerefMut for Writer<'a, R, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

#[derive(Debug, PartialEq, Eq, Hash)]
#[doc(hidden)]
/// A service struct that can be used by backends to track the mapping status
pub struct Status<R: Resources> {
    cpu_wrote: bool,
    gpu_access: Option<handle::Fence<R>>,
}

#[doc(hidden)]
impl<R: Resources> Status<R> {
    pub fn clean() -> Self {
        Status {
            cpu_wrote: false,
            gpu_access: None,
        }
    }

    pub fn cpu_access<F>(&mut self, wait_fence: F)
        where F: FnOnce(handle::Fence<R>)
    {
        self.gpu_access.take().map(wait_fence);
    }

    pub fn cpu_write_access<F>(&mut self, wait_fence: F)
        where F: FnOnce(handle::Fence<R>)
    {
        self.cpu_access(wait_fence);
        self.cpu_wrote = true;
    }

    pub fn gpu_access(&mut self, fence: handle::Fence<R>) {
        self.gpu_access = Some(fence);
    }

    pub fn ensure_flushed<F>(&mut self, flush: F)
        where F: FnOnce()
    {
        if self.cpu_wrote {
            flush();
            self.cpu_wrote = false;
        }
    }
}
