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

use std::default::Default;

/// Number of bits per component
pub type Bits = u8;

/// Describes the component layout of each texel.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum Components {
    /// Red only
    R,
    /// Red and green
    RG,
    /// Red, green, blue
    RGB,
    /// Red, green, blue, alpha
    RGBA,
}

/// Describes the layout of each texel within a surface/texture.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum Format {
    /// Floating point.
    Float(Components, ::attrib::FloatSize),
    /// Signed integer.
    Integer(Components, Bits, ::attrib::IntSubType),
    /// Unsigned integer.
    Unsigned(Components, Bits, ::attrib::IntSubType),
    /// Normalized integer, with 3 bits for R and G, but only 2 for B.
    R3G3B2,
    /// 5 bits each for RGB, 1 for Alpha.
    RGB5A1,
    /// 10 bits each for RGB, 2 for Alpha.
    RGB10A2,
    /// 10 bits each for RGB, 2 for Alpha, as unsigned integers.
    RGB10A2UI,
    /// This uses special 11 and 10-bit floating-point values without sign bits.
    R11FG11FB10F,
    /// This s an RGB format of type floating-point. The 3 color values have
    /// 9 bits of precision, and they share a single exponent.
    RGB9E5,
    // TODO: sRGB, compression
}

/// A commonly used RGBA8 format
pub static RGBA8: Format = Unsigned(RGBA, 8, ::attrib::IntNormalized);

/// Describes the storage of a surface
#[allow(missing_doc)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct SurfaceInfo {
    pub width: u16,
    pub height: u16,
    pub format: Format,
    // TODO: Multisampling
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
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
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

/// Specifies how a given texture may be used. The available texture types are
/// restricted by what Metal exposes, though this could conceivably be
/// extended in the future. Note that a single texture can *only* ever be of
/// one kind. A texture created as `Texture2D` will forever be `Texture2D`.
// TODO: "Texture views" let you get around that limitation.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum TextureKind {
    /// A single row of texels.
    Texture1D,
    /// An array of rows of texels. Equivalent to Texture2D except that texels
    /// in a different row are not sampled.
    Texture1DArray,
    /// A traditional 2D texture, with rows arranged contiguously.
    Texture2D,
    /// An array of 2D textures. Equivalent to Texture3D except that texels in
    /// a different depth level are not sampled.
    Texture2DArray,
    /// A set of 6 2D textures, one for each face of a cube.
    // TODO: implement this, and document it better. cmr doesn't really understand them well enough
    // to explain without rambling.
    TextureCube,
    /// A volume texture, with each 2D layer arranged contiguously.
    Texture3D,
    // TODO: Multisampling?
}

/// Describes the storage of a texture.
///
/// # Portability note
///
/// Textures larger than 1024px in any dimension are unlikely to be supported
/// by mobile platforms.
#[allow(missing_doc)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct TextureInfo {
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    /// Mipmap levels outside the range of `[lo, hi]` will never be used for
    /// this texture. Defaults to `(0, -1)`, that is, every mipmap level
    /// available. 0 is the base mipmap level, with the full-sized texture,
    /// and every level after that shrinks each dimension by a factor of 2.
    pub mipmap_range: (u8, u8),
    pub kind: TextureKind,
    pub format: Format,
}

/// Describes a subvolume of a texture, which image data can be uploaded into.
#[allow(missing_doc)]
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct ImageInfo {
    pub xoffset: u16,
    pub yoffset: u16,
    pub zoffset: u16,
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    /// Format of each texel.
    pub format: Format,
    /// Which mipmap to select.
    pub mipmap: u8,
}

impl Default for ImageInfo {
    fn default() -> ImageInfo {
        ImageInfo {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: 0,
            height: 1,
            depth: 1,
            format: RGBA8,
            mipmap: 0
        }
    }
}

impl Default for TextureInfo {
    fn default() -> TextureInfo {
        TextureInfo {
            width: 0,
            height: 1,
            depth: 1,
            mipmap_range: (0, -1),
            kind: Texture2D,
            format: RGBA8,
        }
    }
}

impl TextureInfo {
    /// Create a new empty texture info
    pub fn new() -> TextureInfo {
        Default::default()
    }

    /// Convert to a default ImageInfo that could be used
    /// to update the contents of the whole texture
    pub fn to_image_info(&self) -> ImageInfo {
        ImageInfo {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: self.width,
            height: self.height,
            depth: self.depth,
            format: self.format,
            mipmap: self.mipmap_range.val0(),
        }
    }

    /// Check if given ImageInfo is a part of the texture
    pub fn contains(&self, img: &ImageInfo) -> bool {
        self.width <= img.xoffset + img.width &&
        self.height <= img.yoffset + img.height &&
        self.depth <= img.zoffset + img.depth &&
        self.format == img.format &&
        self.mipmap_range.val0() <= img.mipmap && img.mipmap < self.mipmap_range.val1()
    }
}

impl ImageInfo {
    /// Create a new `ImageInfo`, using default values.
    pub fn new() -> ImageInfo { Default::default() }
}

/// Specifies how texture coordinates outside the range `[0, 1]` are handled.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum WrapMode {
    /// Tile the texture. That is, sample the coordinate modulo `1.0`. This is
    /// the default.
    Tile,
    /// Mirror the texture. Like tile, but uses abs(coord) before the modulo.
    Mirror,
    /// Clamp the texture to the value at `0.0` or `1.0` respectively.
    Clamp,
}

/// Specifies how to sample from a texture.
// TODO: document the details of sampling.
#[deriving(PartialEq, PartialOrd, Clone, Show)]
pub struct SamplerInfo {
    /// Filter method to use.
    pub filtering: FilterMethod,
    /// Wrapping mode for each of the U, V, and W axis (S, T, and R in OpenGL
    /// speak)
    pub wrap_mode: (WrapMode, WrapMode, WrapMode),
    /// This bias is added to every computed mipmap level (N + lod_bias). For
    /// example, if it would select mipmap level 2 and lod_bias is 1, it will
    /// use mipmap level 3.
    pub lod_bias: f32,
    /// This range is used to clamp LOD level used for sampling
    pub lod_range: (f32, f32),
    // TODO: comparison mode
}

impl SamplerInfo {
    /// Create a new sampler description with a given filter method and wrapping mode, using no LOD
    /// modifications.
    pub fn new(filtering: FilterMethod, wrap: WrapMode) -> SamplerInfo {
        SamplerInfo {
            filtering: filtering,
            wrap_mode: (wrap, wrap, wrap),
            lod_bias: 0.0,
            lod_range: (-1000.0, 1000.0),
        }
    }
}
