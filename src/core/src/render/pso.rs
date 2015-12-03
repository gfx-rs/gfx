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
use device as d;

pub struct RawDataSet<R: d::Resources>{
    pub vertex_buffers: d::pso::VertexBufferSet<R>,
    pub constant_buffers: d::pso::ConstantBufferSet<R>,
    pub constants: Vec<(d::pso::Register, d::shade::UniformValue)>,
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

pub trait PipelineInit<'a> {
    type Meta;
    fn declare(&self) -> d::pso::LinkMap<'a>;
    fn register(&self, &d::pso::RegisterMap<'a>) -> Self::Meta;
}

pub trait PipelineData<R: d::Resources> {
    type Meta;
    fn define(&self, meta: &Self::Meta, &mut d::handle::Manager<R>)
              -> RawDataSet<R>;
}

/// Strongly-typed compiled pipeline state
pub struct PipelineState<R: d::Resources, M>(
    d::handle::RawPipelineState<R>, d::PrimitiveType, M);

impl<R: d::Resources, M> PipelineState<R, M> {
    pub fn new(raw: d::handle::RawPipelineState<R>, pt: d::PrimitiveType,
               meta: M) -> PipelineState<R, M> {
        PipelineState(raw, pt, meta)
    }
    pub fn get_handle(&self) -> &d::handle::RawPipelineState<R> {
        &self.0
    }
    pub fn prepare_data<D: PipelineData<R, Meta=M>>(&self, data: &D,
                        handle_man: &mut d::handle::Manager<R>) -> RawDataSet<R>
    {
        data.define(&self.2, handle_man)
    }
}


pub trait DataLink<'a>: Sized {
    type Init;
    fn declare_to(&mut d::pso::LinkMap<'a>, &Self::Init);
    fn link(&d::pso::RegisterMap<'a>, &Self::Init) -> Option<Self>;
}

pub trait DataBind<R: d::Resources> {
    type Data;
    fn bind_to(&self, &mut RawDataSet<R>, &Self::Data, &mut d::handle::Manager<R>);
}

pub trait Structure {
    type Meta;
    fn iter_fields<F: FnMut(&'static str, d::attrib::Format)>(F);
    fn make_meta<F: Fn(&str) -> Option<d::pso::Register>>(F) -> Self::Meta;
    fn iter_meta<F: FnMut(d::pso::Register)>(&Self::Meta, F);
}

pub trait TextureFormat {
    fn get_format() -> d::tex::Format;
}
pub trait BlendFormat: TextureFormat {}
pub trait DepthStencilFormat: TextureFormat {}
pub trait DepthFormat: DepthStencilFormat {}
pub trait StencilFormat: DepthStencilFormat {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FetchRate(d::attrib::InstanceRate);
pub static PER_VERTEX  : FetchRate = FetchRate(0);
pub static PER_INSTANCE: FetchRate = FetchRate(1);

pub struct VertexBuffer<T: Structure>(T::Meta);
pub struct ConstantBuffer<T: Structure>(d::pso::Register, PhantomData<T>);
pub struct Constant<T: d::attrib::format::ToFormat>(d::pso::Register, PhantomData<T>);
pub struct RenderTargetCommon<T: TextureFormat, I>(d::ColorSlot, PhantomData<(T, I)>);
pub type RenderTarget<T: TextureFormat> = RenderTargetCommon<T, d::state::ColorMask>;
pub type BlendTarget<T: BlendFormat> = RenderTargetCommon<T, d::state::Blend>;
pub struct DepthStencilCommon<T: DepthStencilFormat, I>(PhantomData<(T, I)>);
pub type DepthTarget<T: DepthFormat> = DepthStencilCommon<T, d::state::Depth>;
pub type StencilTarget<T: StencilFormat> = DepthStencilCommon<T, d::state::Stencil>;
pub type DepthStencilTarget<T: DepthStencilFormat> = DepthStencilCommon<T, (d::state::Depth, d::state::Stencil)>;


impl<'a, T: Structure> DataLink<'a> for VertexBuffer<T> {
    type Init = FetchRate;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, init: &Self::Init) {
        T::iter_fields(|name, mut format| {
            format.instance_rate = init.0;
            map.insert(name, d::pso::Link::Attribute(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>, _: &Self::Init) -> Option<Self> {
        let meta = T::make_meta(|name| map.get(name).map(|&reg| reg));
        Some(VertexBuffer(meta))
    }
}

impl<R: d::Resources, T: Structure> DataBind<R> for VertexBuffer<T> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(data.raw()), 0));
        T::iter_meta(&self.0, |reg| out.vertex_buffers.0[reg as usize] = value);
    }
}

impl<'a, T: Structure> DataLink<'a> for ConstantBuffer<T> {
    type Init = &'a str;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, init: &Self::Init) {
        map.insert(*init, d::pso::Link::ConstantBuffer);
        T::iter_fields(|name, format| {
            map.insert(name, d::pso::Link::Constant(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>, init: &Self::Init) -> Option<Self> {
        map.get(*init).map(|&reg| ConstantBuffer(reg, PhantomData))
    }
}

impl<R: d::Resources, T: Structure> DataBind<R> for ConstantBuffer<T> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        out.constant_buffers.0[self.0 as usize] = Some(man.ref_buffer(data.raw()));
    }
}

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
