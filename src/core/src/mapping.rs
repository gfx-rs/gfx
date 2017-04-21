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

use std::error::Error as StdError;
use std::fmt;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{self, AtomicBool};
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

#[doc(hidden)]
#[derive(Debug)]
pub struct Raw<R: Resources> {
    resource: UnsafeCell<R::Mapping>,
    accessible: AtomicBool,
}

#[doc(hidden)]
impl<R: Resources> Raw<R> {
    pub fn new(resource: R::Mapping) -> Self {
        Raw {
            resource: UnsafeCell::new(resource),
            accessible: AtomicBool::new(true),
        }
    }

    pub unsafe fn take_access(&self) -> bool {
        self.accessible.swap(false, atomic::Ordering::Relaxed)
    }

    pub unsafe fn release_access(&self) {
        if cfg!(debug) {
            assert!(self.accessible.swap(true, atomic::Ordering::Relaxed) == false);
        } else {
            self.accessible.store(true, atomic::Ordering::Relaxed)
        }
    }

    pub unsafe fn use_access(&self) -> &mut R::Mapping {
        &mut *self.resource.get()
    }
}

unsafe impl<R: Resources> Sync for Raw<R> {}

#[derive(Debug)]
struct Guard<'a, R: Resources> {
    raw: &'a Raw<R>,
}

impl<'a, R: Resources> Guard<'a, R> {
    fn new(raw: &'a Raw<R>) -> Result<Self, Error> {
        unsafe {
            if raw.take_access() {
                Ok(Guard { raw: raw })
            } else {
                Err(Error::AccessOverlap)
            }
        }
    }
}

impl<'a, R: Resources> Deref for Guard<'a, R> {
    type Target = R::Mapping;
    fn deref(&self) -> &R::Mapping {
        unsafe { self.raw.use_access() }
    }
}

impl<'a, R: Resources> DerefMut for Guard<'a, R> {
    fn deref_mut(&mut self) -> &mut R::Mapping {
        unsafe { self.raw.use_access() }
    }
}

impl<'a, R: Resources> Drop for Guard<'a, R> {
    fn drop(&mut self) {
        unsafe { self.raw.release_access(); }
    }
}

fn take_access_checked<R>(access: memory::Access, buffer: &buffer::Raw<R>)
                          -> Result<Guard<R>, Error>
    where R: Resources
{
    let usage = buffer.get_info().usage;
    use memory::Usage::*;
    match usage {
        Upload if access == memory::WRITE => (),
        Download if access == memory::READ => (),
        _ => return Err(Error::InvalidAccess(access, usage)),
    }

    Guard::new(buffer.mapping().unwrap())
}

#[doc(hidden)]
pub unsafe fn read<R, T, S>(buffer: &buffer::Raw<R>, sync: S)
                            -> Result<Reader<R, T>, Error>
    where R: Resources, T: Copy, S: FnOnce(&mut R::Mapping)
{
    let mut mapping = try!(take_access_checked(memory::READ, buffer));
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
    let mut mapping = try!(take_access_checked(memory::WRITE, buffer));
    sync(&mut mapping);

    Ok(Writer {
        slice: mapping.mut_slice(buffer.len::<T>()),
        mapping: mapping,
    })
}

/// Mapping reader
#[derive(Debug)]
pub struct Reader<'a, R: Resources, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)] mapping: Guard<'a, R>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Reader<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
#[derive(Debug)]
pub struct Writer<'a, R: Resources, T: 'a + Copy> {
    slice: &'a mut [T],
    #[allow(dead_code)] mapping: Guard<'a, R>,
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Writer<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &*self.slice }
}

impl<'a, R: Resources, T: 'a + Copy> DerefMut for Writer<'a, R, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

/// A service struct that can be used by backends to track the mapping status
#[derive(Debug, Eq, Hash, PartialEq)]
#[doc(hidden)]
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
