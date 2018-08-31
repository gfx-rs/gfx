use ash::vk;
use byteorder::{NativeEndian, WriteBytesExt};
use smallvec::SmallVec;

use hal::{buffer, command, format, image, pass, pso, query};
use hal::{IndexType, Primitive, PresentMode};
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
    if (format.as_raw() as usize) < format::NUM_FORMATS &&
        format != vk::Format::UNDEFINED
    {
        // Safe due to equivalence of HAL format values and Vulkan format values
        Some(unsafe { mem::transmute(format.as_raw()) })
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
        Zero => vk::ComponentSwizzle::ZERO,
        One  => vk::ComponentSwizzle::ONE,
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
        IndexType::U16 => vk::IndexType::UINT16,
        IndexType::U32 => vk::IndexType::UINT32,
    }
}

pub fn map_image_layout(layout: image::Layout) -> vk::ImageLayout {
    use hal::image::Layout as Il;
    match layout {
        Il::General => vk::ImageLayout::GENERAL,
        Il::ColorAttachmentOptimal => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        Il::DepthStencilAttachmentOptimal => vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        Il::DepthStencilReadOnlyOptimal => vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        Il::ShaderReadOnlyOptimal => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        Il::TransferSrcOptimal => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        Il::TransferDstOptimal => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        Il::Undefined => vk::ImageLayout::UNDEFINED,
        Il::Preinitialized => vk::ImageLayout::PREINITIALIZED,
        Il::Present => vk::ImageLayout::PRESENT_SRC_KHR,
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

pub fn map_subresource(
    sub: &image::Subresource,
) -> vk::ImageSubresource {
    vk::ImageSubresource {
        aspect_mask: map_image_aspects(sub.aspects),
        mip_level: sub.level as _,
        array_layer: sub.layer as _,
    }
}

pub fn map_subresource_layers(
    sub: &image::SubresourceLayers,
) -> vk::ImageSubresourceLayers {
    vk::ImageSubresourceLayers {
        aspect_mask: map_image_aspects(sub.aspects),
        mip_level: sub.level as _,
        base_array_layer: sub.layers.start as _,
        layer_count: (sub.layers.end - sub.layers.start) as _,
    }
}

pub fn map_subresource_range(
    range: &image::SubresourceRange,
) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: map_image_aspects(range.aspects),
        base_mip_level: range.levels.start as _,
        level_count: (range.levels.end - range.levels.start) as _,
        base_array_layer: range.layers.start as _,
        layer_count: (range.layers.end - range.layers.start) as _,
    }
}

pub fn map_attachment_load_op(op: pass::AttachmentLoadOp) -> vk::AttachmentLoadOp {
    use hal::pass::AttachmentLoadOp as Alo;
    match op {
        Alo::Load => vk::AttachmentLoadOp::LOAD,
        Alo::Clear => vk::AttachmentLoadOp::CLEAR,
        Alo::DontCare => vk::AttachmentLoadOp::DONT_CARE,
    }
}

