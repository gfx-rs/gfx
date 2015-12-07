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

use std::default::Default;
use std::marker::PhantomData;
use gfx_core as d;
pub use gfx_core::pso::Descriptor;

pub struct RawDataSet<R: d::Resources>{
    pub vertex_buffers: d::pso::VertexBufferSet<R>,
    pub constant_buffers: d::pso::ConstantBufferSet<R>,
    pub constants: Vec<(d::shade::Location, d::shade::UniformValue)>,
    pub pixel_targets: d::pso::PixelTargetSet<R>,
    pub ref_values: d::state::RefValues,
    pub scissor: Option<d::target::Rect>,
}

impl<R: d::Resources> RawDataSet<R> {
    pub fn new() -> RawDataSet<R> {
        RawDataSet {
            vertex_buffers: d::pso::VertexBufferSet::new(),
            constant_buffers: d::pso::ConstantBufferSet::new(),
            constants: Vec::new(),
            pixel_targets: d::pso::PixelTargetSet::new(),
            ref_values: Default::default(),
            scissor: None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum InitError {
    /// Vertex attribute mismatch between the shader and the link data.
    VertexImport(d::AttributeSlot, Option<d::attrib::Format>),
    /// Constant buffer mismatch between the shader and the link data.
    ConstantBuffer(d::ConstantBufferSlot, Option<()>),
    /// Pixel target mismatch between the shader and the link data.
    PixelExport(d::ColorSlot, Option<d::tex::Format>),
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
    pub fn prepare_data<D: PipelineData<R, Meta=M>>(&self, data: &D,
                        handle_man: &mut d::handle::Manager<R>) -> RawDataSet<R>
    {
        data.bake(&self.2, handle_man)
    }
}

pub trait DataLink<'a>: Sized {
    type Init: 'a;
    fn new() -> Self;
    fn link_input(&mut self, _: &d::shade::AttributeVar, _: &Self::Init) ->
                  Option<Result<d::pso::AttributeDesc, d::attrib::Format>> { None }
    fn link_constant_buffer(&mut self, _: &d::shade::ConstantBufferVar, _: &Self::Init) ->
                            Option<Result<(), d::attrib::Format>> { None }
    fn link_constant(&mut self, _: &d::shade::UniformVar, _: &Self::Init) ->
                     Option<Result<(), d::attrib::Format>> { None }
    fn link_output(&mut self, _: &d::shade::OutputVar, _: &Self::Init) ->
                   Option<Result<d::pso::ColorTargetDesc, d::tex::Format>> { None }
}

pub trait DataBind<R: d::Resources> {
    type Data;
    fn bind_to(&self, &mut RawDataSet<R>, &Self::Data, &mut d::handle::Manager<R>);
}

pub trait Structure {
    fn query(&str) -> Option<d::attrib::Format>;
}

pub trait TextureFormat {
    fn get_format() -> d::tex::Format;
}
pub trait BlendFormat: TextureFormat {}
pub trait DepthStencilFormat: TextureFormat {}
pub trait DepthFormat: DepthStencilFormat {}
pub trait StencilFormat: DepthStencilFormat {}

pub type AttributeSlotSet = usize;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FetchRate(d::attrib::InstanceRate);
pub static PER_VERTEX  : FetchRate = FetchRate(0);
pub static PER_INSTANCE: FetchRate = FetchRate(1);

pub struct VertexBuffer<T: Structure>(AttributeSlotSet, PhantomData<T>);
pub struct ConstantBuffer<T: Structure>(Option<d::ConstantBufferSlot>, PhantomData<T>);
pub struct Constant<T: d::attrib::format::ToFormat>(d::shade::Location, PhantomData<T>);
pub struct RenderTargetCommon<T: TextureFormat, I>(d::ColorSlot, PhantomData<(T, I)>);
pub type RenderTarget<T: TextureFormat> = RenderTargetCommon<T, d::state::ColorMask>;
pub type BlendTarget<T: BlendFormat> = RenderTargetCommon<T, d::state::Blend>;
pub struct DepthStencilCommon<T: DepthStencilFormat, I>(PhantomData<(T, I)>);
pub type DepthTarget<T: DepthFormat> = DepthStencilCommon<T, d::state::Depth>;
pub type StencilTarget<T: StencilFormat> = DepthStencilCommon<T, d::state::Stencil>;
pub type DepthStencilTarget<T: DepthStencilFormat> = DepthStencilCommon<T, (d::state::Depth, d::state::Stencil)>;

fn match_attribute(_: &d::shade::AttributeVar, _: d::attrib::Format) -> bool {
    true //TODO
}

impl<'a, T: Structure> DataLink<'a> for VertexBuffer<T> {
    type Init = FetchRate;
    fn new() -> Self {
        VertexBuffer(0, PhantomData)
    }
    fn link_input(&mut self, at: &d::shade::AttributeVar, init: &Self::Init) ->
                  Option<Result<d::pso::AttributeDesc, d::attrib::Format>> {
        T::query(&at.name).map(|format| {
            self.0 |= 1 << (at.slot as AttributeSlotSet);
            if match_attribute(at, format) {
                Ok((format, init.0))
            }else {
                Err(format)
            }
        })
    }
}

impl<R: d::Resources, T: Structure> DataBind<R> for VertexBuffer<T> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(data.raw()), 0));
        for i in 0 .. d::MAX_VERTEX_ATTRIBUTES {
            if (self.0 & (1<<i)) != 0 {
                out.vertex_buffers.0[i] = value;
            }
        }
    }
}

