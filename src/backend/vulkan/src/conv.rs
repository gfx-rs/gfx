use ash::vk;
use byteorder::{NativeEndian, WriteBytesExt};
use smallvec::SmallVec;

use hal::{buffer, command, format, image, pass, pso, query};
use hal::{IndexType, Primitive};
use hal::range::RangeArg;

use native as n;

use std::{io, mem};
use std::borrow::Borrow;
use std::ptr;


pub fn map_format(format: format::Format) -> vk::Format {
    // Safe due to equivalence of HAL format values and Vulkan format values
    unsafe { mem::transmute(format) }
}

pub fn map_vk_format(format: vk::Format) -> Option<format::Format> {
    if (format as usize) < format::NUM_FORMATS &&
        format != vk::Format::Undefined
    {
        // Safe due to equivalence of HAL format values and Vulkan format values
        Some(unsafe { mem::transmute(format) })
    } else {
        None
    }
}

pub fn map_tiling(tiling: image::Tiling) -> vk::ImageTiling {
    unsafe { mem::transmute(tiling) }
}

pub fn map_component(component: format::Component) -> vk::ComponentSwizzle {
    use hal::format::Component::*;
    match component {
        Zero => vk::ComponentSwizzle::Zero,
        One  => vk::ComponentSwizzle::One,
        R    => vk::ComponentSwizzle::R,
        G    => vk::ComponentSwizzle::G,
        B    => vk::ComponentSwizzle::B,
        A    => vk::ComponentSwizzle::A,
    }
}

pub fn map_swizzle(swizzle: format::Swizzle) -> vk::ComponentMapping {
    vk::ComponentMapping {
        r: map_component(swizzle.0),
        g: map_component(swizzle.1),
        b: map_component(swizzle.2),
        a: map_component(swizzle.3),
    }
}

pub fn map_index_type(index_type: IndexType) -> vk::IndexType {
    match index_type {
        IndexType::U16 => vk::IndexType::Uint16,
        IndexType::U32 => vk::IndexType::Uint32,
    }
}

pub fn map_image_layout(layout: image::Layout) -> vk::ImageLayout {
    use hal::image::Layout as Il;
    match layout {
        Il::General => vk::ImageLayout::General,
        Il::ColorAttachmentOptimal => vk::ImageLayout::ColorAttachmentOptimal,
        Il::DepthStencilAttachmentOptimal => vk::ImageLayout::DepthStencilAttachmentOptimal,
        Il::DepthStencilReadOnlyOptimal => vk::ImageLayout::DepthStencilReadOnlyOptimal,
        Il::ShaderReadOnlyOptimal => vk::ImageLayout::ShaderReadOnlyOptimal,
        Il::TransferSrcOptimal => vk::ImageLayout::TransferSrcOptimal,
        Il::TransferDstOptimal => vk::ImageLayout::TransferDstOptimal,
        Il::Undefined => vk::ImageLayout::Undefined,
        Il::Preinitialized => vk::ImageLayout::Preinitialized,
        Il::Present => vk::ImageLayout::PresentSrcKhr,
    }
}

pub fn map_image_aspects(aspects: format::Aspects) -> vk::ImageAspectFlags {
    // Safe due to equivalence of HAL format values and Vulkan format values
    unsafe { mem::transmute(aspects.bits() as u32) }
}

pub fn map_clear_color(value: command::ClearColor) -> vk::ClearColorValue {
    match value {
        command::ClearColor::Float(v) => vk::ClearColorValue { float32: v },
        command::ClearColor::Int(v)   => vk::ClearColorValue { int32: v },
        command::ClearColor::Uint(v)  => vk::ClearColorValue { uint32: v },
    }
}

pub fn map_clear_depth_stencil(value: command::ClearDepthStencil) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth: value.0,
        stencil: value.1,
    }
}

pub fn map_clear_depth(depth: pso::DepthValue) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth,
        stencil: 0,
    }
}

