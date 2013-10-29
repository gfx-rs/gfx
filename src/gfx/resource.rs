// Copyright 2013 The Gfx-rs Developers.
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

use std::vec;
use std::u16;
use std::unstable::intrinsics;

/// A handle to a data element holding a type `T`
#[deriving(Clone, Eq)]
pub struct Handle<T> {
    /// The index of the handle in the array of elements
    priv index: u16,
    /// The generation of the index, used for checking if the handle is invalid
    priv count: u16,
}

struct ElementIndex {
    /// The index of the element
    index: Option<u16>,
    /// The generation of the index
    count: u16,
}

/// A resource manager
///
/// Performance is optimised towards batch operations on the stored data, at
/// the expense of direct handle lookups and element addition and removal.
///
pub struct ResourceManager<T> {
    //
    //                             +---+
    //                             |   |  Handle
    //                             +-.-+
    //                               |
    // +- ResourceManager -----------|---------------------------------------+
    // |                             |                                       |
    // |   +---+---+---+---+---+---+-V-+---+---+---+ - - - +---+             |
    // |   | i | i |   | i | i | i | i |   |   | i |       |   |  indices    |
    // |   +-.-+-.-+---+-.-+-.-+-.-+-.-+---+---+-.-+ - - - +---+             |
    // |     |   |       |   |   |   |           |                           |
    // |     |   |   +---|---+   |   |           |                           |
    // |     |   |   |   |       |   |           |                           |
    // |     |   |   |   |   +---|---|-----------+                           |
    // |     |   |   |   |   |   |   |                                       |
    // |   +-V-+-V-+-V-+-V-+-V-+-V-+-V-+---+---+---+ - - - +---+             |
    // |   | T | T | T | T | T | T | T |   |   |   |       |   |  elements   |
    // |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+             |
    // |                                                                     |
    // +---------------------------------------------------------------------+
    //

    /// A sparse array of indices pointing to elements
    priv indices: ~[ElementIndex],  // TODO: Should be a fixed vector

    /// A packed array of elements.
    ///
    /// # Safety note
    ///
    /// The tail of this vector (after len) is unitialised memory.
    ///
    priv elements: ~[T],            // TODO: Should be a fixed vector

    /// The number of elements currently in storage
    priv len: u16,
}

impl<T> ResourceManager<T> {
    pub fn new() -> ResourceManager<T> {
        ResourceManager::new_sized(u16::max_value)
    }

    pub fn new_sized(len: u16) -> ResourceManager<T> {
        let len = len as uint;
        ResourceManager {
            indices: vec::from_fn(len, |_| ElementIndex { index: None, count: 0 }),
            elements: vec::from_fn(len, |_| unsafe { intrinsics::uninit() }),
            len: 0,
        }
    }

    pub fn capacity(&self) -> u16 {
        self.elements.len() as u16
    }

    pub fn len(&self) -> u16 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds the element to the manager and returns a handle to it
    pub fn add(&mut self, element: T) -> Handle<T> {
        let ei = self.indices.mut_iter().position(|i| i.index == None)
                     .expect("Could not find a free hole to store the element index.");
        self.elements[self.len as uint] = element;
        self.indices[ei].index = Some(self.len);
        self.len = self.len.checked_add(&1).unwrap();
        Handle { index: ei as u16, count: self.indices[ei].count }
    }

    /// Work with the entry associated with a handle
    pub fn with<U>(&self, handle: Handle<T>, f: &fn(Option<&T>) -> U) -> U {
        match self.indices[handle.index as uint].index {
            Some(i) if handle.count >= self.indices[handle.index as uint].count => {
                f(Some(&self.elements[i as uint]))
            }
            _ => f(None),
        }
    }

    /// Work with the entry associated with a handle, allowing for mutation
    pub fn with_mut<U>(&mut self, handle: Handle<T>, f: &fn(Option<&mut T>) -> U) -> U {
        match self.indices[handle.index as uint].index {
            Some(i) if handle.count >= self.indices[handle.index as uint].count => {
                f(Some(&mut self.elements[i as uint]))
            }
            _ => f(None),
        }
    }

