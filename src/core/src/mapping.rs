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

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};
use {Resources, Factory};
use {memory, handle};

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

fn valid_access(access: memory::Access, usage: memory::Usage) -> Result<(), Error> {
    use memory::Usage::*;
    match usage {
        Mappable(a) if a.contains(access) => Ok(()),
        _ => Err(Error::InvalidAccess(access, usage)),
    }
}

/// Would mapping this buffer with this memory access be an error ?
fn is_ok<R: Resources>(access: memory::Access, buffer: &handle::RawBuffer<R>) -> Result<(), Error> {
    try!(valid_access(access, buffer.get_info().usage));
    if buffer.mapping().is_some() {
        Err(Error::AlreadyMapped)
    } else {
        Ok(())
    }
}

/// Error mapping a buffer.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
    /// The memory was already mapped
    AlreadyMapped,
    /// Desired mapping access not supported for the current backend.
    Unsupported,
}

#[derive(Debug)]
#[doc(hidden)]
pub struct RawInner<R: Resources> {
    pub resource: R::Mapping,
    pub buffer: handle::RawBuffer<R>,
    pub access: memory::Access,
}

impl<R: Resources> Drop for RawInner<R> {
    fn drop(&mut self) {
        self.buffer.was_unmapped();
    }
}

/// Raw mapping providing status tracking
#[derive(Debug)]
pub struct Raw<R: Resources>(Mutex<RawInner<R>>);

impl<R: Resources> Raw<R> {
    #[doc(hidden)]
    pub fn new<F>(access: memory::Access,
                  buffer: &handle::RawBuffer<R>,
                  f: F)
                  -> Result<Self, Error>
        where F: FnOnce() -> R::Mapping
    {
        try!(is_ok(access, buffer));
        Ok(Raw(Mutex::new(RawInner {
            resource: f(),
            buffer: buffer.clone(),
            access: access,
        })))
    }

    #[doc(hidden)]
    pub fn access(&self) -> Option<MutexGuard<RawInner<R>>> {
        self.0.try_lock().ok()
    }

    unsafe fn read<T: Copy, S>(&self, len: usize, sync: S) -> Reader<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        sync(&mut inner);

        Reader {
            slice: inner.resource.slice(len),
            inner: inner,
        }
    }

    unsafe fn write<T: Copy, S>(&self, len: usize, sync: S) -> Writer<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        sync(&mut inner);

        Writer {
            slice: inner.resource.mut_slice(len),
            inner: inner,
        }
    }

    unsafe fn read_write<T: Copy, S>(&self, len: usize, sync: S) -> RWer<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        sync(&mut inner);

        RWer {
            slice: inner.resource.mut_slice(len),
            inner: inner,
        }
    }
}

/// Mapping reader
pub struct Reader<'a, R: Resources, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)]
    inner: MutexGuard<'a, RawInner<R>>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Reader<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.slice
    }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
pub type Writer<'a, R, T> = RWer<'a, R, T>;

/// Mapping reader & writer
pub struct RWer<'a, R: Resources, T: 'a + Copy> {
    slice: &'a mut [T],
    #[allow(dead_code)]
    inner: MutexGuard<'a, RawInner<R>>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for RWer<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &*self.slice
    }
}

impl<'a, R: Resources, T: 'a + Copy> DerefMut for RWer<'a, R, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.slice
    }
}

/// Readable mapping.
pub trait Readable<R: Resources, T: Copy> {
    #[doc(hidden)]
    unsafe fn read<S>(&mut self, sync: S) -> Reader<R, T> where S: FnOnce(&mut RawInner<R>);
}

/// Writable mapping.
pub trait Writable<R: Resources, T: Copy> {
    #[doc(hidden)]
    unsafe fn write<S>(&mut self, sync: S) -> Writer<R, T> where S: FnOnce(&mut RawInner<R>);
}

/// Readable only mapping.
pub struct ReadableOnly<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>,
}

impl<R: Resources, T: Copy> Readable<R, T> for ReadableOnly<R, T> {
    unsafe fn read<S>(&mut self, sync: S) -> Reader<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        self.raw.read(self.len, sync)
    }
}

/// Writable only mapping.
pub struct WritableOnly<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>,
}

impl<R: Resources, T: Copy> Writable<R, T> for WritableOnly<R, T> {
    unsafe fn write<S>(&mut self, sync: S) -> Writer<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        self.raw.write(self.len, sync)
    }
}

/// Readable & writable mapping.
pub struct RWable<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>,
}

impl<R: Resources, T: Copy> RWable<R, T> {
    #[doc(hidden)]
    pub unsafe fn read_write<S>(&mut self, sync: S) -> RWer<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        self.raw.read_write(self.len, sync)
    }
}

impl<R: Resources, T: Copy> Readable<R, T> for RWable<R, T> {
    unsafe fn read<S>(&mut self, sync: S) -> Reader<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        self.raw.read(self.len, sync)
    }
}

impl<R: Resources, T: Copy> Writable<R, T> for RWable<R, T> {
    unsafe fn write<S>(&mut self, sync: S) -> Writer<R, T>
        where S: FnOnce(&mut RawInner<R>)
    {
        self.raw.write(self.len, sync)
    }
}

/// A service trait with methods for mapping already implemented.
/// To be used by device back ends.
#[doc(hidden)]
pub trait Builder<R: Resources>: Factory<R> {
    fn map_readable<T: Copy>(&mut self, handle::RawMapping<R>, usize) -> ReadableOnly<R, T>;
    fn map_writable<T: Copy>(&mut self, handle::RawMapping<R>, usize) -> WritableOnly<R, T>;
    fn map_read_write<T: Copy>(&mut self, handle::RawMapping<R>, usize) -> RWable<R, T>;
}

impl<R: Resources, F: Factory<R>> Builder<R> for F {
    fn map_readable<T: Copy>(&mut self,
                             raw: handle::RawMapping<R>,
                             len: usize)
                             -> ReadableOnly<R, T> {
        ReadableOnly {
            raw: raw,
            len: len,
            phantom: PhantomData,
        }
    }

    fn map_writable<T: Copy>(&mut self,
                             raw: handle::RawMapping<R>,
                             len: usize)
                             -> WritableOnly<R, T> {
        WritableOnly {
            raw: raw,
            len: len,
            phantom: PhantomData,
        }
    }

    fn map_read_write<T: Copy>(&mut self, raw: handle::RawMapping<R>, len: usize) -> RWable<R, T> {
        RWable {
            raw: raw,
            len: len,
            phantom: PhantomData,
        }
    }
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