pub fn map_clear_stencil(stencil: pso::StencilValue) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth: 0.0,
        stencil,
    }
}

pub fn map_offset(offset: image::Offset) -> vk::Offset3D {
    vk::Offset3D {
        x: offset.x,
        y: offset.y,
        z: offset.z,
    }
}

pub fn map_extent(offset: image::Extent) -> vk::Extent3D {
    vk::Extent3D {
        width: offset.width,
        height: offset.height,
        depth: offset.depth,
    }
}

pub fn map_subresource_layers(
    sub: &image::SubresourceLayers,
) -> vk::ImageSubresourceLayers {
    let layer_start = sub.layers.0;
    let layer_count = sub.layers.1.map_or(vk::VK_REMAINING_ARRAY_LAYERS, |end| (end - layer_start) as u32);
    vk::ImageSubresourceLayers {
        aspect_mask: map_image_aspects(sub.aspects),
        mip_level: sub.level as _,
        base_array_layer: layer_start as _,
        layer_count,
    }
}

pub fn map_subresource_range(
    range: &image::SubresourceRange,
) -> vk::ImageSubresourceRange {
    let level_start = range.levels.0;
    let level_count = range.levels.1.map_or(vk::VK_REMAINING_MIP_LEVELS, |end| (end - level_start) as u32);
    let layer_start = range.layers.0;
    let layer_count = range.layers.1.map_or(vk::VK_REMAINING_ARRAY_LAYERS, |end| (end - layer_start) as u32);
    vk::ImageSubresourceRange {
        aspect_mask: map_image_aspects(range.aspects),
        base_mip_level: level_start as _,
        level_count,
        base_array_layer: layer_start as _,
        layer_count,
    }
}

pub fn map_attachment_load_op(op: pass::AttachmentLoadOp) -> vk::AttachmentLoadOp {
    use hal::pass::AttachmentLoadOp as Alo;
    match op {
        Alo::Load => vk::AttachmentLoadOp::Load,
        Alo::Clear => vk::AttachmentLoadOp::Clear,
        Alo::DontCare => vk::AttachmentLoadOp::DontCare,
    }
}

pub fn map_attachment_store_op(op: pass::AttachmentStoreOp) -> vk::AttachmentStoreOp {
    use hal::pass::AttachmentStoreOp as Aso;
    match op {
        Aso::Store => vk::AttachmentStoreOp::Store,
        Aso::DontCare => vk::AttachmentStoreOp::DontCare,
    }
}

pub fn map_buffer_access(access: buffer::Access) -> vk::AccessFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(access) }
}

pub fn map_image_access(access: image::Access) -> vk::AccessFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(access) }
}

pub fn map_pipeline_stage(stage: pso::PipelineStage) -> vk::PipelineStageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(stage) }
}

pub fn map_buffer_usage(usage: buffer::Usage) -> vk::BufferUsageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(usage) }
}

pub fn map_image_usage(usage: image::Usage) -> vk::ImageUsageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(usage) }
}

pub fn map_descriptor_type(ty: pso::DescriptorType) -> vk::DescriptorType {
    // enums have to match exactly
    unsafe { mem::transmute(ty) }
}

pub fn map_stage_flags(stages: pso::ShaderStageFlags) -> vk::ShaderStageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(stages) }
}


pub fn map_filter(filter: image::Filter) -> vk::Filter {
    // enums have to match exactly
    unsafe { mem::transmute(filter as u32) }
}

pub fn map_mip_filter(filter: image::Filter) -> vk::SamplerMipmapMode {
    // enums have to match exactly
    unsafe { mem::transmute(filter as u32) }
}

pub fn map_wrap(wrap: image::WrapMode) -> vk::SamplerAddressMode {
    use hal::image::WrapMode as Wm;
    match wrap {
        Wm::Tile   => vk::SamplerAddressMode::Repeat,
        Wm::Mirror => vk::SamplerAddressMode::MirroredRepeat,
        Wm::Clamp  => vk::SamplerAddressMode::ClampToEdge,
        Wm::Border => vk::SamplerAddressMode::ClampToBorder,
    }
}

