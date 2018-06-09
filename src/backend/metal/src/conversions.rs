use PrivateCapabilities;

use hal::{pass, image, pso, IndexType};
use hal::format::{Format, Swizzle};
use hal::pso::{Comparison, StencilOp};
use metal::*;

impl PrivateCapabilities {
    pub fn map_format(&self, format: Format) -> Option<MTLPixelFormat> {
        use metal::MTLPixelFormat::*;
        use hal::format::Format as f;
        Some(match format {
            f::B5g6r5Unorm   if self.format_b5 => B5G6R5Unorm,
            f::B5g5r5a1Unorm if self.format_b5 => BGR5A1Unorm,
            f::R8Srgb  if self.format_min_srgb_channels <= 1 => R8Unorm_sRGB,
            f::Rg8Srgb if self.format_min_srgb_channels <= 2 => RG8Unorm_sRGB,
            f::D24UnormS8Uint if self.format_depth24_stencil8 => Depth24Unorm_Stencil8,
            f::D32FloatS8Uint if self.format_depth32_stencil8 => Depth32Float_Stencil8,
            f::R8Unorm           => R8Unorm,
            f::R8Inorm           => R8Snorm,
            f::R8Uint            => R8Uint,
            f::R8Int             => R8Sint,
            f::Rg8Unorm          => RG8Unorm,
            f::Rg8Inorm          => RG8Snorm,
            f::Rg8Uint           => RG8Uint,
            f::Rg8Int            => RG8Sint,
            f::Rgba8Unorm        => RGBA8Unorm,
            f::Rgba8Inorm        => RGBA8Snorm,
            f::Rgba8Srgb         => RGBA8Unorm_sRGB,
            f::Rgba8Uint         => RGBA8Uint,
            f::Rgba8Int          => RGBA8Sint,
            f::Bgra8Unorm        => BGRA8Unorm,
            f::Bgra8Srgb         => BGRA8Unorm_sRGB,
            f::R16Unorm          => R16Unorm,
            f::R16Inorm          => R16Snorm,
            f::R16Uint           => R16Uint,
            f::R16Int            => R16Sint,
            f::R16Float          => R16Float,
            f::Rg16Unorm         => RG16Unorm,
            f::Rg16Inorm         => RG16Snorm,
            f::Rg16Uint          => RG16Uint,
            f::Rg16Int           => RG16Sint,
            f::Rg16Float         => RG16Float,
            f::Rgba16Unorm       => RGBA16Unorm,
            f::Rgba16Inorm       => RGBA16Snorm,
            f::Rgba16Uint        => RGBA16Uint,
            f::Rgba16Int         => RGBA16Sint,
            f::Rgba16Float       => RGBA16Float,
            f::R32Uint           => R32Uint,
            f::R32Int            => R32Sint,
            f::R32Float          => R32Float,
            f::Rg32Uint          => RG32Uint,
            f::Rg32Int           => RG32Sint,
            f::Rg32Float         => RG32Float,
            f::Rgba32Uint        => RGBA32Uint,
            f::Rgba32Int         => RGBA32Sint,
            f::Rgba32Float       => RGBA32Float,
            f::D16Unorm          => Depth16Unorm,
            f::D32Float          => Depth32Float,
            f::Bc1RgbUnorm       => BC1_RGBA,
            f::Bc1RgbSrgb        => BC1_RGBA_sRGB,
            f::Bc2Unorm          => BC2_RGBA,
            f::Bc2Srgb           => BC2_RGBA_sRGB,
            f::Bc3Unorm          => BC3_RGBA,
            f::Bc3Srgb           => BC3_RGBA_sRGB,
            f::Bc4Unorm          => BC4_RUnorm,
            f::Bc4Inorm          => BC4_RSnorm,
            f::Bc5Unorm          => BC5_RGUnorm,
            f::Bc5Inorm          => BC5_RGSnorm,
            f::Bc6hUfloat        => BC6H_RGBUfloat,
            f::Bc6hFloat         => BC6H_RGBFloat,
            f::Bc7Unorm          => BC7_RGBAUnorm,
            f::Bc7Srgb           => BC7_RGBAUnorm_sRGB,
            f::A2r10g10b10Unorm  => BGR10A2Unorm,
            f::A2b10g10r10Unorm  => RGB10A2Unorm,
            //f::EacR11Unorm       => EAC_R11Unorm, // !macOS
            //f::EacR11Inorm       => EAC_R11Snorm, // !macOS
            //f::EacR11g11Unorm    => EAC_RG11Unorm, // !macOS
            //f::EacR11g11Inorm    => EAC_RG11Snorm, // !macOS
            //f::Etc2R8g8b8Unorm   => ETC2_RGB8, // !macOS
            //f::Etc2R8g8b8Srgb    => ETC2_RGB8_sRGB, // !macOS
            //f::Etc2R8g8b8a1Unorm => ETC2_RGB8A1, // !macOS
            //f::Etc2R8g8b8a1Srgb  => ETC2_RGBA1_sRGB, // !macOS
            //f::Astc4x4Unorm      => ASTC_4x4_LDR, // !macOS
            //f::Astc4x4Srgb       => ASTC_4x4_sRGB, // !macOS
            //f::Astc5x4Unorm      => ASTC_5x4_LDR, // !macOS
            //f::Astc5x4Srgb       => ASTC_5x4_sRGB, // !macOS
            //f::Astc5x5Unorm      => ASTC_5x5_LDR, // !macOS
            //f::Astc5x5Srgb       => ASTC_5x5_sRGB, // !macOS
            //f::Astc6x5Unorm      => ASTC_6x5_LDR, // !macOS
            //f::Astc6x5Srgb       => ASTC_6x5_sRGB, // !macOS
            //f::Astc6x6Unorm      => ASTC_6x6_LDR, // !macOS
            //f::Astc6x6Srgb       => ASTC_6x6_sRGB, // !macOS
            //f::Astc8x5Unorm      => ASTC_8x5_LDR, // !macOS
            //f::Astc8x5Srgb       => ASTC_8x5_sRGB, // !macOS
            //f::Astc8x6Unorm      => ASTC_8x6_LDR, // !macOS
            //f::Astc8x6Srgb       => ASTC_8x6_sRGB, // !macOS
            //f::Astc8x8Unorm      => ASTC_8x8_LDR, // !macOS
            //f::Astc8x8Srgb       => ASTC_8x8_sRGB, // !macOS
            //f::Astc10x5Unorm     => ASTC_10x5_LDR, // !macOS
            //f::Astc10x5Srgb      => ASTC_10x5_sRGB, // !macOS
            //f::Astc10x6Unorm     => ASTC_10x6_LDR, // !macOS
            //f::Astc10x6Srgb      => ASTC_10x6_sRGB, // !macOS
            //f::Astc10x8Unorm     => ASTC_10x8_LDR, // !macOS
            //f::Astc10x8Srgb      => ASTC_10x8_sRGB, // !macOS
            //f::Astc10x10Unorm    => ASTC_10x10_LDR, // !macOS
            //f::Astc10x10Srgb     => ASTC_10x10_sRGB, // !macOS
            //f::Astc12x10Unorm    => ASTC_12x10_LDR, // !macOS
            //f::Astc12x10Srgb     => ASTC_12x10_sRGB, // !macOS
            //f::Astc12x12Unorm    => ASTC_12x12_LDR, // !macOS
            //f::Astc12x12Srgb     => ASTC_12x12_sRGB, // !macOS
            //f::Bgra4Unorm =>
            //f::R5g6b5Unorm =>
            //f::A1r5g5b5Unorm =>
            _ => return None,
        })
    }

