// Copyright 2015 The Gfx-rs Developers.
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

//! Universal format specification.
//! Applicable to textures, views, and vertex buffers.

//TODO:
//  ETC2_RGB, // Use the EXT2 algorithm on 3 components.
//  ETC2_SRGB, // Use the EXT2 algorithm on 4 components (RGBA) in the sRGB color space.
//  ETC2_EAC_RGBA8, // Use the EXT2 EAC algorithm on 4 components.


macro_rules! impl_surface_type {
    { $($name:ident [$bits:expr] $(=$tr:ty)* ,)* } => {
        /// Type of the allocated texture surface. It is supposed to only
        /// carry information about the number of bits per each channel.
        /// The actual types are up to the views to decide and interpret.
        /// The actual components are up to the swizzle to define.
        #[repr(u8)]
        #[allow(missing_docs, non_camel_case_types)]
        #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
        pub enum SurfaceType {
            $( $name, )*
        }
        impl SurfaceType {
            /// Return the total number of bits for this format.
            pub fn get_bit_size(&self) -> u8 {
                match *self {
                    $( SurfaceType::$name => $bits, )*
                }
            }
        }
        $(
            #[allow(missing_docs, non_camel_case_types)]
            #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
            pub enum $name {}
            impl SurfaceTyped for $name {
                fn get_surface_type() -> SurfaceType {
                    SurfaceType::$name
                }
                fn get_bit_size() -> u8 {
                    $bits
                }
            }
            $(
                impl $tr for $name {}
            )*
        )*
    }
}

impl_surface_type! {
    R3_G3_B2        [8]   = TextureSurface,
    R4_G4           [8]   = TextureSurface = RenderSurface,
    R4_G4_B4_A4     [16]  = TextureSurface = RenderSurface,
    R5_G5_B5_A1     [16]  = TextureSurface = RenderSurface,
    R5_G6_B5        [16]  = TextureSurface = RenderSurface,
    R8              [8]   = BufferSurface = TextureSurface = RenderSurface,
    R8_G8           [16]  = BufferSurface = TextureSurface = RenderSurface,
    R8_G8_B8        [24]  = BufferSurface = TextureSurface = RenderSurface,
    R8_G8_B8_A8     [32]  = BufferSurface = TextureSurface = RenderSurface,
    R10_G10_B10_A2  [32]  = BufferSurface = TextureSurface = RenderSurface,
    R16             [16]  = BufferSurface = TextureSurface = RenderSurface,
    R16_G16         [32]  = BufferSurface = TextureSurface = RenderSurface,
    R16_G16_B16     [48]  = BufferSurface = TextureSurface = RenderSurface,
    R16_G16_B16_A16 [64]  = BufferSurface = TextureSurface = RenderSurface,
    R32             [32]  = BufferSurface = TextureSurface = RenderSurface,
    R32_G32         [64]  = BufferSurface = TextureSurface = RenderSurface,
    R32_G32_B32     [96]  = BufferSurface = TextureSurface = RenderSurface,
    R32_G32_B32_A32 [128] = BufferSurface = TextureSurface = RenderSurface,
    D16             [16]  = TextureSurface = DepthSurface,
    D24             [24]  = TextureSurface = DepthSurface,
    D24_S8          [32]  = TextureSurface = DepthSurface = StencilSurface,
    D32             [32]  = TextureSurface = DepthSurface,
}

macro_rules! impl_channel_type {
    { $($name:ident : $shtype:ident $(=$tr:ident)* ,)* } => {
        /// Type of a surface channel. This is how we interpret the
        /// storage allocated with `SurfaceType`.
        #[allow(missing_docs)]
        #[repr(u8)]
        #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
        pub enum ChannelType {
            $( $name, )*
        }
        $(
            #[allow(missing_docs)]
            #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
            pub enum $name {}
            impl ChannelTyped for $name {
                type ShaderType = $shtype;
                fn get_channel_type() -> ChannelType {
                    ChannelType::$name
                }
            }
            $(
                impl $tr for $name {}
            )*
        )*
    }
}

impl_channel_type! {
    Int             : i32 = TextureChannel = RenderChannel,
    Uint            : u32 = TextureChannel = RenderChannel,
    IntScaled       : f32 = TextureChannel,
    UintScaled      : f32 = TextureChannel,
    IntNormalized   : f32 = TextureChannel = RenderChannel = BlendChannel,
    UintNormalized  : f32 = TextureChannel = RenderChannel = BlendChannel,
    Float           : f32 = TextureChannel = RenderChannel = BlendChannel,
    Srgb            : f32 = TextureChannel = RenderChannel = BlendChannel,
}

