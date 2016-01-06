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

//! Render target components for the PSO macro

#![allow(missing_docs)]

use std::marker::PhantomData;
use gfx_core::{format, handle, pso, state, target};
use gfx_core::{ColorSlot, Resources};
use gfx_core::factory::Phantom;
use gfx_core::shade::OutputVar;
use super::{DataLink, DataBind, RawDataSet};


pub struct RenderTarget<T>(Option<ColorSlot>, PhantomData<T>);
pub struct BlendTarget<T>(Option<ColorSlot>, PhantomData<T>);
pub struct DepthStencilCommon<T, I>(bool, bool, PhantomData<(T, I)>);
pub type DepthTarget<T> = DepthStencilCommon<T, state::Depth>;
pub type StencilTarget<T> = DepthStencilCommon<T, state::Stencil>;
pub type DepthStencilTarget<T> = DepthStencilCommon<T, (state::Depth, state::Stencil)>;
pub struct Scissor;
pub struct BlendRef;


impl<'a, T: format::RenderFormat> DataLink<'a> for RenderTarget<T> {
    type Init = &'a str;
    fn new() -> Self {
        RenderTarget(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_output(&mut self, out: &OutputVar, init: &Self::Init) ->
                   Option<Result<pso::ColorTargetDesc, format::Format>> {
        if out.name.is_empty() || &out.name == init {
            self.0 = Some(out.slot);
            let desc = (T::get_format(), state::MASK_ALL.into());
            Some(Ok(desc))
        }else {
            None
        }
    }
}

impl<R: Resources, T> DataBind<R> for RenderTarget<T> {
    type Data = handle::RenderTargetView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
        if let Some(slot) = self.0 {
            out.pixel_targets.add_color(slot, man.ref_rtv(data.raw()), data.raw().get_dimensions());
        }
    }
}


impl<'a, T: format::BlendFormat> DataLink<'a> for BlendTarget<T> {
    type Init = (&'a str, state::ColorMask, state::Blend);
    fn new() -> Self {
        BlendTarget(None, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_output(&mut self, out: &OutputVar, init: &Self::Init) ->
                   Option<Result<pso::ColorTargetDesc, format::Format>> {
        if out.name.is_empty() || &out.name == init.0 {
            self.0 = Some(out.slot);
            let desc = (T::get_format(), pso::BlendInfo {
                mask: init.1,
                color: Some(init.2.color),
                alpha: Some(init.2.alpha),
            });
            Some(Ok(desc))
        }else {
            None
        }
    }
}

impl<R: Resources, T> DataBind<R> for BlendTarget<T> {
    type Data = handle::RenderTargetView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
        if let Some(slot) = self.0 {
            out.pixel_targets.add_color(slot, man.ref_rtv(data.raw()), data.raw().get_dimensions());
        }
    }
}

impl<'a,
    T: format::Formatted,
    I: 'a + Copy + Into<pso::DepthStencilInfo>
> DataLink<'a> for DepthStencilCommon<T, I> {
    type Init = I;
    fn new() -> Self {
        DepthStencilCommon(false, false, PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0 || self.1
    }
    fn link_depth_stencil(&mut self, init: &Self::Init) ->
                          Option<pso::DepthStencilDesc> {
        let format = T::get_format();
        let info = (*init).into();
        self.0 = info.depth.is_some();
        self.1 = info.front.is_some() || info.back.is_some();
        Some((format.0, info))
    }
}

impl<R: Resources, T, I> DataBind<R> for DepthStencilCommon<T, I> {
    type Data = handle::DepthStencilView<R, T>;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, man: &mut handle::Manager<R>) {
        out.pixel_targets.add_depth_stencil(man.ref_dsv(data.raw()),
            self.0, self.1, data.raw().get_dimensions());
    }
}

impl<'a> DataLink<'a> for Scissor {
    type Init = ();
    fn new() -> Self { Scissor }
    fn is_active(&self) -> bool { true }
}

impl<R: Resources> DataBind<R> for Scissor {
    type Data = target::Rect;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, _: &mut handle::Manager<R>) {
        out.scissor = Some(*data);
    }
}

impl<'a> DataLink<'a> for BlendRef {
    type Init = ();
    fn new() -> Self { BlendRef }
    fn is_active(&self) -> bool { true }
}

impl<R: Resources> DataBind<R> for BlendRef {
    type Data = target::ColorValue;
    fn bind_to(&self, out: &mut RawDataSet<R>, data: &Self::Data, _: &mut handle::Manager<R>) {
        out.ref_values.blend = *data;
    }
}
