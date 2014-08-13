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

use t = device::target;
use backend = device::dev;

static MAX_COLOR_TARGETS: uint = 4;

#[deriving(Clone, PartialEq, Show)]
/// A single buffer that can be bound to a render target.
pub enum Plane {
    /// No buffer, the results will not be stored.
    PlaneEmpty,
    /// Render to a `Surface` (corresponds to a renderbuffer in GL).
    PlaneSurface(backend::Surface),
    /// Render to a texture at a specific mipmap level
    /// If `Layer` is set, it is selecting a single 2D slice of a given 3D texture
    PlaneTexture(backend::Texture, t::Level, Option<t::Layer>),
}

/// A complete `Frame`, which is the result of rendering.
pub struct Frame {
    /// The width of the viewport.
    pub width: u16,
    /// The height of the viewport.
    pub height: u16,
    /// Each color component has its own buffer.
    pub colors: [Plane, ..MAX_COLOR_TARGETS],
    /// The depth buffer for this frame.
    pub depth: Plane,
    /// The stencil buffer for this frame.
    pub stencil: Plane,
}

impl Frame {
    /// Create an empty `Frame`, which corresponds to the 'default framebuffer', which for now
    /// renders directly to the window that was created with the OpenGL context.
    pub fn new(width: u16, height: u16) -> Frame {
        Frame {
            width: width,
            height: height,
            colors: [PlaneEmpty, ..MAX_COLOR_TARGETS],
            depth: PlaneEmpty,
            stencil: PlaneEmpty,
        }
    }

    /// Returns true if this framebuffer is associated with the main window (matches `Frame::new`
    /// exactly).
    pub fn is_default(&self) -> bool {
        self.colors.iter().all(|&p| p==PlaneEmpty) &&
        self.depth == PlaneEmpty &&
        self.stencil == PlaneEmpty
    }
}
