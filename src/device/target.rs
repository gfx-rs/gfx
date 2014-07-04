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

use std::fmt;

pub type TextureLayer = u16;
pub type TextureLevel = u8;
pub type Depth = f32;
pub type Stencil = u8;

pub struct Color(pub [f32, ..4]);

impl fmt::Show for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Color([r,g,b,a]) = *self;
        write!(f, "Color({}, {}, {}, {})", r, g, b, a)
    }
}


#[deriving(Show)]
pub struct ClearData {
    pub color: Option<Color>,
    pub depth: Option<Depth>,
    pub stencil: Option<Stencil>,
}

#[deriving(Eq, PartialEq, Show)]
pub enum Plane {
    PlaneEmpty,
    PlaneSurface(super::dev::Surface),
    PlaneTexture(super::dev::Texture, TextureLevel),
    PlaneTextureLayer(super::dev::Texture, TextureLevel, TextureLayer),
}

#[deriving(Show)]
pub enum Target {
    TargetColor(u8),
    TargetDepth,
    TargetStencil,
    TargetDepthStencil,
}