/// Source channel in a swizzle configuration. Some may redirect onto
/// different physical channels, some may be hardcoded to 0 or 1.
#[allow(missing_docs)]
#[repr(u8)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum ChannelSource {
    Zero,
    One,
    X,
    Y,
    Z,
    W,
}

/// Channel swizzle configuration for the resource views.
/// Note: It's not currently mirrored at compile-time,
/// thus providing less safety and convenience.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Swizzle(pub ChannelSource, pub ChannelSource, pub ChannelSource, pub ChannelSource);

impl Swizzle {
    /// Create a new swizzle where each channel is unmapped.
    pub fn new() -> Swizzle {
        Swizzle(ChannelSource::X, ChannelSource::Y, ChannelSource::Z, ChannelSource::W)
    }
}

/// Complete run-time surface format.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Format(pub SurfaceType, pub ChannelType);


/// Compile-time surface type trait.
pub trait SurfaceTyped {
    /// Return the run-time value of the type.
    fn get_surface_type() -> SurfaceType;
    /// Return the total number of bits.
    fn get_bit_size() -> u8;
}
/// An ability of a surface type to be used for vertex buffers.
pub trait BufferSurface: SurfaceTyped {}
/// An ability of a surface type to be used for textures.
pub trait TextureSurface: SurfaceTyped {}
/// An ability of a surface type to be used for render targets.
pub trait RenderSurface: SurfaceTyped {}
/// An ability of a surface type to be used for depth targets.
pub trait DepthSurface: SurfaceTyped {}
/// An ability of a surface type to be used for stencil targets.
pub trait StencilSurface: SurfaceTyped {}

/// Compile-time channel type trait.
pub trait ChannelTyped {
    /// Shader-visible type that corresponds to this channel.
    /// For example, normalized and scaled integers are visible as floats.
    type ShaderType;
    /// Return the run-time value of the type.
    fn get_channel_type() -> ChannelType;
}
/// An ability of a channel type to be used for textures.
pub trait TextureChannel: ChannelTyped {}
/// An ability of a channel type to be used for render targets.
pub trait RenderChannel: ChannelTyped {}
/// An ability of a channel type to be used for blended render targets.
pub trait BlendChannel: RenderChannel {}

/// Compile-time full format trait.
pub trait Formatted {
    /// Associated surface type.
    type Surface: SurfaceTyped;
    /// Associated channel type.
    type Channel: ChannelTyped;
    /// Shader view type of this format.
    type View;
    /// Return the run-time value of the type.
    fn get_format() -> Format {
        Format(
            Self::Surface::get_surface_type(),
            Self::Channel::get_channel_type())
    }
}
/// Ability to be used for vertex buffers.
pub trait BufferFormat: Formatted {}
/// Ability to be used for depth targets.
pub trait DepthFormat: Formatted {}
/// Ability to be used for vertex buffers.
pub trait StencilFormat: Formatted {}
/// Ability to be used for depth+stencil targets.
pub trait DepthStencilFormat: DepthFormat + StencilFormat {}
/// Ability to be used for textures.
pub trait TextureFormat: Formatted {}
/// Ability to be used for render targets.
pub trait RenderFormat: Formatted {}
/// Ability to be used for blended render targets.
pub trait BlendFormat: RenderFormat {}

impl<S: SurfaceTyped, C: ChannelTyped, T> Formatted for (S, C, T) {
    type Surface = S;
    type Channel = C;
    type View = T;
}

impl<F: Formatted> BufferFormat for F where
    F::Surface: BufferSurface,
    F::Channel: ChannelTyped,
{}
impl<F: Formatted> DepthFormat for F where
    F::Surface: DepthSurface,
    F::Channel: RenderChannel,
{}
impl<F> StencilFormat for F where
    F: DepthFormat + StencilFormat,
    F::Surface: StencilSurface,
    F::Channel: RenderChannel,
{}
impl<F: Formatted> DepthStencilFormat for F where
    F::Surface: DepthSurface + StencilSurface,
    F::Channel: RenderChannel,
{}
impl<F: Formatted> TextureFormat for F where
    F::Surface: TextureSurface,
    F::Channel: TextureChannel,
{}
impl<F: Formatted> RenderFormat for F where
    F::Surface: RenderSurface,
    F::Channel: RenderChannel,
{}
impl<F: Formatted> BlendFormat for F where
    F::Surface: RenderSurface,
    F::Channel: BlendChannel,
{}

