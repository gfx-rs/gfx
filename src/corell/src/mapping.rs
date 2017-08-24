//! Memory mapping

use std::error::Error as StdError;
use std::fmt;
use std::ops::{Deref, DerefMut};
use Resources;

/// Error accessing a mapping.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping exceeds the resource size.
    OutOfBounds,
    /// The requested mapping access overlaps with another.
    AccessOverlap,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            OutOfBounds => write!(f, "{}", self.description()),
            AccessOverlap => write!(f, "{}", self.description()),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            OutOfBounds => "The requested mapping exceeds the resource size",
            AccessOverlap => "The requested mapping access overlaps with another",
        }
    }
}

/// Mapping reader
pub struct Reader<'a, R: Resources, T: 'a + Copy> {
    slice: &'a [T],
    #[allow(dead_code)] mapping: R::Mapping,
}

impl<'a, R: Resources, T: 'a + Copy> Reader<'a, R, T> {
    pub unsafe fn new(slice: &'a [T], mapping: R::Mapping) -> Self {
        Reader {
            slice: slice,
            mapping: mapping,
        }
    }
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
    #[allow(dead_code)] mapping: R::Mapping,
}

impl<'a, R: Resources, T: 'a + Copy> Writer<'a, R, T> {
    pub unsafe fn new(slice: &'a mut [T], mapping: R::Mapping) -> Self {
        Writer {
            slice: slice,
            mapping: mapping,
        }
    }
}

impl<'a, R: Resources, T: 'a + Copy> Deref for Writer<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { self.slice }
}

impl<'a, R: Resources, T: 'a + Copy> DerefMut for Writer<'a, R, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.slice }
}
