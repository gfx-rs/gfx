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

use gfx_core::state;
use gfx_core::{IndexType, Primitive};
use gfx_core::shade;
use gfx_core::factory;
use gfx_core::factory::{Bind, MapAccess, Usage};
use gfx_core::format::{Format, ChannelType, SurfaceType};
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

pub fn map_winding(wind: state::FrontFace) -> MTLWinding {
    match wind {
        state::FrontFace::Clockwise => MTLWinding::Clockwise,
        state::FrontFace::CounterClockwise => MTLWinding::CounterClockwise,
    }
}

pub fn map_cull(cull: state::CullFace) -> MTLCullMode {
    match cull {
        state::CullFace::Nothing => MTLCullMode::None,
        state::CullFace::Front => MTLCullMode::Front,
        state::CullFace::Back => MTLCullMode::Back,
    }
}

pub fn map_fill(fill: state::RasterMethod) -> MTLTriangleFillMode {
    match fill {
        state::RasterMethod::Point => {
            error!("Point rasterization is not supported");
            MTLTriangleFillMode::Fill
        },
        state::RasterMethod::Line(_) => MTLTriangleFillMode::Lines,
        state::RasterMethod::Fill => MTLTriangleFillMode::Fill,
    }
}

pub fn map_index_type(ty: IndexType) -> MTLIndexType {
    match ty {
        IndexType::U16 => MTLIndexType::UInt16,
        IndexType::U32 => MTLIndexType::UInt32
    }
}