pub fn map_border_color(col: image::PackedColor) -> Option<vk::BorderColor> {
    match col.0 {
        0x00000000 => Some(vk::BorderColor::FloatTransparentBlack),
        0xFF000000 => Some(vk::BorderColor::FloatOpaqueBlack),
        0xFFFFFFFF => Some(vk::BorderColor::FloatOpaqueWhite),
        _ => None
    }
}

pub fn map_topology(prim: Primitive) -> vk::PrimitiveTopology {
    match prim {
        Primitive::PointList              => vk::PrimitiveTopology::PointList,
        Primitive::LineList               => vk::PrimitiveTopology::LineList,
        Primitive::LineListAdjacency      => vk::PrimitiveTopology::LineListWithAdjacency,
        Primitive::LineStrip              => vk::PrimitiveTopology::LineStrip,
        Primitive::LineStripAdjacency     => vk::PrimitiveTopology::LineStripWithAdjacency,
        Primitive::TriangleList           => vk::PrimitiveTopology::TriangleList,
        Primitive::TriangleListAdjacency  => vk::PrimitiveTopology::TriangleListWithAdjacency,
        Primitive::TriangleStrip          => vk::PrimitiveTopology::TriangleStrip,
        Primitive::TriangleStripAdjacency => vk::PrimitiveTopology::TriangleStripWithAdjacency,
        Primitive::PatchList(_)           => vk::PrimitiveTopology::PatchList,
    }
}

pub fn map_polygon_mode(rm: pso::PolygonMode) -> (vk::PolygonMode, f32) {
    match rm {
        pso::PolygonMode::Point   => (vk::PolygonMode::Point, 1.0),
        pso::PolygonMode::Line(w) => (vk::PolygonMode::Line, w),
        pso::PolygonMode::Fill    => (vk::PolygonMode::Fill, 1.0),
    }
}

pub fn map_cull_face(cf: pso::CullFace) -> vk::CullModeFlags {
    match cf {
        pso::CullFace::Front   => vk::CULL_MODE_FRONT_BIT,
        pso::CullFace::Back    => vk::CULL_MODE_BACK_BIT,
    }
}

pub fn map_front_face(ff: pso::FrontFace) -> vk::FrontFace {
    match ff {
        pso::FrontFace::Clockwise        => vk::FrontFace::Clockwise,
        pso::FrontFace::CounterClockwise => vk::FrontFace::CounterClockwise,
    }
}

pub fn map_comparison(fun: pso::Comparison) -> vk::CompareOp {
    use hal::pso::Comparison::*;
    match fun {
        Never        => vk::CompareOp::Never,
        Less         => vk::CompareOp::Less,
        LessEqual    => vk::CompareOp::LessOrEqual,
        Equal        => vk::CompareOp::Equal,
        GreaterEqual => vk::CompareOp::GreaterOrEqual,
        Greater      => vk::CompareOp::Greater,
        NotEqual     => vk::CompareOp::NotEqual,
        Always       => vk::CompareOp::Always,
    }
}

pub fn map_stencil_op(op: pso::StencilOp) -> vk::StencilOp {
    use hal::pso::StencilOp::*;
    match op {
        Keep           => vk::StencilOp::Keep,
        Zero           => vk::StencilOp::Zero,
        Replace        => vk::StencilOp::Replace,
        IncrementClamp => vk::StencilOp::IncrementAndClamp,
        IncrementWrap  => vk::StencilOp::IncrementAndWrap,
        DecrementClamp => vk::StencilOp::DecrementAndClamp,
        DecrementWrap  => vk::StencilOp::DecrementAndWrap,
        Invert         => vk::StencilOp::Invert,
    }
}

