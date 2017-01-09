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

use winapi::*;
use core::memory::{self, Bind, Usage};
use core::format::{Format, SurfaceType};
use core::state::Comparison;
use core::texture::{AaMode, FilterMethod, WrapMode, DepthStencilFlags};


pub fn map_function(fun: Comparison) -> D3D11_COMPARISON_FUNC {
    match fun {
        Comparison::Never => D3D11_COMPARISON_NEVER,
        Comparison::Less => D3D11_COMPARISON_LESS,
        Comparison::LessEqual => D3D11_COMPARISON_LESS_EQUAL,
        Comparison::Equal => D3D11_COMPARISON_EQUAL,
        Comparison::GreaterEqual => D3D11_COMPARISON_GREATER_EQUAL,
        Comparison::Greater => D3D11_COMPARISON_GREATER,
        Comparison::NotEqual => D3D11_COMPARISON_NOT_EQUAL,
        Comparison::Always => D3D11_COMPARISON_ALWAYS,
    }
}

pub fn map_format(format: Format, is_target: bool) -> Option<DXGI_FORMAT> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
    Some(match format.0 {
        R4_G4 | R4_G4_B4_A4 | R5_G5_B5_A1 | R5_G6_B5 => return None,
        R8 => match format.1 {
            Int   => DXGI_FORMAT_R8_SINT,
            Uint  => DXGI_FORMAT_R8_UINT,
            Inorm => DXGI_FORMAT_R8_SNORM,
            Unorm => DXGI_FORMAT_R8_UNORM,
            _ => return None,
        },
        R8_G8 => match format.1 {
            Int   => DXGI_FORMAT_R8G8_SINT,
            Uint  => DXGI_FORMAT_R8G8_UINT,
            Inorm => DXGI_FORMAT_R8G8_SNORM,
            Unorm => DXGI_FORMAT_R8G8_UNORM,
            _ => return None,
        },
        R8_G8_B8_A8 => match format.1 {
            Int   => DXGI_FORMAT_R8G8B8A8_SINT,
            Uint  => DXGI_FORMAT_R8G8B8A8_UINT,
            Inorm => DXGI_FORMAT_R8G8B8A8_SNORM,
            Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
            Srgb  => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
            _ => return None,
        },
        R10_G10_B10_A2 => match format.1 {
            Uint  => DXGI_FORMAT_R10G10B10A2_UINT,
            Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
            _ => return None,
        },
        R11_G11_B10 => match format.1 {
            Float => DXGI_FORMAT_R11G11B10_FLOAT,
            _ => return None,
        },
        R16 => match format.1 {
            Int   => DXGI_FORMAT_R16_SINT,
            Uint  => DXGI_FORMAT_R16_UINT,
            Inorm => DXGI_FORMAT_R16_SNORM,
            Unorm => DXGI_FORMAT_R16_UNORM,
            Float => DXGI_FORMAT_R16_FLOAT,
            _ => return None,
        },
        R16_G16 => match format.1 {
            Int   => DXGI_FORMAT_R16G16_SINT,
            Uint  => DXGI_FORMAT_R16G16_UINT,
            Inorm => DXGI_FORMAT_R16G16_SNORM,
            Unorm => DXGI_FORMAT_R16G16_UNORM,
            Float => DXGI_FORMAT_R16G16_FLOAT,
            _ => return None,
        },
        R16_G16_B16 => return None,
        R16_G16_B16_A16 => match format.1 {
            Int   => DXGI_FORMAT_R16G16B16A16_SINT,
            Uint  => DXGI_FORMAT_R16G16B16A16_UINT,
            Inorm => DXGI_FORMAT_R16G16B16A16_SNORM,
            Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
            Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
            _ => return None,
        },
        R32 => match format.1 {
            Int   => DXGI_FORMAT_R32_SINT,
            Uint  => DXGI_FORMAT_R32_UINT,
            Float => DXGI_FORMAT_R32_FLOAT,
            _ => return None,
        },
        R32_G32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32_SINT,
            Uint  => DXGI_FORMAT_R32G32_UINT,
            Float => DXGI_FORMAT_R32G32_FLOAT,
            _ => return None,
        },
        R32_G32_B32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32B32_SINT,
            Uint  => DXGI_FORMAT_R32G32B32_UINT,
            Float => DXGI_FORMAT_R32G32B32_FLOAT,
            _ => return None,
        },
        R32_G32_B32_A32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32B32A32_SINT,
            Uint  => DXGI_FORMAT_R32G32B32A32_UINT,
            Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
            _ => return None,
        },
        B8_G8_R8_A8 => match format.1 {
            Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
            _ => return None,
        },
        D16 => match (is_target, format.1) {
            (true, _)      => DXGI_FORMAT_D16_UNORM,
            (false, Unorm) => DXGI_FORMAT_R16_UNORM,
            _ => return None,
        },
        D24 => match (is_target, format.1) {
            (true, _)      => DXGI_FORMAT_D24_UNORM_S8_UINT,
            (false, Unorm) => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
            _ => return None,
        },
        D24_S8 => match (is_target, format.1) {
            (true, _)      => DXGI_FORMAT_D24_UNORM_S8_UINT,
            (false, Unorm) => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
            (false, Uint)  => DXGI_FORMAT_X24_TYPELESS_G8_UINT,
            _ => return None,
        },
        D32 => match (is_target, format.1) {
            (true, _)      => DXGI_FORMAT_D32_FLOAT,
            (false, Float) => DXGI_FORMAT_R32_FLOAT,
            _ => return None,
        },
    })
}

