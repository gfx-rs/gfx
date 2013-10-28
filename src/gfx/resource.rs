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

/// A handle to a data element holding a type `T`
pub struct Handle<T> {
    /// The index of the handle in the array of elements
    priv index: u16,
    /// The generation of the index, used for checking if the handle is invalid
    priv age: u16,
}

struct ElementIndex {
    /// The index of the element
    index: Option<u16>,
    /// The generation of the index
    age: u16,
}

impl ElementIndex {
    /// Create a new, uninitialised element index
    fn new() -> ElementIndex {
        ElementIndex { index: None, age: 0 }
    }
}

/// A resource manager
///
/// Performance is optimised towards batch operations on the stored data, at
/// the slight expense of handle lookups and element deletions.
///
/// ~~~
///                             +---+
///                             |   |  Handle
///                             +-.-+
///                               |
/// +- ResourceManager -----------|---------------------------------------+
/// |                             |                                       |
/// |   +---+---+---+---+---+---+-V-+---+---+---+ - - - +---+             |
/// |   | i | i |   | i | i | i | i |   |   | i |       |   |  indices    |
/// |   +-.-+-.-+---+-.-+-.-+-.-+-.-+---+---+-.-+ - - - +---+             |
/// |     |   |       |   |   |   |           |                           |
/// |     |   |   +---|---+   |   |           |                           |
/// |     |   |   |   |       |   |           |                           |
/// |     |   |   |   |   +---|---|-----------+                           |
/// |     |   |   |   |   |   |   |                                       |
/// |   +-V-+-V-+-V-+-V-+-V-+-V-+-V-+---+---+---+ - - - +---+             |
/// |   | T | T | T | T | T | T | T |   |   |   |       |   |  elements   |
/// |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+             |
/// |                                                                     |
/// +---------------------------------------------------------------------+
/// ~~~
pub struct ResourceManager<T> {
    /// A sparse array of indices pointing to elements
    priv indices: ~[ElementIndex],  // Should be a fixed vector
    /// A packed array of elements.
    priv elements: ~[T],            // Should be a fixed vector
    /// The number of elements currently in storage
    priv len: u16,
}

impl<T> ResourceManager<T> {
    pub fn new() -> ResourceManager<T> {
        ResourceManager {
            indices: ~[],
            elements: ~[],
            len: 0,
        }
    }

    pub fn len(&self) -> u16 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds the element to the manager and returns a handle to it
    pub fn add(&mut self, _element: T) -> Handle<T> {
        // Check if adding the new element does not exeed the capacity of the
        // array of elements
        // At the first hole, set ElementIndex.index to Some(self.len)
        // Increment self.len
        // Increment the repective ElementIndex.age
        fail!("Not yet implemented.");
    }

    /// Work with the entry associated with a handle
    pub fn with<U>(&self, handle: Handle<T>, f: &fn(Option<&T>) -> U) -> U {
        match self.indices[handle.index as uint].index {
            Some(i) if handle.age >= self.indices[handle.index as uint].age => {
                f(Some(&self.elements[i as uint]))
            }
            _ => f(None),
        }
    }

    /// Work with the entry associated with a handle, allowing for mutation
    pub fn with_mut<U>(&mut self, handle: Handle<T>, f: &fn(Option<&mut T>) -> U) -> U {
        match self.indices[handle.index as uint].index {
            Some(i) if handle.age >= self.indices[handle.index as uint].age => {
                f(Some(&mut self.elements[i as uint]))
            }
            _ => f(None),
        }
    }

    /// Returns an iterator over the stored elements. This is fast, and
    /// therefore useful for batch operations over the elements.
    pub fn iter<'a>(&'a self) -> ElementIterator<'a, T> {
        ElementIterator { manager: self, index: 0 }
    }

    /// Returns an iterator over the stored elements, allowing for their
    /// modification. This is fast, and therefore useful for batch operations
    /// over the elements.
    pub fn mut_iter<'a>(&'a mut self) -> MutElementIterator<'a, T> {
        MutElementIterator { manager: self, index: 0 }
    }

    // Remove the
    pub fn remove(&mut self, _handle: Handle<T>) {
        // Check if the age is correct (fail or log if not?)
        // Set ElementIndex.index to None
        // Increment the respective ElementIndex.age
        // Repack array from the removed element up to the last element
        // Update the element indicies
        fail!("Not yet implemented.");
    }
}

// Element Iterators

/// An iterator over the elements stored in a `ResourceManager`
pub struct ElementIterator<'self, T> {
    priv manager: &'self ResourceManager<T>,
    priv index: uint,
}

impl<'self, T> Iterator<&'self T> for ElementIterator<'self, T> {
    #[inline]
    fn next(&mut self) -> Option<&'self T> {
        if self.index < self.manager.len as uint {
            let old = self.index;
            self.index += 1;
            Some(&self.manager.elements[old])
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (uint, Option<uint>) {
        let l = self.manager.len as uint;
        (l, Some(l))
    }
}

/// An iterator for modifying the elements stored in a `ResourceManager`
pub struct MutElementIterator<'self, T> {
    priv manager: &'self mut ResourceManager<T>,
    priv index: uint,
}

impl<'self, T> Iterator<&'self mut T> for MutElementIterator<'self, T> {
    #[inline]
    fn next(&mut self) -> Option<&'self mut T> {
        if self.index < self.manager.len as uint {
            let old = self.index;
            self.index += 1;
            Some(&mut self.manager.elements[old])
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (uint, Option<uint>) {
        let l = self.manager.len as uint;
        (l, Some(l))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut hm = ResourceManager::<int>::new();
        let h = hm.add(1);
        do hm.with(h) |x| {
            assert!(x.is_some());
        };
    }

    #[test]
    #[should_fail]
    fn test_add_overflow() {}

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
        let h = hm.add(1);
        hm.remove(h);
        assert!(hm.is_empty())
    }

    #[test]
    #[should_fail]
    fn test_remove_invalid_handle() {}
}
