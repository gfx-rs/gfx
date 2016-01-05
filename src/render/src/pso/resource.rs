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

//! Buffer components for PSO macro

#![allow(missing_docs)]

use std::marker::PhantomData;
use gfx_core::{handle, shade};
use gfx_core::format::Format;
use gfx_core::{ResourceViewSlot, UnorderedViewSlot, SamplerSlot, Resources};
use gfx_core::factory::Phantom;
use super::{DataLink, DataBind, RawDataSet};


pub struct ShaderResource<T>(Option<ResourceViewSlot>, PhantomData<T>);
pub struct UnorderedAccess<T>(Option<UnorderedViewSlot>, PhantomData<T>);
pub struct Sampler(Option<SamplerSlot>);
/// A convenience type for a texture paired with a sampler.
/// It only makes sense for DX9 class hardware, since everything newer
/// has samplers totally separated from the textures.
pub struct TextureSampler<T>(ShaderResource<T>, Sampler);


impl<'a, T> DataLink<'a> for ShaderResource<T> {
    type Init = &'a str;
    fn new() -> Self {
        ShaderResource(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_resource_view(&mut self, var: &shade::TextureVar, init: &Self::Init)
                          -> Option<Result<(), Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: Resources, T> DataBind<R> for ShaderResource<T> {
    type Data = handle::ShaderResourceView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
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
    fn link_unordered_view(&mut self, var: &shade::UnorderedVar, init: &Self::Init)
                           -> Option<Result<(), Format>> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(Ok(())) //TODO: check format
        }else {
            None
        }
    }
}

impl<R: Resources, T> DataBind<R> for UnorderedAccess<T> {
    type Data = handle::UnorderedAccessView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
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
    fn link_sampler(&mut self, var: &shade::SamplerVar, init: &Self::Init) -> Option<()> {
        if *init == var.name {
            self.0 = Some(var.slot);
            Some(())
        }else {
            None
        }
    }
}

impl<R: Resources> DataBind<R> for Sampler {
    type Data = handle::Sampler<R>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
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
    fn link_resource_view(&mut self, var: &shade::TextureVar, init: &Self::Init)
                          -> Option<Result<(), Format>> {
        self.0.link_resource_view(var, init)
    }
    fn link_sampler(&mut self, var: &shade::SamplerVar, init: &Self::Init) -> Option<()> {
        self.1.link_sampler(var, init)
    }
}

impl<R: Resources, T> DataBind<R> for TextureSampler<T> {
    type Data = (handle::ShaderResourceView<R, T>, handle::Sampler<R>);
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
        self.0.bind_to(out, &data.0, man);
        self.1.bind_to(out, &data.1, man);
    }
}