    /// Remove the element associated with the handle
    pub fn remove(&mut self, handle: Handle<T>) {
        let hi = handle.index as uint;

        // TODO: improve these error messages
        assert!(handle.count >= self.indices[hi].count,
               "The element associated with this handle has already been \
                removed. (handle.count: {}, current.count: {})",
                handle.count, self.indices[hi].count);
        let ei = self.indices[hi].index.unwrap();

        // Remove the reference to the element from the indices and clear
        // it from the vector of elements
        self.indices[hi].index = None;
        self.indices[hi].count = self.indices[hi].count.checked_add(&1)
                                     .expect("The maximum age of the element was reached.");
        unsafe { self.elements[ei as uint] = intrinsics::uninit(); }

        // Swap the last element into the hole and update its index
        let lasti = self.len.checked_sub(&1).unwrap();
        self.elements.swap(ei as uint, lasti as uint);
        for elem_index in self.indices.mut_iter() {
            for i in elem_index.index.mut_iter() {
                if *i == lasti { *i = ei };
            }
        }
        // We reuse `lasti` so that we don't have to perform a checked
        // subtraction two times
        self.len = lasti;
    }

    /// Returns an iterator over the stored elements. This is fast, and
    /// therefore useful for batch operations over the elements.
    #[inline]
    pub fn iter<'a>(&'a self) -> ElementIterator<'a, T> {
        ElementIterator {
            iter: self.elements.slice_to(self.len as uint).iter()
        }
    }

    /// Returns a reversed iterator over the stored elements.
    #[inline]
    pub fn rev_iter<'a>(&'a self) -> ElementRevIterator<'a, T> {
        ElementRevIterator {
            iter: self.elements.slice_to(self.len as uint).rev_iter()
        }
    }

    /// Returns an iterator over the stored elements, allowing for their
    /// modification. This is fast, and therefore useful for batch operations
    /// over the elements.
    #[inline]
    pub fn mut_iter<'a>(&'a mut self) -> ElementMutIterator<'a, T> {
        ElementMutIterator {
            iter: self.elements.mut_slice_to(self.len as uint).mut_iter()
        }
    }

    /// Returns a reversed iterator over the stored elements, allowing for
    /// their modification.
    #[inline]
    fn mut_rev_iter<'a>(&'a mut self) -> ElementMutRevIterator<'a, T> {
        ElementMutRevIterator {
            iter: self.elements.mut_slice_to(self.len as uint).mut_rev_iter()
        }
    }
}

// Element Iterators

/// An iterator over the elements stored in a `ResourceManager`
pub struct ElementIterator<'self, T> {
    priv iter: vec::VecIterator<'self, T>,
}

impl<'self, T> Iterator<&'self T> for ElementIterator<'self, T> {
    #[inline] fn next(&mut self) -> Option<&'self T> { self.iter.next() }
    #[inline] fn size_hint(&self) -> (uint, Option<uint>) { self.iter.size_hint() }
}

/// A reversed iterator over the elements stored in a `ResourceManager`
pub struct ElementRevIterator<'self, T> {
    priv iter: vec::RevIterator<'self, T>,
}

impl<'self, T> Iterator<&'self T> for ElementRevIterator<'self, T> {
    #[inline] fn next(&mut self) -> Option<&'self T> { self.iter.next() }
    #[inline] fn size_hint(&self) -> (uint, Option<uint>) { self.iter.size_hint() }
}

/// An iterator for modifying the elements stored in a `ResourceManager`
pub struct ElementMutIterator<'self, T> {
    priv iter: vec::VecMutIterator<'self, T>,
}

impl<'self, T> Iterator<&'self mut T> for ElementMutIterator<'self, T> {
    #[inline] fn next(&mut self) -> Option<&'self mut T> { self.iter.next() }
    #[inline] fn size_hint(&self) -> (uint, Option<uint>) { self.iter.size_hint() }
}

