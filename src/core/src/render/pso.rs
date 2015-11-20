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
    fn declare_to(&mut d::pso::LinkMap<'a>);
    fn link(&d::pso::RegisterMap<'a>) -> Option<Self::Link>;
    fn bind_to(&self, &mut ShaderDataSet<R>, &Self::Link, &mut d::handle::Manager<R>);
}

pub trait Structure {
    type Meta;
    fn iter_fields<F: FnMut(&'static str, d::attrib::Format)>(F);
    fn make_meta<F: Fn(&str) -> Option<d::pso::Register>>(F) -> Self::Meta;
    fn iter_meta<F: FnMut(d::pso::Register)>(&Self::Meta, F);
}

#[derive(Clone, Debug)]
pub struct VertexBuffer<R: d::Resources, T>(pub d::handle::Buffer<R, T>);
#[derive(Clone, Debug)]
pub struct InstanceBuffer<R: d::Resources, T>(pub d::handle::Buffer<R, T>);
#[derive(Clone, Debug)]
pub struct ConstantBuffer<R: d::Resources, T>(pub d::handle::Buffer<R, T>);

impl<'a, R: d::Resources, T: Structure> DataLink<'a, R> for VertexBuffer<R, T> {
    type Link = T::Meta;
    fn declare_to(map: &mut d::pso::LinkMap<'a>) {
        T::iter_fields(|name, format| {
            map.insert(name, d::pso::Link::Attribute(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>) -> Option<T::Meta> {
        Some(T::make_meta(|name| map.get(name).map(|&reg| reg)))
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &T::Meta, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(self.0.raw()), 0));
        T::iter_meta(meta, |reg| data.vertex_buffers.0[reg as usize] = value);
    }
}

impl<'a, R: d::Resources, T: Structure> DataLink<'a, R> for InstanceBuffer<R, T> {
    type Link = T::Meta;
    fn declare_to(map: &mut d::pso::LinkMap<'a>) {
        T::iter_fields(|name, mut format| {
            format.instance_rate = 1;
            map.insert(name, d::pso::Link::Attribute(format));
        });
    }
    fn link(map: &d::pso::RegisterMap<'a>) -> Option<T::Meta> {
        Some(T::make_meta(|name| map.get(name).map(|&reg| reg)))
    }
    fn bind_to(&self, data: &mut ShaderDataSet<R>, meta: &T::Meta, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(self.0.raw()), 0));
        T::iter_meta(meta, |reg| data.vertex_buffers.0[reg as usize] = value);
    }
}
