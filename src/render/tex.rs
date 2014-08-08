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

use device::tex;

// Dimension size
pub type Size = u16;

/// A type-safe wrapper for the texture handle
pub struct View<Kind, Comp, Aa, Texel>(super::TextureHandle);

/// The texture format received from the user
pub struct Format<Kind, Comp, Aa, Texel>(pub Kind, pub Comp, pub Aa, pub Texel);

/// Convertable to TextureInfo
pub trait ToTextureInfo {
	/// Convert to TextureInfo
    fn to_texture_info(&self) -> tex::TextureInfo;
}
impl<Kind, Comp, Aa, Texel> ToTextureInfo for Format<Kind, Comp, Aa, Texel> {
    fn to_texture_info(&self) -> tex::TextureInfo {
        unimplemented!()
    }
}

// Kind
/// 1D texture
pub struct Tex1D(pub Size);
/// 2D texture or 1D array
pub struct Tex2D(pub Size, pub Size);
/// 3D texture or 2D array
pub struct Tex3D(pub Size, pub Size, pub Size);
/// Cube texture
pub struct TexCube(pub Size);

// Comp
/// Red only
pub struct R;
/// Red and green
pub struct RG;
/// Red, green, blue
pub struct RGB;
/// Red, green, blue, alpha
pub struct RGBA;

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