/// A reversed iterator for modifying the elements stored in a `ResourceManager`
pub struct ElementMutRevIterator<'self, T> {
    priv iter: vec::MutRevIterator<'self, T>,
}

impl<'self, T> Iterator<&'self mut T> for ElementMutRevIterator<'self, T> {
    #[inline] fn next(&mut self) -> Option<&'self mut T> { self.iter.next() }
    #[inline] fn size_hint(&self) -> (uint, Option<uint>) { self.iter.size_hint() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::u16;

    #[test]
    fn test_add() {
        let mut hm = ResourceManager::<int>::new();
        let h = hm.add(1);
        do hm.with(h) |x| {
            assert!(x.is_some());
        };
    }

    #[test]
    fn test_capacity() {
        assert_eq!(ResourceManager::<int>::new().capacity(), u16::max_value);
        assert_eq!(ResourceManager::<int>::new_sized(5).capacity(), 5);
    }

    #[test]
    fn test_len() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let _ = hm.add(1);
        let _ = hm.add(1);
        assert_eq!(hm.len(), 3);
    }

    #[test]
    fn test_is_empty() {
        let mut hm = ResourceManager::<int>::new();
        assert!(hm.is_empty());
        let _ = hm.add(1);
        assert!(!hm.is_empty());
    }

    #[test]
    #[should_fail]
    fn test_overflow() {
        let mut hm = ResourceManager::<int>::new_sized(4);
        let _ = hm.add(1);
        let _ = hm.add(2);
        let _ = hm.add(3);
        let _ = hm.add(4);
        let _ = hm.add(5);
    }

    #[test]
    fn test_with() {
        let mut hm = ResourceManager::<int>::new();
        let h = hm.add(1);
        do hm.with(h) |x| {
            let x = x.unwrap();
            assert_eq!(*x, 1);
        };
    }

    #[test]
    fn test_with_mut() {
        let mut hm = ResourceManager::<int>::new();
        let h = hm.add(1);
        do hm.with_mut(h) |x| {
            x.map(|x| *x = 12);
        };
        do hm.with(h) |x| {
            let x = x.unwrap();
            assert_eq!(*x, 12);
        };
    }

    #[test]
    fn test_remove_handle() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let h = hm.add(2);
        let _ = hm.add(3);
        hm.remove(h);
        let mut it = hm.iter();
        assert_eq!(*it.next().unwrap(), 1);
        assert_eq!(*it.next().unwrap(), 3);
        assert_eq!(it.next(), None);
    }

    #[test]
    #[should_fail]
    fn test_remove_invalid_handle() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let h = hm.add(2);
        let _ = hm.add(3);
        hm.remove(h);
        hm.remove(h);
    }

    #[test]
    fn test_iter() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let _ = hm.add(2);
        let _ = hm.add(3);
        let _ = hm.add(4);
        let _ = hm.add(5);
        let v = hm.iter().map(|x| *x).to_owned_vec();
        assert_eq!(v, ~[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_rev_iter() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let _ = hm.add(2);
        let _ = hm.add(3);
        let _ = hm.add(4);
        let _ = hm.add(5);
        let v = hm.rev_iter().map(|x| *x).to_owned_vec();
        assert_eq!(v, ~[5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_mut_iter() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let _ = hm.add(2);
        let _ = hm.add(3);
        let _ = hm.add(4);
        let _ = hm.add(5);
        for x in hm.mut_iter() { *x *= 2; }
        let v = hm.iter().map(|x| *x).to_owned_vec();
        assert_eq!(v, ~[2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_mut_rev_iter() {
        let mut hm = ResourceManager::<int>::new();
        let _ = hm.add(1);
        let _ = hm.add(2);
        let _ = hm.add(3);
        let _ = hm.add(4);
        let _ = hm.add(5);
        for x in hm.mut_rev_iter() { *x *= 2; }
        let v = hm.iter().map(|x| *x).to_owned_vec();
        assert_eq!(v, ~[2, 4, 6, 8, 10]);
    }
}
