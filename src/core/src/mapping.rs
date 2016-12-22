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
    if buffer.mapping().is_some() { Err(Error::AlreadyMapped) }
    else { Ok(()) }
}

#[derive(Debug)]
#[doc(hidden)]
pub struct Status<R: Resources> {
    pub cpu_write: bool,
    pub gpu_access: Option<handle::Fence<R>>,
}

impl<R: Resources> Status<R> {
    fn clean() -> Self {
        Status {
            cpu_write: false,
            gpu_access: None,
        }
    }

    fn access<F>(&mut self, wait_fence: F)
        where F: FnOnce(handle::Fence<R>)
    {
        self.gpu_access.take().map(wait_fence);
    }

    fn write_access<F>(&mut self, wait_fence: F)
        where F: FnOnce(handle::Fence<R>)
    {
        self.access(wait_fence);
        self.cpu_write = true;
    }
}

/// Error mapping a buffer.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
    /// The memory was already mapped
    AlreadyMapped,
}

#[derive(Debug)]
#[doc(hidden)]
pub struct RawInner<R: Resources> {
    pub resource: R::Mapping,
    pub buffer: handle::RawBuffer<R>,
    pub access: memory::Access,
    pub status: Status<R>,
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
    pub fn new<F>(access: memory::Access, buffer: &handle::RawBuffer<R>, f: F) -> Result<Self, Error>
        where F: FnOnce() -> R::Mapping
    {
        try!(is_ok(access, buffer));
        Ok(Raw(Mutex::new(RawInner {
            resource: f(),
            buffer: buffer.clone(),
            access: access,
            status: Status::clean(),
        })))
    }

    #[doc(hidden)]
    pub fn access(&self) -> Option<MutexGuard<RawInner<R>>> {
        self.0.try_lock().ok()
    }

    unsafe fn read<T: Copy, F, H>(&self,
                                  len: usize,
                                  wait_fence: F,
                                  hook: H) -> Reader<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        hook(&mut inner);
        inner.status.access(wait_fence);

        Reader {
            slice: inner.resource.slice(len),
            inner: inner,
        }
    }

    unsafe fn write<T: Copy, F, H>(&self,
                                   len: usize,
                                   wait_fence: F,
                                   hook: H) -> Writer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        hook(&mut inner);
        inner.status.write_access(wait_fence);

        Writer {
            len: len,
            inner: inner,
            phantom: PhantomData,
        }
    }

    unsafe fn read_write<T: Copy, F, H>(&self,
                                        len: usize,
                                        wait_fence: F,
                                        hook: H) -> RWer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        let mut inner = self.access().unwrap();
        hook(&mut inner);
        inner.status.write_access(wait_fence);

        RWer {
            slice: inner.resource.mut_slice(len),
            inner: inner,
        }
    }
}

/// Mapping reader
pub struct Reader<'a, R: Resources, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)] inner: MutexGuard<'a, RawInner<R>>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Reader<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer
pub struct Writer<'a, R: Resources, T: 'a + Copy> {
    len: usize,
    inner: MutexGuard<'a, RawInner<R>>,
    phantom: PhantomData<T>,
}

impl<'a, R: Resources, T: 'a + Copy> Writer<'a, R, T> {
    /// Set a value in the buffer
    pub fn set(&mut self, index: usize, value: T) {
        if index >= self.len {
            panic!("tried to write out of bounds of a mapped buffer");
        }
        unsafe { self.inner.resource.set(index, value); }
    }
}

/// Mapping reader & writer
pub struct RWer<'a, R: Resources, T: 'a + Copy> {
    slice: &'a mut [T],
    #[allow(dead_code)] inner: MutexGuard<'a, RawInner<R>>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for RWer<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &*self.slice }
}

impl<'a, R: Resources, T: 'a + Copy> DerefMut for RWer<'a, R, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

/// Readable mapping.
pub trait Readable<R: Resources, T: Copy> {
    #[doc(hidden)]
    unsafe fn read<F, H>(&mut self,
                         wait_fence: F,
                         hook: H) -> Reader<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>);
}

/// Writable mapping.
pub trait Writable<R: Resources, T: Copy> {
    #[doc(hidden)]
    unsafe fn write<F, H>(&mut self,
                          wait_fence: F,
                          hook: H) -> Writer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>);
}

/// Readable only mapping.
pub struct ReadableOnly<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>,
}

impl<R: Resources, T: Copy> Readable<R, T> for ReadableOnly<R, T> {
    unsafe fn read<F, H>(&mut self,
                         wait_fence: F,
                         hook: H) -> Reader<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        self.raw.read(self.len, wait_fence, hook)
    }
}

/// Writable only mapping.
pub struct WritableOnly<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>,
}

impl<R: Resources, T: Copy> Writable<R, T> for WritableOnly<R, T> {
    unsafe fn write<F, H>(&mut self,
                          wait_fence: F,
                          hook: H) -> Writer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        self.raw.write(self.len, wait_fence, hook)
    }
}

/// Readable & writable mapping.
pub struct RWable<R: Resources, T: Copy> {
    raw: handle::RawMapping<R>,
    len: usize,
    phantom: PhantomData<T>
}

impl<R: Resources, T: Copy> RWable<R, T> {
    #[doc(hidden)]
    pub unsafe fn read_write<F, H>(&mut self,
                                   wait_fence: F,
                                   hook: H) -> RWer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        self.raw.read_write(self.len, wait_fence, hook)
    }
}

impl<R: Resources, T: Copy> Readable<R, T> for RWable<R, T> {
    unsafe fn read<F, H>(&mut self,
                         wait_fence: F,
                         hook: H) -> Reader<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        self.raw.read(self.len, wait_fence, hook)
    }
}

impl<R: Resources, T: Copy> Writable<R, T> for RWable<R, T> {
    unsafe fn write<F, H>(&mut self,
                          wait_fence: F,
                          hook: H) -> Writer<R, T>
        where F: FnOnce(handle::Fence<R>),
              H: FnOnce(&mut RawInner<R>)
    {
        self.raw.write(self.len, wait_fence, hook)
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
    fn map_readable<T: Copy>(&mut self, raw: handle::RawMapping<R>, len: usize) -> ReadableOnly<R, T> {
        ReadableOnly {
            raw: raw,
            len: len,
            phantom: PhantomData,
        }
    }

    fn map_writable<T: Copy>(&mut self, raw: handle::RawMapping<R>, len: usize) -> WritableOnly<R, T> {
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
