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

pub mod buffer;
pub mod resource;
pub mod target;

use std::default::Default;
use gfx_core as d;
pub use gfx_core::pso::{Descriptor};


pub struct RawDataSet<R: d::Resources>{
    pub vertex_buffers: d::pso::VertexBufferSet<R>,
    pub constant_buffers: d::pso::ConstantBufferSet<R>,
    pub global_constants: Vec<(d::shade::Location, d::shade::UniformValue)>,
    pub resource_views: d::pso::ResourceViewSet<R>,
    pub unordered_views: d::pso::UnorderedViewSet<R>,
    pub samplers: d::pso::SamplerSet<R>,
    pub pixel_targets: d::pso::PixelTargetSet<R>,
    pub ref_values: d::state::RefValues,
    pub scissor: Option<d::target::Rect>,
}

impl<R: d::Resources> RawDataSet<R> {
    pub fn new() -> RawDataSet<R> {
        RawDataSet {
            vertex_buffers: d::pso::VertexBufferSet::new(),
            constant_buffers: d::pso::ConstantBufferSet::new(),
            global_constants: Vec::new(),
            resource_views: d::pso::ResourceViewSet::new(),
            unordered_views: d::pso::UnorderedViewSet::new(),
            samplers: d::pso::SamplerSet::new(),
            pixel_targets: d::pso::PixelTargetSet::new(),
            ref_values: Default::default(),
            scissor: None,
        }
    }
}

/// Failure to initilize the link between the shader and the data.
#[derive(Clone, PartialEq, Debug)]
pub enum InitError {
    /// Vertex attribute mismatch.
    VertexImport(d::AttributeSlot, Option<d::format::Format>),
    /// Constant buffer mismatch.
    ConstantBuffer(d::ConstantBufferSlot, Option<()>),
    /// Global constant mismatch.
    GlobalConstant(d::shade::Location, Option<()>),
    /// Shader resource view mismatch.
    ResourceView(d::ResourceViewSlot, Option<()>),
    /// Unordered access view mismatch.
    UnorderedView(d::UnorderedViewSlot, Option<()>),
    /// Sampler mismatch.
    Sampler(d::SamplerSlot, Option<()>),
    /// Pixel target mismatch.
    PixelExport(d::ColorSlot, Option<d::format::Format>),
}

pub trait PipelineInit {
    type Meta;
    fn link_to(&self, &mut Descriptor, &d::shade::ProgramInfo)
               -> Result<Self::Meta, InitError>;
}

pub trait PipelineData<R: d::Resources> {
    type Meta;
    fn bake(&self, meta: &Self::Meta, &mut d::handle::Manager<R>)
              -> RawDataSet<R>;
}

/// Strongly-typed compiled pipeline state
pub struct PipelineState<R: d::Resources, M>(
    d::handle::RawPipelineState<R>, d::Primitive, M);

impl<R: d::Resources, M> PipelineState<R, M> {
    pub fn new(raw: d::handle::RawPipelineState<R>, prim: d::Primitive, meta: M)
               -> PipelineState<R, M> {
        PipelineState(raw, prim, meta)
    }
    pub fn get_handle(&self) -> &d::handle::RawPipelineState<R> {
        &self.0
    }
    pub fn get_meta(&self) -> &M {
        &self.2
    }
    pub fn prepare_data<D: PipelineData<R, Meta=M>>(&self, data: &D,
                        handle_man: &mut d::handle::Manager<R>) -> RawDataSet<R>
    {
        data.bake(&self.2, handle_man)
    }
}


pub trait DataLink<'a>: Sized {
    type Init: 'a;
    fn new() -> Self;
    fn is_active(&self) -> bool;
    fn link_input(&mut self, _: &d::shade::AttributeVar, _: &Self::Init) ->
                  Option<Result<d::pso::AttributeDesc, d::format::Format>> { None }
    fn link_constant_buffer(&mut self, _: &d::shade::ConstantBufferVar, _: &Self::Init) ->
                            Option<Result<(), d::shade::ConstFormat>> { None }
    fn link_global_constant(&mut self, _: &d::shade::ConstVar, _: &Self::Init) ->
                            Option<Result<(), d::shade::UniformValue>> { None }
    fn link_output(&mut self, _: &d::shade::OutputVar, _: &Self::Init) ->
                   Option<Result<d::pso::ColorTargetDesc, d::format::Format>> { None }
    fn link_depth_stencil(&mut self, _: &Self::Init) ->
                          Option<d::pso::DepthStencilDesc> { None }
    fn link_resource_view(&mut self, _: &d::shade::TextureVar, _: &Self::Init) ->
                          Option<Result<(), d::format::Format>> { None }
    fn link_unordered_view(&mut self, _: &d::shade::UnorderedVar, _: &Self::Init) ->
                           Option<Result<(), d::format::Format>> { None }
    fn link_sampler(&mut self, _: &d::shade::SamplerVar, _: &Self::Init) -> Option<()> { None }
}

pub trait DataBind<R: d::Resources> {
    type Data;
    fn bind_to(&self, &mut RawDataSet<R>, &Self::Data, &mut d::handle::Manager<R>);
}
