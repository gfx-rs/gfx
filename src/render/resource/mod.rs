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

///! Resource management

use std::fmt::Show;

use device;
use backend = device::back;
use device::shade::{CreateShaderError, ProgramInfo};
use device::tex::{SamplerInfo, SurfaceInfo, TextureInfo};

pub use self::handle::Handle;

///! Handle management
pub mod handle;

/// A deferred resource
#[deriving(PartialEq, Show)]
pub enum Future<T, E> {
    /// Still loading
    Pending,
    /// Successfully loaded
    Loaded(T),
    /// Failed to load
    Failed(E),
}

impl<T, E: Show> Future<T, E> {
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
    /// Buffer storage
    pub buffers: handle::Storage<Future<backend::Buffer, ()>>,
    /// Array buffer storage
    pub array_buffers: handle::Storage<Future<backend::ArrayBuffer, ()>>,
    /// Shader storage
    pub shaders: handle::Storage<Future<backend::Shader, CreateShaderError>>,
    /// Program storage
    pub programs: handle::Storage<Future<device::ProgramHandle, ()>>,
    /// Frame buffer storage
    pub frame_buffers: handle::Storage<Future<backend::FrameBuffer, ()>>,
    /// Surface storage
    pub surfaces: handle::Storage<(Future<backend::Surface, ()>, SurfaceInfo)>,
    /// Texture storage
    pub textures: handle::Storage<(Future<backend::Texture, ()>, TextureInfo)>,
    /// Sampler storage
    pub samplers: handle::Storage<(Future<backend::Sampler, ()>, SamplerInfo)>,
}

impl Cache {
    /// Create a new cache instance (to serve a single device)
    pub fn new() -> Cache {
        Cache {
            buffers: handle::Storage::new(),
            array_buffers: handle::Storage::new(),
            shaders: handle::Storage::new(),
            programs: handle::Storage::new(),
            frame_buffers: handle::Storage::new(),
            surfaces: handle::Storage::new(),
            textures: handle::Storage::new(),
            samplers: handle::Storage::new(),
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