pub fn map_attachment_store_op(op: pass::AttachmentStoreOp) -> vk::AttachmentStoreOp {
    use hal::pass::AttachmentStoreOp as Aso;
    match op {
        Aso::Store => vk::AttachmentStoreOp::STORE,
        Aso::DontCare => vk::AttachmentStoreOp::DONT_CARE,
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

pub fn map_vk_image_usage(usage: vk::ImageUsageFlags) -> image::Usage {
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
        Wm::Tile   => vk::SamplerAddressMode::REPEAT,
        Wm::Mirror => vk::SamplerAddressMode::MIRRORED_REPEAT,
        Wm::Clamp  => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        Wm::Border => vk::SamplerAddressMode::CLAMP_TO_BORDER,
    }
}

pub fn map_border_color(col: image::PackedColor) -> Option<vk::BorderColor> {
    match col.0 {
        0x00000000 => Some(vk::BorderColor::FLOAT_TRANSPARENT_BLACK),
        0xFF000000 => Some(vk::BorderColor::FLOAT_OPAQUE_BLACK),
        0xFFFFFFFF => Some(vk::BorderColor::FLOAT_OPAQUE_WHITE),
        _ => None
    }
}

pub fn map_topology(prim: Primitive) -> vk::PrimitiveTopology {
    match prim {
        Primitive::PointList              => vk::PrimitiveTopology::POINT_LIST,
        Primitive::LineList               => vk::PrimitiveTopology::LINE_LIST,
        Primitive::LineListAdjacency      => vk::PrimitiveTopology::LINE_LIST_WITH_ADJACENCY,
        Primitive::LineStrip              => vk::PrimitiveTopology::LINE_STRIP,
        Primitive::LineStripAdjacency     => vk::PrimitiveTopology::LINE_STRIP_WITH_ADJACENCY,
        Primitive::TriangleList           => vk::PrimitiveTopology::TRIANGLE_LIST,
        Primitive::TriangleListAdjacency  => vk::PrimitiveTopology::TRIANGLE_LIST_WITH_ADJACENCY,
        Primitive::TriangleStrip          => vk::PrimitiveTopology::TRIANGLE_STRIP,
        Primitive::TriangleStripAdjacency => vk::PrimitiveTopology::TRIANGLE_STRIP_WITH_ADJACENCY,
        Primitive::PatchList(_)           => vk::PrimitiveTopology::PATCH_LIST,
    }
}

pub fn map_polygon_mode(rm: pso::PolygonMode) -> (vk::PolygonMode, f32) {
    match rm {
        pso::PolygonMode::Point   => (vk::PolygonMode::POINT, 1.0),
        pso::PolygonMode::Line(w) => (vk::PolygonMode::LINE, w),
        pso::PolygonMode::Fill    => (vk::PolygonMode::FILL, 1.0),
    }
}

pub fn map_cull_face(cf: pso::Face) -> vk::CullModeFlags {
    match cf {
        pso::Face::NONE => vk::CullModeFlags::NONE,
        pso::Face::FRONT => vk::CullModeFlags::FRONT,
        pso::Face::BACK => vk::CullModeFlags::BACK,
        _ => vk::CullModeFlags::FRONT_AND_BACK,
    }
}

pub fn map_front_face(ff: pso::FrontFace) -> vk::FrontFace {
    match ff {
        pso::FrontFace::Clockwise        => vk::FrontFace::CLOCKWISE,
        pso::FrontFace::CounterClockwise => vk::FrontFace::COUNTER_CLOCKWISE,
    }
}

pub fn map_comparison(fun: pso::Comparison) -> vk::CompareOp {
    use hal::pso::Comparison::*;
    match fun {
        Never        => vk::CompareOp::NEVER,
        Less         => vk::CompareOp::LESS,
        LessEqual    => vk::CompareOp::LESS_OR_EQUAL,
        Equal        => vk::CompareOp::EQUAL,
        GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
        Greater      => vk::CompareOp::GREATER,
        NotEqual     => vk::CompareOp::NOT_EQUAL,
        Always       => vk::CompareOp::ALWAYS,
    }
}

pub fn map_stencil_op(op: pso::StencilOp) -> vk::StencilOp {
    use hal::pso::StencilOp::*;
    match op {
        Keep           => vk::StencilOp::KEEP,
        Zero           => vk::StencilOp::ZERO,
        Replace        => vk::StencilOp::REPLACE,
        IncrementClamp => vk::StencilOp::INCREMENT_AND_CLAMP,
        IncrementWrap  => vk::StencilOp::INCREMENT_AND_WRAP,
        DecrementClamp => vk::StencilOp::DECREMENT_AND_CLAMP,
        DecrementWrap  => vk::StencilOp::DECREMENT_AND_WRAP,
        Invert         => vk::StencilOp::INVERT,
    }
}

pub fn map_stencil_side(side: &pso::StencilFace) -> vk::StencilOpState {
    vk::StencilOpState {
        fail_op: map_stencil_op(side.op_fail),
        pass_op: map_stencil_op(side.op_pass),
        depth_fail_op: map_stencil_op(side.op_depth_fail),
        compare_op: map_comparison(side.fun),
        compare_mask: match side.mask_read {
            pso::State::Static(mr) => mr,
            pso::State::Dynamic => !0,
        },
        write_mask: match side.mask_write {
            pso::State::Static(mw) => mw,
            pso::State::Dynamic => !0,
        },
        reference: match side.reference {
            pso::State::Static(r) => r,
            pso::State::Dynamic => 0,
        },
    }
}

pub fn map_blend_factor(factor: pso::Factor) -> vk::BlendFactor {
    use hal::pso::Factor::*;
    match factor {
        Zero => vk::BlendFactor::ZERO,
        One => vk::BlendFactor::ONE,
        SrcColor => vk::BlendFactor::SRC_COLOR,
        OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
        DstColor => vk::BlendFactor::DST_COLOR,
        OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
        SrcAlpha => vk::BlendFactor::SRC_ALPHA,
        OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        DstAlpha => vk::BlendFactor::DST_ALPHA,
        OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        ConstColor => vk::BlendFactor::CONSTANT_COLOR,
        OneMinusConstColor => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
        ConstAlpha => vk::BlendFactor::CONSTANT_ALPHA,
        OneMinusConstAlpha => vk::BlendFactor::ONE_MINUS_CONSTANT_ALPHA,
        SrcAlphaSaturate => vk::BlendFactor::SRC_ALPHA_SATURATE,
        Src1Color => vk::BlendFactor::SRC1_COLOR,
        OneMinusSrc1Color => vk::BlendFactor::ONE_MINUS_SRC1_COLOR,
        Src1Alpha => vk::BlendFactor::SRC1_ALPHA,
        OneMinusSrc1Alpha => vk::BlendFactor::ONE_MINUS_SRC1_ALPHA,
    }
}

pub fn map_blend_op(
    operation: pso::BlendOp
) -> (vk::BlendOp, vk::BlendFactor, vk::BlendFactor) {
    use hal::pso::BlendOp::*;
    match operation {
        Add { src, dst } => (
            vk::BlendOp::ADD,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        Sub { src, dst } => (
            vk::BlendOp::SUBTRACT,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        RevSub { src, dst } => (
            vk::BlendOp::REVERSE_SUBTRACT,
            map_blend_factor(src),
            map_blend_factor(dst),
        ),
        Min => (vk::BlendOp::MIN, vk::BlendFactor::ZERO, vk::BlendFactor::ZERO),
        Max => (vk::BlendOp::MAX, vk::BlendFactor::ZERO, vk::BlendFactor::ZERO),
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

pub fn map_query_control_flags(flags: query::ControlFlags) -> vk::QueryControlFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    vk::QueryControlFlags::from_flags_truncate(flags.bits())
}

pub fn map_query_result_flags(flags: query::ResultFlags) -> vk::QueryResultFlags {
    vk::QueryResultFlags::from_flags_truncate(flags.bits())
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
                s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
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
/// Unbound end indices will be mapped to WHOLE_SIZE.
pub fn map_range_arg<R>(range: &R) -> (u64, u64)
where
    R: RangeArg<u64>,
{
    let offset = *range.start().unwrap_or(&0);
    let size = match range.end() {
        Some(end) => end - offset,
        None => vk::WHOLE_SIZE,
    };

    (offset, size)
}

pub fn map_command_buffer_flags(flags: command::CommandBufferFlags) -> vk::CommandBufferUsageFlags {
    // Safe due to equivalence of HAL values and Vulkan values
    unsafe { mem::transmute(flags) }
}

pub fn map_command_buffer_level(level: command::RawLevel) -> vk::CommandBufferLevel {
    match level {
        command::RawLevel::Primary => vk::CommandBufferLevel::PRIMARY,
        command::RawLevel::Secondary => vk::CommandBufferLevel::SECONDARY,
    }
}

pub fn map_view_kind(
    kind: image::ViewKind, ty: vk::ImageType, is_cube: bool
) -> Option<vk::ImageViewType> {
    
    use hal::image::ViewKind::*;

    Some(match (ty, kind) {
        (vk::ImageType::TYPE_1D, D1) => vk::ImageViewType::TYPE_1D,
        (vk::ImageType::TYPE_1D, D1Array) => vk::ImageViewType::TYPE_1D_ARRAY,
        (vk::ImageType::TYPE_2D, D2) => vk::ImageViewType::TYPE_2D,
        (vk::ImageType::TYPE_2D, D2Array) => vk::ImageViewType::TYPE_2D_ARRAY,
        (vk::ImageType::TYPE_3D, D3) => vk::ImageViewType::TYPE_3D,
        (vk::ImageType::TYPE_2D, Cube) if is_cube => vk::ImageViewType::CUBE,
        (vk::ImageType::TYPE_2D, CubeArray) if is_cube => vk::ImageViewType::CUBE_ARRAY,
        (vk::ImageType::TYPE_3D, Cube) if is_cube => vk::ImageViewType::CUBE,
        (vk::ImageType::TYPE_3D, CubeArray) if is_cube => vk::ImageViewType::CUBE_ARRAY,
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

pub fn map_clear_rect(rect: &pso::ClearRect) -> vk::ClearRect {
    vk::ClearRect {
        base_array_layer: rect.layers.start as _,
        layer_count: (rect.layers.end - rect.layers.start) as _,
        rect: map_rect(&rect.rect),
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

pub fn map_vk_present_mode(mode: vk::PresentModeKHR) -> PresentMode {
    // the enum variants have to match Vulkan
    unsafe { mem::transmute(mode) }
}