pub fn map_stencil_op(op: state::StencilOp) -> MTLStencilOperation {
    use gfx_core:: state::StencilOp::*;

    match op {
        Keep => MTLStencilOperation::Keep,
        Zero => MTLStencilOperation::Zero,
        Replace => MTLStencilOperation::Replace,
        IncrementClamp => MTLStencilOperation::IncrementClamp,
        IncrementWrap => MTLStencilOperation::IncrementWrap,
        DecrementClamp => MTLStencilOperation::DecrementClamp,
        DecrementWrap => MTLStencilOperation::DecrementWrap,
        Invert => MTLStencilOperation::Invert,
    }
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

pub fn map_topology(primitive: Primitive) -> MTLPrimitiveTopologyClass {
    match primitive {
        Primitive::PointList       => MTLPrimitiveTopologyClass::Point,
        Primitive::LineList        => MTLPrimitiveTopologyClass::Line,
        Primitive::TriangleList    => MTLPrimitiveTopologyClass::Triangle,

        // TODO: can we work around not having line/triangle strip?
        Primitive::LineStrip       => MTLPrimitiveTopologyClass::Unspecified,
        Primitive::TriangleStrip   => MTLPrimitiveTopologyClass::Unspecified,
    }
}

pub fn map_vertex_format(format: Format) -> Option<MTLVertexFormat> {
    use gfx_core::format::SurfaceType::*;
    use gfx_core::format::ChannelType::*;

    // TODO: review enums
    Some(match format.0 {
        R8_G8 => match format.1 {
            Int   => MTLVertexFormat::Char2,
            Uint  => MTLVertexFormat::UChar2,
            Inorm => MTLVertexFormat::Char2Normalized,
            Unorm => MTLVertexFormat::UChar2Normalized,
            _ => return None,
        },
        R8_G8_B8_A8 => match format.1 {
            Int   => MTLVertexFormat::Char4,
            Uint  => MTLVertexFormat::UChar4,
            Inorm => MTLVertexFormat::Char4Normalized,
            Unorm => MTLVertexFormat::UChar4Normalized,
            _ => return None,
        },
        R10_G10_B10_A2 => match format.1 {
            Inorm => MTLVertexFormat::Int1010102Normalized,
            Unorm => MTLVertexFormat::UInt1010102Normalized,
            _ => return None,
        },
        R16_G16 => match format.1 {
            Int   => MTLVertexFormat::Short2,
            Uint  => MTLVertexFormat::UShort2,
            Inorm => MTLVertexFormat::Short2Normalized,
            Unorm => MTLVertexFormat::UShort2Normalized,
            Float => MTLVertexFormat::Half2,
            _ => return None,
        },
        R16_G16_B16 => match format.1 {
            Int   => MTLVertexFormat::Short3,
            Uint  => MTLVertexFormat::UShort3,
            Inorm => MTLVertexFormat::Short3Normalized,
            Unorm => MTLVertexFormat::UShort3Normalized,
            Float => MTLVertexFormat::Half3,
            _ => return None,
        },
        R16_G16_B16_A16 => match format.1 {
            Int   => MTLVertexFormat::Short4,
            Uint  => MTLVertexFormat::UShort4,
            Inorm => MTLVertexFormat::Short4Normalized,
            Unorm => MTLVertexFormat::UShort4Normalized,
            Float => MTLVertexFormat::Half4,
            _ => return None,
        },
        R32 => match format.1 {
            Int   => MTLVertexFormat::Int,
            Uint  => MTLVertexFormat::UInt,
            Float => MTLVertexFormat::Float,
            _ => return None,
        },
        R32_G32 => match format.1 {
            Int   => MTLVertexFormat::Int2,
            Uint  => MTLVertexFormat::UInt2,
            Float => MTLVertexFormat::Float2,
            _ => return None,
        },
        R32_G32_B32 => match format.1 {
            Int   => MTLVertexFormat::Int3,
            Uint  => MTLVertexFormat::UInt3,
            Float => MTLVertexFormat::Float3,
            _ => return None,
        },
        R32_G32_B32_A32 => match format.1 {
            Int   => MTLVertexFormat::Int4,
            Uint  => MTLVertexFormat::UInt4,
            Float => MTLVertexFormat::Float4,
            _ => return None,
        },
        _ => return None
    })
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

pub fn map_channel_hint(hint: SurfaceType) -> Option<ChannelType> {
    use gfx_core::format::SurfaceType::*;
    use gfx_core::format::ChannelType::*;

    Some(match hint {
        R4_G4 | R4_G4_B4_A4 | R5_G5_B5_A1 | R5_G6_B5 | R16_G16_B16 |
        R32_G32_B32 | D16 => return None,
        R8 | R8_G8 | R8_G8_B8_A8 | R10_G10_B10_A2 | R16 | R16_G16 |
        R16_G16_B16_A16 | R32 | R32_G32 | R32_G32_B32_A32 => Uint,
        R11_G11_B10 => Float,
        D24 => Unorm,
        D24_S8 => Unorm,
        D32 => Float,
    })
}

pub fn format_supports_usage(feature_set: MTLFeatureSet, format: MTLPixelFormat, usage: FormatUsage) -> bool {
    use metal::MTLPixelFormat::*;
    use metal::MTLFeatureSet::*;

    use FormatUsage::*;

    // TODO: can we simplify this with macros maybe?

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

pub fn map_depth_surface(surface: SurfaceType) -> Option<MTLPixelFormat> {
    use gfx_core::format::SurfaceType::*;

    use metal::MTLPixelFormat::*;

    Some(match surface {
        D24_S8 => Depth24Unorm_Stencil8,
        D32 => Depth32Float,
        _ => return None
    })
}


pub fn map_container_type(ty: MTLDataType) -> shade::ContainerType {
    use metal::MTLDataType::*;

    match ty {
        Float | Half | Int | UInt | Short | UShort | Char | UChar | Bool
            => shade::ContainerType::Single,
        Float2 | Half2 | Int2 | UInt2 | Short2 | UShort2 | Char2 | UChar2 | Bool2
            => shade::ContainerType::Vector(2),
        Float3 | Half3 | Int3 | UInt3 | Short3 | UShort3 | Char3 | UChar3 | Bool3
            => shade::ContainerType::Vector(3),
        Float4 | Half4 | Int4 | UInt4 | Short4 | UShort4 | Char4 | UChar4 | Bool4
            => shade::ContainerType::Vector(4),
        Float2x2 | Half2x2
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 2, 2),
        Float2x3 | Half2x3
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 2, 3),
        Float2x4 | Half2x4
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 2, 4),
        Float3x2 | Half3x2
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 3, 2),
        Float3x3 | Half3x3
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 3, 3),
        Float3x4 | Half3x4
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 3, 4),
        Float4x2 | Half4x2
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 4, 2),
        Float4x3 | Half4x3
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 4, 3),
        Float4x4 | Half4x4
            => shade::ContainerType::Matrix(shade::MatrixFormat::ColumnMajor, 4, 4),
        _ => {
            error!("Unknown container type {:?}", ty);
            shade::ContainerType::Single
        }
    }
}

