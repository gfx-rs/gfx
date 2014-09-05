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

#[deriving(Clone, PartialEq, Show)]
/// A single buffer that can be bound to a render target.
pub enum Plane {
    /// Render to a `Surface` (corresponds to a renderbuffer in GL).
    PlaneSurface(device::SurfaceHandle),
    /// Render to a texture at a specific mipmap level
    /// If `Layer` is set, it is selecting a single 2D slice of a given 3D texture
    PlaneTexture(device::TextureHandle, device::target::Level,
                 Option<device::target::Layer>),
}

/// A complete `Frame`, which is the result of rendering.
#[deriving(Clone, PartialEq, Show)]
pub struct Frame {
    /// The width of the viewport.
    pub width: u16,
    /// The height of the viewport.
    pub height: u16,
    /// Each color component has its own buffer.
    pub colors: Vec<Plane>,
    /// The depth buffer for this frame.
    pub depth: Option<Plane>,
    /// The stencil buffer for this frame.
    pub stencil: Option<Plane>,
}

impl Frame {
    /// Create an empty `Frame`, which corresponds to the 'default framebuffer',
    /// which renders directly to the window that was created with the OpenGL context.
    pub fn new(width: u16, height: u16) -> Frame {
        Frame {
            width: width,
            height: height,
            colors: Vec::new(),
            depth: None,
            stencil: None,
        }
    }

    /// Returns true if this framebuffer is associated with the main window
    /// (matches `Frame::new` exactly).
    pub fn is_default(&self) -> bool {
        self.colors.is_empty() &&
        self.depth.is_none() &&
        self.stencil.is_none()
    }
}
