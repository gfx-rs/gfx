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

use r = device::rast;

#[deriving(Clone, PartialEq, Show)]
pub struct DrawState {
    pub primitive: r::Primitive,
    pub stencil: Option<r::Stencil>,
    pub depth: Option<r::Depth>,
    pub blend: Option<r::Blend>,
}

impl DrawState {
	pub fn new() -> DrawState {
		DrawState {
			primitive: r::Primitive {
				front_face: r::Ccw,
				method: r::Fill(r::DrawFront, r::DrawBack),
				offset: r::NoOffset,
			},
			stencil: None,
			depth: None,
			blend: None,
		}
	}
}
