//! Output Merger(OM) stage description.

use command::StencilValue;

/// A pixel-wise comparison function.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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


bitflags!(
    /// Target output color mask.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ColorMask: u8 {
        ///
        const RED     = 0x1;
        ///
        const GREEN   = 0x2;
        ///
        const BLUE    = 0x4;
        ///
        const ALPHA   = 0x8;
        ///
        const COLOR   = 0x7;
        ///
        const ALL     = 0xF;
        ///
        const NONE    = 0x0;
    }
);

impl Default for ColorMask {
    fn default() -> Self {
        Self::ALL
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Factor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    ConstColor,
    OneMinusConstColor,
    ConstAlpha,
    OneMinusConstAlpha,
    SrcAlphaSaturate,
    Src1Color,
    OneMinusSrc1Color,
    Src1Alpha,
    OneMinusSrc1Alpha,
}

/// Blending operation.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BlendOp {
    /// Adds source and destination.
    /// Source and destination are multiplied by factors before addition.
    Add { src: Factor, dst: Factor },
    /// Subtracts destination from source.
    /// Source and destination are multiplied by factors before subtraction.
    Sub { src: Factor, dst: Factor },
    /// Subtracts source from destination.
    /// Source and destination are multiplied by factors before subtraction.
    RevSub { src: Factor, dst: Factor },
    /// Component-wise minimum value of source and destination.
    Min,
    /// Component-wise maximum value of source and destination.
    Max,
}

impl BlendOp {
    ///
    pub const REPLACE: Self = BlendOp::Add {
        src: Factor::One,
        dst: Factor::Zero,
    };
    ///
    pub const ADD: Self = BlendOp::Add {
        src: Factor::One,
        dst: Factor::One,
    };
    ///
    pub const ALPHA: Self = BlendOp::Add {
        src: Factor::SrcAlpha,
        dst: Factor::OneMinusSrcAlpha,
    };
    ///
    pub const PREMULTIPLIED_ALPHA: Self = BlendOp::Add {
        src: Factor::One,
        dst: Factor::OneMinusSrcAlpha,
    };
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BlendState {
    On {
        color: BlendOp,
        alpha: BlendOp,
    },
    Off,
}

impl BlendState {
    /// Additive blending
    pub const ADD: Self = BlendState::On {
        color: BlendOp::ADD,
        alpha: BlendOp::ADD,
    };
    /// Multiplicative blending
    pub const MULTIPLY: Self = BlendState::On {
        color: BlendOp::Add {
            src: Factor::Zero,
            dst: Factor::SrcColor,
        },
        alpha: BlendOp::Add {
            src: Factor::Zero,
            dst: Factor::SrcAlpha,
        },
    };
    /// Alpha blending.
    pub const ALPHA: Self = BlendState::On {
        color: BlendOp::ALPHA,
        alpha: BlendOp::PREMULTIPLIED_ALPHA,
    };
    /// Pre-multiplied alpha blending.
    pub const PREMULTIPLIED_ALPHA: Self = BlendState::On {
        color: BlendOp::PREMULTIPLIED_ALPHA,
        alpha: BlendOp::PREMULTIPLIED_ALPHA,
    };
}

impl Default for BlendState {
    fn default() -> Self {
        BlendState::Off
    }
}

/// PSO color target descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ColorBlendDesc(pub ColorMask, pub BlendState);

impl ColorBlendDesc {
    /// Empty blend descriptor just writes out the color without blending.
    pub const EMPTY: Self = ColorBlendDesc(ColorMask::ALL, BlendState::Off);
}


/// Depth test state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DepthTest {
    /// Enabled depth testing.
    On {
        /// Comparison function to use.
        fun: Comparison,
        /// Specify whether to write to the depth buffer or not.
        write: bool,
    },
    /// Disabled depth testing.
    Off,
}

impl Default for DepthTest {
    fn default() -> Self {
        DepthTest::Off
    }
}

impl DepthTest {
    ///
    pub const FAIL: Self = DepthTest::On {
        fun: Comparison::Never,
        write: false,
    };
    ///
    pub const PASS_TEST: Self = DepthTest::On {
        fun: Comparison::Always,
        write: false,
    };
    ///
    pub const PASS_WRITE: Self = DepthTest::On {
        fun: Comparison::Always,
        write: true,
    };
}

/// Stencil mask operation.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StencilOp {
    /// Keep the current value in the stencil buffer (no change).
    Keep,
    /// Set the value in the stencil buffer to zero.
    Zero,
    /// Set the stencil buffer value to `value` from `StencilSide`
    Replace,
    /// Increment the stencil buffer value, clamping to its maximum value.
    IncrementClamp,
    /// Increment the stencil buffer value, wrapping around to 0 on overflow.
    IncrementWrap,
    /// Decrement the stencil buffer value, clamping to its minimum value.
    DecrementClamp,
    /// Decrement the stencil buffer value, wrapping around to the maximum value on overflow.
    DecrementWrap,
    /// Bitwise invert the current value in the stencil buffer.
    Invert,
}

/// Complete stencil state for a given side of a face.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StencilFace {
    /// Comparison function to use to determine if the stencil test passes.
    pub fun: Comparison,
    /// A mask that is ANDd with both the stencil buffer value and the reference value when they
    /// are read before doing the stencil test.
    pub mask_read: StencilValue,
    /// A mask that is ANDd with the stencil value before writing to the stencil buffer.
    pub mask_write: StencilValue,
    /// What operation to do if the stencil test fails.
    pub op_fail: StencilOp,
    /// What operation to do if the stencil test passes but the depth test fails.
    pub op_depth_fail: StencilOp,
    /// What operation to do if both the depth and stencil test pass.
    pub op_pass: StencilOp,
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StencilTest {
    On {
        front: StencilFace,
        back: StencilFace,
    },
    Off,
}

impl Default for StencilTest {
    fn default() -> Self {
        StencilTest::Off
    }
}

/// PSO depth-stencil target descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DepthStencilDesc {
    /// Optional depth testing/writing.
    pub depth: DepthTest,
    /// Enable depth bounds testing.
    pub depth_bounds: bool,
    /// Stencil test/write.
    pub stencil: StencilTest,
}
