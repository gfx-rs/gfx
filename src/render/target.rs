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

//! Render target specification.

use device;
use device::Resources;
use device::draw::Gamma;
use device::tex::Size;
use draw_state::target::{Layer, Level, Mask};


#[derive(Clone, PartialEq, Debug)]
/// A single buffer that can be bound to a render target.
pub enum Plane<R: Resources> {
    /// Render to a `Surface` (corresponds to a renderbuffer in GL).
    Surface(device::handle::Surface<R>),
    /// Render to a texture at a specific mipmap level
    /// If `Layer` is set, it is selecting a single 2D slice of a given 3D texture
    Texture(device::handle::Texture<R>, Level, Option<Layer>),
}

impl<R: Resources> Plane<R> {
    /// Get the surface info.
    pub fn get_surface_info(&self) -> device::tex::SurfaceInfo {
        match *self {
            Plane::Surface(ref suf) => *suf.get_info(),
            Plane::Texture(ref tex, _, _) => (*tex.get_info()).into(),
        }
    }

    /// Get surface/texture format.
    pub fn get_format(&self) -> device::tex::Format {
        match *self {
            Plane::Surface(ref suf) => suf.get_info().format,
            Plane::Texture(ref tex, _, _) => tex.get_info().format,
        }
    }
}

/// A generic rendering output, consisting of multiple planes.
pub trait Output<R: Resources> {
    /// Get an associated device handle, if any.
    fn get_handle(&self) -> Option<&device::handle::FrameBuffer<R>> { None }
    /// Get canvas dimensions.
    fn get_size(&self) -> (Size, Size);
    /// Get array of color planes.
    fn get_colors(&self) -> &[Plane<R>] { &[] }
    /// Get depth plane, if any.
    fn get_depth(&self) -> Option<&Plane<R>> { None }
    /// Get stencil plane, if any.
    fn get_stencil(&self) -> Option<&Plane<R>> { None }
    /// Check if it converts gamma of the output colors.
    fn get_gamma(&self) -> Gamma { Gamma::Original }
    /// Get the output surface mask.
    fn get_mask(&self) -> Mask {
        use draw_state::target as t;
        let mut mask = match self.get_colors().len() {
            0 => Mask::empty(),
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
    fn get_size(&self) -> (Size, Size) {
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

/// A complete `Frame`, which is the result of rendering.
#[derive(Clone, PartialEq, Debug)]
pub struct Frame<R: Resources> {
    /// The width of the viewport.
    pub width: Size,
    /// The height of the viewport.
    pub height: Size,
    /// Each color component has its own buffer.
    pub colors: Vec<Plane<R>>,
    /// The depth buffer for this frame.
    pub depth: Option<Plane<R>>,
    /// The stencil buffer for this frame.
    pub stencil: Option<Plane<R>>,
    /// Color space.
    pub gamma: Gamma,
}

impl<R: Resources> Frame<R> {
    /// Create an empty `Frame`.
    pub fn empty(width: Size, height: Size) -> Frame<R> {
        Frame {
            width: width,
            height: height,
            colors: Vec::new(),
            depth: None,
            stencil: None,
            gamma: Gamma::Original,
        }
    }
}

impl<R: Resources> Output<R> for Frame<R> {
    fn get_size(&self) -> (Size, Size) {
        (self.width, self.height)
    }

    fn get_colors(&self) -> &[Plane<R>] {
        &self.colors
    }

    fn get_depth(&self) -> Option<&Plane<R>> {
        self.depth.as_ref()
    }

    fn get_stencil(&self) -> Option<&Plane<R>> {
        self.stencil.as_ref()
    }

    fn get_gamma(&self) -> Gamma {
        self.gamma
    }
}
