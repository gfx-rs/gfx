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

use core::{shade, state, memory, Primitive};
use core::memory::{Bind, Usage};
use core::format::{SurfaceType, ChannelType, Swizzle, ChannelSource};
use core::pso::ColorInfo;
use core::texture::{FilterMethod, Kind, Layer, LayerError, PackedColor, WrapMode};
use vk;


pub fn map_image_type(kind: Kind) -> vk::ImageType {
   match kind {
        Kind::D1(..) | Kind::D1Array(..) => vk::IMAGE_TYPE_1D,
        Kind::D2(..) | Kind::D2Array(..) => vk::IMAGE_TYPE_2D,
        Kind::D3(..) => vk::IMAGE_TYPE_3D,
        Kind::Cube(..) | Kind::CubeArray(..) => vk::IMAGE_TYPE_2D,
    }
}

pub fn map_image_view_type(kind: Kind, layer: Option<Layer>) -> Result<vk::ImageViewType, LayerError> {
    match (kind, layer) {
        (Kind::D1(..), Some(_)) | (Kind::D2(..), Some(_)) | (Kind::D3(..), Some(_)) |
        (Kind::Cube(..), Some(_)) => Err(LayerError::NotExpected(kind)),
        (Kind::D1Array(_, n),       Some(l)) if n<=l => Err(LayerError::OutOfBounds(l, n)),
        (Kind::D2Array(_, _, n, _), Some(l)) if n<=l => Err(LayerError::OutOfBounds(l, n)),
        (Kind::CubeArray(_, n),     Some(l)) if n<=l => Err(LayerError::OutOfBounds(l, n)),
        (Kind::D1(..), None) | (Kind::D1Array(..), Some(_)) => Ok(vk::IMAGE_VIEW_TYPE_1D),
        (Kind::D1Array(..), None) => Ok(vk::IMAGE_VIEW_TYPE_1D_ARRAY),
        (Kind::D2(..), None) | (Kind::D2Array(..), Some(_)) => Ok(vk::IMAGE_VIEW_TYPE_2D),
        (Kind::D2Array(..), None) => Ok(vk::IMAGE_VIEW_TYPE_2D_ARRAY),
        (Kind::D3(..), None) => Ok(vk::IMAGE_VIEW_TYPE_3D),
        (Kind::Cube(..), None) | (Kind::CubeArray(..), Some(_)) => Ok(vk::IMAGE_VIEW_TYPE_CUBE),
        (Kind::CubeArray(..), None) => Ok(vk::IMAGE_VIEW_TYPE_CUBE_ARRAY),
    }
}

pub fn map_image_aspect(surface: SurfaceType, channel: ChannelType, is_target: bool) -> vk::ImageAspectFlags {
    match surface {
        SurfaceType::D16 | SurfaceType::D24 | SurfaceType::D24_S8 | SurfaceType::D32 => match (is_target, channel) {
            (true, _) => vk::IMAGE_ASPECT_DEPTH_BIT | vk::IMAGE_ASPECT_STENCIL_BIT,
            (false, ChannelType::Float) | (false, ChannelType::Unorm) => vk::IMAGE_ASPECT_DEPTH_BIT,
            (false, ChannelType::Uint)  => vk::IMAGE_ASPECT_STENCIL_BIT,
            _ => {
                error!("Unexpected depth/stencil channel {:?}", channel);
                vk::IMAGE_ASPECT_DEPTH_BIT
            }
        },
        _ => vk::IMAGE_ASPECT_COLOR_BIT,
    }
}

pub fn map_channel_source(source: ChannelSource) -> vk::ComponentSwizzle {
    match source {
        ChannelSource::Zero => vk::COMPONENT_SWIZZLE_ZERO,
        ChannelSource::One  => vk::COMPONENT_SWIZZLE_ONE,
        ChannelSource::X    => vk::COMPONENT_SWIZZLE_R,
        ChannelSource::Y    => vk::COMPONENT_SWIZZLE_G,
        ChannelSource::Z    => vk::COMPONENT_SWIZZLE_B,
        ChannelSource::W    => vk::COMPONENT_SWIZZLE_A,
    }
}

pub fn map_swizzle(swizzle: Swizzle) -> vk::ComponentMapping {
    vk::ComponentMapping {
        r: map_channel_source(swizzle.0),
        g: map_channel_source(swizzle.1),
        b: map_channel_source(swizzle.2),
        a: map_channel_source(swizzle.3),
    }
}