    pub fn map_format_with_swizzle(&self, format: Format, swizzle: Swizzle) -> Option<MTLPixelFormat> {
        use metal::MTLPixelFormat as Pf;
        use hal::format::{Format::*, Component::*};
        match (format, swizzle) {
            (R8Unorm, Swizzle(Zero, Zero, Zero, R)) => Some(Pf::A8Unorm),
            (Rgba8Unorm, Swizzle(B, G, R, A)) => Some(Pf::BGRA8Unorm),
            (Bgra8Unorm, Swizzle(B, G, R, A)) => Some(Pf::RGBA8Unorm),
            (Bgra8Srgb, Swizzle(B, G, R, A)) => Some(Pf::RGBA8Unorm_sRGB),
            _ => {
                if swizzle != Swizzle::NO {
                    error!("Unsupported swizzle {:?} for format {:?}", swizzle, format);
                }
                self.map_format(format)
            }
        }
    }
}

pub fn map_load_operation(operation: pass::AttachmentLoadOp) -> MTLLoadAction {
    use self::pass::AttachmentLoadOp::*;

    match operation {
        Load => MTLLoadAction::Load,
        Clear => MTLLoadAction::Clear,
        DontCare => MTLLoadAction::DontCare,
    }
}

pub fn map_store_operation(operation: pass::AttachmentStoreOp) -> MTLStoreAction {
    use self::pass::AttachmentStoreOp::*;

    match operation {
        Store => MTLStoreAction::Store,
        DontCare => MTLStoreAction::DontCare,
    }
}

