//! Output Merger(OM) stage description.

use {format, state as s};

/// A pixel-wise comparison function.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
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

/// PSO color target descriptor
pub type ColorTargetDesc = (format::Format, ColorInfo);
/// PSO depth-stencil target descriptor
pub type DepthStencilDesc = (format::Format, DepthStencilInfo);

/// Color output configuration of the PSO.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct ColorInfo {
    /// Color channel mask
    pub mask: s::ColorMask,
    /// Optional color blending
    pub color: Option<s::BlendChannel>,
    /// Optional alpha blending
    pub alpha: Option<s::BlendChannel>,
}
impl From<s::ColorMask> for ColorInfo {
    fn from(mask: s::ColorMask) -> ColorInfo {
        ColorInfo {
            mask: mask,
            color: None,
            alpha: None,
        }
    }
}
impl From<s::Blend> for ColorInfo {
    fn from(blend: s::Blend) -> ColorInfo {
        ColorInfo {
            mask: s::MASK_ALL,
            color: Some(blend.color),
            alpha: Some(blend.alpha),
        }
    }
}

/// Depth and stencil state of the PSO.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct DepthStencilInfo {
    /// Optional depth test configuration
    pub depth: Option<s::Depth>,
    /// Optional stencil test on the front faces
    pub front: Option<s::StencilSide>,
    /// Optional stencil test on the back faces
    pub back: Option<s::StencilSide>,
}
impl From<s::Depth> for DepthStencilInfo {
    fn from(depth: s::Depth) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(depth),
            front: None,
            back: None,
        }
    }
}
impl From<s::Stencil> for DepthStencilInfo {
    fn from(stencil: s::Stencil) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: None,
            front: Some(stencil.front),
            back: Some(stencil.back),
        }
    }
}
impl From<(s::Depth, s::Stencil)> for DepthStencilInfo {
    fn from(ds: (s::Depth, s::Stencil)) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(ds.0),
            front: Some(ds.1.front),
            back: Some(ds.1.back),
        }
    }
}
