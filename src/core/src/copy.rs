//! # Copy routines
//!
//! This module provides reusable primitives for resources that need to
//! implement `Copy` for Vulkan portability.

use std::{fmt, ops};

/// A copyable range replacement for `std::ops::Range`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Range<T> {
    ///
    pub start: T,
    ///
    pub end: T,
}

impl<T> From<ops::Range<T>> for Range<T> {
    fn from(other: ops::Range<T>) -> Self {
        Range {
            start: other.start,
            end: other.end,
        }
    }
}


/// A horribly unsafe copyable mutable pointer.
pub struct Pointer<T>(*mut T);
unsafe impl<T> Send for Pointer<T> {}
unsafe impl<T> Sync for Pointer<T> {}
impl<T> Pointer<T> {
    #[doc(hidden)]
    pub unsafe fn new(ptr: *mut T) -> Self {
        Pointer(ptr)
    }
    #[doc(hidden)]
    pub unsafe fn as_mut(&self) -> &mut T {
        &mut *self.0
    }
}
impl<T> Clone for Pointer<T> {
    fn clone(&self) -> Self {
        Pointer(self.0)
    }
}
impl<T> Copy for Pointer<T> {}
impl<T> ops::Deref for Pointer<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0 }
    }
}
impl<T> ops::DerefMut for Pointer<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0 }
    }
}
impl<T> fmt::Debug for Pointer<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Pointer({:p})", self.0)
    }
}
