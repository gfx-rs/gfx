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

//! Texture creation and modification.
//!
//! "Texture" is an overloaded term. In gfx-rs, a texture consists of two
//! separate pieces of information: image storage description (which is
//! immutable for a single texture object), and image data. To actually use a
//! texture, a "sampler" is needed, which provides a way of accessing the
//! image data.  Image data consists of an array of "texture elements", or
//! texels.

use factory::{Bind, Usage};
use format;
use state;
pub use target::{Layer, Level};

/// Pure texture object creation error.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    /// Failed to map a given format to the device.
    Format(format::SurfaceType, Option<format::ChannelType>),
    /// The kind doesn't support a particular operation.
    Kind,
    /// Failed to map a given multisampled kind to the device.
    Samples(AaMode),
    /// Unsupported size in one of the dimensions.
    Size(Size),
    /// The given data has a different size than the target texture slice.
    Data(usize),
}

/// Dimension size
pub type Size = u16;
/// Number of bits per component
pub type Bits = u8;
/// Number of MSAA samples
pub type NumSamples = u8;
/// Number of EQAA fragments
pub type NumFragments = u8;

/// Dimensions: width, height, depth, and samples.
pub type Dimensions = (Size, Size, Size, AaMode);

/// Describes the configuration of samples inside each texel.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum AaMode {
    /// No additional sample information
    Single,
    /// MultiSampled Anti-Aliasing (MSAA)
    Multi(NumSamples),
    /// Coverage Sampling Anti-Aliasing (CSAA/EQAA)
    Coverage(NumSamples, NumFragments),
}

impl From<NumSamples> for AaMode {
    fn from(ns: NumSamples) -> AaMode {
        if ns > 1 {
            AaMode::Multi(ns)
        }else {
            AaMode::Single
        }
    }
}

impl AaMode {
    /// Return the number of actual data fragments stored per texel.
    pub fn get_num_fragments(&self) -> NumFragments {
        match *self {
            AaMode::Single => 1,
            AaMode::Multi(n) => n,
            AaMode::Coverage(_, nf) => nf,
        }
    }
    /// Return true if the surface has to be resolved before sampling.
    pub fn needs_resolve(&self) -> bool {
        self.get_num_fragments() > 1
    }
}


/// How to [filter](https://en.wikipedia.org/wiki/Texture_filtering) the
/// texture when sampling. They correspond to increasing levels of quality,
/// but also cost. They "layer" on top of each other: it is not possible to
/// have bilinear filtering without mipmapping, for example.
///
/// These names are somewhat poor, in that "bilinear" is really just doing
/// linear filtering on each axis, and it is only bilinear in the case of 2D
/// textures. Similarly for trilinear, it is really Quadralinear(?) for 3D
/// textures. Alas, these names are simple, and match certain intuitions
/// ingrained by many years of public use of inaccurate terminology.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum FilterMethod {
    /// The dumbest filtering possible, nearest-neighbor interpolation.
    Scale,
    /// Add simple mipmapping.
    Mipmap,
    /// Sample multiple texels within a single mipmap level to increase
    /// quality.
    Bilinear,
    /// Sample multiple texels across two mipmap levels to increase quality.
    Trilinear,
    /// Anisotropic filtering with a given "max", must be between 1 and 16,
    /// inclusive.
    Anisotropic(u8)
}

/// The face of a cube texture to do an operation on.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(missing_docs)]
pub enum CubeFace {
    PosZ,
    NegZ,
    PosX,
    NegX,
    PosY,
    NegY
}

/// Specifies the kind of a texture storage to be allocated.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Kind {
    /// A single row of texels.
    D1(Size),
    /// An array of rows of texels. Equivalent to Texture2D except that texels
    /// in a different row are not sampled.
    D1Array(Size, Layer),
    /// A traditional 2D texture, with rows arranged contiguously.
    D2(Size, Size, AaMode),
    /// An array of 2D textures. Equivalent to Texture3D except that texels in
    /// a different depth level are not sampled.
    D2Array(Size, Size, Layer, AaMode),
    /// A volume texture, with each 2D layer arranged contiguously.
    D3(Size, Size, Size),
    /// A set of 6 2D textures, one for each face of a cube.
    Cube(Size),
    /// An array of Cube textures.
    CubeArray(Size, Layer),
}