impl<'a, T: Structure> DataLink<'a> for ConstantBuffer<T> {
    type Init = &'a str;
    fn new() -> Self {
        ConstantBuffer(None, PhantomData)
    }
    fn link_constant_buffer(&mut self, cb: &d::shade::ConstantBufferVar, init: &Self::Init) ->
                  Option<Result<(), d::attrib::Format>> {
        if &cb.name == *init {
            self.0 = Some(cb.slot);
            Some(Ok(()))
        }else {
            None
        }
    }
}

impl<R: d::Resources, T: Structure> DataBind<R> for ConstantBuffer<T> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            out.constant_buffers.0[slot as usize] = Some(man.ref_buffer(data.raw()));
        }
    }
}
/*
impl<'a, T: d::attrib::format::ToFormat> DataLink<'a> for Constant<T> {
    type Init = &'a str;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, init: &Self::Init) {
        let (count, etype) = T::describe();
        let format = d::attrib::Format {
            elem_count: count,
            elem_type: etype,
            offset: 0,
            stride: 0,
            instance_rate: 0,
        };
        map.insert(*init, d::pso::Link::Constant(format));
    }
    fn link(map: &d::pso::RegisterMap<'a>, init: &Self::Init) -> Option<Self> {
        map.get(*init).map(|&reg| Constant(reg, PhantomData))
    }
}

impl<R: d::Resources, T: d::attrib::format::ToFormat> DataBind<R> for Constant<T> {
    type Data = d::shade::UniformValue;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, _: &mut d::handle::Manager<R>) {
        out.constants.push((self.0, *data));
    }
}


impl<'a,
    T: TextureFormat,
    I: Copy + Into<d::pso::BlendInfo>
> DataLink<'a> for RenderTargetCommon<T, I> {
    type Init = (&'a str, I);
    fn declare_to(map: &mut d::pso::LinkMap<'a>, init: &Self::Init) {
        map.insert(init.0,
            d::pso::Link::Target(T::get_format(), init.1.into()));
    }
    fn link(map: &d::pso::RegisterMap<'a>, init: &Self::Init) -> Option<Self> {
        map.get(init.0).map(|&reg| RenderTargetCommon(reg as d::ColorSlot, PhantomData))
    }
}

impl<R: d::Resources, T: TextureFormat, I> DataBind<R> for RenderTargetCommon<T, I> {
    type Data = d::handle::RenderTargetView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        out.pixel_targets.0[self.0 as usize] = Some(man.ref_rtv(data.raw()));
    }
}

impl<'a,
    T: DepthStencilFormat,
    I: Copy + Into<d::pso::DepthStencilInfo>
> DataLink<'a> for DepthStencilCommon<T, I> {
    type Init = I;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, init: &Self::Init) {
        map.insert(d::pso::DEPTH_STENCIL_TAG,
            d::pso::Link::DepthStencil(T::get_format(), (*init).into()));
    }
    fn link(map: &d::pso::RegisterMap<'a>, _: &Self::Init) -> Option<Self> {
        map.get(d::pso::DEPTH_STENCIL_TAG).map(|_| DepthStencilCommon(PhantomData))
    }
}

impl<R: d::Resources, T: DepthStencilFormat, I> DataBind<R> for DepthStencilCommon<T, I> {
    type Data = d::handle::DepthStencilView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        out.pixel_targets.1 = Some(man.ref_dsv(data.raw()));
    }
}
*/