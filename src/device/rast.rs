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

use std::default::Default;
use StencilValue = super::target::Stencil;

#[deriving(Clone, PartialEq, Show)]
pub enum FrontType {
    Cw,
    Ccw,
}

pub type LineWidth = f32;
pub type OffsetFactor = f32;
pub type OffsetUnits = u32;

#[deriving(Clone, PartialEq, Show)]
pub enum OffsetType {
    NoOffset,
    Offset(OffsetFactor, OffsetUnits),
}

#[deriving(Clone, PartialEq, Show)]
pub enum CullMode {
    CullNothing,
    CullFront,
    CullBack,
}

#[deriving(Clone, PartialEq, Show)]
pub enum RasterMethod {
    Point,
    Line(LineWidth),
    Fill(CullMode),
}

/// Primitive rasterization state. Note that GL allows different raster
/// method to be used for front and back, while this abstraction does not.
#[deriving(Clone, PartialEq, Show)]
pub struct Primitive {
    pub front_face: FrontType,
    pub method: RasterMethod,
    pub offset: OffsetType,
}

impl Primitive {
    pub fn get_cull_mode(&self) -> CullMode {
        match self.method {
            Fill(mode) => mode,
            _ => CullNothing,
        }
    }
}

impl Default for Primitive {
    fn default() -> Primitive {
        Primitive {
            front_face: CounterClockwise,
            method: Fill(CullNothing),
            offset: NoOffset,
        }
    }
}

/// A pixel-wise comparison function.
#[deriving(Clone, PartialEq, Show)]
pub enum Comparison {
    /// `false`
    Never,
    /// `x < y`
    Less,
    /// `x <= y`
    LessEqual,
    /// `x == y`
    Equal,
    /// `x >= y`
    GreaterEqual,
    /// `x > y`
    Greater,
    /// `x != y`
    NotEqual,
    /// `true`
    Always,
}

#[deriving(Clone, PartialEq, Show)]
pub enum StencilOp {
    OpKeep,
    OpZero,
    OpReplace,
    OpIncrementClamp,
    OpIncrementWrap,
    OpDecrementClamp,
    OpDecrementWrap,
    OpInvert,
}

#[deriving(Clone, PartialEq, Show)]
pub struct StencilSide {
    pub fun: Comparison,
    pub value: StencilValue,
    pub mask_read: StencilValue,
    pub mask_write: StencilValue,
    pub op_fail: StencilOp,
    pub op_depth_fail: StencilOp,
    pub op_pass: StencilOp,
}

impl Default for StencilSide {
    fn default() -> StencilSide {
        StencilSide {
            fun: Always,
            value: 0,
            mask_read: -1,
            mask_write: -1,
            op_fail: OpKeep,
            op_depth_fail: OpKeep,
            op_pass: OpKeep,
        }
    }
}

#[deriving(Clone, PartialEq, Show)]
pub struct Stencil {
    pub front: StencilSide,
    pub back: StencilSide,
}

#[deriving(Clone, PartialEq, Show)]
pub struct Depth {
    pub fun: Comparison,
    pub write: bool,
}

impl Default for Depth {
    fn default() -> Depth {
        Depth {
            fun: Always,
            write: false,
        }
    }
}

#[deriving(Clone, PartialEq, Show)]
pub enum Equation {
    FuncAdd,
    FuncSub,
    FuncRevSub,
    FuncMin,
    FuncMax,
}

#[deriving(Clone, PartialEq, Show)]
pub enum InverseFlag {
    Normal,
    Inverse,
}

#[deriving(Clone, PartialEq, Show)]
pub enum BlendValue {
    Zero,
    SourceColor,
    SourceAlpha,
    SourceAlphaSaturated,
    DestColor,
    DestAlpha,
    ConstColor,
    ConstAlpha,
}

#[deriving(Clone, PartialEq, Show)]
pub struct Factor(pub InverseFlag, pub BlendValue);

#[deriving(Clone, PartialEq, Show)]
pub struct BlendChannel {
    pub equation: Equation,
    pub source: Factor,
    pub destination: Factor,
}

impl Default for BlendChannel {
    fn default() -> BlendChannel {
        BlendChannel {
            equation: FuncAdd,
            source: Factor(Inverse, Zero),
            destination: Factor(Normal, Zero),
        }
    }
}

#[deriving(Clone, PartialEq, Show)]
pub struct Blend {
    pub color: BlendChannel,
    pub alpha: BlendChannel,
    pub value: super::target::Color,
}

impl Default for Blend {
    fn default() -> Blend {
        Blend {
            color: Default::default(),
            alpha: Default::default(),
            value: Default::default(),
        }
    }
}
