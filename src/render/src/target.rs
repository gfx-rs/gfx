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

use gfx_core::{Resources};
use gfx_core::draw::Gamma;
use gfx_core::output::{Output, Plane};
use gfx_core::tex::Size;


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