pub fn map_stencil_side(side: &pso::StencilFace) -> vk::StencilOpState {
    vk::StencilOpState {
        fail_op: map_stencil_op(side.op_fail),
        pass_op: map_stencil_op(side.op_pass),
        depth_fail_op: map_stencil_op(side.op_depth_fail),
        compare_op: map_comparison(side.fun),
        compare_mask: side.mask_read as u32,
        write_mask: side.mask_write as u32,
        reference: 0,
    }
}

pub fn map_blend_factor(factor: pso::Factor) -> vk::BlendFactor {
    use hal::pso::Factor::*;
    match factor {
        Zero => vk::BlendFactor::Zero,
        One => vk::BlendFactor::One,
        SrcColor => vk::BlendFactor::SrcColor,
        OneMinusSrcColor => vk::BlendFactor::OneMinusSrcColor,
        DstColor => vk::BlendFactor::DstColor,
        OneMinusDstColor => vk::BlendFactor::OneMinusDstColor,
        SrcAlpha => vk::BlendFactor::SrcAlpha,
        OneMinusSrcAlpha => vk::BlendFactor::OneMinusSrcAlpha,
        DstAlpha => vk::BlendFactor::DstAlpha,
        OneMinusDstAlpha => vk::BlendFactor::OneMinusDstAlpha,
        ConstColor => vk::BlendFactor::ConstantColor,
        OneMinusConstColor => vk::BlendFactor::OneMinusConstantColor,
        ConstAlpha => vk::BlendFactor::ConstantAlpha,
        OneMinusConstAlpha => vk::BlendFactor::OneMinusConstantAlpha,
        SrcAlphaSaturate => vk::BlendFactor::SrcAlphaSaturate,
        Src1Color => vk::BlendFactor::Src1Color,
        OneMinusSrc1Color => vk::BlendFactor::OneMinusSrc1Color,
        Src1Alpha => vk::BlendFactor::Src1Alpha,
        OneMinusSrc1Alpha => vk::BlendFactor::OneMinusSrc1Alpha,
    }
}

pub fn map_blend_op(
    operation: pso::BlendOp
) -> (vk::BlendOp, vk::BlendFactor, vk::BlendFactor) {
    use hal::pso::BlendOp::*;
    match operation {
        Add { src, dst } => (
            vk::BlendOp::Add,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        Sub { src, dst } => (
            vk::BlendOp::Subtract,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        RevSub { src, dst } => (
            vk::BlendOp::ReverseSubtract,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        Min => (vk::BlendOp::Min, vk::BlendFactor::Zero, vk::BlendFactor::Zero),
        Max => (vk::BlendOp::Max, vk::BlendFactor::Zero, vk::BlendFactor::Zero),
    }
}

pub fn map_specialization_constants(
    specialization: &[pso::Specialization],
    data: &mut SmallVec<[u8; 64]>,
) -> Result<SmallVec<[vk::SpecializationMapEntry; 16]>, io::Error> {
    specialization
        .iter()
        .map(|constant| {
            let offset = data.len();
            match constant.value {
                pso::Constant::Bool(v) => { data.write_u32::<NativeEndian>(v as u32) }
                pso::Constant::U32(v)  => { data.write_u32::<NativeEndian>(v) }
                pso::Constant::U64(v)  => { data.write_u64::<NativeEndian>(v) }
                pso::Constant::I32(v)  => { data.write_i32::<NativeEndian>(v) }
                pso::Constant::I64(v)  => { data.write_i64::<NativeEndian>(v) }
                pso::Constant::F32(v)  => { data.write_f32::<NativeEndian>(v) }
                pso::Constant::F64(v)  => { data.write_f64::<NativeEndian>(v) }
            }?;

            Ok(vk::SpecializationMapEntry {
                constant_id: constant.id,
                offset: offset as _,
                size: (data.len() - offset) as _,
            })
        })
        .collect::<Result<_, _>>()
}

pub fn map_pipeline_statistics(
    statistics: query::PipelineStatistic,
) -> vk::QueryPipelineStatisticFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(statistics) }
}

pub fn map_query_control_flags(flags: query::QueryControl) -> vk::QueryControlFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(flags) }
}