macro_rules! alias {
    { $( $name:ident = $ty:ty, )* } => {
        $(
            #[allow(missing_docs)]
            #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
            pub struct $name(pub $ty);
            impl From<$ty> for $name {
                fn from(v: $ty) -> $name {
                    $name(v)
                }
            }
            impl $name {
                /// Convert a 2-element slice.
                pub fn cast2(v: [$ty; 2]) -> [$name; 2] {
                    [$name(v[0]), $name(v[1])]
                }
                /// Convert a 3-element slice.
                pub fn cast3(v: [$ty; 3]) -> [$name; 3] {
                    [$name(v[0]), $name(v[1]), $name(v[2])]
                }
                /// Convert a 4-element slice.
                pub fn cast4(v: [$ty; 4]) -> [$name; 4] {
                    [$name(v[0]), $name(v[1]), $name(v[2]), $name(v[3])]
                }
                /// Convert a generic slice by transmutation.
                pub fn cast_slice(slice: &[$ty]) -> &[$name] {
                    use std::mem::transmute;
                    unsafe { transmute(slice) }
                }
            }
        )*
    }
}

alias! {
    U8Norm = u8,
    I8Norm = i8,
    U8Scaled = u8,
    I8Scaled = i8,
    U16Norm = u16,
    I16Norm = i16,
    F16 = u16, // half-float
}

/// Abstracted 1-element container for macro internal use
pub type Vec1<T> = T;
/// Abstracted 2-element container for macro internal use
pub type Vec2<T> = [T; 2];
/// Abstracted 3-element container for macro internal use
pub type Vec3<T> = [T; 3];
/// Abstracted 4-element container for macro internal use
pub type Vec4<T> = [T; 4];

macro_rules! impl_format {
    { $( $container:ident< $ty:ty > = $channel:ident $surface:ident, )* } => {
        $(
            impl Formatted for $container<$ty> {
                type Surface = $surface;
                type Channel = $channel;
                type View = $container<<$channel as ChannelTyped>::ShaderType>;
            }
        )*
    }
}

macro_rules! impl_formats_8bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            Vec1<$ty> = $channel R8,
            Vec2<$ty> = $channel R8_G8,
            Vec3<$ty> = $channel R8_G8_B8,
            Vec4<$ty> = $channel R8_G8_B8_A8,
        )*}
    }
}

macro_rules! impl_formats_16bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            Vec1<$ty> = $channel R16,
            Vec2<$ty> = $channel R16_G16,
            Vec3<$ty> = $channel R16_G16_B16,
            Vec4<$ty> = $channel R16_G16_B16_A16,
        )*}
    }
}

macro_rules! impl_formats_32bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            Vec1<$ty> = $channel R32,
            Vec2<$ty> = $channel R32_G32,
            Vec3<$ty> = $channel R32_G32_B32,
            Vec4<$ty> = $channel R32_G32_B32_A32,
        )*}
    }
}

impl_formats_8bit! {
    u8 = Uint,
    i8 = Int,
    U8Norm = UintNormalized,
    I8Norm = IntNormalized,
    U8Scaled = UintScaled,
    I8Scaled = IntScaled,
}

impl_formats_16bit! {
    u16 = Uint,
    i16 = Int,
    U16Norm = UintNormalized,
    I16Norm = IntNormalized,
    F16 = Float,
}

impl_formats_32bit! {
    u32 = Uint,
    i32 = Int,
    f32 = Float,
}


/// Standard 8bits RGBA format.
pub type Rgba8 = [U8Norm; 4]; //(R8_G8_B8_A8, UintNormalized);
/// Standard HDR floating-point format with 10 bits for RGB components
/// and 2 bits for the alpha.
pub type Rgb10a2F = (R10_G10_B10_A2, Float, [f32; 4]);
/// Standard 16-bit floating-point RGBA format.
pub type Rgba16F = [F16; 4]; //(R16_G16_B16_A16, Float);
/// Standard 32-bit floating-point RGBA format.
pub type Rgba32F = [f32; 4]; //(R32_G32_B32_A32, Float);
/// Standard 24-bit depth format.
pub type Depth = (D24, UintNormalized, f32);
/// Standard 24-bit depth format with 8-bit stencil.
pub type DepthStencil = (D24_S8, UintNormalized, f32);
/// Standard 32-bit floating-point depth format.
pub type Depth32F = (D32, Float, f32);