pub fn map_write_mask(mask: pso::ColorMask) -> MTLColorWriteMask {
    let mut mtl_mask = MTLColorWriteMask::empty();

    if mask.contains(pso::ColorMask::RED) {
        mtl_mask |= MTLColorWriteMask::Red;
    }
    if mask.contains(pso::ColorMask::GREEN) {
        mtl_mask |= MTLColorWriteMask::Green;
    }
    if mask.contains(pso::ColorMask::BLUE) {
        mtl_mask |= MTLColorWriteMask::Blue;
    }
    if mask.contains(pso::ColorMask::ALPHA) {
        mtl_mask |= MTLColorWriteMask::Alpha;
    }

    mtl_mask
}

fn map_factor(factor: pso::Factor) -> MTLBlendFactor {
    use hal::pso::Factor::*;

    match factor {
        Zero => MTLBlendFactor::Zero,
        One => MTLBlendFactor::One,
        SrcColor => MTLBlendFactor::SourceColor,
        OneMinusSrcColor => MTLBlendFactor::OneMinusSourceColor,
        DstColor => MTLBlendFactor::DestinationColor,
        OneMinusDstColor => MTLBlendFactor::OneMinusDestinationColor,
        SrcAlpha => MTLBlendFactor::SourceAlpha,
        OneMinusSrcAlpha => MTLBlendFactor::OneMinusSourceAlpha,
        DstAlpha => MTLBlendFactor::DestinationAlpha,
        OneMinusDstAlpha => MTLBlendFactor::OneMinusDestinationAlpha,
        ConstColor => MTLBlendFactor::BlendColor,
        OneMinusConstColor => MTLBlendFactor::OneMinusBlendColor,
        ConstAlpha => MTLBlendFactor::BlendAlpha,
        OneMinusConstAlpha => MTLBlendFactor::OneMinusBlendAlpha,
        SrcAlphaSaturate => MTLBlendFactor::SourceAlphaSaturated,
        Src1Color => MTLBlendFactor::Source1Color,
        OneMinusSrc1Color => MTLBlendFactor::OneMinusSource1Color,
        Src1Alpha => MTLBlendFactor::Source1Alpha,
        OneMinusSrc1Alpha => MTLBlendFactor::OneMinusSource1Alpha,
    }
}

