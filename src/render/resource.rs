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

#[deriving(PartialEq, Show)]
pub enum MaybeLoaded<R, E> {
    Pending,
    Loaded(R),
    Failed(E),
}

impl<R, E: Show> MaybeLoaded<R, E> {
    pub fn is_loaded(&self) -> bool {
        match *self {
            Pending => false,
            _ => true,
        }
    }

    pub fn unwrap<'a>(&'a self) -> &'a R {
        match *self {
            Pending => fail!("Resource not loaded yet"),
            Loaded(ref res) => res,
            Failed(ref e) => fail!("Resource load fail: {}", e),
        }
    }
}

pub type Vector<R, E> = Vec<MaybeLoaded<R, E>>;

/// Storage for all loaded objects
pub struct Cache {
    pub buffers: Vector<backend::Buffer, ()>,
    pub array_buffers: Vector<backend::ArrayBuffer, ()>,
    pub shaders: Vector<backend::Shader, CreateShaderError>,
    pub programs: Vector<ProgramMeta, ()>,
    pub frame_buffers: Vector<backend::FrameBuffer, ()>,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            buffers: Vec::new(),
            array_buffers: Vec::new(),
            shaders: Vec::new(),
            programs: Vec::new(),
            frame_buffers: Vec::new(),
        }
    }

    pub fn process(&mut self, reply: device::Reply<super::Token>) {
        match reply {
            device::ReplyNewBuffer(token, buf) => {
                *self.buffers.get_mut(token) = Loaded(buf);
            },
            device::ReplyNewArrayBuffer(token, result) => {
                *self.array_buffers.get_mut(token) = match result {
                    Ok(vao) => Loaded(vao),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewShader(token, result) => {
                *self.shaders.get_mut(token) = match result {
                    Ok(sh) => Loaded(sh),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewProgram(token, result) => {
                *self.programs.get_mut(token) = match result {
                    Ok(prog) => Loaded(prog),
                    Err(e) => Failed(e),
                };
            },
            device::ReplyNewFrameBuffer(token, fbo) => {
                *self.frame_buffers.get_mut(token) = Loaded(fbo);
            },
        }
    }
}
