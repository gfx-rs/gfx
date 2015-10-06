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
use super::{Resources, Factory};

/// Unsafe operations for a buffer mapping
pub trait Raw {
    /// Set the element at `index` to `val`. Not bounds-checked.
    unsafe fn set<T>(&self, index: usize, val: T);
    /// Returns a slice of the specified length.
    unsafe fn to_slice<T>(&self, len: usize) -> &[T];
    /// Returns a mutable slice of the specified length.
    unsafe fn to_mut_slice<T>(&self, len: usize) -> &mut [T];
}

/// A handle to a readable map, which can be sliced.
pub struct Readable<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> where
    F::Mapper: 'a
{
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Deref for Readable<'a, T, R, F> where
    F::Mapper: 'a,
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for Readable<'a, T, R, F> where
    F::Mapper: 'a,
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a writable map, which only allows setting elements.
pub struct Writable<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> where
    F::Mapper: 'a
{
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Writable<'a, T, R, F> where
    F::Mapper: 'a
{
    /// Set a value in the buffer
    pub fn set(&mut self, idx: usize, val: T) {
        if idx >= self.len {
            panic!("Tried to write out of bounds to a WritableMapping!")
        }
        unsafe { self.raw.set(idx, val); }
    }
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for Writable<'a, T, R, F> where
    F::Mapper: 'a,
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a complete readable/writable map, which can be sliced both ways.
pub struct RW<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> where
    F::Mapper: 'a
{
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Deref for RW<'a, T, R, F> where
    F::Mapper: 'a
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> DerefMut for RW<'a, T, R, F> where
    F::Mapper: 'a
{
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { self.raw.to_mut_slice(self.len) }
    }
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for RW<'a, T, R, F> where
    F::Mapper: 'a
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}

/// A service trait with methods for mapping already implemented.
/// To be used by device back ends.
#[allow(missing_docs)]
pub trait Builder<'a, R: Resources> {
    type RawMapping: Raw;

    fn map_readable<T: Copy>(&'a mut self, Self::RawMapping, usize) -> Readable<T, R, Self> where
        Self: Sized + Factory<R>;
    fn map_writable<T: Copy>(&'a mut self, Self::RawMapping, usize) -> Writable<T, R, Self> where
        Self: Sized + Factory<R>;
    fn map_read_write<T: Copy>(&'a mut self, Self::RawMapping, usize) -> RW<T, R, Self> where
        Self: Sized + Factory<R>;
}


impl<'a, R: Resources, F: Factory<R>> Builder<'a, R> for F where
    F::Mapper: 'a
{
    type RawMapping = F::Mapper;

    fn map_readable<T: Copy>(&'a mut self, map: F::Mapper,
                    length: usize) -> Readable<T, R, Self> {
        Readable {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }

    fn map_writable<T: Copy>(&'a mut self, map: F::Mapper,
                    length: usize) -> Writable<T, R, Self> {
        Writable {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }

    fn map_read_write<T: Copy>(&'a mut self, map: F::Mapper,
                      length: usize) -> RW<T, R, Self> {
        RW {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }
}