impl Kind {
    /// Get texture dimensions, with 0 values where not applicable.
    pub fn get_dimensions(&self) -> Dimensions {
        let s0 = AaMode::Single;
        match *self {
            Kind::D1(w) => (w, 0, 0, s0),
            Kind::D1Array(w, a) => (w, 0, a as Size, s0),
            Kind::D2(w, h, s) => (w, h, 0, s),
            Kind::D2Array(w, h, a, s) => (w, h, a as Size, s),
            Kind::D3(w, h, d) => (w, h, d, s0),
            Kind::Cube(w) => (w, w, 6, s0),
            Kind::CubeArray(w, a) => (w, w, 6 * (a as Size), s0)
        }
    }
    /// Get the dimensionality of a particular mipmap level.
    pub fn get_level_dimensions(&self, level: Level) -> Dimensions {
        use std::cmp::max;
        let (w, h, d, _) = self.get_dimensions();
        (max(1, w >> level), max(1, h >> level), max(1, d >> level), AaMode::Single)
    }
}


/// Describes a subvolume of a texture, which image data can be uploaded into.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct ImageInfoCommon<F> {
    pub xoffset: Size,
    pub yoffset: Size,
    pub zoffset: Size,
    pub width: Size,
    pub height: Size,
    pub depth: Size,
    /// Format of each texel.
    pub format: F,
    /// Which mipmap to select.
    pub mipmap: Level,
}

/// New raw image info based on the universal format spec.
pub type RawImageInfo = ImageInfoCommon<format::Format>;
/// New image info based on the universal format spec.
/// The format is suppsed to come from compile-time information
/// as opposed to run-time enum values.
pub type NewImageInfo = ImageInfoCommon<()>;

impl<F> ImageInfoCommon<F> {
    /// Get the total number of texels.
    pub fn get_texel_count(&self) -> usize {
        self.width as usize *
        self.height as usize *
        self.depth as usize
    }

    /// Convert into a differently typed format.
    pub fn convert<T>(&self, new_format: T) -> ImageInfoCommon<T> {
        ImageInfoCommon {
            xoffset: self.xoffset,
            yoffset: self.yoffset,
            zoffset: self.zoffset,
            width: self.width,
            height: self.height,
            depth: self.depth,
            format: new_format,
            mipmap: self.mipmap,
        }
    }

    /// Check if it fits inside given dimensions.
    pub fn is_inside(&self, (w, h, d, aa): Dimensions) -> bool {
        aa == AaMode::Single &&
        self.xoffset + self.width <= w &&
        self.yoffset + self.height <= h &&
        self.zoffset + self.depth <= d
    }
}

/// Specifies how texture coordinates outside the range `[0, 1]` are handled.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum WrapMode {
    /// Tile the texture. That is, sample the coordinate modulo `1.0`. This is
    /// the default.
    Tile,
    /// Mirror the texture. Like tile, but uses abs(coord) before the modulo.
    Mirror,
    /// Clamp the texture to the value at `0.0` or `1.0` respectively.
    Clamp,
    /// Use border color.
    Border,
}

/// A wrapper for the LOD level of a texture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub struct Lod(i16);

impl From<f32> for Lod {
    fn from(v: f32) -> Lod {
        Lod((v * 8.0) as i16)
    }
}

impl Into<f32> for Lod {
    fn into(self) -> f32 {
        self.0 as f32 / 8.0
    }
}

/// A wrapper for the 8bpp RGBA color, encoded as u32.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub struct PackedColor(pub u32);

