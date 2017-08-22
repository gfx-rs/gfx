// Copyright 2017 The Gfx-rs Developers.
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

use core::target::{Layer, Level};
use core::texture as t;
use texture;
use gl;

pub type Buffer      = gl::types::GLuint;
pub type Shader      = gl::types::GLuint;
pub type Program     = gl::types::GLuint;
pub type FrameBuffer = gl::types::GLuint;
pub type Surface     = gl::types::GLuint;
pub type Texture     = gl::types::GLuint;
pub type Sampler     = gl::types::GLuint;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fence(pub gl::types::GLsync);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceView {
    pub(crate) object: Texture,
    pub(crate) bind: gl::types::GLenum,
    pub(crate) owned: bool,
}

impl ResourceView {
    pub fn new_texture(t: Texture, kind: t::Kind) -> ResourceView {
        ResourceView {
            object: t,
            bind: texture::kind_to_gl(kind),
            owned: false,
        }
    }
    pub fn new_buffer(b: Texture) -> ResourceView {
        ResourceView {
            object: b,
            bind: gl::TEXTURE_BUFFER,
            owned: true,
        }
    }
}


#[derive(Clone, Debug, Copy)]
pub struct GraphicsPipeline {
    program: Program,
}

#[derive(Clone, Debug, Copy)]
pub struct ComputePipeline {
    program: Program,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Image {
    Surface(Surface),
    Texture(Texture),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FatSampler {
    pub(crate) object: Sampler,
    pub(crate) info: t::SamplerInfo,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TargetView {
    Surface(Surface),
    Texture(Texture, Level),
    TextureLayer(Texture, Level, Layer),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorHeap;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSetLayout;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSet;
