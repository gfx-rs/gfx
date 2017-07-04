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

use core::{buffer, memory, HeapType};
use core::format::Format;
use core::image::{FilterMethod, WrapMode};
use core::state::Comparison;
use winapi::*;


pub fn map_heap_properties(props: memory::HeapProperties) -> D3D12_HEAP_PROPERTIES {
    //TODO: ensure the flags are valid
    D3D12_HEAP_PROPERTIES {
        Type: if !props.contains(memory::CPU_VISIBLE) {
            D3D12_HEAP_TYPE_DEFAULT
        } else if props.contains(memory::WRITE_COMBINED) {
            D3D12_HEAP_TYPE_UPLOAD
        } else {
            D3D12_HEAP_TYPE_READBACK
        },
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
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

pub fn map_buffer_resource_state(usage: buffer::Usage) -> D3D12_RESOURCE_STATES {
    unimplemented!()
}

pub fn map_image_resource_state(access: memory::ImageAccess, _layout: memory::ImageLayout) -> D3D12_RESOURCE_STATES {
    let mut state = D3D12_RESOURCE_STATE_COMMON;

    if access.contains(memory::RENDER_TARGET_CLEAR) {
        state = state | D3D12_RESOURCE_STATE_RENDER_TARGET;
    }

    if access.contains(memory::RESOLVE_SRC) {
        state = state | D3D12_RESOURCE_STATE_RESOLVE_SOURCE;
    }
    if access.contains(memory::RESOLVE_DST) {
        state = state | D3D12_RESOURCE_STATE_RESOLVE_DEST;
    }

    if access.contains(memory::TRANSFER_READ) {
        state = state | D3D12_RESOURCE_STATE_COPY_SOURCE;
    }
    if access.contains(memory::TRANSFER_WRITE) {
        state = state | D3D12_RESOURCE_STATE_COPY_DEST;
    }

    state
}

pub fn map_function(fun: Comparison) -> D3D12_COMPARISON_FUNC {
    match fun {
        Comparison::Never => D3D12_COMPARISON_FUNC_NEVER,
        Comparison::Less => D3D12_COMPARISON_FUNC_LESS,
        Comparison::LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
        Comparison::Equal => D3D12_COMPARISON_FUNC_EQUAL,
        Comparison::GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        Comparison::Greater => D3D12_COMPARISON_FUNC_GREATER,
        Comparison::NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
        Comparison::Always => D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

pub fn map_wrap(wrap: WrapMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    match wrap {
        WrapMode::Tile   => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        WrapMode::Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        WrapMode::Clamp  => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        WrapMode::Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
    }
}

pub enum FilterOp {
    Product,
    Comparison,
    //Maximum, TODO
    //Minimum, TODO
}

pub fn map_filter(filter: FilterMethod, op: FilterOp) -> D3D12_FILTER {
    use core::image::FilterMethod::*;
    match op {
        FilterOp::Product => match filter {
            Scale          => D3D12_FILTER_MIN_MAG_MIP_POINT,
            Mipmap         => D3D12_FILTER_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D12_FILTER_ANISOTROPIC,
        },
        FilterOp::Comparison => match filter {
            Scale          => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
            Mipmap         => D3D12_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D12_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D12_FILTER_COMPARISON_ANISOTROPIC,
        },
    }
}
