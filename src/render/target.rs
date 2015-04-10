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
    /// Get the surface info
    pub fn get_surface_info(&self) -> device::tex::SurfaceInfo {
        match *self {
            Plane::Surface(ref suf) => *suf.get_info(),
            Plane::Texture(ref tex, _, _) => tex.get_info().to_surface_info(),
        }
    }
}

/// A complete `Frame`, which is the result of rendering.
#[derive(Clone, PartialEq, Debug)]
pub struct Frame<R: Resources> {
    /// The width of the viewport.
    pub width: u16,
    /// The height of the viewport.
    pub height: u16,
    /// Each color component has its own buffer.
    pub colors: Vec<Plane<R>>,
    /// The depth buffer for this frame.
    pub depth: Option<Plane<R>>,
    /// The stencil buffer for this frame.
    pub stencil: Option<Plane<R>>,
    /// Convert to sRGB color space.
    pub convert_gamma: bool,
}

impl<R: Resources> Frame<R> {
    /// Create an empty `Frame`, which corresponds to the 'default framebuffer',
    /// which renders directly to the window that was created with the OpenGL context.
    pub fn new(width: u16, height: u16) -> Frame<R> {
        Frame {
            width: width,
            height: height,
            colors: Vec::new(),
            depth: None,
            stencil: None,
            convert_gamma: false,
        }
    }

    /// Return true if this framebuffer is associated with the main window
    /// (matches `Frame::new` exactly).
    pub fn is_default(&self) -> bool {
        self.colors.is_empty() &&
        self.depth.is_none() &&
        self.stencil.is_none()
    }

    /// Return a mask of contained planes.
    pub fn get_mask(&self) -> Mask {
        use draw_state::target as t;
        let mut mask = match self.colors.len() {
            0 => Mask::empty(),
            1 => t::COLOR0,
            2 => t::COLOR0 | t::COLOR1,
            3 => t::COLOR0 | t::COLOR1 | t::COLOR2,
            _ => t::COLOR0 | t::COLOR1 | t::COLOR2 | t::COLOR3,
        };
        if self.depth.is_some() {
            mask.insert(t::DEPTH);
        }
        if self.stencil.is_some() {
            mask.insert(t::STENCIL);
        }
        if mask.is_empty() {
            // hack: assuming the default FBO has all planes
            t::COLOR | t::DEPTH | t::STENCIL
        } else {
            mask
        }
    }
}
