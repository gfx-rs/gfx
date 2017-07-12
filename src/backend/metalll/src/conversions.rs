use core;
use core::pass;
use core::format::Format;
use metal::*;

// The boolean indicates whether this is a depth format
pub fn map_format(format: Format) -> Option<(MTLPixelFormat, bool)> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;

    // TODO: more formats
    match format {
        Format(R8_G8_B8_A8, Unorm) => Some((MTLPixelFormat::RGBA8Unorm, false)),
        Format(R8_G8_B8_A8, Srgb) => Some((MTLPixelFormat::RGBA8Unorm_sRGB, false)),
        Format(B8_G8_R8_A8, Unorm) => Some((MTLPixelFormat::BGRA8Unorm, false)),
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

pub fn map_write_mask(mask: core::state::ColorMask) -> MTLColorWriteMask {
    use core::state;

    let mut mtl_mask = MTLColorWriteMaskNone;

    if mask.contains(state::RED) {
        mtl_mask |= MTLColorWriteMaskRed;
    }
    if mask.contains(state::GREEN) {
        mtl_mask |= MTLColorWriteMaskGreen;
    }
    if mask.contains(state::BLUE) {
        mtl_mask |= MTLColorWriteMaskBlue;
    }
    if mask.contains(state::ALPHA) {
        mtl_mask |= MTLColorWriteMaskAlpha;
    }

    mtl_mask
}

pub fn map_blend_op(equation: core::state::Equation) -> MTLBlendOperation {
    use core::state::Equation::*;

    match equation {
        Add => MTLBlendOperation::Add,
        Sub => MTLBlendOperation::Subtract,
        RevSub => MTLBlendOperation::ReverseSubtract,
        Min => MTLBlendOperation::Min,
        Max => MTLBlendOperation::Max,
    }
}

pub fn map_blend_factor(factor: core::state::Factor, scalar: bool) -> MTLBlendFactor {
    use core::state::BlendValue::*;
    use core::state::Factor::*;

    match factor {
        Zero => MTLBlendFactor::Zero,
        One => MTLBlendFactor::One,
        SourceAlphaSaturated => MTLBlendFactor::SourceAlphaSaturated,
        ZeroPlus(SourceColor) if !scalar => MTLBlendFactor::SourceColor,
        ZeroPlus(SourceAlpha) => MTLBlendFactor::SourceAlpha,
        ZeroPlus(DestColor) if !scalar => MTLBlendFactor::DestinationColor,
        ZeroPlus(DestAlpha) => MTLBlendFactor::DestinationAlpha,
        ZeroPlus(ConstColor) if !scalar => MTLBlendFactor::BlendColor,
        ZeroPlus(ConstAlpha) => MTLBlendFactor::BlendAlpha,
        OneMinus(SourceColor) if !scalar => MTLBlendFactor::OneMinusSourceColor,
        OneMinus(SourceAlpha) => MTLBlendFactor::OneMinusSourceAlpha,
        OneMinus(DestColor) if !scalar => MTLBlendFactor::OneMinusDestinationColor,
        OneMinus(DestAlpha) => MTLBlendFactor::OneMinusDestinationAlpha,
        OneMinus(ConstColor) if !scalar => MTLBlendFactor::OneMinusBlendColor,
        OneMinus(ConstAlpha) => MTLBlendFactor::OneMinusBlendAlpha,
        _ => {
            error!("Invalid blend factor requested for {}: {:?}",
                if scalar {"alpha"} else {"color"}, factor);
            MTLBlendFactor::Zero
        }
    }
}


pub fn map_vertex_format(format: Format) -> Option<MTLVertexFormat> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
   
    // TODO: more formats
    Some(match format {
        Format(R32_G32_B32_A32, Float) => MTLVertexFormat::Float4,
        Format(R32_G32_B32, Float) => MTLVertexFormat::Float3,
        Format(R32_G32, Float) => MTLVertexFormat::Float2,
        _ => return None,
    })
}
