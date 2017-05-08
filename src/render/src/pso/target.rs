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

//! Render target components for a PSO.

use std::marker::PhantomData;
use core::{ColorSlot, Resources};
use core::{format, handle, pso, state, target};
use core::memory::Typed;
use core::shade::OutputVar;
use super::{DataLink, DataBind, RawDataSet, AccessInfo};

/// Render target component. Typically points to a color-formatted texture.
///
/// - init: `&str` = name of the target
/// - data: `RenderTargetView<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RenderTarget<T>(
    Option<ColorSlot>,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Render target component with active blending mode.
///
/// - init: (`&str`, `ColorMask`, `Blend` = blending state)
/// - data: `RenderTargetView<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BlendTarget<T>(
    RawRenderTarget,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Raw (untyped) render target component with optional blending.
///
/// - init: (`&str`, `Format`, `ColorMask`, `Option<Blend>`)
/// - data: `RawRenderTargetView`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RawRenderTarget(Option<ColorSlot>);

/// Depth target component.
///
/// - init: `Depth` = depth state
/// - data: `DepthStencilView<T>`
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DepthTarget<T>(
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Stencil target component.
///
/// - init: `Stencil` = stencil state
/// - data: (`DepthStencilView<T>`, `(front, back)` = stencil reference values)
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StencilTarget<T>(
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Depth + stencil target component.
///
/// - init: (`Depth` = depth state, `Stencil` = stencil state)
/// - data: (`DepthStencilView<T>`, `(front, back)` = stencil reference values)
#[derive(Derivative)]
#[derivative(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DepthStencilTarget<T>(
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    PhantomData<T>
);

/// Scissor component. Sets up the scissor test for rendering.
///
/// - init: `()`
/// - data: `Rect` = target area
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Scissor(bool);

/// Blend reference component. Sets up the reference color for blending.
///
/// - init: `()`
/// - data: `ColorValue`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
        } else {
            None
        }
    }
}

impl<R: Resources, T> DataBind<R> for RenderTarget<T> {
    type Data = handle::RenderTargetView<R, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        if let Some(slot) = self.0 {
            out.pixel_targets.add_color(slot, man.ref_rtv(data.raw()), data.raw().get_dimensions());
        }
    }
}


impl<'a, T: format::BlendFormat> DataLink<'a> for BlendTarget<T> {
    type Init = (&'a str, state::ColorMask, state::Blend);
    fn new() -> Self {
        BlendTarget(RawRenderTarget(None), PhantomData)
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn link_output(&mut self, out: &OutputVar, init: &Self::Init) ->
                   Option<Result<pso::ColorTargetDesc, format::Format>> {
        self.0.link_output(out, &(init.0, T::get_format(), init.1, Some(init.2)))
    }
}

impl<R: Resources, T> DataBind<R> for BlendTarget<T> {
    type Data = handle::RenderTargetView<R, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               access: &mut AccessInfo<R>) {
        self.0.bind_to(out, data.raw(), man, access)
    }
}

impl<'a> DataLink<'a> for RawRenderTarget {
    type Init = (&'a str, format::Format, state::ColorMask, Option<state::Blend>);
    fn new() -> Self {
        RawRenderTarget(None)
    }
    fn is_active(&self) -> bool {
        self.0.is_some()
    }
    fn link_output(&mut self, out: &OutputVar, init: &Self::Init) ->
                   Option<Result<pso::ColorTargetDesc, format::Format>> {
        if out.name.is_empty() || &out.name == init.0 {
            self.0 = Some(out.slot);
            let desc = (init.1, pso::ColorInfo {
                mask: init.2,
                color: init.3.map(|b| b.color),
                alpha: init.3.map(|b| b.alpha),
            });
            Some(Ok(desc))
        }else {
            None
        }
    }
}

impl<R: Resources> DataBind<R> for RawRenderTarget {
    type Data = handle::RawRenderTargetView<R>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        if let Some(slot) = self.0 {
            out.pixel_targets.add_color(slot, man.ref_rtv(data), data.get_dimensions());
        }
    }
}


impl<'a, T: format::DepthFormat> DataLink<'a> for DepthTarget<T> {
    type Init = state::Depth;
    fn new() -> Self { DepthTarget(PhantomData) }
    fn is_active(&self) -> bool { true }
    fn link_depth_stencil(&mut self, init: &Self::Init) -> Option<pso::DepthStencilDesc> {
        Some((T::get_format(), (*init).into()))
    }
}

impl<R: Resources, T> DataBind<R> for DepthTarget<T> {
    type Data = handle::DepthStencilView<R, T>;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        let dsv = data.raw();
        out.pixel_targets.add_depth_stencil(man.ref_dsv(dsv), true, false, dsv.get_dimensions());
    }
}

impl<'a, T: format::StencilFormat> DataLink<'a> for StencilTarget<T> {
    type Init = state::Stencil;
    fn new() -> Self { StencilTarget(PhantomData) }
    fn is_active(&self) -> bool { true }
    fn link_depth_stencil(&mut self, init: &Self::Init) -> Option<pso::DepthStencilDesc> {
        Some((T::get_format(), (*init).into()))
    }
}

impl<R: Resources, T> DataBind<R> for StencilTarget<T> {
    type Data = (handle::DepthStencilView<R, T>, (target::Stencil, target::Stencil));
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        let dsv = data.0.raw();
        out.pixel_targets.add_depth_stencil(man.ref_dsv(dsv), false, true, dsv.get_dimensions());
        out.ref_values.stencil = data.1;
    }
}

impl<'a, T: format::DepthStencilFormat> DataLink<'a> for DepthStencilTarget<T> {
    type Init = (state::Depth, state::Stencil);
    fn new() -> Self { DepthStencilTarget(PhantomData) }
    fn is_active(&self) -> bool { true }
    fn link_depth_stencil(&mut self, init: &Self::Init) -> Option<pso::DepthStencilDesc> {
        Some((T::get_format(), (*init).into()))
    }
}

impl<R: Resources, T> DataBind<R> for DepthStencilTarget<T> {
    type Data = (handle::DepthStencilView<R, T>, (target::Stencil, target::Stencil));
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               man: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        let dsv = data.0.raw();
        out.pixel_targets.add_depth_stencil(man.ref_dsv(dsv), true, true, dsv.get_dimensions());
        out.ref_values.stencil = data.1;
    }
}


impl<'a> DataLink<'a> for Scissor {
    type Init = ();
    fn new() -> Self { Scissor(false) }
    fn is_active(&self) -> bool { self.0 }
    fn link_scissor(&mut self) -> bool { self.0 = true; true }
}

impl<R: Resources> DataBind<R> for Scissor {
    type Data = target::Rect;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               _: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        out.scissor = *data;
    }
}

impl<'a> DataLink<'a> for BlendRef {
    type Init = ();
    fn new() -> Self { BlendRef }
    fn is_active(&self) -> bool { true }
}

impl<R: Resources> DataBind<R> for BlendRef {
    type Data = target::ColorValue;
    fn bind_to(&self,
               out: &mut RawDataSet<R>,
               data: &Self::Data,
               _: &mut handle::Manager<R>,
               _: &mut AccessInfo<R>) {
        out.ref_values.blend = *data;
    }
}
