use hal::{pass, image, memory, pso, IndexType};
use hal::format::Format;
use hal::pso::Comparison;
use metal::*;

// The boolean indicates whether this is a depth format
pub fn map_format(format: Format) -> Option<(MTLPixelFormat, bool)> {
    Some(match format {
        Format::Rgba8Unorm => (MTLPixelFormat::RGBA8Unorm, false),
        Format::Rgba8Srgb => (MTLPixelFormat::RGBA8Unorm_sRGB, false),
        Format::Bgra8Unorm => (MTLPixelFormat::BGRA8Unorm, false),
        Format::Bgra8Srgb => (MTLPixelFormat::BGRA8Unorm_sRGB, false),
        Format::Rgba32Float => (MTLPixelFormat::RGBA32Float, false),
        Format::D32Float => (MTLPixelFormat::Depth32Float, true),
        Format::D24UnormS8Uint => (MTLPixelFormat::Depth24Unorm_Stencil8, true),
        Format::D32FloatS8Uint => (MTLPixelFormat::Depth32Float_Stencil8, true),
        _ => return None,
    })
}

pub fn get_format_bytes_per_pixel(format: MTLPixelFormat) -> usize {
    // TODO: more formats
    match format {
        MTLPixelFormat::RGBA8Unorm => 4,
        MTLPixelFormat::RGBA8Unorm_sRGB => 4,
        MTLPixelFormat::BGRA8Unorm => 4,
        _ => unimplemented!(),
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
    let mut mtl_mask = MTLColorWriteMask::MTLColorWriteMaskNone;

    if mask.contains(pso::ColorMask::RED) {
        mtl_mask |= MTLColorWriteMask::MTLColorWriteMaskRed;
    }
    if mask.contains(pso::ColorMask::GREEN) {
        mtl_mask |= MTLColorWriteMask::MTLColorWriteMaskGreen;
    }
    if mask.contains(pso::ColorMask::BLUE) {
        mtl_mask |= MTLColorWriteMask::MTLColorWriteMaskBlue;
    }
    if mask.contains(pso::ColorMask::ALPHA) {
        mtl_mask |= MTLColorWriteMask::MTLColorWriteMaskAlpha;
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
    // TODO: more formats
    Some(match format {
        Format::Rgba32Float => MTLVertexFormat::Float4,
        Format::Rgb32Float => MTLVertexFormat::Float3,
        Format::Rg32Float => MTLVertexFormat::Float2,
        _ => return None,
    })
}

pub fn map_memory_properties_to_options(properties: memory::Properties) -> MTLResourceOptions {
    let mut options = MTLResourceOptions::empty();
    if properties.contains(memory::Properties::CPU_VISIBLE) {
        if properties.contains(memory::Properties::COHERENT) {
            options |= MTLResourceOptions::StorageModeShared;
        } else {
            options |= MTLResourceOptions::StorageModeManaged;
        }
    } else if properties.contains(memory::Properties::DEVICE_LOCAL) {
        options |= MTLResourceOptions::StorageModePrivate;
    } else {
        panic!("invalid heap properties");
    }
    if !properties.contains(memory::Properties::CPU_CACHED) {
        options |= MTLResourceOptions::CPUCacheModeWriteCombined;
    }
    options
}

pub fn map_memory_properties_to_storage_and_cache(properties: memory::Properties) -> (MTLStorageMode, MTLCPUCacheMode) {
    let storage = if properties.contains(memory::Properties::CPU_VISIBLE) {
        if properties.contains(memory::Properties::COHERENT) {
            MTLStorageMode::Shared
        } else {
            MTLStorageMode::Managed
        }
    } else if properties.contains(memory::Properties::DEVICE_LOCAL) {
        MTLStorageMode::Private
    } else {
        panic!("invalid heap properties");
    };
    let cpu = if properties.contains(memory::Properties::CPU_CACHED) {
        MTLCPUCacheMode::DefaultCache
    } else {
        MTLCPUCacheMode::WriteCombined
    };
    (storage, cpu)
}

pub fn resource_options_from_storage_and_cache(storage: MTLStorageMode, cache: MTLCPUCacheMode) -> MTLResourceOptions {
    MTLResourceOptions::from_bits(
        ((storage as u64) << MTLResourceStorageModeShift) | ((cache as u64) << MTLResourceCPUCacheModeShift)
    ).unwrap()
}

pub fn map_texture_usage(usage: image::Usage) -> MTLTextureUsage {
    let mut texture_usage = MTLTextureUsage::MTLTextureUsagePixelFormatView;
    if usage.contains(image::Usage::COLOR_ATTACHMENT) || usage.contains(image::Usage::DEPTH_STENCIL_ATTACHMENT) {
        texture_usage |= MTLTextureUsage::MTLTextureUsageRenderTarget;
    }
    if usage.contains(image::Usage::SAMPLED) {
        texture_usage |= MTLTextureUsage::MTLTextureUsageShaderRead;
    }
    if usage.contains(image::Usage::STORAGE) {
        texture_usage |= MTLTextureUsage::MTLTextureUsageShaderRead | MTLTextureUsage::MTLTextureUsageShaderWrite;
    }
    // TODO shader write
    texture_usage
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
