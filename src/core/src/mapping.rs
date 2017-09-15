#![deny(missing_docs, missing_copy_implementations)]

//! Memory mapping
use std::{fmt, ops};
use std::error::Error as StdError;
use Backend;

// TODO
/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess,
    /// The requested mapping access overlaps with another.
    AccessOverlap,
    /// The requested mapping range is outside of the resource.
    OutOfBounds,
    ///
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
            AccessOverlap => "The requested mapping access overlaps with another",
            OutOfBounds => "The requested mapping range is outside of the resource",
            OutOfMemory => "Not enough physical or virtual memory",
        }
    }
}

/// Mapping reader
pub struct Reader<'a, B: Backend, T: 'a + Copy> {
    pub(crate) slice: &'a [T],
    pub(crate) _mapping: B::Mapping,
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Reader<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

/// Mapping writer.
/// Currently is not possible to make write-only slice so while it is technically possible
/// to read from Writer, it will lead to an undefined behavior. Please do not read from it.
pub struct Writer<'a, B: Backend, T: 'a + Copy> {
    pub(crate) slice: &'a mut [T],
    pub(crate) _mapping: B::Mapping,
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Writer<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

impl<'a, B: Backend, T: 'a + Copy> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}

