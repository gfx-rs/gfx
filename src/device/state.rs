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
//! Configures the primitive assembly (PA), rasterizer, and output merger (OM) blocks.

use std::default::Default;
use std::fmt;

use target;

/// The winding order of a set of vertices.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum WindingOrder {
    /// Clockwise winding order.
    Clockwise,
    /// Counter-clockwise winding order.
    CounterClockwise,
}

/// Width of a line.
pub type LineWidth = f32;
#[allow(missing_docs)]
pub type OffsetFactor = f32;
#[allow(missing_docs)]
pub type OffsetUnits = u32;

/// How to offset vertices in screen space, if at all.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, Show)]
pub enum OffsetType {
    NoOffset,
    Offset(OffsetFactor, OffsetUnits),
}

/// Which face, if any, to cull.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, Show)]
pub enum CullMode {
    CullNothing,
    CullFront,
    CullBack,
}

/// How to rasterize a primitive.
#[deriving(Clone, PartialEq, Show)]
pub enum RasterMethod {
    /// Rasterize as a point.
    Point,
    /// Rasterize as a line with the given width.
    Line(LineWidth),
    /// Rasterize as a face with a given cull mode.
    Fill(CullMode),
}

/// Primitive rasterization state. Note that GL allows different raster
/// method to be used for front and back, while this abstraction does not.
#[deriving(Clone, PartialEq, Show)]
pub struct Primitive {
    /// Which vertex winding is considered to be the front face for culling.
    pub front_face: WindingOrder,
    /// How to rasterize this primitive.
    pub method: RasterMethod,
    /// Any polygon offset to apply.
    pub offset: OffsetType,
}

impl Primitive {
    /// Get the cull mode, if any, for this primitive state.
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

/// Multi-sampling rasterization mode
#[deriving(Eq, Ord, PartialEq, PartialOrd, Clone, Show)]
pub struct MultiSample;
    //sample_mask: u16,
    //alpha_to_coverage: bool,

/// A pixel-wise comparison function.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
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

/// Stencil mask operation.
#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum StencilOp {
    /// Keep the current value in the stencil buffer (no change).
    OpKeep,
    /// Set the value in the stencil buffer to zero.
    OpZero,
    /// Set the stencil buffer value to `value` from `StencilSide`
    OpReplace,
    /// Increment the stencil buffer value, clamping to its maximum value.
    OpIncrementClamp,
    /// Increment the stencil buffer value, wrapping around to 0 on overflow.
    OpIncrementWrap,
    /// Decrement the stencil buffer value, clamping to its minimum value.
    OpDecrementClamp,
    /// Decrement the stencil buffer value, wrapping around to the maximum value on overflow.
    OpDecrementWrap,
    /// Bitwise invert the current value in the stencil buffer.
    OpInvert,
}

/// Complete stencil state for a given side of a face.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct StencilSide {
    /// Comparison function to use to determine if the stencil test passes.
    pub fun: Comparison,
    /// Reference value to compare the value in the stencil buffer with.
    pub value: target::Stencil,
    /// A mask that is ANDd with both the stencil buffer value and the reference value when they
    /// are read before doing the stencil test.
    pub mask_read: target::Stencil,
    /// A mask that is ANDd with the stencil value before writing to the stencil buffer.
    pub mask_write: target::Stencil,
    /// What operation to do if the stencil test fails.
    pub op_fail: StencilOp,
    /// What operation to do if the stenil test passes but the depth test fails.
    pub op_depth_fail: StencilOp,
    /// What operation to do if both the depth and stencil test pass.
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

/// Complete stencil state, specifying how to handle the front and back side of a face.
#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct Stencil {
    pub front: StencilSide,
    pub back: StencilSide,
}

/// Depth test state.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct Depth {
    /// Comparison function to use.
    pub fun: Comparison,
    /// Specify whether to write to the depth buffer or not.
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

#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum Equation {
    FuncAdd,
    FuncSub,
    FuncRevSub,
    FuncMin,
    FuncMax,
}

#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum InverseFlag {
    Normal,
    Inverse,
}

#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
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

#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct Factor(pub InverseFlag, pub BlendValue);

#[allow(missing_docs)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
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

#[allow(missing_docs)]
pub struct Blend {
    pub color: BlendChannel,
    pub alpha: BlendChannel,
    pub value: ::target::ColorValue,
}

impl Default for Blend {
    fn default() -> Blend {
        Blend {
            color: Default::default(),
            alpha: Default::default(),
            value: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl PartialEq for Blend {
    fn eq(&self, other: &Blend) -> bool {
        self.color == other.color &&
        self.alpha == other.alpha &&
        self.value == other.value
    }
}

impl Clone for Blend {
    fn clone(&self) -> Blend {
        Blend {
            color: self.color.clone(),
            alpha: self.alpha.clone(),
            value: self.value,
        }
    }
}

impl fmt::Show for Blend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blend {{ color: {}, alpha: {}, value: {} }}",
               self.color, self.alpha, self.value.as_slice())
    }
}

#[deriving(Clone, PartialEq)]
bitflags!(
    #[allow(missing_docs)]
    flags ColorMask: u32 {  //u8 is preferred, but doesn't seem to work well
        const RED     = 0x1,
        const GREEN   = 0x2,
        const BLUE    = 0x4,
        const ALPHA   = 0x8,
        const MASK_ALL = 0xF
    }
)

impl fmt::Show for ColorMask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let values = [
            (RED,   "Red"  ),
            (GREEN, "Green"),
            (BLUE,  "Blue" ),
            (ALPHA, "Alpha"),
        ];

        try!(write!(f, "ColorMask("));
        for (i, &(_, name)) in values.iter()
            .filter(|&&(flag, _)| self.contains(flag))
            .enumerate()
        {
            if i == 0 {
                try!(write!(f, "{}", name))
            } else {
                try!(write!(f, " | {}", name))
            }
        }
        try!(write!(f, ")"));

        Ok(())
    }
}