pub fn map_image_features(features: vk::FormatFeatureFlags) -> format::ImageFeature {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(features) }
}

pub fn map_buffer_features(features: vk::FormatFeatureFlags) -> format::BufferFeature {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(features) }
}

pub fn map_memory_ranges<'a, I, R>(ranges: I) -> Vec<vk::MappedMemoryRange>
where
    I: IntoIterator,
    I::Item: Borrow<(&'a n::Memory, R)>,
    R: RangeArg<u64>,
{
     ranges
        .into_iter()
        .map(|range| {
            let &(ref memory, ref range) = range.borrow();
            let (offset, size) = map_range_arg(range);
            vk::MappedMemoryRange {
                s_type: vk::StructureType::MappedMemoryRange,
                p_next: ptr::null(),
                memory: memory.raw,
                offset,
                size,
            }
        })
        .collect()
}

/// Returns (offset, size) of the range.
///
/// Unbound start indices will be mapped to 0.
/// Unbound end indices will be mapped to VK_WHOLE_SIZE.
pub fn map_range_arg<R>(range: &R) -> (u64, u64)
where
    R: RangeArg<u64>,
{
    let offset = *range.start().unwrap_or(&0);
    let size = match range.end() {
        Some(end) => end - offset,
        None => vk::VK_WHOLE_SIZE,
    };

    (offset, size)
}

pub fn map_command_buffer_flags(flags: command::CommandBufferFlags) -> vk::CommandBufferUsageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(flags) }
}

pub fn map_command_buffer_level(level: command::RawLevel) -> vk::CommandBufferLevel {
    match level {
        command::RawLevel::Primary => vk::CommandBufferLevel::Primary,
        command::RawLevel::Secondary => vk::CommandBufferLevel::Secondary,
    }
}

pub fn map_view_kind(
    kind: image::ViewKind, ty: vk::ImageType, is_cube: bool
) -> Option<vk::ImageViewType> {
    use vk::ImageType::*;
    use hal::image::ViewKind::*;

    Some(match (ty, kind) {
        (Type1d, D1) => vk::ImageViewType::Type1d,
        (Type1d, D1Array) => vk::ImageViewType::Type1dArray,
        (Type2d, D2) => vk::ImageViewType::Type2d,
        (Type2d, D2Array) => vk::ImageViewType::Type2dArray,
        (Type3d, D3) => vk::ImageViewType::Type3d,
        (Type2d, Cube) if is_cube => vk::ImageViewType::Cube,
        (Type2d, CubeArray) if is_cube => vk::ImageViewType::CubeArray,
        (Type3d, Cube) if is_cube => vk::ImageViewType::Cube,
        (Type3d, CubeArray) if is_cube => vk::ImageViewType::CubeArray,
        _ => return None
    })
}

pub fn map_rect(rect: &pso::Rect) -> vk::Rect2D {
    vk::Rect2D {
        offset: vk::Offset2D {
            x: rect.x as _,
            y: rect.y as _,
        },
        extent: vk::Extent2D {
            width: rect.w as _,
            height: rect.h as _,
        },
    }
}

pub fn map_viewport(vp: &pso::Viewport) -> vk::Viewport {
    vk::Viewport {
        x: vp.rect.x as _,
        y: vp.rect.y as _,
        width: vp.rect.w as _,
        height: vp.rect.h as _,
        min_depth: vp.depth.start,
        max_depth: vp.depth.end,
    }
}

pub fn map_image_flags(flags: image::StorageFlags) -> vk::ImageCreateFlags {
    // the flag values have to match Vulkan
    unsafe { mem::transmute(flags) }
}
