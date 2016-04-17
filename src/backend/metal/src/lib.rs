// Copyright 2016 The Gfx-rs Developers.
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

#[macro_use]
extern crate log;
extern crate objc;
extern crate cocoa;
extern crate gfx_core;
extern crate metal;

use cocoa::base::{selector, class};
use cocoa::foundation::{NSUInteger};

use metal::*;

use gfx_core::format::Format;

mod factory;
mod map;

pub use self::factory::Factory;
pub use self::map::*;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct InputLayout {
    layout: MTLVertexDescriptor
}

unsafe impl Send for InputLayout {}
unsafe impl Sync for InputLayout {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shader {
    func: MTLFunction
}

unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    vs: MTLFunction,
    ps: MTLFunction
}

unsafe impl Send for Program {}
unsafe impl Sync for Program {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pipeline {
    pipeline: MTLRenderPipelineState
}

unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Texture {
    texture: MTLTexture
}

unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

pub struct Device {
    device: MTLDevice,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl gfx_core::Resources for Resources {
    type Buffer              = ();
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = Pipeline;
    type Texture             = Texture;
    type RenderTargetView    = ();
    type DepthStencilView    = ();
    type ShaderResourceView  = ();
    type UnorderedAccessView = ();
    type Sampler             = ();
    type Fence               = ();
}
