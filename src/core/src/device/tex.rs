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

use attrib::{FloatSize, IntSubType};
use std::default::Default;
use std::fmt;

use state;

/// Surface creation/update error.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SurfaceError {
    /// Failed to map a given format to the device.
    UnsupportedFormat,
    /// Failed to provide sRGB formats.
    UnsupportedGamma,
}

/// Texture creation/update error.
#[derive(Copy, Clone, PartialEq)]
pub enum TextureError {
    /// Failed to map a given format to the device.
    UnsupportedFormat,
    /// Failed to provide sRGB formats.
    UnsupportedGamma,
    /// Failed to map a given multisampled kind to the device.
    UnsupportedSampling,
    /// The given TextureInfo contains invalid values.
    InvalidInfo(TextureInfo),
    /// The given data has a different size than the target texture slice.
    IncorrectSize(usize),
}

impl fmt::Debug for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &TextureError::UnsupportedFormat =>
                write!(f, "Failed to map a given format to the device"),

            &TextureError::UnsupportedGamma =>
                write!(f, "Failed to provide sRGB formats"),

            &TextureError::UnsupportedSampling =>
                write!(
                    f,
                    "Failed to map a given multisampled kind to the device"
                ),

            &TextureError::InvalidInfo(info) =>
                write!(
                    f,
                    "Invalid TextureInfo (width, height, and levels must not \
                    be zero): {:?}\n",
                    info
                ),
            &TextureError::IncorrectSize(expected) =>
                write!(
                    f,
                    "Invalid data size provided to update the texture, \
                    expected size {:?}",
                    expected
                ),
        }
    }
}

/// Dimension size
pub type Size = u16;
/// Number of bits per component
pub type Bits = u8;
/// Number of MSAA samples
pub type NumSamples = u8;
/// Number of EQAA fragments
pub type NumFragments = u8;

/// Describes the configuration of samples inside each texel.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum AaMode {
    /// MultiSampled Anti-Aliasing
    Msaa(NumSamples),
    /// Enhanced Quality Anti-Aliasing
    Eqaa(NumSamples, NumFragments),
}

/// Describes the color components of each texel.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
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

impl Components {
    /// Get the number of components.
    pub fn get_count(&self) -> u8 {
        match *self {
            Components::R     => 1,
            Components::RG   => 2,
            Components::RGB  => 3,
            Components::RGBA => 4,
        }
    }
}

/// Codec used to compress image data.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum Compression {
    /// Use the EXT2 algorithm on 3 components.
    ETC2_RGB,
    /// Use the EXT2 algorithm on 4 components (RGBA) in the sRGB color space.
    ETC2_SRGB,
    /// Use the EXT2 EAC algorithm on 4 components.
    ETC2_EAC_RGBA8,
}

/// Describes the layout of each texel within a surface/texture.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum Format {
    /// Floating point.
    Float(Components, FloatSize),
    /// Signed integer.
    Integer(Components, Bits, IntSubType),
    /// Unsigned integer.
    Unsigned(Components, Bits, IntSubType),
    /// Compressed data.
    Compressed(Compression),
    /// 3 bits for RG, 2 for B.
    R3_G3_B2,
    /// 5 bits for RB, 6 for G
    R5_G6_B5,
    /// 5 bits each for RGB, 1 for Alpha.
    RGB5_A1,
    /// 10 bits each for RGB, 2 for Alpha.
    RGB10_A2,
    /// 10 bits each for RGB, 2 for Alpha, as unsigned integers.
    RGB10_A2UI,
    /// This uses special 11 and 10-bit floating-point values without sign bits.
    R11F_G11F_B10F,
    /// This s an RGB format of type floating-point. The 3 color values have
    /// 9 bits of precision, and they share a single exponent.
    RGB9_E5,
    /// Swizzled RGBA color format, used for interaction with Windows DIBs
    BGRA8,
    /// Gamma-encoded RGB8
    SRGB8,
    /// Gamma-encoded RGB8, unchanged alpha
    SRGB8_A8,
    /// 16-bit bits depth
    DEPTH16,
    /// 24 bits depth
    DEPTH24,
    /// 32 floating-point bits depth
    DEPTH32F,
    /// 24 bits for depth, 8 for stencil
    DEPTH24_STENCIL8,
    /// 32 floating point bits for depth, 8 for stencil
    DEPTH32F_STENCIL8,
}

