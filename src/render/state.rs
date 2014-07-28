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

use s = device::state;
use device::target::{Color, Rect, Stencil};

/// An assembly of states that affect regular draw calls
#[deriving(Clone, PartialEq, Show)]
pub struct DrawState {
    pub primitive: s::Primitive,
    pub scissor: Option<Rect>,
    pub stencil: Option<s::Stencil>,
    pub depth: Option<s::Depth>,
    pub blend: Option<s::Blend>,
    pub color_mask: s::ColorMask,
}

#[deriving(Clone, PartialEq, Show)]
pub enum BlendPreset {
    BlendAdditive,
    BlendAlpha,
}

impl DrawState {
    pub fn new() -> DrawState {
        DrawState {
            primitive: s::Primitive {
                front_face: s::CounterClockwise,
                method: s::Fill(s::CullBack),
                offset: s::NoOffset,
            },
            scissor: None,
            stencil: None,
            depth: None,
            blend: None,
            color_mask: s::MaskAll,
        }
    }

    /// set the stencil test to a simple expression
    pub fn stencil(mut self, fun: s::Comparison, value: Stencil) -> DrawState {
        let side = s::StencilSide {
            fun: fun,
            value: value,
            mask_read: -1,
            mask_write: -1,
            op_fail: s::OpKeep,
            op_depth_fail: s::OpKeep,
            op_pass: s::OpKeep,
        };
        self.stencil = Some(s::Stencil {
            front: side,
            back: side,
        });
        self
    }

    /// set the depth test with the mask
    pub fn depth(mut self, fun: s::Comparison, write: bool) -> DrawState {
        self.depth = Some(s::Depth {
            fun: fun,
            write: write,
        });
        self
    }

    /// set the blend mode to one of the presets
    pub fn blend(mut self, preset: BlendPreset) -> DrawState {
        self.blend = Some(match preset {
            BlendAdditive => s::Blend {
                color: s::BlendChannel {
                    equation: s::FuncAdd,
                    source: s::Factor(s::Inverse, s::Zero),
                    destination: s::Factor(s::Inverse, s::Zero),
                },
                alpha: s::BlendChannel {
                    equation: s::FuncAdd,
                    source: s::Factor(s::Inverse, s::Zero),
                    destination: s::Factor(s::Inverse, s::Zero),
                },
                value: Color::new(),
            },
            BlendAlpha => s::Blend {
                color: s::BlendChannel {
                    equation: s::FuncAdd,
                    source: s::Factor(s::Normal, s::SourceAlpha),
                    destination: s::Factor(s::Inverse, s::SourceAlpha),
                },
                alpha: s::BlendChannel {
                    equation: s::FuncAdd,
                    source: s::Factor(s::Inverse, s::Zero),
                    destination: s::Factor(s::Inverse, s::Zero),
                },
                value: Color::new(),
            },
        });
        self
    }
}
