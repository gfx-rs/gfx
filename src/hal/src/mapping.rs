#![deny(missing_docs, missing_copy_implementations)]

//! Memory mapping
use std::error::Error as StdError;
use std::fmt;
use std::ops::{self, Range};
use Backend;

// TODO
/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess,
    /// The requested mapping range is outside of the resource.
    OutOfBounds,
    /// There is not enough memory to provide the requested mapping.
    OutOfMemory,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            InvalidAccess => "The requested mapping access did not match the expected usage",
            OutOfBounds => "The requested mapping range is outside of the resource",
            OutOfMemory => "Not enough physical or virtual memory",
        }
    }
}

/// Mapping reader
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
    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
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
    fn deref(&self) -> &[T] { self.slice }
}

impl<'a, B: Backend, T: 'a> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}
