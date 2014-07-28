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

//! Rasterizer state.

use s = device::state;
use device::target::{Color, Rect, Stencil};

/// An assembly of states that affect regular draw calls
#[deriving(Clone, PartialEq, Show)]
pub struct DrawState {
    /// How to rasteriz geometric primitives.
    pub primitive: s::Primitive,
    /// (Optional) scissor mask. No pixel outside of the rectangle, if specified, will be written
    /// to as a result of rendering. If None, acts as if the scissor mask covers the entire
    /// `Frame`.
    pub scissor: Option<Rect>,
    /// Stencil mask to use. If None, no stencil testing is done.
    pub stencil: Option<s::Stencil>,
    /// Depth test to use. If None, no depth testing is done.
    pub depth: Option<s::Depth>,
    /// Blend function to use. If None, no blending is done.
    pub blend: Option<s::Blend>,
    /// Color mask to use. Each flag indicates that the given color channel can be written to, and
    /// they can be OR'd together.
    pub color_mask: s::ColorMask,
}

/// Blend function presets for ease of use.
#[deriving(Clone, PartialEq, Show)]
pub enum BlendPreset {
    /// When combining two fragments, add their values together, saturating at 1.0
    BlendAdditive,
    /// When combining two fragments, add the value of the source times its alpha channel with the
    /// value of the destination multiplied by the inverse of the source alpha channel. Has the
    /// usual transparency effect: mixes the two colors using a fraction of each one specified by
    /// the alpha of the source.
    BlendAlpha,
}

impl DrawState {
    /// Create a default `DrawState`. Uses counter-clockwise winding, culls the backface of each
    /// primitive, and does no scissor/stencil/depth/blend/color masking.
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

    /// Set the stencil test to a simple expression
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

    /// Set the depth test with the mask
    pub fn depth(mut self, fun: s::Comparison, write: bool) -> DrawState {
        self.depth = Some(s::Depth {
            fun: fun,
            write: write,
        });
        self
    }

    /// Set the blend mode to one of the presets
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
