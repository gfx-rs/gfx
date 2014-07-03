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

pub use device::Color;
use device::dev;

pub type TextureLayer = u16;
pub type TextureLevel = u8;

static MAX_COLOR_TARGETS: uint = 4;


pub struct ClearData {
    pub color: Option<Color>,
    pub depth: Option<f32>,
    pub stencil: Option<u8>,
}


pub enum Plane {
    PlaneEmpty,
    PlaneSurface(dev::Surface),
    PlaneTexture(dev::Texture, TextureLevel),
    PlaneTextureLayer(dev::Texture, TextureLevel, TextureLayer),
}


pub struct Frame {
    pub colors: [Plane, ..MAX_COLOR_TARGETS],
    pub depth: Plane,
    pub stencil: Plane,
}

impl Frame {
    pub fn new() -> Frame {
        Frame {
            colors: [PlaneEmpty, ..MAX_COLOR_TARGETS],
            depth: PlaneEmpty,
            stencil: PlaneEmpty,
        }
    }
}
