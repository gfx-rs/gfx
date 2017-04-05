// Copyright 2017 The Gfx-rs Developers.
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

use ash::vk;
use core::format::{SurfaceType, ChannelType};

pub fn map_format(surface: SurfaceType, chan: ChannelType) -> Option<vk::Format> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
    Some(match surface {
        R4_G4 => match chan {
            Unorm => vk::Format::R4g4UnormPack8,
            _ => return None,
        },
        R4_G4_B4_A4 => match chan {
            Unorm => vk::Format::R4g4b4a4UnormPack16,
            _ => return None,
        },
        R5_G5_B5_A1 => match chan {
            Unorm => vk::Format::R5g5b5a1UnormPack16,
             _ => return None,
        },
        R5_G6_B5 => match chan {
            Unorm => vk::Format::R5g6b5UnormPack16,
             _ => return None,
        },
        R8 => match chan {
            Int   => vk::Format::R8Sint,
            Uint  => vk::Format::R8Uint,
            Inorm => vk::Format::R8Snorm,
            Unorm => vk::Format::R8Unorm,
            Srgb  => vk::Format::R8Srgb,
            _ => return None,
        },
        R8_G8 => match chan {
            Int   => vk::Format::R8g8Sint,
            Uint  => vk::Format::R8g8Uint,
            Inorm => vk::Format::R8g8Snorm,
            Unorm => vk::Format::R8g8Unorm,
            Srgb  => vk::Format::R8g8Srgb,
            _ => return None,
        },
        R8_G8_B8_A8 => match chan {
            Int   => vk::Format::R8g8b8a8Sint,
            Uint  => vk::Format::R8g8b8a8Uint,
            Inorm => vk::Format::R8g8b8a8Snorm,
            Unorm => vk::Format::R8g8b8a8Unorm,
            Srgb  => vk::Format::R8g8b8a8Srgb,
            _ => return None,
        },
        R10_G10_B10_A2 => match chan {
            Int   => vk::Format::A2r10g10b10SintPack32,
            Uint  => vk::Format::A2r10g10b10UintPack32,
            Inorm => vk::Format::A2r10g10b10SnormPack32,
            Unorm => vk::Format::A2r10g10b10UnormPack32,
            _ => return None,
        },
        R11_G11_B10 => match chan {
            Float => vk::Format::B10g11r11UfloatPack32,
            _ => return None,
        },
        R16 => match chan {
            Int   => vk::Format::R16Sint,
            Uint  => vk::Format::R16Uint,
            Inorm => vk::Format::R16Snorm,
            Unorm => vk::Format::R16Unorm,
            Float => vk::Format::R16Sfloat,
            _ => return None,
        },
        R16_G16 => match chan {
            Int   => vk::Format::R16g16Sint,
            Uint  => vk::Format::R16g16Uint,
            Inorm => vk::Format::R16g16Snorm,
            Unorm => vk::Format::R16g16Unorm,
            Float => vk::Format::R16g16Sfloat,
            _ => return None,
        },
        R16_G16_B16 => match chan {
            Int   => vk::Format::R16g16b16Sint,
            Uint  => vk::Format::R16g16b16Uint,
            Inorm => vk::Format::R16g16b16Snorm,
            Unorm => vk::Format::R16g16b16Unorm,
            Float => vk::Format::R16g16b16Sfloat,
            _ => return None,
        },
        R16_G16_B16_A16 => match chan {
            Int   => vk::Format::R16g16b16a16Sint,
            Uint  => vk::Format::R16g16b16a16Uint,
            Inorm => vk::Format::R16g16b16a16Snorm,
            Unorm => vk::Format::R16g16b16a16Unorm,
            Float => vk::Format::R16g16b16a16Sfloat,
            _ => return None,
        },
        R32 => match chan {
            Int   => vk::Format::R32Sint,
            Uint  => vk::Format::R32Uint,
            Float => vk::Format::R32Sfloat,
            _ => return None,
        },
        R32_G32 => match chan {
            Int   => vk::Format::R32g32Sint,
            Uint  => vk::Format::R32g32Uint,
            Float => vk::Format::R32g32Sfloat,
            _ => return None,
        },
        R32_G32_B32 => match chan {
            Int   => vk::Format::R32g32b32Sint,
            Uint  => vk::Format::R32g32b32Uint,
            Float => vk::Format::R32g32b32Sfloat,
            _ => return None,
        },
        R32_G32_B32_A32 => match chan {
            Int   => vk::Format::R32g32b32a32Sint,
            Uint  => vk::Format::R32g32b32a32Uint,
            Float => vk::Format::R32g32b32a32Sfloat,
            _ => return None,
        },
        B8_G8_R8_A8 => match chan {
            Unorm => vk::Format::B8g8r8a8Unorm,
            _ => return None,
        },
        D16 => match chan {
            Unorm  => vk::Format::D16Unorm,
            _ => return None,
        },
        D24 => match chan {
            Unorm => vk::Format::X8D24UnormPack32,
            _ => return None,
        },
        D24_S8 => match chan {
            Unorm => vk::Format::D24UnormS8Uint,
            _ => return None,
        },
        D32 => match chan {
            Float => vk::Format::D32Sfloat,
            _ => return None,
        },
    })
}

