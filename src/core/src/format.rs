// Copyright 2016 The Gfx-rs Developers.
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
//  DXT 1-5, BC7
//  ETC2_RGB, // Use the EXT2 algorithm on 3 components.
//  ETC2_SRGB, // Use the EXT2 algorithm on 4 components (RGBA) in the sRGB color space.
//  ETC2_EAC_RGBA8, // Use the EXT2 EAC algorithm on 4 components.
use memory::Pod;

macro_rules! impl_channel_type {
    { $($name:ident = $shader_type:ident [ $($imp_trait:ident),* ] ,)* } => {
        /// Type of a surface channel. This is how we interpret the
        /// storage allocated with `SurfaceType`.
        #[allow(missing_docs)]
        #[repr(u8)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
        pub enum ChannelType {
            $( $name, )*
        }
        $(
            #[allow(missing_docs)]
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
            pub enum $name {}
            impl ChannelTyped for $name {
                type ShaderType = $shader_type;
                fn get_channel_type() -> ChannelType {
                    ChannelType::$name
                }
            }
            $(
                impl $imp_trait for $name {}
            )*
        )*
    }
}

impl_channel_type! {
    Int     = i32 [TextureChannel, RenderChannel],
    Uint    = u32 [TextureChannel, RenderChannel],
    Inorm   = f32 [TextureChannel, RenderChannel, BlendChannel],
    Unorm   = f32 [TextureChannel, RenderChannel, BlendChannel],
    Float   = f32 [TextureChannel, RenderChannel, BlendChannel],
    Srgb    = f32 [TextureChannel, RenderChannel, BlendChannel],
}

macro_rules! impl_formats {
    { $($name:ident : $container:ident < $($channel:ident),* > = $data_type:ty {$alpha_bits:expr} [ $($imp_trait:ident),* ] ,)* } => {
        /// Type of the allocated texture surface. It is supposed to only
        /// carry information about the number of bits per each channel.
        /// The actual types are up to the views to decide and interpret.
        /// The actual components are up to the swizzle to define.
        #[repr(u8)]
        #[allow(missing_docs, non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
        pub enum SurfaceType {
            $( $name, )*
        }
        impl SurfaceType {
            /// Return the total number of bits for this format.
            pub fn get_total_bits(&self) -> u8 {
                use std::mem::size_of;
                match *self {
                    $( SurfaceType::$name => (size_of::<$data_type>() * 8) as u8, )*
                }
            }
            /// Return the number of bits allocated for alpha and stencil.
            pub fn get_alpha_stencil_bits(&self) -> u8  {
                match *self {
                    $( SurfaceType::$name => $alpha_bits, )*
                }
            }
        }
        $(
            #[allow(missing_docs, non_camel_case_types)]
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
            pub enum $name {}
            impl SurfaceTyped for $name {
                type DataType = $data_type;
                fn get_surface_type() -> SurfaceType {
                    SurfaceType::$name
                }
            }
            $(
                impl $imp_trait for $name {}
            )*
            $(
                impl Formatted for ($name, $channel) {
                    type Surface = $name;
                    type Channel = $channel;
                    type View = $container< <$channel as ChannelTyped>::ShaderType >;
                }
            )*
        )*
    }
}


