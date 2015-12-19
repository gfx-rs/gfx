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

use std::marker::PhantomData;

macro_rules! impl_surface_type {
    { $($name:ident [$bits:expr] $(=$tr:ty)* ,)* } => {
        #[repr(u8)]
        #[allow(non_camel_case_types)]
        #[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
        pub enum SurfaceType {
            $( $name, )*
        }
        impl SurfaceType {
            fn get_bit_size(&self) -> u8 {
                match *self {
                    $( SurfaceType::$name => $bits, )*
                }
            }
        }
        $(
            #[allow(non_camel_case_types)]
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
    R8_G8_B8_A8     [32]  = BufferSurface = TextureSurface = RenderSurface,
    R10_G10_B10_A2  [32]  = BufferSurface = TextureSurface = RenderSurface,
    R16_G16_B16_A16 [64]  = BufferSurface = TextureSurface = RenderSurface,
    R32_G32_B32_A32 [128] = BufferSurface = TextureSurface = RenderSurface,
    D24_S8          [32]  = TextureSurface = DepthStencilSurface,
}

macro_rules! impl_channel_type {
    { $($name:ident $(=$tr:ident)* ,)* } => {
        #[derive(Copy, Clone)]
        #[repr(u8)]
        pub enum ChannelType {
            $( $name, )*
        }
        $(
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

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Channel {
    Zero,
    One,
    X,
    Y,
    Z,
    W,
}

#[derive(Copy, Clone)]
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

// compile-time specification

pub trait SurfaceTyped {
    fn get_surface_type() -> SurfaceType;
    fn get_bit_size() -> u8;
}
pub trait BufferSurface: SurfaceTyped {}
pub trait TextureSurface: SurfaceTyped {}
pub trait RenderSurface: SurfaceTyped {}
pub trait DepthStencilSurface: SurfaceTyped {}

pub trait ChannelTyped {
    fn get_channel_type() -> ChannelType;
}
pub trait TextureChannel: ChannelTyped {}
pub trait RenderChannel: ChannelTyped {}
pub trait BlendChannel: RenderChannel {}

pub trait Formatted {
    fn get_format() -> (SurfaceType, View);
}
pub trait BufferFormat: Formatted {}
pub trait DepthStencilFormat: Formatted {}
pub trait TextureFormat: Formatted {}
pub trait RenderFormat: Formatted {}
pub trait BlendFormat: RenderFormat {}

impl<S: SurfaceTyped, C: ChannelTyped> Formatted for (S, C) {
    fn get_format() -> (SurfaceType, View) {
        (S::get_surface_type(), C::get_channel_type().into())
    }
}
impl<S: BufferSurface, C: ChannelTyped> BufferFormat for (S, C) {}
impl<S: DepthStencilSurface, C: RenderChannel> DepthStencilFormat for (S, C) {}
impl<S: TextureSurface, C: TextureChannel> TextureFormat for (S, C) {}
impl<S: RenderSurface, C: RenderChannel> RenderFormat for (S, C) {}
impl<S: RenderSurface, C: BlendChannel> BlendFormat for (S, C) {}

pub type Rgba8 = (R8_G8_B8_A8, UintNormalized);
pub type Rgb10a2F = (R10_G10_B10_A2, Float);
pub type Rgba16F = (R16_G16_B16_A16, Float);
pub type Rgba32F = (R32_G32_B32_A32, Float);
pub type DepthStencil = (D24_S8, UintNormalized);
