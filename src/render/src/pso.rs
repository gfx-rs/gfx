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
    VertexImport(d::AttributeSlot, Option<d::attrib::Format>),
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
                  Option<Result<d::pso::AttributeDesc, d::attrib::Format>> { None }
    fn link_constant_buffer(&mut self, _: &d::shade::ConstantBufferVar, _: &Self::Init) ->
                            Option<Result<(), d::attrib::Format>> { None }
    fn link_global_constant(&mut self, _: &d::shade::ConstVar, _: &Self::Init) ->
                            Option<Result<(), d::attrib::Format>> { None }
    fn link_output(&mut self, _: &d::shade::OutputVar, _: &Self::Init) ->
                   Option<Result<d::pso::ColorTargetDesc, d::tex::Format>> { None }
    fn link_depth_stencil(&mut self, _: &Self::Init) ->
                          Option<d::pso::DepthStencilDesc> { None }
    fn link_resource_view(&mut self, _: &d::shade::TextureVar, _: &Self::Init) ->
                          Option<Result<(), d::tex::Format>> { None }
    fn link_unordered_view(&mut self, _: &d::shade::UnorderedVar, _: &Self::Init) ->
                           Option<Result<(), d::tex::Format>> { None }
    fn link_sampler(&mut self, _: &d::shade::SamplerVar, _: &Self::Init) -> Option<()> { None }
}

pub trait DataBind<R: d::Resources> {
    type Data;
    fn bind_to(&self, &mut RawDataSet<R>, &Self::Data, &mut d::handle::Manager<R>);
}

pub trait Structure {
    fn query(&str) -> Option<d::attrib::Format>;
}

pub type AttributeSlotSet = usize;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FetchRate(d::attrib::InstanceRate);
pub static PER_VERTEX  : FetchRate = FetchRate(0);
pub static PER_INSTANCE: FetchRate = FetchRate(1);

pub struct VertexBuffer<T: Structure>(AttributeSlotSet, PhantomData<T>);
pub struct ConstantBuffer<T: Structure>(Option<d::ConstantBufferSlot>, PhantomData<T>);
pub struct Global<T: d::attrib::format::ToFormat>(Option<d::shade::Location>, PhantomData<T>);
pub struct ResourceView<T>(Option<d::ResourceViewSlot>, PhantomData<T>);
pub struct UnorderedView<T>(Option<d::UnorderedViewSlot>, PhantomData<T>);
pub struct Sampler(Option<d::SamplerSlot>);
pub struct RenderTargetCommon<T, I>(Option<d::ColorSlot>, PhantomData<(T, I)>);
pub type RenderTarget<T: d::format::RenderFormat> = RenderTargetCommon<T, d::state::ColorMask>;
pub type BlendTarget<T: d::format::BlendFormat> = RenderTargetCommon<T, d::state::Blend>;
pub struct DepthStencilCommon<T, I>(PhantomData<(T, I)>);
pub type DepthTarget<T: d::format::DepthStencilFormat> = DepthStencilCommon<T, d::state::Depth>;
pub type StencilTarget<T: d::format::DepthStencilFormat> = DepthStencilCommon<T, d::state::Stencil>;
pub type DepthStencilTarget<T: d::format::DepthStencilFormat> = DepthStencilCommon<T, (d::state::Depth, d::state::Stencil)>;

fn match_attribute(_: &d::shade::AttributeVar, _: d::attrib::Format) -> bool {
    true //TODO
}

impl<'a, T: Structure> DataLink<'a> for VertexBuffer<T> {
    type Init = FetchRate;
    fn new() -> Self {
        VertexBuffer(0, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0 != 0
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
        let value = Some((man.ref_buffer(data.raw()).clone(), 0));
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
    fn is_active(&self) -> bool {
        self.0.is_some()
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
            let value = Some(man.ref_buffer(data.raw()).clone());
            out.constant_buffers.0[slot as usize] = value;
        }
    }
}

impl<'a, T: d::attrib::format::ToFormat> DataLink<'a> for Global<T> {
    type Init = &'a str;
    fn new() -> Self {
        Global(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_global_constant(&mut self, var: &d::shade::ConstVar, init: &Self::Init) ->
                            Option<Result<(), d::attrib::Format>> {
        if &var.name == *init {
            //if match_constant(var, ())
            self.0 = Some(var.location);
            Some(Ok(()))
        }else {
            None
        }
    }
}

impl<R: d::Resources, T: d::attrib::format::ToFormat> DataBind<R> for Global<T> {
    type Data = d::shade::UniformValue;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, _: &mut d::handle::Manager<R>) {
        if let Some(loc) = self.0 {
            out.global_constants.push((loc, *data));
        }
    }
}

impl<'a, T> DataLink<'a> for ResourceView<T> {
    type Init = &'a str;
    fn new() -> Self {
        ResourceView(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_resource_view(&mut self, var: &d::shade::TextureVar, init: &Self::Init)
                          -> Option<Result<(), d::tex::Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: d::Resources, T> DataBind<R> for ResourceView<T> {
    type Data = d::handle::ShaderResourceView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_srv(data.raw()).clone());
            out.resource_views.0[slot as usize] = value;
        }
    }
}

impl<'a, T> DataLink<'a> for UnorderedView<T> {
    type Init = &'a str;
    fn new() -> Self {
        UnorderedView(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_unordered_view(&mut self, var: &d::shade::UnorderedVar, init: &Self::Init)
                           -> Option<Result<(), d::tex::Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: d::Resources, T> DataBind<R> for UnorderedView<T> {
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

impl<'a,
    T: d::format::RenderFormat,
    I: 'a + Copy + Into<d::pso::BlendInfo>
> DataLink<'a> for RenderTargetCommon<T, I> {
    type Init = (&'a str, I);
    fn new() -> Self {
        RenderTargetCommon(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_output(&mut self, out: &d::shade::OutputVar, init: &Self::Init) ->
                   Option<Result<d::pso::ColorTargetDesc, d::tex::Format>> {
        if &out.name == init.0 {
            self.0 = Some(out.slot);
            let (st, view) = T::get_format();
            let desc = (st, view, init.1.into());
            Some(Ok(desc))
        }else {
            None
        }
    }
}

impl<R: d::Resources, T, I> DataBind<R> for RenderTargetCommon<T, I> {
    type Data = d::handle::RenderTargetView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        if let Some(slot) = self.0 {
            let value = Some(man.ref_rtv(data.raw()).clone());
            out.pixel_targets.colors[slot as usize] = value;
        }
    }
}

impl<'a,
    T: d::format::DepthStencilFormat,
    I: 'a + Copy + Into<d::pso::DepthStencilInfo>
> DataLink<'a> for DepthStencilCommon<T, I> {
    type Init = I;
    fn new() -> Self {
        DepthStencilCommon(PhantomData)
    }
    fn is_active(&self) -> bool {
        true
    }
    fn link_depth_stencil(&mut self, init: &Self::Init) ->
                          Option<d::pso::DepthStencilDesc> {
        let (st, _) = T::get_format();
        let desc = (st, (*init).into());
        Some(desc)
    }
}

impl<R: d::Resources, T, I> DataBind<R> for DepthStencilCommon<T, I> {
    type Data = d::handle::DepthStencilView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut d::handle::Manager<R>) {
        let value = Some(man.ref_dsv(data.raw()).clone());
        out.pixel_targets.depth_stencil = value;
    }
}