pub fn map_usage_tiling(gfx_usage: Usage, bind: Bind) -> (vk::ImageUsageFlags, vk::ImageTiling) {
    let mut usage = 0;
    if bind.contains(memory::TRANSFER_SRC) {
        usage |= vk::IMAGE_USAGE_TRANSFER_SRC_BIT;
    }
    if bind.contains(memory::TRANSFER_DST) {
        usage |= vk::IMAGE_USAGE_TRANSFER_DST_BIT;
    }
    if bind.contains(memory::RENDER_TARGET) {
        usage |= vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    }
    if bind.contains(memory::DEPTH_STENCIL) {
        usage |= vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT;
    }
    if bind.contains(memory::SHADER_RESOURCE) {
        usage |= vk::IMAGE_USAGE_SAMPLED_BIT;
    }
    if bind.contains(memory::UNORDERED_ACCESS) {
        usage |= vk::IMAGE_USAGE_STORAGE_BIT;
    }
    let tiling = match gfx_usage {
        Usage::Data => vk::IMAGE_TILING_OPTIMAL,
        Usage::Dynamic => {
            usage |= vk::IMAGE_USAGE_TRANSFER_DST_BIT;
            vk::IMAGE_TILING_LINEAR
        },
        Usage::Upload | Usage::Download => vk::IMAGE_TILING_LINEAR,
    };
    (usage, tiling)
}

pub fn map_image_layout(bind: Bind) -> vk::ImageLayout {
    //use gfx_core::factory as f;
    // can't use optimal layouts for the fact PSO descriptor doesn't know about them
    match bind {
        //f::RENDER_TARGET   => vk::IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
        //f::DEPTH_STENCIL   => vk::IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        //f::SHADER_RESOURCE => vk::IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
        _                  => vk::IMAGE_LAYOUT_GENERAL,
    }
}

