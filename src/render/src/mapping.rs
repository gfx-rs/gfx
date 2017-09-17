use std::{fmt, ops};
use std::error::Error as StdError;

use {core, memory, buffer};
use {Backend, Device};

/// Error accessing a mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Error {
    /// The requested mapping access did not match the expected usage.
    InvalidAccess(memory::Access, memory::Usage),
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
            Core(ref c) => c.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            InvalidAccess(..) => "The requested access did not match the expected usage",
            Core(ref c) => c.description(),
        }
    }
}

pub struct Reader<'a, B: Backend, T: 'a> {
    pub(crate) inner: core::mapping::Reader<'a, B, T>,
    pub(crate) info: &'a buffer::Info,
}

impl<'a, B: Backend, T: 'a> ops::Deref for Reader<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &self.inner }
}

pub struct Writer<'a, B: Backend, T: 'a> {
    pub(crate) inner: core::mapping::Writer<'a, B, T>,
    pub(crate) info: &'a buffer::Info,
}

impl<'a, B: Backend, T: 'a> ops::Deref for Writer<'a, B, T> {
    type Target = [T];

    fn deref(&self) -> &[T] { &self.inner }
}

impl<'a, B: Backend, T: 'a> ops::DerefMut for Writer<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { &mut self.inner }
}


pub struct ReadScope<'a, B: Backend, T: 'a> {
    pub(crate) reader: Option<Reader<'a, B, T>>,
    pub(crate) device: &'a mut Device<B>,
}

impl<'a, B: Backend, T: 'a> ops::Deref for ReadScope<'a, B, T> {
    type Target = [T];
    fn deref(&self) -> &[T] { self.reader.as_ref().unwrap() }
}

impl<'a, B: Backend, T: 'a> Drop for ReadScope<'a, B, T> {
    fn drop(&mut self) {
        self.device.release_mapping_reader(self.reader.take().unwrap());
    }
}

pub struct WriteScope<'a, B: Backend, T: 'a> {
    pub(crate) writer: Option<Writer<'a, B, T>>,
    pub(crate) device: &'a mut Device<B>,
}

impl<'a, B: Backend, T: 'a> ops::Deref for WriteScope<'a, B, T> {
    type Target = [T];
    fn deref(&self) -> &[T] { self.writer.as_ref().unwrap() }
}

impl<'a, B: Backend, T: 'a> ops::DerefMut for WriteScope<'a, B, T> {
    fn deref_mut(&mut self) -> &mut [T] { self.writer.as_mut().unwrap() }
}

impl<'a, B: Backend, T: 'a> Drop for WriteScope<'a, B, T> {
    fn drop(&mut self) {
        self.device.release_mapping_writer(self.writer.take().unwrap());
    }
}
