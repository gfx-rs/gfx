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

//! Resource management.
#![allow(missing_doc)] // doc after kvark's patch lands
use std;

use device;
use backend = device::dev;
use device::shade::{CreateShaderError, ProgramMeta};
use device::tex::{SamplerInfo, SurfaceInfo, TextureInfo};


pub type Index = u16;
pub type Generation = u16;

static LAST_GENERATION: Generation = std::u16::MAX;

/// A generic resource handle, exposed to the user
#[deriving(Clone, PartialEq, Show)]
pub struct Handle {
    index: Index,
    generation: Generation,
}

/// Resource access error
#[deriving(Clone, PartialEq, Show)]
pub enum StorageError {
    InvalidIndex,
    InvalidGeneration,
    InvalidData,
}

/// A room for a single resource
struct Room<T> {
    data: Option<T>,
    generation: Generation,
}

impl<T> Room<T> {
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
    pub fn new() -> Storage<T> {
        Storage {
            rooms: Vec::new(),
            first_vacant: None,
        }
    }

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

    pub fn add(&mut self, data: T) -> Handle {
        match self.first_vacant {
            Some(index) => {
                // find the next vacant room
                self.first_vacant = self.rooms.slice_from(index as uint + 1).
                    iter().position(|r| r.is_vacant()).
                    map(|i| i as Index + index + 1);
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


/// A deferred resource
#[deriving(PartialEq, Show)]
pub enum Future<T, E> {
    Pending,
    Loaded(T),
    Failed(E),
}

impl<T, E: std::fmt::Show> Future<T, E> {
    /// Returns `true` if the resource is still pending
    pub fn is_pending(&self) -> bool {
        match *self { Pending => true, _ => false }
    }

    /// Get the resource, triggering a task failure if it is either `Pending`
    /// or has `Failed`.
    pub fn unwrap<'a>(&'a self) -> &'a T {
        match *self {
            Pending => fail!("Resource not loaded yet"),
            Loaded(ref res) => res,
            Failed(ref e) => fail!("Resource load fail: {}", e),
        }
    }
}


/// Storage for all loaded graphics objects
pub struct Cache {
    pub buffers: Storage<Future<backend::Buffer, ()>>,
    pub array_buffers: Storage<Future<backend::ArrayBuffer, ()>>,
    pub shaders: Storage<Future<backend::Shader, CreateShaderError>>,
    pub programs: Storage<Future<ProgramMeta, ()>>,
    pub frame_buffers: Storage<Future<backend::FrameBuffer, ()>>,
    pub surfaces: Storage<(Future<backend::Surface, ()>, SurfaceInfo)>,
    pub textures: Storage<(Future<backend::Texture, ()>, TextureInfo)>,
    pub samplers: Storage<(Future<backend::Sampler, ()>, SamplerInfo)>,
}

impl Cache {
    /// Create a new cache instance (to serve a single device)
    pub fn new() -> Cache {
        Cache {
            buffers: Storage::new(),
            array_buffers: Storage::new(),
            shaders: Storage::new(),
            programs: Storage::new(),
            frame_buffers: Storage::new(),
            surfaces: Storage::new(),
            textures: Storage::new(),
            samplers: Storage::new(),
        }
    }

    /// Process a given device reply by updating the appropriate resource
    pub fn process(&mut self, reply: device::Reply<super::Token>) -> Result<(), super::DeviceError> {
        let mut ret = Ok(());
        match reply {
            device::ReplyNewBuffer(token, buf) => {
                match self.buffers.get_mut(token) {
                    Ok(f) => *f = Loaded(buf),
                    Err(_) => (),
                }
            },
            device::ReplyNewArrayBuffer(token, result) => {
                match (self.array_buffers.get_mut(token), result) {
                    (Ok(f), Ok(vao)) => *f = Loaded(vao),
                    (Ok(f), Err(e)) => {
                        ret = Err(super::ErrorNewArrayBuffer);
                        *f = Failed(e);
                    },
                    _ => (),
                }
            },
            device::ReplyNewShader(token, result) => {
                match (self.shaders.get_mut(token), result) {
                    (Ok(f), Ok(sh)) => *f = Loaded(sh),
                    (Ok(f), Err(e)) => {
                        ret = Err(super::ErrorNewShader(super::ShaderHandle(token), e));
                        *f = Failed(e);
                    },
                    _ => (),
                }
            },
            device::ReplyNewProgram(token, result) => {
                match (self.programs.get_mut(token), result) {
                    (Ok(f), Ok(prog)) => *f = Loaded(prog),
                    (Ok(f), Err(e)) => {
                        ret = Err(super::ErrorNewProgram(super::ProgramHandle(token)));
                        *f = Failed(e);
                    },
                    _ => ()
                }
            },
            device::ReplyNewFrameBuffer(token, fbo) => {
                match self.frame_buffers.get_mut(token) {
                    Ok(f) => *f = Loaded(fbo),
                    Err(_) => (),
                }
            },
            device::ReplyNewSurface(token, suf) => {
                match self.surfaces.get_mut(token) {
                    Ok(&(ref mut f, _)) => *f = Loaded(suf),
                    Err(_) => (),
                }
            },
            device::ReplyNewTexture(token, tex) => {
                match self.textures.get_mut(token) {
                    Ok(&(ref mut future, _)) => *future = Loaded(tex),
                    Err(_) => (),
                }
            },
            device::ReplyNewSampler(token, sam) => {
                match self.samplers.get_mut(token) {
                    Ok(&(ref mut future, _)) => *future = Loaded(sam),
                    Err(_) => (),
                }
            },
        }
        ret
    }
}