pub fn map_format(surface: SurfaceType, chan: ChannelType) -> Option<vk::Format> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
    Some(match surface {
        R4_G4 => match chan {
            Unorm => vk::FORMAT_R4G4_UNORM_PACK8,
            _ => return None,
        },
        R4_G4_B4_A4 => match chan {
            Unorm => vk::FORMAT_R4G4B4A4_UNORM_PACK16,
            _ => return None,
        },
        R5_G5_B5_A1 => match chan {
            Unorm => vk::FORMAT_R5G5B5A1_UNORM_PACK16,
             _ => return None,
        },
        R5_G6_B5 => match chan {
            Unorm => vk::FORMAT_R5G6B5_UNORM_PACK16,
             _ => return None,
        },
        R8 => match chan {
            Int   => vk::FORMAT_R8_SINT,
            Uint  => vk::FORMAT_R8_UINT,
            Inorm => vk::FORMAT_R8_SNORM,
            Unorm => vk::FORMAT_R8_UNORM,
            Srgb  => vk::FORMAT_R8_SRGB,
            _ => return None,
        },
        R8_G8 => match chan {
            Int   => vk::FORMAT_R8G8_SINT,
            Uint  => vk::FORMAT_R8G8_UINT,
            Inorm => vk::FORMAT_R8G8_SNORM,
            Unorm => vk::FORMAT_R8G8_UNORM,
            Srgb  => vk::FORMAT_R8G8_SRGB,
            _ => return None,
        },
        R8_G8_B8_A8 => match chan {
            Int   => vk::FORMAT_R8G8B8A8_SINT,
            Uint  => vk::FORMAT_R8G8B8A8_UINT,
            Inorm => vk::FORMAT_R8G8B8A8_SNORM,
            Unorm => vk::FORMAT_R8G8B8A8_UNORM,
            Srgb  => vk::FORMAT_R8G8B8A8_SRGB,
            _ => return None,
        },
        R10_G10_B10_A2 => match chan {
            Int   => vk::FORMAT_A2R10G10B10_SINT_PACK32,
            Uint  => vk::FORMAT_A2R10G10B10_UINT_PACK32,
            Inorm => vk::FORMAT_A2R10G10B10_SNORM_PACK32,
            Unorm => vk::FORMAT_A2R10G10B10_UNORM_PACK32,
            _ => return None,
        },
        R11_G11_B10 => match chan {
            Float => vk::FORMAT_B10G11R11_UFLOAT_PACK32,
            _ => return None,
        },
        R16 => match chan {
            Int   => vk::FORMAT_R16_SINT,
            Uint  => vk::FORMAT_R16_UINT,
            Inorm => vk::FORMAT_R16_SNORM,
            Unorm => vk::FORMAT_R16_UNORM,
            Float => vk::FORMAT_R16_SFLOAT,
            _ => return None,
        },
        R16_G16 => match chan {
            Int   => vk::FORMAT_R16G16_SINT,
            Uint  => vk::FORMAT_R16G16_UINT,
            Inorm => vk::FORMAT_R16G16_SNORM,
            Unorm => vk::FORMAT_R16G16_UNORM,
            Float => vk::FORMAT_R16G16_SFLOAT,
            _ => return None,
        },
        R16_G16_B16 => match chan {
            Int   => vk::FORMAT_R16G16B16_SINT,
            Uint  => vk::FORMAT_R16G16B16_UINT,
            Inorm => vk::FORMAT_R16G16B16_SNORM,
            Unorm => vk::FORMAT_R16G16B16_UNORM,
            Float => vk::FORMAT_R16G16B16_SFLOAT,
            _ => return None,
        },
        R16_G16_B16_A16 => match chan {
            Int   => vk::FORMAT_R16G16B16A16_SINT,
            Uint  => vk::FORMAT_R16G16B16A16_UINT,
            Inorm => vk::FORMAT_R16G16B16A16_SNORM,
            Unorm => vk::FORMAT_R16G16B16A16_UNORM,
            Float => vk::FORMAT_R16G16B16A16_SFLOAT,
            _ => return None,
        },
        R32 => match chan {
            Int   => vk::FORMAT_R32_SINT,
            Uint  => vk::FORMAT_R32_UINT,
            Float => vk::FORMAT_R32_SFLOAT,
            _ => return None,
        },
        R32_G32 => match chan {
            Int   => vk::FORMAT_R32G32_SINT,
            Uint  => vk::FORMAT_R32G32_UINT,
            Float => vk::FORMAT_R32G32_SFLOAT,
            _ => return None,
        },
        R32_G32_B32 => match chan {
            Int   => vk::FORMAT_R32G32B32_SINT,
            Uint  => vk::FORMAT_R32G32B32_UINT,
            Float => vk::FORMAT_R32G32B32_SFLOAT,
            _ => return None,
        },
        R32_G32_B32_A32 => match chan {
            Int   => vk::FORMAT_R32G32B32A32_SINT,
            Uint  => vk::FORMAT_R32G32B32A32_UINT,
            Float => vk::FORMAT_R32G32B32A32_SFLOAT,
            _ => return None,
        },
        B8_G8_R8_A8 => match chan {
            Unorm => vk::FORMAT_B8G8R8A8_UNORM,
            _ => return None,
        },
        D16 => match chan {
            Unorm  => vk::FORMAT_D16_UNORM,
            _ => return None,
        },
        D24 => match chan {
            Unorm => vk::FORMAT_X8_D24_UNORM_PACK32,
            _ => return None,
        },
        D24_S8 => match chan {
            Unorm => vk::FORMAT_D24_UNORM_S8_UINT,
            _ => return None,
        },
        D32 => match chan {
            Float => vk::FORMAT_D32_SFLOAT,
            _ => return None,
        },
    })
}

pub fn map_filter(filter: FilterMethod) -> (vk::Filter, vk::Filter, vk::SamplerMipmapMode, f32) {
    match filter {
        FilterMethod::Scale          => (vk::FILTER_NEAREST, vk::FILTER_NEAREST, vk::SAMPLER_MIPMAP_MODE_NEAREST, 0.0),
        FilterMethod::Mipmap         => (vk::FILTER_NEAREST, vk::FILTER_NEAREST, vk::SAMPLER_MIPMAP_MODE_LINEAR,  0.0),
        FilterMethod::Bilinear       => (vk::FILTER_LINEAR,  vk::FILTER_LINEAR,  vk::SAMPLER_MIPMAP_MODE_NEAREST, 0.0),
        FilterMethod::Trilinear      => (vk::FILTER_LINEAR,  vk::FILTER_LINEAR,  vk::SAMPLER_MIPMAP_MODE_LINEAR,  0.0),
        FilterMethod::Anisotropic(a) => (vk::FILTER_LINEAR,  vk::FILTER_LINEAR,  vk::SAMPLER_MIPMAP_MODE_LINEAR,  a as f32),
    }
}

pub fn map_wrap(wrap: WrapMode) -> vk::SamplerAddressMode {
    match wrap {
        WrapMode::Tile   => vk::SAMPLER_ADDRESS_MODE_REPEAT,
        WrapMode::Mirror => vk::SAMPLER_ADDRESS_MODE_MIRRORED_REPEAT,
        WrapMode::Clamp  => vk::SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
        WrapMode::Border => vk::SAMPLER_ADDRESS_MODE_CLAMP_TO_BORDER,
    }
}