pub fn map_blend_op(operation: &pso::BlendOp) -> (MTLBlendOperation, MTLBlendFactor, MTLBlendFactor) {
    use hal::pso::BlendOp::*;

    match *operation {
        Add    { src, dst } => (MTLBlendOperation::Add,             map_factor(src), map_factor(dst)),
        Sub    { src, dst } => (MTLBlendOperation::Subtract,        map_factor(src), map_factor(dst)),
        RevSub { src, dst } => (MTLBlendOperation::ReverseSubtract, map_factor(src), map_factor(dst)),
        Min => (MTLBlendOperation::Min, MTLBlendFactor::Zero, MTLBlendFactor::Zero),
        Max => (MTLBlendOperation::Max, MTLBlendFactor::Zero, MTLBlendFactor::Zero),
    }
}


pub fn map_vertex_format(format: Format) -> Option<MTLVertexFormat> {
    use metal::MTLVertexFormat::*;
    use hal::format::Format as f;
    Some(match format {  
        f::R8Unorm     => UCharNormalized,
        f::R8Inorm     => CharNormalized,
        f::R8Uint      => UChar,
        f::R8Int       => Char,
        f::Rg8Unorm    => UChar2Normalized,
        f::Rg8Inorm    => Char2Normalized,
        f::Rg8Uint     => UChar2,
        f::Rg8Int      => Char2,
        f::Rgb8Unorm   => UChar3Normalized,
        f::Rgb8Inorm   => Char3Normalized,
        f::Rgb8Uint    => UChar3,
        f::Rgb8Int     => Char3,
        f::Rgba8Unorm  => UChar4Normalized,
        f::Rgba8Inorm  => Char4Normalized,
        f::Rgba8Uint   => UChar4,
        f::Rgba8Int    => Char4,
        f::Bgra8Unorm  => UChar4Normalized_BGRA,
        f::R16Unorm    => UShortNormalized,
        f::R16Inorm    => ShortNormalized,
        f::R16Uint     => UShort,
        f::R16Int      => Short,
        f::R16Float    => Half,
        f::Rg16Unorm   => UShort2Normalized,
        f::Rg16Inorm   => Short2Normalized,
        f::Rg16Uint    => UShort2,
        f::Rg16Int     => Short2,
        f::Rg16Float   => Half2,
        f::Rgb16Unorm  => UShort3Normalized,
        f::Rgb16Inorm  => Short3Normalized,
        f::Rgb16Uint   => UShort3,
        f::Rgb16Int    => Short3,
        f::Rgb16Float  => Half3,
        f::Rgba16Unorm => UShort4Normalized,
        f::Rgba16Inorm => Short4Normalized,
        f::Rgba16Uint  => UShort4,
        f::Rgba16Int   => Short4,
        f::Rgba16Float => Half4,
        f::R32Uint     => UInt,
        f::R32Int      => Int,
        f::R32Float    => Float,
        f::Rg32Uint    => UInt2,
        f::Rg32Int     => Int2,
        f::Rg32Float   => Float2,
        f::Rgb32Uint   => UInt3,
        f::Rgb32Int    => Int3,
        f::Rgb32Float  => Float3,
        f::Rgba32Uint  => UInt4,
        f::Rgba32Int   => Int4,
        f::Rgba32Float => Float4,
        _ => return None,
    })
}

pub fn resource_options_from_storage_and_cache(storage: MTLStorageMode, cache: MTLCPUCacheMode) -> MTLResourceOptions {
    MTLResourceOptions::from_bits(
        ((storage as u64) << MTLResourceStorageModeShift) | ((cache as u64) << MTLResourceCPUCacheModeShift)
    ).unwrap()
}

