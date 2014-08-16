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

use std::{default, fmt};

// TODO: Really tighten up the terminology here.

/// A depth value, specifying which plane to select out of a 3D texture.
pub type Layer = u16;
/// Mipmap level to select in a texture.
pub type Level = u8;
/// A single depth value from a depth buffer.
pub type Depth = f32;
/// A single value from a stencil stencstencil buffer.
pub type Stencil = u8;

/// A screen space rectangle
#[allow(missing_doc)]
#[deriving(Clone, PartialEq, Show)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// A color with floating-point components. Used for `ClearData`.
pub struct Color(pub [f32, ..4]);

// manual impls due to array...

impl Color {
    /// Returns black.
    pub fn new() -> Color {
        Color([0.0, 0.0, 0.0, 0.0])
    }
}

impl Clone for Color {
    fn clone(&self) -> Color {
        let Color(ref x) = *self;
        Color([x[0], x[1], x[2], x[3]])
    }
}

impl PartialEq for Color {
    fn eq(&self, other: &Color) -> bool {
        let Color(ref x) = *self;
        let Color(ref y) = *other;
        x[0] == y[0] && x[1] == y[1] && x[2] == y[2] && x[3] == y[3]
    }
}

impl fmt::Show for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Color([r,g,b,a]) = *self;
        write!(f, "Color({}, {}, {}, {})", r, g, b, a)
    }
}

impl default::Default for Color {
    fn default() -> Color {
        Color([0.0, 0.0, 0.0, 0.0])
    }
}

#[deriving(Clone, Show)]
/// How to clear a frame.
pub struct ClearData {
    /// If set, the color buffer of the frame will be cleared to this.
    pub color: Option<Color>,
    /// If set, the depth buffer of the frame will be cleared to this.
    pub depth: Option<Depth>,
    /// If set, the stencil buffer of the frame will be cleared to this.
    pub stencil: Option<Stencil>,
}

/// When rendering, each "output" of the fragment shader goes to a specific target. A `Plane` can
/// be bound to a target, causing writes to that target to affect the `Plane`.
#[deriving(Clone, Show)]
pub enum Target {
    /// Color data.
    ///
    /// # Portability Note
    ///
    /// The device is only required to expose one color target.
    TargetColor(u8),
    /// Depth data.
    TargetDepth,
    /// Stencil data.
    TargetStencil,
    /// A target for both depth and stencil data at once.
    TargetDepthStencil,
}