impl Format {
    /// Extract the components format
    pub fn get_components(&self) -> Option<Components> {
        Some(match *self {
            Format::Float(c, _)       => c,
            Format::Integer(c, _, _)  => c,
            Format::Unsigned(c, _, _) => c,
            Format::Compressed(_)     => {
                error!("Tried to get components of compressed texel!");
                return None
            },
            Format::R3_G3_B2          |
            Format::R5_G6_B5          |
            Format::R11F_G11F_B10F    |
            Format::RGB9_E5           |
            Format::SRGB8             => Components::RGB,
            Format::RGB5_A1           |
            Format::RGB10_A2          |
            Format::RGB10_A2UI        |
            Format::BGRA8             |
            Format::SRGB8_A8          => Components::RGBA,
            // not sure about depth/stencil
            Format::DEPTH16           |
            Format::DEPTH24           |
            Format::DEPTH32F          |
            Format::DEPTH24_STENCIL8  |
            Format::DEPTH32F_STENCIL8 => return None,
        })
    }

    /// Check if it's a color format.
    pub fn is_color(&self) -> bool {
        match *self {
            Format::DEPTH16           |
            Format::DEPTH24           |
            Format::DEPTH32F          |
            Format::DEPTH24_STENCIL8  |
            Format::DEPTH32F_STENCIL8 => false,
            _ => true,
        }
    }

    /// Check if it has a depth component.
    pub fn has_depth(&self) -> bool {
        match *self {
            Format::DEPTH16           |
            Format::DEPTH24           |
            Format::DEPTH32F          |
            Format::DEPTH24_STENCIL8  |
            Format::DEPTH32F_STENCIL8 => true,
            _ => false,
        }
    }

    /// Check if it has a stencil component.
    pub fn has_stencil(&self) -> bool {
        match *self {
            Format::DEPTH24_STENCIL8  |
            Format::DEPTH32F_STENCIL8 => true,
            _ => false,
        }
    }

    /// Check if it's a compressed format.
    pub fn is_compressed(&self) -> bool {
        match *self {
            Format::Compressed(_) => true,
            _ => false
        }
    }

    /// Check if it's a sRGB color space.
    pub fn does_convert_gamma(&self) -> bool {
        match *self {
            Format::SRGB8    |
            Format::SRGB8_A8 |
            Format::Compressed(Compression::ETC2_SRGB) => true,
            _ => false,
        }
    }

    /// Get size of the texel in bytes.
    pub fn get_size(&self) -> Option<u8> {
        Some(match *self {
            Format::Float(c, FloatSize::F16) => c.get_count() * 2,
            Format::Float(c, FloatSize::F32) => c.get_count() * 4,
            Format::Float(c, FloatSize::F64) => c.get_count() * 8,
            Format::Integer(c, bits, _) => (c.get_count() * bits) >> 3,
            Format::Unsigned(c, bits, _) => (c.get_count() * bits) >> 3,
            Format::Compressed(_) => return None,
            Format::R3_G3_B2 => 1,
            Format::R5_G6_B5 => 2,
            Format::RGB5_A1 => 2,
            Format::RGB10_A2 => 4,
            Format::RGB10_A2UI => 4,
            Format::R11F_G11F_B10F => 4,
            Format::RGB9_E5 => 4,
            Format::BGRA8 => 4,
            Format::SRGB8 => 4,
            Format::SRGB8_A8 => 4,
            Format::DEPTH16 => 2,
            Format::DEPTH24 => 4,
            Format::DEPTH32F => 4,
            Format::DEPTH24_STENCIL8 => 4,
            Format::DEPTH32F_STENCIL8 => 8,
        })
    }
}

/// A single R-component 8-bit normalized format.
pub static R8     : Format = Format::Unsigned(Components::R, 8, IntSubType::Normalized);
/// A standard RGBA 8-bit normalized format.
pub static RGBA8  : Format = Format::Unsigned(Components::RGBA, 8, IntSubType::Normalized);
/// A standard RGBA 16-bit floating-point format.
pub static RGBA16F: Format = Format::Float(Components::RGBA, FloatSize::F16);
/// A standard RGBA 32-bit floating-point format.
pub static RGBA32F: Format = Format::Float(Components::RGBA, FloatSize::F32);

