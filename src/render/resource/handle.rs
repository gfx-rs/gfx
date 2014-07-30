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

///! Resource handle module

use std;

pub type Index = u16;
pub type Generation = u16;

static LAST_GENERATION: Generation = std::u16::MAX;

// The generation logic works as follows. When a handle is created for the first
// time, the room it is using is initialized with generation 0. However, the
// handle may be freely cloned and copied, so that when the resource is later
// freed via remove, handles can still refer to that slot. So the generation is
// incremented, and if a handle with an older generation tries to access the
// room, it is rejected, because that old handle is now invalid. If the handle
// hits the max value, instead of overflowing, we just never reuse the room.

/// A generic resource handle, exposed to the user
#[deriving(Clone, PartialEq, Show)]
pub struct Handle {
    index: Index,
    generation: Generation,
}

/// Resource access error
#[deriving(Clone, PartialEq, Show)]
pub enum StorageError {
    /// The index is out of valid bounds
    InvalidIndex,
    /// The generation is outdated
    InvalidGeneration,
    /// The data is not there
    InvalidData,
}

/// A room for a single resource
struct Room<T> {
    data: Option<T>,
    generation: Generation,
}

impl<T> Room<T> {
    /// Check if the room is available for a resource
    pub fn is_vacant(&self) -> bool {
        self.data.is_none() && self.generation < LAST_GENERATION
    }
}

/// A generic resource storage
pub struct Storage<T> {
    rooms: Vec<Room<T>>,
    first_vacant: Option<Index>,
}

impl<T> Storage<T> {
    /// Create a new storage
    pub fn new() -> Storage<T> {
        Storage {
            rooms: Vec::new(),
            first_vacant: None,
        }
    }

    /// Try getting a resource reference by the handle
    pub fn get<'a>(&'a self, handle: Handle) -> Result<&'a T, StorageError> {
        let room = &self.rooms[handle.index as uint];
        if room.generation == handle.generation {
            match room.data {
                Some(ref d) => Ok(d),
                None => Err(InvalidData),
            }
        }else {
            Err(InvalidGeneration)
        }
    }

    /// Try getting a mutable reference to a resource
    pub fn get_mut<'a>(&'a mut self, handle: Handle) -> Result<&'a mut T, StorageError> {
        let room = self.rooms.get_mut(handle.index as uint);
        if room.generation == handle.generation {
            match room.data {
                Some(ref mut d) => Ok(d),
                None => Err(InvalidData),
            }
        }else {
            Err(InvalidGeneration)
        }
    }

    /// Add a new resource
    pub fn add(&mut self, data: T) -> Handle {
        match self.first_vacant {
            Some(index) => {
                // find the next vacant room
                self.first_vacant = self.rooms.slice_from(index as uint + 1)
                    .iter().position(|r| r.is_vacant())
                    .map(|i| i as Index + index + 1);
                // fill the current room
                let room = self.rooms.get_mut(index as uint);
                debug_assert!(room.is_vacant());
                room.data = Some(data);
                Handle {
                    index: index,
                    generation: room.generation,
                }
            },
            None => {
                // create a new room
                self.rooms.push(Room {
                    data: Some(data),
                    generation: 0,
                });
                Handle {
                    index: self.rooms.len() as Index -1,
                    generation: 0,
                }
            }
        }
    }

    /// Remove a resource by the handle
    pub fn remove(&mut self, handle: Handle) -> Result<T, StorageError> {
        let room = self.rooms.get_mut(handle.index as uint);
        if room.generation == handle.generation {
            if room.data.is_some() {
                debug_assert!(room.generation < LAST_GENERATION);
                room.generation += 1;
                if room.generation != LAST_GENERATION {
                    // update first vacant
                    match self.first_vacant {
                        Some(index) if index <= handle.index => (),
                        _ => self.first_vacant = Some(handle.index),
                    }
                }
                Ok(room.data.take_unwrap())
            }else {
                Err(InvalidData)
            }
        }else {
            Err(InvalidGeneration)
        }
    }
}

impl<T> std::ops::Index<Handle, T> for Storage<T> {
    fn index<'a>(&'a self, index: &Handle) -> &'a T {
        self.get(*index).unwrap()
    }
}
