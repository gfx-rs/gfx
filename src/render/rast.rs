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

	/// set the cull mode to back faces
	pub fn cull(mut self) -> DrawState {
		self.primitive.method = r::Fill(r::DrawFront, r::CullBack);
		self
	}

	/// set the depth test with the mask
	pub fn depth(mut self, fun: &str, write: bool) -> DrawState {
		let cmp = match fun {
			"!"  => r::Comparison(r::NoLess, r::NoEqual, r::NoGreater),
			"<"  => r::Comparison(r::Less,   r::NoEqual, r::NoGreater),
			"==" => r::Comparison(r::NoLess, r::Equal,   r::NoGreater),
			"<=" => r::Comparison(r::Less,   r::Equal,   r::NoGreater),
			">"  => r::Comparison(r::NoLess, r::NoEqual, r::Greater),
			"!=" => r::Comparison(r::Less,   r::NoEqual, r::Greater),
			">=" => r::Comparison(r::NoLess, r::Equal,   r::Greater),
			"*"  => r::Comparison(r::Less,   r::Equal,   r::Greater),
			_	 => fail!("Unknown depth func: {}", fun)
		};
		self.depth = Some(r::Depth {
			fun: cmp,
			write: write,
		});
		self
	}
}