/// Describes the storage of a surface.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct SurfaceInfo {
    pub width: Size,
    pub height: Size,
    pub format: Format,
    pub aa_mode: Option<AaMode>,
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

/// Specifies how a given texture may be used. The available texture types are
/// restricted by what Metal exposes, though this could conceivably be
/// extended in the future. Note that a single texture can *only* ever be of
/// one kind. A texture created as `Texture2D` will forever be `Texture2D`.
// TODO: "Texture views" let you get around that limitation.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Kind {
    /// A single row of texels.
    D1,
    /// An array of rows of texels. Equivalent to Texture2D except that texels
    /// in a different row are not sampled.
    D1Array,
    /// A traditional 2D texture, with rows arranged contiguously.
    D2,
    /// An array of 2D textures. Equivalent to Texture3D except that texels in
    /// a different depth level are not sampled.
    D2Array,
    /// A multi-sampled 2D texture. Each pixel may have more than one data value
    /// (sample) associated with it.
    D2MultiSample(AaMode),
    /// A array of multi-sampled 2D textures.
    D2MultiSampleArray(AaMode),
    /// A set of 6 2D textures, one for each face of a cube.
    ///
    /// When creating a cube texture, the face is ignored, and storage for all 6 faces is created.
    /// When updating, only the face specified is updated.
    Cube(CubeFace),
    /// A volume texture, with each 2D layer arranged contiguously.
    D3,
}

impl Kind {
    /// Return the anti-aliasing mode of the texture
    pub fn get_aa_mode(&self) -> Option<AaMode> {
        match *self {
            Kind::D2MultiSample(aa) => Some(aa),
            Kind::D2MultiSampleArray(aa) => Some(aa),
            _ => None,
        }
    }
}

/// Describes the storage of a texture.
///
/// # Portability note
///
/// Textures larger than 1024px in any dimension are unlikely to be supported
/// by mobile platforms.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct TextureInfo {
    pub width: Size,
    pub height: Size,
    pub depth: Size,
    /// Number of mipmap levels. Defaults to -1, which stands for unlimited.
    /// Mipmap levels at equal or above `levels` can not be loaded or sampled
    /// by the shader. width and height of each consecutive mipmap level is
    /// halved, starting from level 0.
    pub levels: u8,
    pub kind: Kind,
    pub format: Format,
}

/// Describes a subvolume of a texture, which image data can be uploaded into.
#[allow(missing_docs)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct ImageInfo {
    pub xoffset: Size,
    pub yoffset: Size,
    pub zoffset: Size,
    pub width: Size,
    pub height: Size,
    pub depth: Size,
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
            levels: !0,
            kind: Kind::D2,
            format: RGBA8,
        }
    }
}

impl TextureInfo {
    /// Create a new empty texture info.
    pub fn new() -> TextureInfo {
        Default::default()
    }

    /// Check if given ImageInfo is a part of the texture.
    pub fn contains(&self, img: &ImageInfo) -> bool {
        self.width <= img.xoffset + img.width &&
        self.height <= img.yoffset + img.height &&
        self.depth <= img.zoffset + img.depth &&
        self.format == img.format &&
        img.mipmap < self.levels &&
        self.kind.get_aa_mode().is_none()
    }
}

impl From<TextureInfo> for ImageInfo {
    fn from(ti: TextureInfo) -> ImageInfo {
        ImageInfo {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: ti.width,
            height: ti.height,
            depth: ti.depth,
            format: ti.format,
            mipmap: 0,
        }
    }
}

impl From<TextureInfo> for SurfaceInfo {
    fn from(ti: TextureInfo) -> SurfaceInfo {
        SurfaceInfo {
            width: ti.width,
            height: ti.height,
            format: ti.format,
            aa_mode: ti.kind.get_aa_mode(),
        }
    }
}

impl ImageInfo {
    /// Create a new `ImageInfo`, using default values.
    pub fn new() -> ImageInfo { Default::default() }
    /// Get the total number of texels.
    pub fn get_texel_count(&self) -> usize {
        self.width as usize *
        self.height as usize *
        self.depth as usize
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
}

/// Specifies how to sample from a texture.
// TODO: document the details of sampling.
#[derive(PartialEq, PartialOrd, Clone, Copy, Debug)]
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
    /// comparison mode, used primary for a shadow map
    pub comparison: Option<state::Comparison>,
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
            comparison: None,
        }
    }
}
