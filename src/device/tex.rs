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
//! texture data.

use std::default::Default;

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

/// Specifies how a given texture may be used. This is mostly due to OpenGL
/// and Metal being braindamaged and not allowing, for example, a texture
/// previously used as Texture2D to be used as Texture3D. The available
/// texture types are restricted by what Metal exposes.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum TextureKind {
    Texture1D,
    Texture1DArray,
    Texture2D,
    Texture2DArray,
    TextureCube,
    Texture3D
    // TODO: Multisampling?
}

/// Describes the layout of each texel within a texture.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum TextureFormat {
    RGB8,
    RGBA8,
}

/// Describes the storage of a texture.
///
/// Portability note: textures larger than 1024px in any dimension are
/// unlikely to be supported by mobile platforms.
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
    pub format: TextureFormat,
}

/// Describes a subvolume of a texture, which image data can be uploaded into.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct ImageInfo {
    pub xoffset: u16,
    pub yoffset: u16,
    pub zoffset: u16,
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub format: TextureFormat,
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
            height: 0,
            depth: 0,
            format: RGBA8,
            mipmap: 0
        }
    }
}

impl Default for TextureInfo {
    fn default() -> TextureInfo {
        TextureInfo {
            width: 0,
            height: 0,
            depth: 0,
            mipmap_range: (0, -1),
            kind: Texture2D,
            format: RGBA8,
        }
    }
}

impl TextureInfo { pub fn new() -> TextureInfo { Default::default() } }

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
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub struct SamplerInfo {
    pub filtering: FilterMethod,
    /// This bias is added to every computed mipmap level (N + lod_bias). For
    /// example, if it would select mipmap level 2 and lod_bias is 1, it will
    /// use mipmap level 3.
    pub lod_bias: u8,
    /// Mipmap levels outside of `[lo, hi]` will never be sampled. Defaults to
    /// `(0, -1)` (every mipmap available), but will be clamped to the
    /// texture's mipmap_range.
    pub mipmap_range: (u8, u8),
    /// Wrapping mode for each of the U, V, and W axis (S, T, and R in OpenGL
    /// speak)
    pub wrap_mode: (WrapMode, WrapMode, WrapMode),
    // TODO: comparison mode
    // TODO: Borders (we don't actually need this, afaik)
}
