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

//! Safe texture types

use device::shade::{IsArray, Array, NoArray};
use device::tex;

// Dimension size
pub type Size = u16;
pub type Layers = u16;

/// The texture format received from the user
pub struct Format<Kind, Comp, Aa, Texel>(pub Kind, pub Comp, pub Aa, pub Texel);

/// Convertable to TextureInfo
pub trait ToTextureInfo {
	/// Convert into TextureInfo
    fn into_texture_info(self) -> tex::TextureInfo;
}
impl<
    Kind: ToKind,
    Comp: ToComponents,
    Aa,
    Texel: ToFormat
> ToTextureInfo
for Format<Kind, Comp, Aa, Texel> {
    fn into_texture_info(self) -> tex::TextureInfo {
        let Format(kind, comp, _aa, texel) = self;
        let (kind, w, h, d) = kind.into_kind();
        let comp = comp.into_components();
        let format = texel.into_format(comp);
        tex::TextureInfo {
            width: w,
            height: h,
            depth: d,
            mipmap_range: (0, -1),
            kind: kind,
            format: format,
        }
    }
}

// Kind
/// 1D texture
pub struct Tex1D(pub Size);
/// An array of 1D textures
pub struct Tex1DArray(pub Size, pub Layers);
/// 2D texture or 1D array
pub struct Tex2D(pub Size, pub Size);
/// An array of 2D textures
pub struct Tex2DArray(pub Size, pub Size, pub Layers);
/// 3D texture or 2D array
pub struct Tex3D(pub Size, pub Size, pub Size);
/// Cube texture
pub struct TexCube(pub Size);
/// An array of Cube textures
pub struct TexCubeArray(pub Size, pub Layers);

/// Convertible to TextureKind and sizes
pub trait ToKind {
    /// Convert into TextureKind
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size);
}

impl ToKind for Tex1D {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let Tex1D(w) = self;
        (tex::Texture1D, w, 0, 0)
    }
}

impl ToKind for Tex1DArray {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let Tex1DArray(w, d) = self;
        (tex::Texture1DArray, w, 0, d)
    }
}

impl ToKind for Tex2D {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let Tex2D(w, h) = self;
        (tex::Texture2D, w, h, 0)
    }
}

impl ToKind for Tex2DArray {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let Tex2DArray(w, h, d) = self;
        (tex::Texture2DArray, w, h, d)
    }
}

impl ToKind for Tex3D {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let Tex3D(w, h, d) = self;
        (tex::Texture3D, w, h, d)
    }
}

impl ToKind for TexCube {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let TexCube(s) = self;
        (tex::TextureCube, s, s, 0)
    }
}

impl ToKind for TexCubeArray {
    fn into_kind(self) -> (tex::TextureKind, Size, Size, Size) {
        let TexCubeArray(s, d) = self;
        (tex::TextureCubeArray, s, s, d)
    }
}


// Comp
/// Red only
pub struct R;
/// Red and green
pub struct RG;
/// Red, green, blue
pub struct RGB;
/// Red, green, blue, alpha
pub struct RGBA;

/// Convertible to texture Components
pub trait ToComponents {
    /// Convert into Components
    fn into_components(self) -> tex::Components;
}

impl ToComponents for R {
    fn into_components(self) -> tex::Components {
        tex::R
    }
}

impl ToComponents for RG {
    fn into_components(self) -> tex::Components {
        tex::RG
    }
}

impl ToComponents for RGB {
    fn into_components(self) -> tex::Components {
        tex::RGB
    }
}

impl ToComponents for RGBA {
    fn into_components(self) -> tex::Components {
        tex::RGBA
    }
}

// Aa
/// No anti-aliasing
pub struct Noaa;
/// Multiple samples
pub struct Msaa;

// Texel
/// Floating point values
pub struct TexelFloat;
/// Integer values
pub struct TexelInteger;
/// Unsigned integer values
pub struct TexelUnsigned;

/// Convertible to texture Format
pub trait ToFormat {
    /// Convert into Format
    fn into_format(self, c: tex::Components) -> tex::Format;
}

impl ToFormat for TexelFloat {
    fn into_format(self, c: tex::Components) -> tex::Format {
        tex::Float(c, tex::F16)
    }
}

impl ToFormat for TexelInteger {
    fn into_format(self, c: tex::Components) -> tex::Format {
        tex::Integer(c, 8, tex::IntNormalized)
    }
}

impl ToFormat for TexelUnsigned {
    fn into_format(self, c: tex::Components) -> tex::Format {
        tex::Unsigned(c, 8, tex::IntNormalized)
    }
}
