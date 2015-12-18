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
    { $($name:ident,)* } => {
        #[derive(Copy, Clone)]
        #[allow(non_camel_case_types)]
        #[repr(u8)]
        pub enum SurfaceType {
            $( $name, )*
        }
        $(
            pub enum $name {}
            impl SurfaceTyped for $name {
                fn get_surface_type() -> SurfaceType {
                    SurfaceType::$name
                }
            }
        )*
    }
}

impl_surface_type! {
    R8_G8_B8_A8,
    R16_G16_B16_A16,
    D24_S8,
}

macro_rules! impl_channel_type {
    { $($name:ident,)* } => {
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
        )*
    }
}

impl_channel_type! {
    Int,
    Uint,
    IntScaled,
    UintScaled,
    IntNormalized,
    UintNormalized,
    Float,
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

pub trait ChannelTyped {
    fn get_channel_type() -> ChannelType;
}

pub trait Formatted {
    fn get_format() -> RuntimeFormat;
}


pub struct CompileFormat<S, C>(PhantomData<(S,C)>);

impl<S: SurfaceTyped, C: ChannelTyped> Formatted for CompileFormat<S, C> {
    fn get_format() -> RuntimeFormat {
        RuntimeFormat(S::get_surface_type(), C::get_channel_type(), NO_SWIZZLE)
    }
}

pub type Rgba8 = CompileFormat<R8_G8_B8_A8, UintNormalized>;
pub type Rgba16F = CompileFormat<R16_G16_B16_A16, Float>;
pub type DepthStencil = CompileFormat<D24_S8, UintNormalized>;