impl From<[f32; 4]> for PackedColor {
    fn from(c: [f32; 4]) -> PackedColor {
        PackedColor(c.iter().rev().fold(0, |u, &c| {
            (u<<8) + (c * 255.0 + 0.5) as u32
        }))
    }
}

impl Into<[f32; 4]> for PackedColor {
    fn into(self) -> [f32; 4] {
        let mut out = [0.0; 4];
        for i in 0 .. 4 {
            let byte = (self.0 >> (i<<3)) & 0xFF;
            out[i] = (byte as f32 + 0.5) / 255.0;
        }
        out
    }
}

/// Specifies how to sample from a texture.
// TODO: document the details of sampling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub struct SamplerInfo {
    /// Filter method to use.
    pub filter: FilterMethod,
    /// Wrapping mode for each of the U, V, and W axis (S, T, and R in OpenGL
    /// speak).
    pub wrap_mode: (WrapMode, WrapMode, WrapMode),
    /// This bias is added to every computed mipmap level (N + lod_bias). For
    /// example, if it would select mipmap level 2 and lod_bias is 1, it will
    /// use mipmap level 3.
    pub lod_bias: Lod,
    /// This range is used to clamp LOD level used for sampling.
    pub lod_range: (Lod, Lod),
    /// Comparison mode, used primary for a shadow map.
    pub comparison: Option<state::Comparison>,
    /// Border color is used when one of the wrap modes is set to border.
    pub border: PackedColor,
}

impl SamplerInfo {
    /// Create a new sampler description with a given filter method and wrapping mode, using no LOD
    /// modifications.
    pub fn new(filter: FilterMethod, wrap: WrapMode) -> SamplerInfo {
        SamplerInfo {
            filter: filter,
            wrap_mode: (wrap, wrap, wrap),
            lod_bias: Lod(0),
            lod_range: (Lod(-8000), Lod(8000)),
            comparison: None,
            border: PackedColor(0),
        }
    }
}

/// Texture storage descriptor.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Descriptor {
    pub kind: Kind,
    pub levels: Level,
    pub format: format::SurfaceType,
    pub bind: Bind,
    pub usage: Usage,
}

impl Descriptor {
    /// Get image info for a given mip.
    pub fn to_image_info(&self, mip: Level) -> NewImageInfo {
        let (w, h, d, _) = self.kind.get_level_dimensions(mip);
        ImageInfoCommon {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: w,
            height: h,
            depth: d,
            format: (),
            mipmap: mip,
        }
    }

    /// Get the raw image info for a given mip and a channel type.
    pub fn to_raw_image_info(&self, cty: format::ChannelType, mip: Level) -> RawImageInfo {
        let format = format::Format(self.format, cty.into());
        self.to_image_info(mip).convert(format)
    }
}

/// Texture resource view descriptor.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct ResourceDesc {
    pub channel: format::ChannelType,
    pub min: Level,
    pub max: Level,
    pub swizzle: format::Swizzle,
}

/// Texture render view descriptor.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct RenderDesc {
    pub channel: format::ChannelType,
    pub level: Level,
    pub layer: Option<Layer>,
}

bitflags!(
    /// Depth-stencil read-only flags
    flags DepthStencilFlags: u8 {
        /// Depth is read-only in the view.
        const RO_DEPTH    = 0x1,
        /// Stencil is read-only in the view.
        const RO_STENCIL  = 0x2,
        /// Both depth and stencil are read-only.
        const RO_DEPTH_STENCIL = 0x3,
    }
);

/// Texture depth-stencil view descriptor.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct DepthStencilDesc {
    pub level: Level,
    pub layer: Option<Layer>,
    pub flags: DepthStencilFlags,
}

impl From<RenderDesc> for DepthStencilDesc {
    fn from(rd: RenderDesc) -> DepthStencilDesc {
        DepthStencilDesc {
            level: rd.level,
            layer: rd.layer,
            flags: DepthStencilFlags::empty(),
        }
    }
}
