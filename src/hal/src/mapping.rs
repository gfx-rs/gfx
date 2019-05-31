#![deny(missing_docs, missing_copy_implementations)]

//! Memory mapping
use crate::device;
use crate::Backend;
use std::ops::{self, Range};

// TODO
/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Fail)]
pub enum Error {
    /// Out of either host or device memory.
    #[fail(display = "{}", _0)]
    OutOfMemory(device::OutOfMemory),
    /// The requested mapping access did not match the expected usage.
    #[fail(display = "The requested mapping access did not match the expected usage")]
    InvalidAccess,
    /// The requested mapping range is outside of the resource.
    #[fail(display = "The requested mapping range is outside of the resource")]
    OutOfBounds,
    /// Failed to map memory range.
    #[fail(display = "Unable to allocate an appropriately sized contiguous virtual address")]
    MappingFailed,
}

impl From<device::OutOfMemory> for Error {
    fn from(error: device::OutOfMemory) -> Self {
        Error::OutOfMemory(error)
    }
}

/// Mapping reader
#[derive(Debug)]
pub struct Reader<'a, B: Backend, T: 'a> {
    pub(crate) slice: &'a [T],
    pub(crate) memory: &'a B::Memory,
    pub(crate) released: bool,
}

impl<'a, B: Backend, T: 'a> Drop for Reader<'a, B, T> {
    fn drop(&mut self) {
        assert!(self.released, "a mapping reader was not released");
    }
}

impl<'a, B: Backend, T: 'a> ops::Deref for Reader<'a, B, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.slice
    }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
#[derive(Debug)]
pub struct Writer<'a, B: Backend, T: 'a> {
    pub(crate) slice: &'a mut [T],
    pub(crate) memory: &'a B::Memory,
    pub(crate) range: Range<u64>,
    pub(crate) released: bool,
}

impl<'a, B: Backend, T: 'a> Drop for Writer<'a, B, T> {
    fn drop(&mut self) {
        assert!(self.released, "a mapping writer was not released");
    }
}

impl<'a, B: Backend, T: 'a> ops::Deref for Writer<'a, B, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.slice
    }
}

impl<'a, B: Backend, T: 'a> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.slice
    }
}