pub fn map_texture_usage(usage: image::Usage, tiling: image::Tiling) -> MTLTextureUsage {
    use hal::image::Usage as U;

    let mut texture_usage = MTLTextureUsage::PixelFormatView;
    if usage.intersects(U::COLOR_ATTACHMENT | U::DEPTH_STENCIL_ATTACHMENT) {
        texture_usage |= MTLTextureUsage::RenderTarget;
    }
    if usage.intersects(U::SAMPLED) {
        texture_usage |= MTLTextureUsage::ShaderRead;
    }
    if usage.intersects(U::STORAGE) {
        texture_usage |= MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite;
    }

    match tiling {
        image::Tiling::Optimal => {
            // Note: for blitting, we do actual rendering, so we add more flags for TRANSFER_* usage
            if usage.contains(U::TRANSFER_DST) {
                texture_usage |= MTLTextureUsage::RenderTarget;
            }
            if usage.contains(U::TRANSFER_SRC) {
                texture_usage |= MTLTextureUsage::ShaderRead;
            }
        }
        image::Tiling::Linear => {}
    }

    texture_usage
}

pub fn map_texture_type(view_kind: image::ViewKind) -> MTLTextureType {
    use hal::image::ViewKind as Vk;
    match view_kind {
        Vk::D1 => MTLTextureType::D1,
        Vk::D1Array => MTLTextureType::D1Array,
        Vk::D2 => MTLTextureType::D2,
        Vk::D2Array => MTLTextureType::D2Array,
        Vk::D3 => MTLTextureType::D3,
        Vk::Cube => MTLTextureType::Cube,
        Vk::CubeArray => MTLTextureType::CubeArray,
    }
}

pub fn map_index_type(index_type: IndexType) -> MTLIndexType {
    match index_type {
        IndexType::U16 => MTLIndexType::UInt16,
        IndexType::U32 => MTLIndexType::UInt32,
    }
}

pub fn map_compare_function(fun: Comparison) -> MTLCompareFunction {
    match fun {
        Comparison::Never => MTLCompareFunction::Never,
        Comparison::Less => MTLCompareFunction::Less,
        Comparison::LessEqual => MTLCompareFunction::LessEqual,
        Comparison::Equal => MTLCompareFunction::Equal,
        Comparison::GreaterEqual => MTLCompareFunction::GreaterEqual,
        Comparison::Greater => MTLCompareFunction::Greater,
        Comparison::NotEqual => MTLCompareFunction::NotEqual,
        Comparison::Always => MTLCompareFunction::Always,
    }
}

pub fn map_filter(filter: image::Filter) -> MTLSamplerMinMagFilter {
    match filter {
        image::Filter::Nearest => MTLSamplerMinMagFilter::Nearest,
        image::Filter::Linear => MTLSamplerMinMagFilter::Linear,
    }
}

pub fn map_wrap_mode(wrap: image::WrapMode) -> MTLSamplerAddressMode {
    match wrap {
        image::WrapMode::Tile => MTLSamplerAddressMode::Repeat,
        image::WrapMode::Mirror => MTLSamplerAddressMode::MirrorRepeat,
        image::WrapMode::Clamp => MTLSamplerAddressMode::ClampToEdge,
        image::WrapMode::Border => MTLSamplerAddressMode::ClampToBorderColor,
    }
}

pub fn map_extent(extent: image::Extent) -> MTLSize {
    MTLSize {
        width: extent.width as _,
        height: extent.height as _,
        depth: extent.depth as _,
    }
}

pub fn map_offset(offset: image::Offset) -> MTLOrigin {
    MTLOrigin {
        x: offset.x as _,
        y: offset.y as _,
        z: offset.z as _,
    }
}

pub fn map_stencil_op(op: StencilOp) -> MTLStencilOperation {
    match op {
        StencilOp::Keep => MTLStencilOperation::Keep,
        StencilOp::Zero => MTLStencilOperation::Zero,
        StencilOp::Replace => MTLStencilOperation::Replace,
        StencilOp::IncrementClamp => MTLStencilOperation::IncrementClamp,
        StencilOp::IncrementWrap => MTLStencilOperation::IncrementWrap,
        StencilOp::DecrementClamp => MTLStencilOperation::DecrementClamp,
        StencilOp::DecrementWrap => MTLStencilOperation::DecrementWrap,
        StencilOp::Invert => MTLStencilOperation::Invert,
    }
}
