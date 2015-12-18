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
    { $($name:ident $(=$tr:ident)* ,)* } => {
        #[derive(Copy, Clone)]
        #[allow(non_camel_case_types)]
        #[repr(u8)]
        pub enum SurfaceType {
            $( $name, )*
        }
        $(
            #[allow(non_camel_case_types)]
            pub enum $name {}
            impl SurfaceTyped for $name {
                fn get_surface_type() -> SurfaceType {
                    SurfaceType::$name
                }
            }
            $(
                impl $tr for $name {}
            )*
        )*
    }
}

impl_surface_type! {
    R8_G8_B8_A8     = BufferSurface = TextureSurface = RenderSurface,
    R16_G16_B16_A16 = BufferSurface = TextureSurface = RenderSurface,
    R32_G32_B32_A32 = BufferSurface = TextureSurface = RenderSurface,
    D24_S8          = TextureSurface = DepthStencilSurface,
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
pub struct Swizzle(pub Channel, pub Channel, pub Channel, pub Channel);

pub const NO_SWIZZLE: Swizzle = Swizzle(Channel::X, Channel::Y, Channel::Z, Channel::W);

#[derive(Copy, Clone)]
pub struct RuntimeFormat(pub SurfaceType, pub ChannelType, pub Swizzle);


// compile-time specification

pub trait SurfaceTyped {
    fn get_surface_type() -> SurfaceType;
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
    fn get_format() -> RuntimeFormat;
}
pub trait BufferFormat: Formatted {}
pub trait DepthStencilFormat: Formatted {}
pub trait TextureFormat: Formatted {}
pub trait RenderFormat: Formatted {}
pub trait BlendFormat: RenderFormat {}

impl ChannelTyped for () {
    fn get_channel_type() -> ChannelType {
        ChannelType::Float
    }
}

pub struct CompileFormat<S, C>(PhantomData<(S,C)>);

impl<S: SurfaceTyped, C: ChannelTyped> Formatted for CompileFormat<S, C> {
    fn get_format() -> RuntimeFormat {
        RuntimeFormat(S::get_surface_type(), C::get_channel_type(), NO_SWIZZLE)
    }
}

impl<S: BufferSurface> BufferFormat                         for CompileFormat<S, ()> {}
impl<S: DepthStencilSurface> DepthStencilFormat             for CompileFormat<S, ()> {}
impl<S: TextureSurface, C: TextureChannel> TextureFormat    for CompileFormat<S, C> {}
impl<S: RenderSurface, C: RenderChannel> RenderFormat       for CompileFormat<S, C> {}
impl<S: RenderSurface, C: BlendChannel> BlendFormat         for CompileFormat<S, C> {}

pub type Rgba8 = CompileFormat<R8_G8_B8_A8, UintNormalized>;
pub type Rgba16F = CompileFormat<R16_G16_B16_A16, Float>;
pub type Rgba32F = CompileFormat<R32_G32_B32_A32, Float>;
pub type DepthStencil = CompileFormat<D24_S8, UintNormalized>;
