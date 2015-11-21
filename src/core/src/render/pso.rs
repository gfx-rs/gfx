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

use std::marker::PhantomData;
use device as d;
use render::target;

pub struct ShaderDataSet<R: d::Resources>{
    pub vertex_buffers: d::pso::VertexBufferSet<R>,
    pub pixel_targets: d::pso::PixelTargetSet<R>,
    //TODO: add more, move to the device side
}

impl<R: d::Resources> ShaderDataSet<R> {
    pub fn new() -> ShaderDataSet<R> {
        ShaderDataSet {
            vertex_buffers: d::pso::VertexBufferSet::new(),
            pixel_targets: d::pso::PixelTargetSet::new(),
        }
    }
}

pub trait LinkBuilder<'a, M> {
    fn declare() -> d::pso::LinkMap<'a>;
    fn register(&d::pso::RegisterMap<'a>) -> M;
}

pub trait ShaderLink<R: d::Resources> {
    type Meta;
    fn define(&self, meta: &Self::Meta, &mut d::handle::Manager<R>)
              -> ShaderDataSet<R>;
}

/// Strongly-typed compiled pipeline state
pub struct PipelineState<R: d::Resources, L: ShaderLink<R>>(
    d::handle::RawPipelineState<R>, d::PrimitiveType, L::Meta);

impl<R: d::Resources, L: ShaderLink<R>> PipelineState<R, L> {
    pub fn new(raw: d::handle::RawPipelineState<R>, pt: d::PrimitiveType,
               meta: L::Meta) -> PipelineState<R, L> {
        PipelineState(raw, pt, meta)
    }
    pub fn get_handle(&self) -> &d::handle::RawPipelineState<R> {
        &self.0
    }
    pub fn prepare_data(&self, data: &L, handle_man: &mut d::handle::Manager<R>)
                        -> ShaderDataSet<R>
    {
        data.define(&self.2, handle_man)
    }
}

pub trait DataLink<'a, R: d::Resources> {
    type Link;
    fn declare_to(&mut d::pso::LinkMap<'a>, &'a str);
    fn link(&d::pso::RegisterMap<'a>, &'a str) -> Option<Self::Link>;
    fn bind_to(&self, &mut ShaderDataSet<R>, &Self::Link, &mut d::handle::Manager<R>);
}

pub trait Structure {
    type Meta;
    fn iter_fields<F: FnMut(&'static str, d::attrib::Format)>(F);
    fn make_meta<F: Fn(&str) -> Option<d::pso::Register>>(F) -> Self::Meta;
    fn iter_meta<F: FnMut(d::pso::Register)>(&Self::Meta, F);
}

pub trait Instancing {
    fn get_rate() -> d::attrib::InstanceRate;
}

#[derive(Clone, Debug)]
pub enum PerInstance {}

impl Instancing for () {
    fn get_rate() -> d::attrib::InstanceRate { 0 }
}

impl Instancing for PerInstance {
    fn get_rate() -> d::attrib::InstanceRate { 1 }
}


#[derive(Clone, Debug)]
pub struct VertexBuffer<R: d::Resources, T, I>(d::handle::Buffer<R, T>, PhantomData<I>);

impl<'a, R: d::Resources, T: Structure, I: Instancing> DataLink<'a, R> for VertexBuffer<R, T, I> {
    type Link = T::Meta;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, _: &'a str) {
        T::iter_fields(|name, mut format| {
            format.instance_rate = I::get_rate();
            map.insert(name, d::pso::Link::Attribute(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>, _: &'a str) -> Option<Self::Link> {
        Some(T::make_meta(|name| map.get(name).map(|&reg| reg)))
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &Self::Link, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(self.0.raw()), 0));
        T::iter_meta(meta, |reg| data.vertex_buffers.0[reg as usize] = value);
    }
}

#[derive(Clone, Debug)]
pub struct ConstantBuffer<R: d::Resources, T>(pub d::handle::Buffer<R, T>);

impl<'a, R: d::Resources, T: Structure> DataLink<'a, R> for ConstantBuffer<R, T> {
    type Link = d::pso::Register;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, buf_name: &'a str) {
        map.insert(buf_name, d::pso::Link::ConstantBuffer);
        T::iter_fields(|name, format| {
            map.insert(name, d::pso::Link::Constant(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>, buf_name: &'a str) -> Option<Self::Link> {
        map.get(buf_name).map(|&reg| reg)
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &Self::Link, man: &mut d::handle::Manager<R>) {
        data.vertex_buffers.0[*meta as usize] = Some((man.ref_buffer(self.0.raw()), 0));
    }
}

pub trait TextureFormat {
    fn get_format() -> d::tex::Format;
}

impl TextureFormat for [f32; 4] {
    fn get_format() -> d::tex::Format {
        d::tex::RGBA8
    }
}

#[derive(Clone, Debug)]
pub struct RenderView<R: d::Resources, T>(target::Plane<R>, PhantomData<T>);

impl<R: d::Resources, T> From<d::handle::Surface<R>> for RenderView<R, T> {
    fn from(h: d::handle::Surface<R>) -> RenderView<R, T> {
        //TODO: match T with surface format
        RenderView(target::Plane::Surface(h), PhantomData)
    }
}

impl<R: d::Resources, T> From<d::handle::Texture<R>> for RenderView<R, T> {
    fn from(h: d::handle::Texture<R>) -> RenderView<R, T> {
        //TODO: match T with texture format
        RenderView(target::Plane::Texture(h, 0, None), PhantomData)
    }
}

impl<'a, R: d::Resources, T: TextureFormat> DataLink<'a, R> for RenderView<R, T> {
    type Link = d::pso::Register;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, name: &'a str) {
        map.insert(name, d::pso::Link::Target(T::get_format()));
    }
    fn link(map: &d::pso::RegisterMap<'a>, name: &'a str) -> Option<Self::Link> {
        map.get(name).map(|&reg| reg)
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &Self::Link, man: &mut d::handle::Manager<R>) {
        let _ = (data, meta, man);
        //data.render_targets.0[*meta as usize] = Some((man.ref_buffer(self.0.raw()), 0)); //TODO!
    }
}

#[derive(Clone, Debug)]
pub struct DepthStencilView<R: d::Resources>(pub target::Plane<R>);

impl<'a, R: d::Resources> DataLink<'a, R> for DepthStencilView<R> {
    type Link = d::pso::Register;
    fn declare_to(map: &mut d::pso::LinkMap<'a>, name: &'a str) {
        map.insert(name, d::pso::Link::DepthStencil(d::tex::Format::DEPTH24_STENCIL8));
    }
    fn link(map: &d::pso::RegisterMap<'a>, name: &'a str) -> Option<Self::Link> {
        map.get(name).map(|&reg| reg)
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &Self::Link, man: &mut d::handle::Manager<R>) {
        let _ = (data, meta, man);
        //data.render_targets.0[*meta as usize] = Some((man.ref_buffer(self.0.raw()), 0)); //TODO!
    }
}
