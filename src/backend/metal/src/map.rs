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

use metal::*;

use gfx_core::factory::{Bind, MapAccess, Usage};
use gfx_core::format::{Format, SurfaceType};
use gfx_core::state::Comparison;
use gfx_core::tex::{AaMode, FilterMethod, WrapMode, DepthStencilFlags};

pub enum FormatUsage {
    Sample,
    Write,
    Render,
    Msaa,
    Resolve,
    Blend
}

pub fn map_function(fun: Comparison) -> MTLCompareFunction {
    use metal::MTLCompareFunction::*;

    match fun {
        Comparison::Never => Never,
        Comparison::Less => Less,
        Comparison::LessEqual => LessEqual,
        Comparison::Equal => Equal,
        Comparison::GreaterEqual => GreaterEqual,
        Comparison::Greater => Greater,
        Comparison::NotEqual => NotEqual,
        Comparison::Always => Always,
    }
}

pub fn map_format(format: Format, is_target: bool) -> Option<MTLPixelFormat> {
    use gfx_core::format::SurfaceType::*;
    use gfx_core::format::ChannelType::*;

    use metal::MTLPixelFormat::*;
    
    Some(match format.0 {
        R4_G4 | R4_G4_B4_A4 | R5_G5_B5_A1 | R5_G6_B5 => return None,
        R8 => match format.1 {
            Int   => R8Sint,
            Uint  => R8Uint,
            Inorm => R8Snorm,
            Unorm => R8Unorm,
            _ => return None,
        },
        R8_G8 => match format.1 {
            Int   => RG8Sint,
            Uint  => RG8Uint,
            Inorm => RG8Snorm,
            Unorm => RG8Unorm,
            _ => return None,
        },
        R8_G8_B8_A8 => match format.1 {
            Int   => RGBA8Sint,
            Uint  => RGBA8Uint,
            Inorm => RGBA8Snorm,
            Unorm => RGBA8Unorm,
            Srgb  => RGBA8Unorm_sRGB,
            _ => return None,
        },
        R10_G10_B10_A2 => match format.1 {
            Uint  => RGB10A2Uint,
            Unorm => RGB10A2Unorm,
            _ => return None,
        },
        R11_G11_B10 => match format.1 {
            Float => RG11B10Float,
            _ => return None,
        },
        R16 => match format.1 {
            Int   => R16Sint,
            Uint  => R16Uint,
            Inorm => R16Snorm,
            Unorm => R16Unorm,
            Float => R16Float,
            _ => return None,
        },
        R16_G16 => match format.1 {
            Int   => RG16Sint,
            Uint  => RG16Uint,
            Inorm => RG16Snorm,
            Unorm => RG16Unorm,
            Float => RG16Float,
            _ => return None,
        },
        R16_G16_B16 => return None,
        R16_G16_B16_A16 => match format.1 {
            Int   => RGBA16Sint,
            Uint  => RGBA16Uint,
            Inorm => RGBA16Snorm,
            Unorm => RGBA16Unorm,
            Float => RGBA16Float,
            _ => return None,
        },
        R32 => match format.1 {
            Int   => R32Sint,
            Uint  => R32Uint,
            Float => R32Float,
            _ => return None,
        },
        R32_G32 => match format.1 {
            Int   => RG32Sint,
            Uint  => RG32Uint,
            Float => RG32Float,
            _ => return None,
        },
        R32_G32_B32 => return None,
        R32_G32_B32_A32 => match format.1 {
            Int   => RGBA32Sint,
            Uint  => RGBA32Uint,
            Float => RGBA32Float,
            _ => return None,
        },
        D16 => return None,
        D24 => match (is_target, format.1) {
            // TODO: stencil?
            (true, _)      => Depth24Unorm_Stencil8,
            (false, Unorm) => Depth24Unorm_Stencil8,
            _ => return None,
        },
        D24_S8 => match (is_target, format.1) {
            (true, _)      => return None,
            (false, Unorm) => Depth24Unorm_Stencil8,
            (false, Uint)  => return None,
            _ => return None,
        },
        D32 => match (is_target, format.1) {
            (true, _)      => Depth32Float,
            (false, Float) => Depth32Float,
            _ => return None,
        },
    })
}

pub fn format_supports_usage(feature_set: MTLFeatureSet, format: MTLPixelFormat, usage: FormatUsage) -> bool {
    use metal::MTLPixelFormat::*;
    use metal::MTLFeatureSet::*;    
    use FormatUsage::*;

    match format {
        A8Unorm => {
            match usage {
                Sample => true,
                _ => false
            }
        },
        R8Unorm => true,
        R8Unorm_sRGB => {
            match feature_set {
                iOS_GPUFamily1_v1 |
                iOS_GPUFamily1_v2 => {
                    match usage {
                        Sample |
                        Render |
                        Msaa |
                        Resolve |
                        Blend => true,
                        _ => false
                    }
                }
                iOS_GPUFamily2_v1 |
                iOS_GPUFamily2_v2 |
                iOS_GPUFamily3_v1 => true,
                OSX_GPUFamily1_v1 => false
            }
        }
    }
    /*match feature_set {
        iOS_GPUFamily1_v1 => {
            
        },
        iOS_GPUFamily2_v1 => {
            
        },
        iOS_GPUFamily1_v2 => {

        },
        iOS_GPUFamily2_v2 => {
            
        },
        iOS_GPUFamily3_v1 => {
            
        },
        OSX_GPUFamily1_v1 => {
            
        },
    }*/
}

pub fn map_surface(surface: SurfaceType) -> Option<MTLPixelFormat> {
    // TODO: handle surface types in metal, look at gl impl.

    None
}

pub fn map_wrap(wrap: WrapMode) -> MTLSamplerAddressMode {
    use metal::MTLSamplerAddressMode::*;

    match wrap {
        WrapMode::Tile   => Repeat,
        WrapMode::Mirror => MirrorRepeat, // TODO: MirrorClampToEdge?
        WrapMode::Clamp  => ClampToEdge, // TODO: MirrorClampToEdge, ClampToZero?
        WrapMode::Border => ClampToZero, // TODO: what border?
    }
}

