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

/// A generic rendering output, typically an FBO.
pub trait Output<R: Resources> {
    /// Get an associated device handle, if any.
    fn get_handle(&self) -> Option<&device::handle::FrameBuffer<R>> { None }
    /// Get canvas dimensions.
    fn get_size(&self) -> (u16, u16);
    /// Get array of color planes.
    fn get_colors(&self) -> &[Plane<R>] { &[] }
    /// Get depth plane, if any.
    fn get_depth(&self) -> Option<&Plane<R>> { None }
    /// Get stencil plane, if any.
    fn get_stencil(&self) -> Option<&Plane<R>> { None }
    /// Check if it converts gamma of the output colors.
    fn does_convert_gamma(&self) -> bool { false }
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
}

impl<R: Resources> Output<R> for Frame<R> {
    fn get_size(&self) -> (u16, u16) {
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

    fn does_convert_gamma(&self) -> bool {
        self.convert_gamma
    }
}
