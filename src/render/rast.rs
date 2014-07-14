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
use device::target::{Color, Stencil};

/// An assembly of states that affect regular draw calls
#[deriving(Clone, PartialEq, Show)]
pub struct DrawState {
    pub primitive: r::Primitive,
    pub stencil: Option<r::Stencil>,
    pub depth: Option<r::Depth>,
    pub blend: Option<r::Blend>,
}

#[deriving(Clone, PartialEq, Show)]
pub enum BlendPreset {
    BlendAdditive,
    BlendAlpha,
}

fn parse_comparison(cmp: &str) -> r::Comparison {
    match cmp {
        "!"  => r::Never,
        "<"  => r::Less,
        "<=" => r::LessEqual,
        "==" => r::Equal,
        ">=" => r::GreaterEqual,
        ">"  => r::Greater,
        "!=" => r::NotEqual,
        "*"  => r::Always,
        _    => fail!("Unknown comparison function: {}", cmp),
    }
}

impl DrawState {
    pub fn new() -> DrawState {
        DrawState {
            primitive: r::Primitive {
                front_face: r::Ccw,
                method: r::Fill(r::CullBack),
                offset: r::NoOffset,
            },
            stencil: None,
            depth: None,
            blend: None,
        }
    }

    /// set the stencil test to a simple expression
    pub fn stencil(mut self, fun: &str, value: Stencil) -> DrawState {
        let side = r::StencilSide {
            fun: parse_comparison(fun),
            value: value,
            mask_read: -1,
            mask_write: -1,
            op_fail: r::OpKeep,
            op_depth_fail: r::OpKeep,
            op_pass: r::OpKeep,
        };
        self.stencil = Some(r::Stencil {
            front: side,
            back: side,
        });
        self
    }

    /// set the depth test with the mask
    pub fn depth(mut self, fun: &str, write: bool) -> DrawState {
        self.depth = Some(r::Depth {
            fun: parse_comparison(fun),
            write: write,
        });
        self
    }

    /// set the blend mode to one of the presets
    pub fn blend(mut self, preset: BlendPreset) -> DrawState {
        self.blend = Some(match preset {
            BlendAdditive => r::Blend {
                color: r::BlendChannel {
                    equation: r::FuncAdd,
                    source: r::Factor(r::Inverse, r::Zero),
                    destination: r::Factor(r::Inverse, r::Zero),
                },
                alpha: r::BlendChannel {
                    equation: r::FuncAdd,
                    source: r::Factor(r::Inverse, r::Zero),
                    destination: r::Factor(r::Inverse, r::Zero),
                },
                value: Color::new(),
            },
            BlendAlpha => r::Blend {
                color: r::BlendChannel {
                    equation: r::FuncAdd,
                    source: r::Factor(r::Normal, r::SourceAlpha),
                    destination: r::Factor(r::Inverse, r::SourceAlpha),
                },
                alpha: r::BlendChannel {
                    equation: r::FuncAdd,
                    source: r::Factor(r::Inverse, r::Zero),
                    destination: r::Factor(r::Inverse, r::Zero),
                },
                value: Color::new(),
            },
        });
        self
    }
}