impl_formats! {
    R4_G4           : Vec2<Unorm> = u8 {0}  [TextureSurface, RenderSurface],
    R4_G4_B4_A4     : Vec4<Unorm> = u16 {4} [TextureSurface, RenderSurface],
    R5_G5_B5_A1     : Vec4<Unorm> = u16 {1} [TextureSurface, RenderSurface],
    R5_G6_B5        : Vec3<Unorm> = u16 {0} [TextureSurface, RenderSurface],
    R8              : Vec1<Int, Uint, Inorm, Unorm> = u8 {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R8_G8           : Vec2<Int, Uint, Inorm, Unorm> = [u8; 2] {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R8_G8_B8_A8     : Vec4<Int, Uint, Inorm, Unorm, Srgb> = [u8; 4] {8}
        [BufferSurface, TextureSurface, RenderSurface],
    R10_G10_B10_A2  : Vec4<Uint, Unorm> = u32 {2}
        [BufferSurface, TextureSurface, RenderSurface],
    R11_G11_B10     : Vec4<Unorm, Float> = u32 {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R16             : Vec1<Int, Uint, Inorm, Unorm, Float> = u16 {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R16_G16         : Vec2<Int, Uint, Inorm, Unorm, Float> = [u16; 2] {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R16_G16_B16     : Vec3<Int, Uint, Inorm, Unorm, Float> = [u16; 3] {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R16_G16_B16_A16 : Vec4<Int, Uint, Inorm, Unorm, Float> = [u16; 4] {16}
        [BufferSurface, TextureSurface, RenderSurface],
    R32             : Vec1<Int, Uint, Float> = u32 {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R32_G32         : Vec2<Int, Uint, Float> = [u32; 2] {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R32_G32_B32     : Vec3<Int, Uint, Float> = [u32; 3] {0}
        [BufferSurface, TextureSurface, RenderSurface],
    R32_G32_B32_A32 : Vec4<Int, Uint, Float> = [u32; 4] {32}
        [BufferSurface, TextureSurface, RenderSurface],
    B8_G8_R8_A8     : Vec4<Unorm, Srgb> = [u8; 4] {32}
        [BufferSurface, TextureSurface, RenderSurface],
    D16             : Vec1<Unorm> = F16 {0} [TextureSurface, DepthSurface],
    D24             : Vec1<Unorm> = f32 {8} [TextureSurface, DepthSurface], //hacky stencil bits
    D24_S8          : Vec1<Unorm, Uint> = u32 {8} [TextureSurface, DepthSurface, StencilSurface],
    D32             : Vec1<Float> = f32 {0} [TextureSurface, DepthSurface],
    D32_S8          : Vec1<Unorm, Float, Uint> = (f32, u32) {32} [TextureSurface, DepthSurface, StencilSurface],
}


/// Source channel in a swizzle configuration. Some may redirect onto
/// different physical channels, some may be hardcoded to 0 or 1.
#[allow(missing_docs)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct Swizzle(pub ChannelSource, pub ChannelSource, pub ChannelSource, pub ChannelSource);

impl Swizzle {
    /// Create a new swizzle where each channel is unmapped.
    pub fn new() -> Swizzle {
        Swizzle(ChannelSource::X, ChannelSource::Y, ChannelSource::Z, ChannelSource::W)
    }
}

/// Complete run-time surface format.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct Format(pub SurfaceType, pub ChannelType);

/// Compile-time surface type trait.
pub trait SurfaceTyped {
    /// The corresponding data type to be passed from CPU.
    type DataType: Pod;
    /// Return the run-time value of the type.
    fn get_surface_type() -> SurfaceType;
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
    /// For example, normalized integers are visible as floats.
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

impl<F> BufferFormat for F where
    F: Formatted,
    F::Surface: BufferSurface,
    F::Channel: ChannelTyped,
{}
impl<F> DepthFormat for F where
    F: Formatted,
    F::Surface: DepthSurface,
    F::Channel: RenderChannel,
{}
impl<F> StencilFormat for F where
    F: Formatted,
    F::Surface: StencilSurface,
    F::Channel: RenderChannel,
{}
impl<F> DepthStencilFormat for F where
    F: DepthFormat + StencilFormat
{}
impl<F> TextureFormat for F where
    F: Formatted,
    F::Surface: TextureSurface,
    F::Channel: TextureChannel,
{}
impl<F> RenderFormat for F where
    F: Formatted,
    F::Surface: RenderSurface,
    F::Channel: RenderChannel,
{}
impl<F> BlendFormat for F where
    F: Formatted,
    F::Surface: RenderSurface,
    F::Channel: BlendChannel,
{}

macro_rules! alias {
    { $( $name:ident = $ty:ty, )* } => {
        $(
            #[allow(missing_docs)]
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
            pub struct $name(pub $ty);
            impl From<$ty> for $name {
                fn from(v: $ty) -> $name {
                    $name(v)
                }
            }

            unsafe impl Pod for $name {}

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

/// Standard 8bits RGBA format.
pub type Rgba8 = (R8_G8_B8_A8, Unorm);
/// Standard 8bit gamma transforming RGB format.
pub type Srgba8 = (R8_G8_B8_A8, Srgb);
/// Standard HDR floating-point format with 10 bits for RGB components
/// and 2 bits for the alpha.
pub type Rgb10a2F = (R10_G10_B10_A2, Float);
/// Standard 16-bit floating-point RGBA format.
pub type Rgba16F = (R16_G16_B16_A16, Float);
/// Standard 32-bit floating-point RGBA format.
pub type Rgba32F = (R32_G32_B32_A32, Float);
/// Standard 8bits BGRA format.
pub type Bgra8 = (B8_G8_R8_A8, Unorm);
/// Standard 24-bit depth format.
pub type Depth = (D24, Unorm);
/// Standard 24-bit depth format with 8-bit stencil.
pub type DepthStencil = (D24_S8, Unorm);
/// Standard 32-bit floating-point depth format.
pub type Depth32F = (D32, Float);

macro_rules! impl_simple_formats {
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
        impl_simple_formats! {$(
            Vec1<$ty> = $channel R8,
            Vec2<$ty> = $channel R8_G8,
            Vec4<$ty> = $channel R8_G8_B8_A8,
        )*}
    }
}

macro_rules! impl_formats_16bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_simple_formats! {$(
            Vec1<$ty> = $channel R16,
            Vec2<$ty> = $channel R16_G16,
            Vec3<$ty> = $channel R16_G16_B16,
            Vec4<$ty> = $channel R16_G16_B16_A16,
        )*}
    }
}

macro_rules! impl_formats_32bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_simple_formats! {$(
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
    U8Norm = Unorm,
    I8Norm = Inorm,
}

impl_formats_16bit! {
    u16 = Uint,
    i16 = Int,
    U16Norm = Unorm,
    I16Norm = Inorm,
    F16 = Float,
}

impl_formats_32bit! {
    u32 = Uint,
    i32 = Int,
    f32 = Float,
}
