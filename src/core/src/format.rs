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

#![allow(missing_docs)]

macro_rules! impl_surface_type {
    { $($name:ident [$bits:expr] $(=$tr:ty)* ,)* } => {
        #[repr(u8)]
        #[allow(non_camel_case_types)]
        #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
        pub enum SurfaceType {
            $( $name, )*
        }
        impl SurfaceType {
            pub fn get_bit_size(&self) -> u8 {
                match *self {
                    $( SurfaceType::$name => $bits, )*
                }
            }
        }
        $(
            #[allow(non_camel_case_types)]
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
    { $($name:ident $(=$tr:ident)* ,)* } => {
        #[repr(u8)]
        #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
        pub enum ChannelType {
            $( $name, )*
        }
        $(
            #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
            pub enum $name {}
            impl ChannelTyped for $name {
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
    Int             = TextureChannel = RenderChannel,
    Uint            = TextureChannel = RenderChannel,
    IntScaled       = TextureChannel,
    UintScaled      = TextureChannel,
    IntNormalized   = TextureChannel = RenderChannel = BlendChannel,
    UintNormalized  = TextureChannel = RenderChannel = BlendChannel,
    Float           = TextureChannel = RenderChannel = BlendChannel,
    Srgb            = TextureChannel = RenderChannel = BlendChannel,
}

#[repr(u8)]
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Channel {
    Zero,
    One,
    X,
    Y,
    Z,
    W,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct View {
    pub ty: ChannelType,
    pub x: Channel,
    pub y: Channel,
    pub z: Channel,
    pub w: Channel,
}

impl From<ChannelType> for View {
    fn from(ty: ChannelType) -> View {
        View {
            ty: ty,
            x: Channel::X,
            y: Channel::Y,
            z: Channel::Z,
            w: Channel::W,
        }
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Format(pub SurfaceType, pub View);

// compile-time specification

pub trait SurfaceTyped {
    fn get_surface_type() -> SurfaceType;
    fn get_bit_size() -> u8;
}
pub trait BufferSurface: SurfaceTyped {}
pub trait TextureSurface: SurfaceTyped {}
pub trait RenderSurface: SurfaceTyped {}
pub trait DepthSurface: SurfaceTyped {}
pub trait StencilSurface: SurfaceTyped {}

pub trait ChannelTyped {
    fn get_channel_type() -> ChannelType;
}
pub trait TextureChannel: ChannelTyped {}
pub trait RenderChannel: ChannelTyped {}
pub trait BlendChannel: RenderChannel {}

pub trait Formatted {
    type Surface: SurfaceTyped;
    type Channel: ChannelTyped;
    fn get_format() -> Format {
        Format(Self::Surface::get_surface_type(),
            Self::Channel::get_channel_type().into())
    }
}
pub trait BufferFormat: Formatted {}
pub trait DepthFormat: Formatted {}
pub trait StencilFormat: Formatted {}
pub trait DepthStencilFormat: DepthFormat + StencilFormat {}
pub trait TextureFormat: Formatted {}
pub trait RenderFormat: Formatted {}
pub trait BlendFormat: RenderFormat {}

impl<S: SurfaceTyped, C: ChannelTyped> Formatted for (S, C) {
    type Surface = S;
    type Channel = C;
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
            #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
            pub struct $name(pub $ty);
            impl From<$ty> for $name {
                fn from(v: $ty) -> $name {
                    $name(v)
                }
            }
            impl $name {
                pub fn cast2(v: [$ty; 2]) -> [$name; 2] {
                    [$name(v[0]), $name(v[1])]
                }
                pub fn cast3(v: [$ty; 3]) -> [$name; 3] {
                    [$name(v[0]), $name(v[1]), $name(v[2])]
                }
                pub fn cast4(v: [$ty; 4]) -> [$name; 4] {
                    [$name(v[0]), $name(v[1]), $name(v[2]), $name(v[3])]
                }
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

macro_rules! impl_format {
    { $( $ty:ty = $surface:ident . $channel:ident ,)* } => {
        $(
            impl Formatted for $ty {
                type Surface = $surface;
                type Channel = $channel;
            }
        )*
    }
}

macro_rules! impl_formats_8bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            $ty = R8 . $channel,
            [$ty; 2] = R8_G8 . $channel,
            [$ty; 3] = R8_G8_B8 . $channel,
            [$ty; 4] = R8_G8_B8_A8 . $channel,
        )*}
    }
}

macro_rules! impl_formats_16bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            $ty = R16 . $channel,
            [$ty; 2] = R16_G16 . $channel,
            [$ty; 3] = R16_G16_B16 . $channel,
            [$ty; 4] = R16_G16_B16_A16 . $channel,
        )*}
    }
}

macro_rules! impl_formats_32bit {
    { $( $ty:ty = $channel:ident, )* } => {
        impl_format! {$(
            $ty = R32 . $channel,
            [$ty; 2] = R32_G32 . $channel,
            [$ty; 3] = R32_G32_B32 . $channel,
            [$ty; 4] = R32_G32_B32_A32 . $channel,
        )*}
    }
}

impl_formats_8bit! {
    u8 = Uint,
    i8 = Int,
    U8Norm = UintNormalized,
    I8Norm = UintNormalized,
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

// Some handy pre-defined formats

pub type Rgba8 = [U8Norm; 4]; //(R8_G8_B8_A8, UintNormalized);
pub type Rgb10a2F = (R10_G10_B10_A2, Float);
pub type Rgba16F = [F16; 4]; //(R16_G16_B16_A16, Float);
pub type Rgba32F = [f32; 4]; //(R32_G32_B32_A32, Float);
pub type Depth = (D24, UintNormalized);
pub type DepthStencil = (D24_S8, UintNormalized);
pub type Depth32F = (D32, Float);
