// Copyright 2014 The Gfx-rs Developers.
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

//! Render output
// This file to be removed!

use {Resources, draw, handle, tex};
use target as t;

#[derive(Clone, PartialEq, Debug)]
/// A single buffer that can be bound to a render target.
pub enum Plane<R: Resources> {
    /// Render to a `Surface` (corresponds to a renderbuffer in GL).
    Surface(handle::Surface<R>),
    /// Render to a texture at a specific mipmap level
    /// If `Layer` is set, it is selecting a single 2D slice of a given 3D texture
    Texture(handle::Texture<R>, t::Level, Option<t::Layer>),
}

impl<R: Resources> Plane<R> {
    /// Get the surface info.
    pub fn get_surface_info(&self) -> tex::SurfaceInfo {
        match *self {
            Plane::Surface(ref suf) => *suf.get_info(),
            Plane::Texture(ref tex, _, _) => (*tex.get_info()).into(),
        }
    }

    /// Get surface/texture format.
    pub fn get_format(&self) -> tex::Format {
        match *self {
            Plane::Surface(ref suf) => suf.get_info().format,
            Plane::Texture(ref tex, _, _) => tex.get_info().format,
        }
    }
}

/// A generic rendering output, consisting of multiple planes.
pub trait Output<R: Resources> {
    /// Get an associated device handle, if any.
    fn get_handle(&self) -> Option<&handle::FrameBuffer<R>> { None }
    /// Get canvas dimensions.
    fn get_size(&self) -> (tex::Size, tex::Size);
    /// Get array of color planes.
    fn get_colors(&self) -> &[Plane<R>] { &[] }
    /// Get depth plane, if any.
    fn get_depth(&self) -> Option<&Plane<R>> { None }
    /// Get stencil plane, if any.
    fn get_stencil(&self) -> Option<&Plane<R>> { None }
    /// Check if it converts gamma of the output colors.
    fn get_gamma(&self) -> draw::Gamma { draw::Gamma::Original }
    /// Get the output surface mask.
    fn get_mask(&self) -> t::Mask {
        let mut mask = match self.get_colors().len() {
            0 => t::Mask::empty(),
            1 => t::COLOR0,
            2 => t::COLOR0 | t::COLOR1,
            3 => t::COLOR0 | t::COLOR1 | t::COLOR2,
            _ => t::COLOR0 | t::COLOR1 | t::COLOR2 | t::COLOR3,
        };
        if self.get_depth().is_some() {
            mask.insert(t::DEPTH);
        }
        if self.get_stencil().is_some() {
            mask.insert(t::STENCIL);
        }
        mask
    }
}

impl<R: Resources> Output<R> for Plane<R> {
    fn get_size(&self) -> (tex::Size, tex::Size) {
        let info = self.get_surface_info();
        (info.width, info.height)
    }

    #[cfg(unstable)]
    fn get_colors(&self) -> &[Plane<R>] {
        use std::slice::ref_slice;
        if self.get_format().is_color() {
            ref_slice(self)
        }else {
            &[]
        }
    }

    #[cfg(not(unstable))]
    fn get_colors(&self) -> &[Plane<R>] {
        if self.get_format().is_color() {
            unimplemented!()
        }else {
            &[]
        }
    }

    fn get_depth(&self) -> Option<&Plane<R>> {
        if self.get_format().has_depth() {
            Some(self)
        }else {
            None
        }
    }

    fn get_stencil(&self) -> Option<&Plane<R>> {
        if self.get_format().has_stencil() {
            Some(self)
        }else {
            None
        }
    }
}