pub fn map_surface(surface: SurfaceType) -> Option<DXGI_FORMAT> {
    use core::format::SurfaceType::*;
    Some(match surface {
        R4_G4 | R4_G4_B4_A4 | R5_G5_B5_A1 | R5_G6_B5 => return None,
        R16_G16_B16 => return None,
        R8              => DXGI_FORMAT_R8_TYPELESS,
        R8_G8           => DXGI_FORMAT_R8G8_TYPELESS,
        R8_G8_B8_A8     => DXGI_FORMAT_R8G8B8A8_TYPELESS,
        R10_G10_B10_A2  => DXGI_FORMAT_R10G10B10A2_TYPELESS,
        R11_G11_B10     => DXGI_FORMAT_R11G11B10_FLOAT, //careful
        R16             => DXGI_FORMAT_R16_TYPELESS,
        R16_G16         => DXGI_FORMAT_R16G16_TYPELESS,
        R16_G16_B16_A16 => DXGI_FORMAT_R16G16B16A16_TYPELESS,
        R32             => DXGI_FORMAT_R32_TYPELESS,
        R32_G32         => DXGI_FORMAT_R32G32_TYPELESS,
        R32_G32_B32     => DXGI_FORMAT_R32G32B32_TYPELESS,
        R32_G32_B32_A32 => DXGI_FORMAT_R32G32B32A32_TYPELESS,
        B8_G8_R8_A8     => DXGI_FORMAT_B8G8R8A8_TYPELESS,
        D16             => DXGI_FORMAT_R16_TYPELESS,
        D24 | D24_S8    => DXGI_FORMAT_R24G8_TYPELESS,
        D32             => DXGI_FORMAT_R32_TYPELESS,
    })
}

pub fn map_anti_alias(aa: AaMode) -> DXGI_SAMPLE_DESC {
    match aa {
        AaMode::Single => DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        AaMode::Multi(count) => DXGI_SAMPLE_DESC {
            Count: count as UINT,
            Quality: 0,
        },
        AaMode::Coverage(samples, fragments) => DXGI_SAMPLE_DESC {
            Count: fragments as UINT,
            Quality: (0..9).find(|q| (fragments<<q) >= samples).unwrap() as UINT,
        },
    }
}

pub fn map_bind(bind: Bind) -> D3D11_BIND_FLAG {
    let mut flags = D3D11_BIND_FLAG(0);
    if bind.contains(memory::RENDER_TARGET) {
        flags = flags | D3D11_BIND_RENDER_TARGET;
    }
    if bind.contains(memory::DEPTH_STENCIL) {
        flags = flags | D3D11_BIND_DEPTH_STENCIL;
    }
    if bind.contains(memory::SHADER_RESOURCE) {
        flags = flags | D3D11_BIND_SHADER_RESOURCE;
    }
    if bind.contains(memory::UNORDERED_ACCESS) {
        flags = flags | D3D11_BIND_UNORDERED_ACCESS;
    }
    flags
}

pub fn map_access(access: memory::Access) -> D3D11_CPU_ACCESS_FLAG {
    let mut r = D3D11_CPU_ACCESS_FLAG(0);
    if access.contains(memory::READ) { r = r | D3D11_CPU_ACCESS_READ }
    if access.contains(memory::WRITE) { r = r | D3D11_CPU_ACCESS_WRITE }
    r
}

pub fn map_usage(usage: Usage, bind: Bind) -> (D3D11_USAGE, D3D11_CPU_ACCESS_FLAG) {
    match usage {
        Usage::Data => if bind.is_mutable() {
            (D3D11_USAGE_DEFAULT, D3D11_CPU_ACCESS_FLAG(0))
        } else {
            (D3D11_USAGE_IMMUTABLE, D3D11_CPU_ACCESS_FLAG(0))
        },
        Usage::Dynamic => (D3D11_USAGE_DYNAMIC, D3D11_CPU_ACCESS_WRITE),
        Usage::Upload => (D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_WRITE),
        Usage::Download => (D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_READ),
    }
}

pub fn map_wrap(wrap: WrapMode) -> D3D11_TEXTURE_ADDRESS_MODE {
    match wrap {
        WrapMode::Tile   => D3D11_TEXTURE_ADDRESS_WRAP,
        WrapMode::Mirror => D3D11_TEXTURE_ADDRESS_MIRROR,
        WrapMode::Clamp  => D3D11_TEXTURE_ADDRESS_CLAMP,
        WrapMode::Border => D3D11_TEXTURE_ADDRESS_BORDER,
    }
}

pub enum FilterOp {
    Product,
    Comparison,
    //Maximum, TODO
    //Minimum, TODO
}

pub fn map_filter(filter: FilterMethod, op: FilterOp) -> D3D11_FILTER {
    use core::texture::FilterMethod::*;
    match op {
        FilterOp::Product => match filter {
            Scale          => D3D11_FILTER_MIN_MAG_MIP_POINT,
            Mipmap         => D3D11_FILTER_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D11_FILTER_ANISOTROPIC,
        },
        FilterOp::Comparison => match filter {
            Scale          => D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
            Mipmap         => D3D11_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D11_FILTER_COMPARISON_ANISOTROPIC,
        },
    }
}

pub fn map_dsv_flags(dsf: DepthStencilFlags) -> D3D11_DSV_FLAG {
    use core::texture as t;
    let mut out = D3D11_DSV_FLAG(0);
    if dsf.contains(t::RO_DEPTH) {
        out = out | D3D11_DSV_READ_ONLY_DEPTH;
    }
    if dsf.contains(t::RO_STENCIL) {
        out = out | D3D11_DSV_READ_ONLY_STENCIL;
    }
    out
}
