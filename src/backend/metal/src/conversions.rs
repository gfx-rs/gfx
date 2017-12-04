use hal::{pass, image, memory, pso};
use hal::format::Format;
use metal::*;

// The boolean indicates whether this is a depth format
pub fn map_format(format: Format) -> Option<(MTLPixelFormat, bool)> {
    use hal::format::SurfaceType::*;
    use hal::format::ChannelType::*;

    // TODO: more formats
    match format {
        Format(R8_G8_B8_A8, Unorm) => Some((MTLPixelFormat::RGBA8Unorm, false)),
        Format(R8_G8_B8_A8, Srgb) => Some((MTLPixelFormat::RGBA8Unorm_sRGB, false)),
        Format(B8_G8_R8_A8, Unorm) => Some((MTLPixelFormat::BGRA8Unorm, false)),
        Format(B8_G8_R8_A8, Srgb) => Some((MTLPixelFormat::BGRA8Unorm_sRGB, false)),
        Format(D32, Float) => Some((MTLPixelFormat::Depth32Float, true)),
        Format(D24_S8, Unorm) => Some((MTLPixelFormat::Depth24Unorm_Stencil8, true)),
        _ => None,
    }
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
    use hal::format::SurfaceType::*;
    use hal::format::ChannelType::*;

    // TODO: more formats
    Some(match format {
        Format(R32_G32_B32_A32, Float) => MTLVertexFormat::Float4,
        Format(R32_G32_B32, Float) => MTLVertexFormat::Float3,
        Format(R32_G32, Float) => MTLVertexFormat::Float2,
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
    // TODO shader write
    texture_usage
}
