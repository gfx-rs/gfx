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

/// A handle to a data entry holding a type `T`
pub struct Handle<T> {
    /// The index of the handle in the array of entries
    priv id: u16,
    /// The generation of entry, used for checking if the handle is invalid
    priv count: u16,
}

pub struct HandleEntry<T>;
pub struct HandleManager<T>;

impl<T> HandleManager<T> {
    pub fn new() -> HandleManager<T> {
        HandleManager
    }

    pub fn add(&mut self, _thing: T) -> Handle<T> {
        fail!("Not yet implemented.");
    }

    pub fn is_valid(&self, _handle: Handle<T>) -> bool {
        fail!("Not yet implemented.");
    }

    pub fn with<U>(&self, _handle: Handle<T>, _f: &fn(Option<&T>) -> U) -> U {
        fail!("Not yet implemented.");
    }

    pub fn with_mut<U>(&mut self, _handle: Handle<T>, _f: &fn(Option<&mut T>) -> U) -> U {
        fail!("Not yet implemented.");
    }

    pub fn free(&mut self, _handle: Handle<T>) {
        fail!("Not yet implemented.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut hm = HandleManager::<int>::new();
        let h = hm.add(1);
        assert!(hm.is_valid(h));
    }

    #[test]
    fn test_with() {
        let mut hm = HandleManager::<int>::new();
        let h = hm.add(1);
        do hm.with(h) |x| {
            let x = x.unwrap();
            assert_eq!(*x, 1);
        };
    }

    #[test]
    fn test_with_mut() {
        let mut hm = HandleManager::<int>::new();
        let h = hm.add(1);
        do hm.with_mut(h) |x| {
            x.map(|x| *x = 12);
        };
        do hm.with(h) |x| {
            let x = x.unwrap();
            assert_eq!(*x, 12);
        };
    }
}
