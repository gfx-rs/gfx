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

pub mod target;

use std::default::Default;
use std::marker::PhantomData;
use gfx_core as d;
use gfx_core::factory::Phantom;
pub use gfx_core::pso::{Element, ElemOffset, ElemStride, Descriptor};
use shade::ToUniform;

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

pub trait Structure<F> {
    fn query(&str) -> Option<Element<F>>;
}

type AttributeSlotSet = usize;

//Note: in order to increase the clarity of potential error messages,
//it would make sense to put the generic bounds right here in the definitions.
//However, this triggers Rust warning E0122, so will have to wait a bit.

pub struct VertexBufferCommon<T, I>(AttributeSlotSet, PhantomData<(T, I)>);
pub type VertexBuffer<T> = VertexBufferCommon<T, [(); 0]>;
pub type InstanceBuffer<T> = VertexBufferCommon<T, [(); 1]>;
pub struct ConstantBuffer<T: Structure<d::shade::ConstFormat>>(Option<d::ConstantBufferSlot>, PhantomData<T>);
pub struct Global<T: ToUniform>(Option<d::shade::Location>, PhantomData<T>);
pub struct ShaderResource<T>(Option<d::ResourceViewSlot>, PhantomData<T>);
pub struct UnorderedAccess<T>(Option<d::UnorderedViewSlot>, PhantomData<T>);
pub struct Sampler(Option<d::SamplerSlot>);
/// A convenience type for a texture paired with a sampler.
/// It only makes sense for DX9 class hardware, since everything newer
/// has samplers totally separated from the textures.
pub struct TextureSampler<T>(ShaderResource<T>, Sampler);

fn match_attribute(_: &d::shade::AttributeVar, _: d::format::Format) -> bool {
    true //TODO
}

impl<'a,
    T: Structure<d::format::Format>,
    I: AsRef<[()]> + Default,
> DataLink<'a> for VertexBufferCommon<T, I> {
    type Init = ();
    fn new() -> Self {
        VertexBufferCommon(0, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0 != 0
    }
    fn link_input(&mut self, at: &d::shade::AttributeVar, _: &Self::Init) ->
                  Option<Result<d::pso::AttributeDesc, d::format::Format>> {
        T::query(&at.name).map(|el| {
            self.0 |= 1 << (at.slot as AttributeSlotSet);
            if match_attribute(at, el.format) {
                let rate = <I as Default>::default().as_ref().len();
                Ok((el, rate as d::pso::InstanceRate))
            }else {
                Err(el.format)
            }
        })
    }
}

impl<R: d::Resources, T, I> DataBind<R> for VertexBufferCommon<T, I> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        let value = Some((man.ref_buffer(data.raw()).clone(), 0));
        for i in 0 .. d::MAX_VERTEX_ATTRIBUTES {
            if (self.0 & (1<<i)) != 0 {
                out.vertex_buffers.0[i] = value;
            }
        }
    }
}

impl<'a, T: Structure<d::shade::ConstFormat>>
DataLink<'a> for ConstantBuffer<T> {
    type Init = &'a str;
    fn new() -> Self {
        ConstantBuffer(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_constant_buffer(&mut self, cb: &d::shade::ConstantBufferVar, init: &Self::Init) ->
                            Option<Result<(), d::shade::ConstFormat>> {
        if &cb.name == *init {
            self.0 = Some(cb.slot);
            Some(Ok(()))
        }else {
            None
        }
    }
}

impl<R: d::Resources, T: Structure<d::shade::ConstFormat>>
DataBind<R> for ConstantBuffer<T> {
    type Data = d::handle::Buffer<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_buffer(data.raw()).clone());
            out.constant_buffers.0[slot as usize] = value;
        }
    }
}

impl<'a, T: ToUniform> DataLink<'a> for Global<T> {
    type Init = &'a str;
    fn new() -> Self {
        Global(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_global_constant(&mut self, var: &d::shade::ConstVar, init: &Self::Init) ->
                            Option<Result<(), d::shade::UniformValue>> {
        if &var.name == *init {
            //if match_constant(var, ())
            self.0 = Some(var.location);
            Some(Ok(()))
        }else {
            None
        }
    }
}

impl<R: d::Resources, T: ToUniform> DataBind<R> for Global<T> {
    type Data = T;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, _: &mut d::handle::Manager<R>) {
        if let Some(loc) = self.0 {
            let value = data.convert();
            out.global_constants.push((loc, value));
        }
    }
}

impl<'a, T> DataLink<'a> for ShaderResource<T> {
    type Init = &'a str;
    fn new() -> Self {
        ShaderResource(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_resource_view(&mut self, var: &d::shade::TextureVar, init: &Self::Init)
                          -> Option<Result<(), d::format::Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: d::Resources, T> DataBind<R> for ShaderResource<T> {
    type Data = d::handle::ShaderResourceView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_srv(data.raw()).clone());
            out.resource_views.0[slot as usize] = value;
        }
    }
}

impl<'a, T> DataLink<'a> for UnorderedAccess<T> {
    type Init = &'a str;
    fn new() -> Self {
        UnorderedAccess(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_unordered_view(&mut self, var: &d::shade::UnorderedVar, init: &Self::Init)
                           -> Option<Result<(), d::format::Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: d::Resources, T> DataBind<R> for UnorderedAccess<T> {
    type Data = d::handle::UnorderedAccessView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_uav(data.raw()).clone());
            out.unordered_views.0[slot as usize] = value;
        }
    }
}

impl<'a> DataLink<'a> for Sampler {
    type Init = &'a str;
    fn new() -> Self {
        Sampler(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_sampler(&mut self, var: &d::shade::SamplerVar, init: &Self::Init) -> Option<()> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(())
        }else {
            None
        }
    }
}

impl<R: d::Resources> DataBind<R> for Sampler {
    type Data = d::handle::Sampler<R>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_sampler(data).clone());
            out.samplers.0[slot as usize] = value;
        }
    }
}

impl<'a, T> DataLink<'a> for TextureSampler<T> {
    type Init = &'a str;
    fn new() -> Self {
        TextureSampler(ShaderResource::new(), Sampler::new())
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_resource_view(&mut self, var: &d::shade::TextureVar, init: &Self::Init)
                          -> Option<Result<(), d::format::Format>> {
        self.0.link_resource_view(var, init)
    }
    fn link_sampler(&mut self, var: &d::shade::SamplerVar, init: &Self::Init) -> Option<()> {
        self.1.link_sampler(var, init)
    }
}

impl<R: d::Resources, T> DataBind<R> for TextureSampler<T> {
    type Data = (d::handle::ShaderResourceView<R, T>, d::handle::Sampler<R>);
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        self.0.bind_to(out, &data.0, man);
        self.1.bind_to(out, &data.1, man);
    }
}