pub fn map_border_color(col: PackedColor) -> Option<vk::BorderColor> {
    match col.0 {
        0x00000000 => Some(vk::BORDER_COLOR_FLOAT_TRANSPARENT_BLACK),
        0xFF000000 => Some(vk::BORDER_COLOR_FLOAT_OPAQUE_BLACK),
        0xFFFFFFFF => Some(vk::BORDER_COLOR_FLOAT_OPAQUE_WHITE),
        _ => None
    }
}

pub fn map_comparison(fun: state::Comparison) -> vk::CompareOp {
    use core::state::Comparison::*;
    match fun {
        Never        => vk::COMPARE_OP_NEVER,
        Less         => vk::COMPARE_OP_LESS,
        LessEqual    => vk::COMPARE_OP_LESS_OR_EQUAL,
        Equal        => vk::COMPARE_OP_EQUAL,
        GreaterEqual => vk::COMPARE_OP_GREATER_OR_EQUAL,
        Greater      => vk::COMPARE_OP_GREATER,
        NotEqual     => vk::COMPARE_OP_NOT_EQUAL,
        Always       => vk::COMPARE_OP_ALWAYS,
    }
}

pub fn map_topology(prim: Primitive) -> vk::PrimitiveTopology {
    match prim {
        Primitive::PointList     => vk::PRIMITIVE_TOPOLOGY_POINT_LIST,
        Primitive::LineList      => vk::PRIMITIVE_TOPOLOGY_LINE_LIST,
        Primitive::LineStrip     => vk::PRIMITIVE_TOPOLOGY_LINE_STRIP,
        Primitive::TriangleList  => vk::PRIMITIVE_TOPOLOGY_TRIANGLE_LIST,
        Primitive::TriangleStrip => vk::PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP,
        Primitive::LineListAdjacency      => vk::PRIMITIVE_TOPOLOGY_LINE_LIST_WITH_ADJACENCY,
        Primitive::LineStripAdjacency     => vk::PRIMITIVE_TOPOLOGY_LINE_STRIP_WITH_ADJACENCY,
        Primitive::TriangleListAdjacency  => vk::PRIMITIVE_TOPOLOGY_TRIANGLE_LIST_WITH_ADJACENCY,
        Primitive::TriangleStripAdjacency => vk::PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP_WITH_ADJACENCY,
        Primitive::PatchList(_)  => vk::PRIMITIVE_TOPOLOGY_PATCH_LIST,
    }
}

pub fn map_polygon_mode(rm: state::RasterMethod) -> (vk::PolygonMode, f32) {
    match rm {
        state::RasterMethod::Point   => (vk::POLYGON_MODE_POINT, 1.0),
        state::RasterMethod::Line(w) => (vk::POLYGON_MODE_LINE, w as f32),
        state::RasterMethod::Fill    => (vk::POLYGON_MODE_FILL, 1.0),
    }
}

pub fn map_cull_face(cf: state::CullFace) -> vk::CullModeFlagBits {
    match cf {
        state::CullFace::Nothing => vk::CULL_MODE_NONE,
        state::CullFace::Front   => vk::CULL_MODE_FRONT_BIT,
        state::CullFace::Back    => vk::CULL_MODE_BACK_BIT,
    }
}

pub fn map_front_face(ff: state::FrontFace) -> vk::FrontFace {
    match ff {
        state::FrontFace::Clockwise        => vk::FRONT_FACE_CLOCKWISE,
        state::FrontFace::CounterClockwise => vk::FRONT_FACE_COUNTER_CLOCKWISE,
    }
}

pub fn map_stencil_op(op: state::StencilOp) -> vk::StencilOp {
    use core::state::StencilOp::*;
    match op {
        Keep           => vk::STENCIL_OP_KEEP,
        Zero           => vk::STENCIL_OP_ZERO,
        Replace        => vk::STENCIL_OP_REPLACE,
        IncrementClamp => vk::STENCIL_OP_INCREMENT_AND_CLAMP,
        IncrementWrap  => vk::STENCIL_OP_INCREMENT_AND_WRAP,
        DecrementClamp => vk::STENCIL_OP_DECREMENT_AND_CLAMP,
        DecrementWrap  => vk::STENCIL_OP_DECREMENT_AND_WRAP,
        Invert         => vk::STENCIL_OP_INVERT,
    }
}

