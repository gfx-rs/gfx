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

//! Fixed-function hardware state.
//!
//! Configures primitive assembly (PA), rasterizer, and output merger (OM) blocks.

use device::state;
use device::state::{BlendValue, CullMode, Equation, InverseFlag, RasterMethod, StencilOp, WindingOrder};
use device::target::{Mask, Rect, Stencil};

/// An assembly of states that affect regular draw calls
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct DrawState {
    /// How to rasterize geometric primitives.
    pub primitive: state::Primitive,
    /// Multi-sampling mode
    pub multi_sample: Option<state::MultiSample>,
    /// Scissor mask to use. If set, no pixel outside of this rectangle (in screen space) will be
    /// written to as a result of rendering.
    pub scissor: Option<Rect>,
    /// Stencil test to use. If None, no stencil testing is done.
    pub stencil: Option<state::Stencil>,
    /// Depth test to use. If None, no depth testing is done.
    pub depth: Option<state::Depth>,
    /// Blend function to use. If None, no blending is done.
    pub blend: Option<state::Blend>,
    /// Color mask to use. Each flag indicates that the given color channel can be written to, and
    /// they can be OR'd together.
    pub color_mask: state::ColorMask,
}

/// Blend function presets for ease of use.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BlendPreset {
    /// When combining two fragments, add their values together, saturating at 1.0
    Additive,
    /// When combining two fragments, add the value of the source times its alpha channel with the
    /// value of the destination multiplied by the inverse of the source alpha channel. Has the
    /// usual transparency effect: mixes the two colors using a fraction of each one specified by
    /// the alpha of the source.
    Alpha,
}

impl DrawState {
    /// Create a default `DrawState`. Uses counter-clockwise winding, culls the backface of each
    /// primitive, and does no scissor/stencil/depth/blend/color masking.
    pub fn new() -> DrawState {
        DrawState {
            primitive: state::Primitive {
                front_face: WindingOrder::CounterClockwise,
                method: RasterMethod::Fill(CullMode::Back),
                offset: None,
            },
            multi_sample: None,
            scissor: None,
            stencil: None,
            depth: None,
            blend: None,
            color_mask: state::MASK_ALL,
        }
    }

    /// Return a target mask that contains all the planes required by this state.
    pub fn get_target_mask(&self) -> Mask {
        use device::target as t;
        (if self.stencil.is_some() {t::STENCIL} else {Mask::empty()}) |
        (if self.depth.is_some()   {t::DEPTH}   else {Mask::empty()}) |
        (if self.blend.is_some()   {t::COLOR}   else {Mask::empty()})
    }

    /// Enable multi-sampled rasterization
    pub fn multi_sample(mut self) -> DrawState {
        self.multi_sample = Some(state::MultiSample);
        self
    }

    /// Set the stencil test to a simple expression
    pub fn stencil(mut self, fun: state::Comparison, value: Stencil) -> DrawState {
        let side = state::StencilSide {
            fun: fun,
            value: value,
            mask_read: -1,
            mask_write: -1,
            op_fail: StencilOp::Keep,
            op_depth_fail: StencilOp::Keep,
            op_pass: StencilOp::Keep,
        };
        self.stencil = Some(state::Stencil {
            front: side,
            back: side,
        });
        self
    }

    /// Set the depth test with the mask
    pub fn depth(mut self, fun: state::Comparison, write: bool) -> DrawState {
        self.depth = Some(state::Depth {
            fun: fun,
            write: write,
        });
        self
    }

    /// Set the blend mode to one of the presets
    pub fn blend(mut self, preset: BlendPreset) -> DrawState {
        self.blend = Some(match preset {
            BlendPreset::Additive => state::Blend {
                color: state::BlendChannel {
                    equation: Equation::Add,
                    source: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                    destination: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                },
                alpha: state::BlendChannel {
                    equation: Equation::Add,
                    source: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                    destination: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                },
                value: [0.0, 0.0, 0.0, 0.0],
            },
            BlendPreset::Alpha => state::Blend {
                color: state::BlendChannel {
                    equation: Equation::Add,
                    source: state::Factor(InverseFlag::Normal, BlendValue::SourceAlpha),
                    destination: state::Factor(InverseFlag::Inverse, BlendValue::SourceAlpha),
                },
                alpha: state::BlendChannel {
                    equation: Equation::Add,
                    source: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                    destination: state::Factor(InverseFlag::Inverse, BlendValue::Zero),
                },
                value: [0.0, 0.0, 0.0, 0.0],
            },
        });
        self
    }
}
