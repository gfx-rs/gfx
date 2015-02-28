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

use std::fmt;

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
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// A color with floating-point components.
pub type ColorValue = [f32; 4];

bitflags!(
    /// Output mask, used for blitting and clearing
    flags Mask: u8 {
        const COLOR     = 0x01,
        const COLOR0    = 0x01,
        const COLOR1    = 0x02,
        const COLOR2    = 0x04,
        const COLOR3    = 0x08,
        const DEPTH     = 0x40,
        const STENCIL   = 0x80
    }
);

impl fmt::Debug for Mask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Mask({})", self.bits())
    }
}

bitflags!(
    /// Mirroring flags, used for blitting
    #[derive(Debug)]
    flags Mirror: u8 {
        const MIRROR_X  = 0x01,
        const MIRROR_Y  = 0x02,
    }
);

/// How to clear a frame.
#[derive(Copy)]
pub struct ClearData {
    /// The color to clear the frame with
    pub color: ColorValue,
    /// The depth value to clear the frame with
    pub depth: Depth,
    /// The stencil value to clear the frame with
    pub stencil: Stencil,
}

impl Clone for ClearData {
    fn clone(&self) -> ClearData {
        ClearData {
            color: self.color,
            depth: self.depth,
            stencil: self.stencil,
        }
    }
}

impl fmt::Debug for ClearData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
            "ClearData {{ color: {:?}, depth: {:?}, stencil: {:?} }}",
            &self.color[..], self.depth, self.stencil)
    }
}

/// Type of the frame buffer access
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Access {
    /// Draw access
    Draw,
    /// Read access
    Read,
}

/// When rendering, each "output" of the fragment shader goes to a specific target. A `Plane` can
/// be bound to a target, causing writes to that target to affect the `Plane`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Target {
    /// Color data.
    ///
    /// # Portability Note
    ///
    /// The device is only required to expose one color target.
    Color(u8),
    /// Depth data.
    Depth,
    /// Stencil data.
    Stencil,
    /// A target for both depth and stencil data at once.
    DepthStencil,
}
