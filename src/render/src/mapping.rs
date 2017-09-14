use std::{fmt, ops};
use std::error::Error as StdError;

use {core, memory, buffer};
use Backend;

/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
    /// The requested mapping access overlaps with another.
    AccessOverlap,
    /// Another error reported by GFX's core
    Core(core::mapping::Error)
}

impl From<core::mapping::Error> for Error {
    fn from(c: core::mapping::Error) -> Self {
        Error::Core(c)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            InvalidAccess(ref access, ref usage) => {
                write!(f, "{}: access = {:?}, usage = {:?}", self.description(), access, usage)
            }
            AccessOverlap => write!(f, "{}", self.description()),
            Core(ref c) => c.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            InvalidAccess(..) => "The requested access did not match the expected usage",
            AccessOverlap => "The requested access overlaps with another",
            Core(ref c) => c.description(),
        }
    }
}

pub struct Reader<'a, B: Backend, T: 'a + Copy> {
    pub(crate) inner: core::mapping::Reader<'a, B, T>,
    pub(crate) info: &'a buffer::Info,
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Reader<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &self.inner }
}

pub struct Writer<'a, B: Backend, T: 'a + Copy> {
    pub(crate) inner: core::mapping::Writer<'a, B, T>,
    pub(crate) info: &'a buffer::Info,
}

impl<'a, B: Backend, T: 'a + Copy> ops::Deref for Writer<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &self.inner }
}

impl<'a, B: Backend, T: 'a + Copy> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { &mut self.inner }
}

// TODO
#[derive(Debug)]
pub struct Info {
    // cpu_access: AtomicBool,
    // gpu_access: AtomicBool,
}