pub fn map_stencil_side(side: &state::StencilSide) -> vk::StencilOpState {
    vk::StencilOpState {
        failOp: map_stencil_op(side.op_fail),
        passOp: map_stencil_op(side.op_pass),
        depthFailOp: map_stencil_op(side.op_depth_fail),
        compareOp: map_comparison(side.fun),
        compareMask: side.mask_read as u32,
        writeMask: side.mask_write as u32,
        reference: 0,
    }
}

pub fn map_blend_factor(factor: state::Factor) -> vk::BlendFactor {
    use core::state::Factor::*;
    use core::state::BlendValue::*;
    match factor {
        Zero                  => vk::BLEND_FACTOR_ZERO,
        One                   => vk::BLEND_FACTOR_ONE,
        SourceAlphaSaturated  => vk::BLEND_FACTOR_SRC_ALPHA_SATURATE,
        ZeroPlus(SourceColor) => vk::BLEND_FACTOR_SRC_COLOR,
        ZeroPlus(SourceAlpha) => vk::BLEND_FACTOR_SRC_ALPHA,
        ZeroPlus(DestColor)   => vk::BLEND_FACTOR_DST_COLOR,
        ZeroPlus(DestAlpha)   => vk::BLEND_FACTOR_DST_ALPHA,
        ZeroPlus(ConstColor)  => vk::BLEND_FACTOR_CONSTANT_COLOR,
        ZeroPlus(ConstAlpha)  => vk::BLEND_FACTOR_CONSTANT_ALPHA,
        OneMinus(SourceColor) => vk::BLEND_FACTOR_ONE_MINUS_SRC_COLOR,
        OneMinus(SourceAlpha) => vk::BLEND_FACTOR_ONE_MINUS_SRC_ALPHA,
        OneMinus(DestColor)   => vk::BLEND_FACTOR_ONE_MINUS_DST_COLOR,
        OneMinus(DestAlpha)   => vk::BLEND_FACTOR_ONE_MINUS_DST_ALPHA,
        OneMinus(ConstColor)  => vk::BLEND_FACTOR_ONE_MINUS_CONSTANT_COLOR,
        OneMinus(ConstAlpha)  => vk::BLEND_FACTOR_ONE_MINUS_CONSTANT_ALPHA,
    }
}

pub fn map_blend_op(op: state::Equation) -> vk::BlendOp {
    use core::state::Equation::*;
    match op {
        Add    => vk::BLEND_OP_ADD,
        Sub    => vk::BLEND_OP_SUBTRACT,
        RevSub => vk::BLEND_OP_REVERSE_SUBTRACT,
        Min    => vk::BLEND_OP_MIN,
        Max    => vk::BLEND_OP_MAX,
    }
}

pub fn map_blend(ci: &ColorInfo) -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        blendEnable: if ci.color.is_some() || ci.alpha.is_some() { vk::TRUE } else { vk::FALSE },
        srcColorBlendFactor: ci.color.map_or(0, |c| map_blend_factor(c.source)),
        dstColorBlendFactor: ci.color.map_or(0, |c| map_blend_factor(c.destination)),
        colorBlendOp: ci.color.map_or(0, |c| map_blend_op(c.equation)),
        srcAlphaBlendFactor: ci.alpha.map_or(0, |a| map_blend_factor(a.source)),
        dstAlphaBlendFactor: ci.alpha.map_or(0, |a| map_blend_factor(a.destination)),
        alphaBlendOp: ci.alpha.map_or(0, |a| map_blend_op(a.equation)),
        colorWriteMask:
            if ci.mask.contains(state::RED)   {vk::COLOR_COMPONENT_R_BIT} else {0} |
            if ci.mask.contains(state::GREEN) {vk::COLOR_COMPONENT_G_BIT} else {0} |
            if ci.mask.contains(state::BLUE)  {vk::COLOR_COMPONENT_B_BIT} else {0} |
            if ci.mask.contains(state::ALPHA) {vk::COLOR_COMPONENT_A_BIT} else {0},
    }
}

pub fn map_stage(usage: shade::Usage) -> vk::ShaderStageFlags {
    (if usage.contains(shade::VERTEX)   { vk::SHADER_STAGE_VERTEX_BIT   } else { 0 }) |
    (if usage.contains(shade::GEOMETRY) { vk::SHADER_STAGE_GEOMETRY_BIT } else { 0 }) |
    (if usage.contains(shade::PIXEL)    { vk::SHADER_STAGE_FRAGMENT_BIT } else { 0 })
}
