// Copyright 2015 The Gfx-rs Developers.
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

//! Pipeline State Objects - typed higher-level version

#![allow(missing_docs)]

use device as d;

pub trait LinkSource<'a> {
    fn declare_to(&mut d::pso::LinkMap<'a>);
}

pub struct ShaderLinkError;

pub trait LinkBuilder<'a>: Sized {
    fn new(&d::pso::LinkResponse<'a>) -> Result<Self, ShaderLinkError>;
    fn declare() -> d::pso::LinkMap<'a>;
}

pub struct ShaderDataSet<R: d::Resources>{
    pub vertex_buffers: d::pso::VertexBufferSet<R>,
    pub pixel_targets: d::pso::PixelTargetSet<R>,
    //TODO: add more, move to the device side
}

pub trait ShaderLink<R: d::Resources> {
    type Data;
    fn define(&self, data: &Self::Data) -> ShaderDataSet<R>;
}

pub struct VertexLinkError;

pub trait VertexFormatBuilder<'a>: LinkSource<'a> + Sized {
    fn new(&d::pso::LinkResponse<'a>) -> Result<Self, VertexLinkError>;
}

pub trait VertexFormat {
    type Vertex;
    fn define_to<R: d::Resources>(&self, out: &mut d::pso::VertexBufferSet<R>,
                &d::handle::Buffer<R, Self::Vertex>, d::pso::BufferOffset);
}

/// Strongly-typed compiled pipeline state
pub struct PipelineState<R: d::Resources, L>(
    d::handle::RawPipelineState<R>, d::PrimitiveType, L);

impl<R: d::Resources, L: ShaderLink<R>> PipelineState<R, L> {
    pub fn new(raw: d::handle::RawPipelineState<R>, pt: d::PrimitiveType,
               link: L) -> PipelineState<R, L> {
        PipelineState(raw, pt, link)
    }
    pub fn get_handle(&self) -> &d::handle::RawPipelineState<R> {
        &self.0
    }
    pub fn prepare_data(&self, data: &L::Data) -> ShaderDataSet<R> {
        self.2.define(data)
    }
}
