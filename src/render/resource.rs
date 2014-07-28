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

use std::fmt::Show;

use device;
use backend = device::dev;
use device::shade::{CreateShaderError, ProgramMeta};
use device::tex::{SamplerInfo, SurfaceInfo, TextureInfo};


/// A deferred resource
#[deriving(PartialEq, Show)]
pub enum Future<T, E> {
    Pending,
    Loaded(T),
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
    pub buffers: Vec<Future<backend::Buffer, ()>>,
    pub array_buffers: Vec<Future<backend::ArrayBuffer, ()>>,
    pub shaders: Vec<Future<backend::Shader, CreateShaderError>>,
    pub programs: Vec<Future<ProgramMeta, ()>>,
    pub frame_buffers: Vec<Future<backend::FrameBuffer, ()>>,
    pub surfaces: Vec<(Future<backend::Surface, ()>, SurfaceInfo)>,
    pub textures: Vec<(Future<backend::Texture, ()>, TextureInfo)>,
    pub samplers: Vec<(Future<backend::Sampler, ()>, SamplerInfo)>,
}

impl Cache {
    /// Create a new cache instance (to serve a single device)
    pub fn new() -> Cache {
        Cache {
            buffers: Vec::new(),
            array_buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            frame_buffers: Vec::new(),
            surfaces: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
        }
    }

    /// Process a given device reply by updating the appropriate resource
    pub fn process(&mut self, reply: device::Reply<super::Token>) -> Result<(), super::DeviceError> {
        let mut ret = Ok(());
        match reply {
            device::ReplyNewBuffer(token, buf) => {
                *self.buffers.get_mut(token) = Loaded(buf);
            },
            device::ReplyNewArrayBuffer(token, result) => {
                *self.array_buffers.get_mut(token) = match result {
                    Ok(vao) => Loaded(vao),
                    Err(e) => {
                        ret = Err(super::ErrorNewArrayBuffer);
                        Failed(e)
                    },
                };
            },
            device::ReplyNewShader(token, result) => {
                *self.shaders.get_mut(token) = match result {
                    Ok(sh) => Loaded(sh),
                    Err(e) => {
                        ret = Err(super::ErrorNewShader(token, e));
                        Failed(e)
                    },
                };
            },
            device::ReplyNewProgram(token, result) => {
                *self.programs.get_mut(token) = match result {
                    Ok(prog) => Loaded(prog),
                    Err(e) => {
                        ret = Err(super::ErrorNewProgram(token));
                        Failed(e)
                    },
                };
            },
            device::ReplyNewFrameBuffer(token, fbo) => {
                *self.frame_buffers.get_mut(token) = Loaded(fbo);
            },
            device::ReplyNewSurface(token, suf) => {
                match *self.surfaces.get_mut(token) {
                    (ref mut future, _) => *future = Loaded(suf),
                }
            },
            device::ReplyNewTexture(token, tex) => {
                match *self.textures.get_mut(token) {
                    (ref mut future, _) => *future = Loaded(tex),
                }
            },
            device::ReplyNewSampler(token, sam) => {
                match *self.samplers.get_mut(token) {
                    (ref mut future, _) => *future = Loaded(sam),
                }
            },
        }
        ret
    }
}