pub fn map_base_type(ty: MTLDataType) -> shade::BaseType {
    use metal::MTLDataType::*;

    match ty {
        Float | Float2 | Float3 | Float4 |
        Float2x2 | Float2x3 | Float2x4 |
        Float3x2 | Float3x3 | Float3x4 |
        Float4x2 | Float4x3 | Float4x4 |
        Half | Half2 | Half3 | Half4 |
        Half2x2 | Half2x3 | Half2x4 |
        Half3x2 | Half3x3 | Half3x4 |
        Half4x2 | Half4x3 | Half4x4 => shade::BaseType::F32,
        Int | Int2 | Int3 | Int4 |
        Short | Short2 | Short3 | Short4 |
        Char | Char2 | Char3 | Char4 => shade::BaseType::I32,
        UInt | UInt2 | UInt3 | UInt4 |
        UShort | UShort2 | UShort3 | UShort4 |
        UChar | UChar2 | UChar3 | UChar4 => shade::BaseType::U32,
        Bool | Bool2 | Bool3 | Bool4 => shade::BaseType::Bool,
        _ => {
            error!("Unknown base type {:?}", ty);
            shade::BaseType::I32
        }
    }
}

pub fn map_texture_type(tex_type: MTLTextureType) -> shade::TextureType {
    use gfx_core::shade::IsArray::*;
    use gfx_core::shade::IsMultiSample::*;

    match tex_type {
        MTLTextureType::D1            => shade::TextureType::D1(NoArray),
        MTLTextureType::D1Array       => shade::TextureType::D1(Array),
        MTLTextureType::D2            => shade::TextureType::D2(NoArray, NoMultiSample),
        MTLTextureType::D2Array       => shade::TextureType::D2(Array, NoMultiSample),
        MTLTextureType::D2Multisample => shade::TextureType::D2(NoArray, MultiSample),
        MTLTextureType::D3            => shade::TextureType::D3,
        MTLTextureType::Cube          => shade::TextureType::Cube(NoArray),
        MTLTextureType::CubeArray     => shade::TextureType::Cube(Array),
    }
}

pub fn map_texture_bind(bind: Bind) -> MTLTextureUsage {
    let mut flags = MTLTextureUsageUnknown;

    if bind.contains(factory::RENDER_TARGET) || bind.contains(factory::DEPTH_STENCIL) {
        flags = flags | MTLTextureUsageRenderTarget;
    }

    if bind.contains(factory::SHADER_RESOURCE) {
        flags = flags | MTLTextureUsageShaderRead;
    }

    if bind.contains(factory::UNORDERED_ACCESS) {
        flags = flags | MTLTextureUsageShaderWrite;
    }

    flags
}

pub fn map_access(access: MapAccess) -> MTLResourceOptions {
    match access {
        MapAccess::Readable => MTLResourceCPUCacheModeDefaultCache,
        MapAccess::Writable => MTLResourceCPUCacheModeWriteCombined,
        MapAccess::RW => MTLResourceCPUCacheModeDefaultCache,
    }
}

pub fn map_texture_usage(usage: Usage) -> (MTLResourceOptions, MTLStorageMode) {
    match usage {
        Usage::GpuOnly => (MTLResourceStorageModePrivate, MTLStorageMode::Private),

        Usage::Const   => (MTLResourceStorageModePrivate, MTLStorageMode::Managed),
        Usage::Dynamic => (MTLResourceCPUCacheModeDefaultCache, MTLStorageMode::Managed),
        Usage::CpuOnly(access) => (map_access(access), MTLStorageMode::Managed),
    }
}

pub fn map_buffer_usage(usage: Usage) -> MTLResourceOptions {
    match usage {
        Usage::GpuOnly => MTLResourceStorageModePrivate,
        Usage::Const   => MTLResourceCPUCacheModeDefaultCache,
        Usage::Dynamic => MTLResourceCPUCacheModeDefaultCache,
        Usage::CpuOnly(access) => map_access(access)
    }
}

pub fn map_filter(filter: FilterMethod) -> (MTLSamplerMinMagFilter, MTLSamplerMipFilter) {
    use metal::MTLSamplerMinMagFilter::*;

    match filter {
        FilterMethod::Scale => (MTLSamplerMinMagFilter::Nearest,
                                MTLSamplerMipFilter::NotMipmapped),
        FilterMethod::Mipmap => (MTLSamplerMinMagFilter::Nearest,
                                 MTLSamplerMipFilter::Nearest),
        FilterMethod::Bilinear => (MTLSamplerMinMagFilter::Linear,
                                   MTLSamplerMipFilter::NotMipmapped),
        FilterMethod::Trilinear => (MTLSamplerMinMagFilter::Linear,
                                    MTLSamplerMipFilter::Linear),
        FilterMethod::Anisotropic(..) => (MTLSamplerMinMagFilter::Linear,
                                          MTLSamplerMipFilter::NotMipmapped),
    }

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

