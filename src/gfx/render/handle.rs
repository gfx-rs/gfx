// Copyright 2014 The Gfx-rs Developers.
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

use std::u16;
use std::vec::Vec;


static LAST_GENERATION: u16 = u16::MAX;
 
#[deriving(Clone, PartialEq)]
pub struct Handle {
    index: u16,
    generation: u16
}
 
pub struct Manager<T> {
    data: Vec<T>,
    generation: Vec<u16>
}
 
impl<T> Manager<T> {
    fn is_valid_handle(&self, handle: Handle) -> bool {
        handle.generation == *self.generation.get(handle.index as uint)
    }
 
    pub fn new() -> Manager<T> {
        Manager {
            data: Vec::new(),
            generation: Vec::new()
        }
    }

    pub fn with_capacity(capacity: u16) -> Manager<T> {
        Manager {
            data: Vec::with_capacity(capacity as uint),
            generation: Vec::from_elem(capacity as uint, 0u16)
        }
    }
 
    pub fn drop(mut self) {
        self.data.clear();
        self.generation.clear();
    }
 
    pub fn add(&mut self, data: T) -> Handle {
        self.data.push(data);
        let index = self.data.len() - 1;
        Handle {
            index: index as u16,
            generation: self.generation.get(index).clone()
        }
    }
 
    pub fn remove(&mut self, handle: Handle) {
        if self.is_valid_handle(handle) {
            self.data.swap_remove(handle.index as uint);
            *self.generation.get_mut(handle.index as uint) += 1;
        }
    }
 
    pub fn get<'a>(&'a self, handle: Handle) -> Option<&'a T> {
        if self.is_valid_handle(handle) {
            Some(self.data.get(handle.index as uint))
        } else {
            None
        }
    }
}
