#![deny(missing_docs, missing_copy_implementations)]

//! Memory mapping

use std::error::Error as StdError;
use std::fmt;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{self, AtomicBool};
use Backend;
use {buffer, memory};

/// Unsafe, backend-provided operations for a buffer mapping
#[doc(hidden)]
pub trait Gate<B: Backend> {
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
pub struct Raw<B: Backend> {
    resource: UnsafeCell<B::Mapping>,
    accessible: AtomicBool,
}

#[doc(hidden)]
impl<B: Backend> Raw<B> {
    pub fn new(resource: B::Mapping) -> Self {
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

    pub unsafe fn use_access(&self) -> &mut B::Mapping {
        &mut *self.resource.get()
    }
}

unsafe impl<B: Backend> Sync for Raw<B> {}

#[derive(Debug)]
struct Guard<'a, B: Backend> {
    raw: &'a Raw<B>,
}

impl<'a, B: Backend> Guard<'a, B> {
    fn new(raw: &'a Raw<B>) -> Result<Self, Error> {
        unsafe {
            if raw.take_access() {
                Ok(Guard { raw: raw })
            } else {
                Err(Error::AccessOverlap)
            }
        }
    }
}

impl<'a, B: Backend> Deref for Guard<'a, B> {
    type Target = B::Mapping;
    fn deref(&self) -> &B::Mapping {
        unsafe { self.raw.use_access() }
    }
}

impl<'a, B: Backend> DerefMut for Guard<'a, B> {
    fn deref_mut(&mut self) -> &mut B::Mapping {
        unsafe { self.raw.use_access() }
    }
}

impl<'a, B: Backend> Drop for Guard<'a, B> {
    fn drop(&mut self) {
        unsafe { self.raw.release_access(); }
    }
}

fn take_access_checked<B>(access: memory::Access, buffer: &buffer::Raw<B>)
                          -> Result<Guard<B>, Error>
    where B: Backend
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
pub unsafe fn read<B, T, S>(buffer: &buffer::Raw<B>, sync: S)
                            -> Result<Reader<B, T>, Error>
    where B: Backend, T: Copy, S: FnOnce(&mut B::Mapping)
{
    let mut mapping = try!(take_access_checked(memory::READ, buffer));
    sync(&mut mapping);

    Ok(Reader {
        slice: mapping.slice(buffer.len::<T>()),
        mapping: mapping,
    })
}

#[doc(hidden)]
pub unsafe fn write<B, T, S>(buffer: &buffer::Raw<B>, sync: S)
                             -> Result<Writer<B, T>, Error>
    where B: Backend, T: Copy, S: FnOnce(&mut B::Mapping)
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
pub struct Reader<'a, B: Backend, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)] mapping: Guard<'a, B>,
}

impl<'a, B: Backend, T: 'a + Copy> Deref for Reader<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
#[derive(Debug)]
pub struct Writer<'a, B: Backend, T: 'a + Copy> {
    slice: &'a mut [T],
    #[allow(dead_code)] mapping: Guard<'a, B>,
}

impl<'a, B: Backend, T: 'a + Copy> Deref for Writer<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &*self.slice }
}

impl<'a, B: Backend, T: 'a + Copy> DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

/// A service struct that can be used by backends to track the mapping status
#[derive(Debug)]
#[doc(hidden)]
pub struct Status<B: Backend> {
    cpu_wrote: bool,
    gpu_access: Option<B::Fence>,
}

#[doc(hidden)]
impl<B: Backend> Status<B> {
    pub fn clean() -> Self {
        Status {
            cpu_wrote: false,
            gpu_access: None,
        }
    }

    pub fn cpu_access<F>(&mut self, wait_fence: F)
        where F: FnOnce(B::Fence)
    {
        self.gpu_access.take().map(wait_fence);
    }

    pub fn cpu_write_access<F>(&mut self, wait_fence: F)
        where F: FnOnce(B::Fence)
    {
        self.cpu_access(wait_fence);
        self.cpu_wrote = true;
    }

    pub fn gpu_access(&mut self, fence: B::Fence) {
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
