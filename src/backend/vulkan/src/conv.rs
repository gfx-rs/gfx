use ash::vk;
use byteorder::{NativeEndian, WriteBytesExt};
use smallvec::SmallVec;

use hal::{buffer, command, format, image, pass, pso, query};
use hal::{IndexType, Primitive};
use hal::device::Extent;
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

pub fn map_image_layout(layout: image::ImageLayout) -> vk::ImageLayout {
    use hal::image::ImageLayout as Il;
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

pub fn map_image_aspects(aspects: format::AspectFlags) -> vk::ImageAspectFlags {
    use self::format::AspectFlags;
    let mut flags = vk::ImageAspectFlags::empty();
    if aspects.contains(AspectFlags::COLOR) {
        flags |= vk::IMAGE_ASPECT_COLOR_BIT;
    }
    if aspects.contains(AspectFlags::DEPTH) {
        flags |= vk::IMAGE_ASPECT_DEPTH_BIT;
    }
    if aspects.contains(AspectFlags::STENCIL) {
        flags |= vk::IMAGE_ASPECT_STENCIL_BIT;
    }
    flags
}

pub fn map_clear_color(value: command::ClearColor) -> vk::ClearColorValue {
    match value {
        command::ClearColor::Float(v) => vk::ClearColorValue::new_float32(v),
        command::ClearColor::Int(v)   => vk::ClearColorValue::new_int32(v),
        command::ClearColor::Uint(v)  => vk::ClearColorValue::new_uint32(v),
    }
}

pub fn map_clear_depth_stencil(value: command::ClearDepthStencil) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth: value.0,
        stencil: value.1,
    }
}

pub fn map_clear_depth(depth: command::DepthValue) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth,
        stencil: 0,
    }
}

pub fn map_clear_stencil(stencil: command::StencilValue) -> vk::ClearDepthStencilValue {
    vk::ClearDepthStencilValue {
        depth: 0.0,
        stencil,
    }
}

pub fn map_offset(offset: command::Offset) -> vk::Offset3D {
    vk::Offset3D {
        x: offset.x,
        y: offset.y,
        z: offset.z,
    }
}

pub fn map_extent(offset: Extent) -> vk::Extent3D {
    vk::Extent3D {
        width: offset.width,
        height: offset.height,
        depth: offset.depth,
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

pub fn map_subresource_with_layers(
    aspects: format::AspectFlags,
    (mip_level, base_layer): image::Subresource,
    layers: image::Layer,
) -> vk::ImageSubresourceLayers {
    map_subresource_layers(&image::SubresourceLayers {
        aspects,
        level: mip_level,
        layers: base_layer..base_layer+layers,
    })
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
    use self::buffer::Access;
    let mut flags = vk::AccessFlags::empty();

    if access.contains(Access::TRANSFER_READ) {
        flags |= vk::ACCESS_TRANSFER_READ_BIT;
    }
    if access.contains(Access::TRANSFER_WRITE) {
        flags |= vk::ACCESS_TRANSFER_WRITE_BIT;
    }
    if access.contains(Access::INDEX_BUFFER_READ) {
        flags |= vk::ACCESS_INDEX_READ_BIT;
    }
    if access.contains(Access::VERTEX_BUFFER_READ) {
        flags |= vk::ACCESS_VERTEX_ATTRIBUTE_READ_BIT;
    }
    if access.contains(Access::CONSTANT_BUFFER_READ) {
        flags |= vk::ACCESS_UNIFORM_READ_BIT;
    }
    if access.contains(Access::INDIRECT_COMMAND_READ) {
        flags |= vk::ACCESS_INDIRECT_COMMAND_READ_BIT;
    }
    if access.contains(Access::SHADER_READ) {
        flags |= vk::ACCESS_SHADER_READ_BIT;
    }
    if access.contains(Access::SHADER_WRITE) {
        flags |= vk::ACCESS_SHADER_WRITE_BIT;
    }
    if access.contains(Access::HOST_READ) {
        flags |= vk::ACCESS_HOST_READ_BIT;
    }
    if access.contains(Access::HOST_WRITE) {
        flags |= vk::ACCESS_HOST_WRITE_BIT;
    }
    if access.contains(Access::MEMORY_READ) {
        flags |= vk::ACCESS_MEMORY_READ_BIT;
    }
    if access.contains(Access::MEMORY_WRITE) {
        flags |= vk::ACCESS_MEMORY_WRITE_BIT;
    }

    flags
}

pub fn map_image_access(access: image::Access) -> vk::AccessFlags {
    use self::image::Access;
    let mut flags = vk::AccessFlags::empty();

    if access.contains(Access::COLOR_ATTACHMENT_READ) {
        flags |= vk::ACCESS_COLOR_ATTACHMENT_READ_BIT;
    }
    if access.contains(Access::COLOR_ATTACHMENT_WRITE) {
        flags |= vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
    }
    if access.contains(Access::TRANSFER_READ) {
        flags |= vk::ACCESS_TRANSFER_READ_BIT;
    }
    if access.contains(Access::TRANSFER_WRITE) {
        flags |= vk::ACCESS_TRANSFER_WRITE_BIT;
    }
    if access.contains(Access::SHADER_READ) {
        flags |= vk::ACCESS_SHADER_READ_BIT;
    }
    if access.contains(Access::SHADER_WRITE) {
        flags |= vk::ACCESS_SHADER_WRITE_BIT;
    }
    if access.contains(Access::DEPTH_STENCIL_ATTACHMENT_READ) {
        flags |= vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT;
    }
    if access.contains(Access::DEPTH_STENCIL_ATTACHMENT_WRITE) {
        flags |= vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT;
    }
    if access.contains(Access::HOST_READ) {
        flags |= vk::ACCESS_HOST_READ_BIT;
    }
    if access.contains(Access::HOST_WRITE) {
        flags |= vk::ACCESS_HOST_WRITE_BIT;
    }
    if access.contains(Access::MEMORY_READ) {
        flags |= vk::ACCESS_MEMORY_READ_BIT;
    }
    if access.contains(Access::MEMORY_WRITE) {
        flags |= vk::ACCESS_MEMORY_WRITE_BIT;
    }
    if access.contains(Access::INPUT_ATTACHMENT_READ) {
        flags |= vk::ACCESS_INPUT_ATTACHMENT_READ_BIT;
    }

    flags
}

pub fn map_pipeline_stage(stage: pso::PipelineStage) -> vk::PipelineStageFlags {
    use self::pso::PipelineStage;
    let mut flags = vk::PipelineStageFlags::empty();

    if stage.contains(PipelineStage::TOP_OF_PIPE) {
        flags |= vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT;
    }
    if stage.contains(PipelineStage::DRAW_INDIRECT) {
        flags |= vk::PIPELINE_STAGE_DRAW_INDIRECT_BIT;
    }
    if stage.contains(PipelineStage::VERTEX_INPUT) {
        flags |= vk::PIPELINE_STAGE_VERTEX_INPUT_BIT;
    }
    if stage.contains(PipelineStage::VERTEX_SHADER) {
        flags |= vk::PIPELINE_STAGE_VERTEX_SHADER_BIT;
    }
    if stage.contains(PipelineStage::HULL_SHADER) {
        flags |= vk::PIPELINE_STAGE_TESSELLATION_CONTROL_SHADER_BIT;
    }
    if stage.contains(PipelineStage::DOMAIN_SHADER) {
        flags |= vk::PIPELINE_STAGE_TESSELLATION_EVALUATION_SHADER_BIT;
    }
    if stage.contains(PipelineStage::GEOMETRY_SHADER) {
        flags |= vk::PIPELINE_STAGE_GEOMETRY_SHADER_BIT;
    }
    if stage.contains(PipelineStage::FRAGMENT_SHADER) {
        flags |= vk::PIPELINE_STAGE_FRAGMENT_SHADER_BIT;
    }
    if stage.contains(PipelineStage::EARLY_FRAGMENT_TESTS) {
        flags |= vk::PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT;
    }
    if stage.contains(PipelineStage::LATE_FRAGMENT_TESTS) {
        flags |= vk::PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT;
    }
    if stage.contains(PipelineStage::COLOR_ATTACHMENT_OUTPUT) {
        flags |= vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
    }
    if stage.contains(PipelineStage::COMPUTE_SHADER) {
        flags |= vk::PIPELINE_STAGE_COMPUTE_SHADER_BIT;
    }
    if stage.contains(PipelineStage::TRANSFER) {
        flags |= vk::PIPELINE_STAGE_TRANSFER_BIT;
    }
    if stage.contains(PipelineStage::BOTTOM_OF_PIPE) {
        flags |= vk::PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT;
    }
    if stage.contains(PipelineStage::HOST) {
        flags |= vk::PIPELINE_STAGE_HOST_BIT;
    }

    flags
}

pub fn map_buffer_usage(usage: buffer::Usage) -> vk::BufferUsageFlags {
    use self::buffer::Usage;
    let mut flags = vk::BufferUsageFlags::empty();

    if usage.contains(Usage::TRANSFER_SRC) {
        flags |= vk::BUFFER_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(Usage::TRANSFER_DST) {
        flags |= vk::BUFFER_USAGE_TRANSFER_DST_BIT;
    }
    if usage.contains(Usage::UNIFORM) {
        flags |= vk::BUFFER_USAGE_UNIFORM_BUFFER_BIT;
    }
    if usage.contains(Usage::STORAGE) {
        flags |= vk::BUFFER_USAGE_STORAGE_BUFFER_BIT;
    }
    if usage.contains(Usage::UNIFORM_TEXEL) {
        flags |= vk::BUFFER_USAGE_UNIFORM_TEXEL_BUFFER_BIT;
    }
    if usage.contains(Usage::STORAGE_TEXEL) {
        flags |= vk::BUFFER_USAGE_STORAGE_TEXEL_BUFFER_BIT;
    }
    if usage.contains(Usage::INDEX) {
        flags |= vk::BUFFER_USAGE_INDEX_BUFFER_BIT;
    }
    if usage.contains(Usage::INDIRECT) {
        flags |= vk::BUFFER_USAGE_INDIRECT_BUFFER_BIT;
    }
    if usage.contains(Usage::VERTEX) {
        flags |= vk::BUFFER_USAGE_VERTEX_BUFFER_BIT;
    }

    flags
}

pub fn map_image_usage(usage: image::Usage) -> vk::ImageUsageFlags {
    use self::image::Usage;
    let mut flags = vk::ImageUsageFlags::empty();

    if usage.contains(Usage::TRANSFER_SRC) {
        flags |= vk::IMAGE_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(Usage::TRANSFER_DST) {
        flags |= vk::IMAGE_USAGE_TRANSFER_DST_BIT;
    }
    if usage.contains(Usage::COLOR_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    }
    if usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT;
    }
    if usage.contains(Usage::STORAGE) {
        flags |= vk::IMAGE_USAGE_STORAGE_BIT;
    }
    if usage.contains(Usage::SAMPLED) {
        flags |= vk::IMAGE_USAGE_SAMPLED_BIT;
    }
    if usage.contains(Usage::TRANSIENT_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_TRANSIENT_ATTACHMENT_BIT;
    }
    if usage.contains(Usage::INPUT_ATTACHMENT) {
        flags |= vk::IMAGE_USAGE_INPUT_ATTACHMENT_BIT;
    }
    
    flags
}

pub fn map_descriptor_type(ty: pso::DescriptorType) -> vk::DescriptorType {
    use hal::pso::DescriptorType as Dt;
    match ty {
        Dt::Sampler            => vk::DescriptorType::Sampler,
        Dt::SampledImage       => vk::DescriptorType::SampledImage,
        Dt::StorageImage       => vk::DescriptorType::StorageImage,
        Dt::UniformTexelBuffer => vk::DescriptorType::UniformTexelBuffer,
        Dt::StorageTexelBuffer => vk::DescriptorType::StorageTexelBuffer,
        Dt::UniformBuffer      => vk::DescriptorType::UniformBuffer,
        Dt::StorageBuffer      => vk::DescriptorType::StorageBuffer,
        Dt::InputAttachment    => vk::DescriptorType::InputAttachment,
        Dt::CombinedImageSampler => vk::DescriptorType::CombinedImageSampler,
    }
}

pub fn map_stage_flags(stages: pso::ShaderStageFlags) -> vk::ShaderStageFlags {
    use self::pso::ShaderStageFlags;
    let mut flags = vk::ShaderStageFlags::empty();

    if stages.contains(ShaderStageFlags::VERTEX) {
        flags |= vk::SHADER_STAGE_VERTEX_BIT;
    }

    if stages.contains(ShaderStageFlags::HULL) {
        flags |= vk::SHADER_STAGE_TESSELLATION_CONTROL_BIT;
    }

    if stages.contains(ShaderStageFlags::DOMAIN) {
        flags |= vk::SHADER_STAGE_TESSELLATION_EVALUATION_BIT;
    }

    if stages.contains(ShaderStageFlags::GEOMETRY) {
        flags |= vk::SHADER_STAGE_GEOMETRY_BIT;
    }

    if stages.contains(ShaderStageFlags::FRAGMENT) {
        flags |= vk::SHADER_STAGE_FRAGMENT_BIT;
    }

    if stages.contains(ShaderStageFlags::COMPUTE) {
        flags |= vk::SHADER_STAGE_COMPUTE_BIT;
    }

    flags
}


pub fn map_filter(filter: image::FilterMethod) -> (vk::Filter, vk::Filter, vk::SamplerMipmapMode, f32) {
    use hal::image::FilterMethod as Fm;
    match filter {
        Fm::Scale          => (vk::Filter::Nearest, vk::Filter::Nearest, vk::SamplerMipmapMode::Nearest, 1.0),
        Fm::Mipmap         => (vk::Filter::Nearest, vk::Filter::Nearest, vk::SamplerMipmapMode::Linear,  1.0),
        Fm::Bilinear       => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Nearest, 1.0),
        Fm::Trilinear      => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Linear,  1.0),
        Fm::Anisotropic(a) => (vk::Filter::Linear,  vk::Filter::Linear,  vk::SamplerMipmapMode::Linear,  a as f32),
    }
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
    use hal::query::PipelineStatistic as stat;

    let mut flags = vk::QueryPipelineStatisticFlags::empty();

    if statistics.contains(stat::INPUT_ASSEMBLY_VERTICES) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_INPUT_ASSEMBLY_VERTICES_BIT;
    }
    if statistics.contains(stat::INPUT_ASSEMBLY_PRIMITIVES) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_INPUT_ASSEMBLY_PRIMITIVES_BIT;
    }
    if statistics.contains(stat::VERTEX_SHADER_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_VERTEX_SHADER_INVOCATIONS_BIT;
    }
    if statistics.contains(stat::GEOMETRY_SHADER_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_GEOMETRY_SHADER_INVOCATIONS_BIT;
    }
    if statistics.contains(stat::GEOMETRY_SHADER_PRIMITIVES) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_GEOMETRY_SHADER_PRIMITIVES_BIT;
    }
    if statistics.contains(stat::CLIPPING_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_CLIPPING_INVOCATIONS_BIT;
    }
    if statistics.contains(stat::CLIPPING_PRIMITIVES) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_CLIPPING_PRIMITIVES_BIT;
    }
    if statistics.contains(stat::FRAGMENT_SHADER_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_FRAGMENT_SHADER_INVOCATIONS_BIT;
    }
    if statistics.contains(stat::HULL_SHADER_PATCHES) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_TESSELLATION_CONTROL_SHADER_PATCHES_BIT;
    }
    if statistics.contains(stat::DOMAIN_SHADER_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_TESSELLATION_EVALUATION_SHADER_INVOCATIONS_BIT;
    }
    if statistics.contains(stat::COMPUTE_SHADER_INVOCATIONS) {
        flags |= vk::QUERY_PIPELINE_STATISTIC_COMPUTE_SHADER_INVOCATIONS_BIT;
    }

    flags
}

pub fn map_image_features(features: vk::FormatFeatureFlags) -> format::ImageFeature {
    let mut flags = format::ImageFeature::empty();

    if features.intersects(vk::FORMAT_FEATURE_SAMPLED_IMAGE_BIT) {
        flags |= format::ImageFeature::SAMPLED;
    }
    if features.intersects(vk::FORMAT_FEATURE_STORAGE_IMAGE_BIT) {
        flags |= format::ImageFeature::STORAGE;
    }
    if features.intersects(vk::FORMAT_FEATURE_STORAGE_IMAGE_ATOMIC_BIT) {
        flags |= format::ImageFeature::STORAGE_ATOMIC;
    }
    if features.intersects(vk::FORMAT_FEATURE_COLOR_ATTACHMENT_BIT) {
        flags |= format::ImageFeature::COLOR_ATTACHMENT;
    }
    if features.intersects(vk::FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND_BIT) {
        flags |= format::ImageFeature::COLOR_ATTACHMENT_BLEND;
    }
    if features.intersects(vk::FORMAT_FEATURE_DEPTH_STENCIL_ATTACHMENT_BIT) {
        flags |= format::ImageFeature::DEPTH_STENCIL_ATTACHMENT;
    }
    if features.intersects(vk::FORMAT_FEATURE_BLIT_SRC_BIT) {
        flags |= format::ImageFeature::BLIT_SRC;
    }
    if features.intersects(vk::FORMAT_FEATURE_BLIT_DST_BIT) {
        flags |= format::ImageFeature::BLIT_DST;
    }
    if features.intersects(vk::FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT) {
        flags |= format::ImageFeature::SAMPLED_LINEAR;
    }

    flags
}

pub fn map_buffer_features(features: vk::FormatFeatureFlags) -> format::BufferFeature {
    let mut flags = format::BufferFeature::empty();

    if features.intersects(vk::FORMAT_FEATURE_UNIFORM_TEXEL_BUFFER_BIT) {
        flags |= format::BufferFeature::UNIFORM_TEXEL;
    }
    if features.intersects(vk::FORMAT_FEATURE_STORAGE_TEXEL_BUFFER_BIT) {
        flags |= format::BufferFeature::STORAGE_TEXEL;
    }
    if features.intersects(vk::FORMAT_FEATURE_STORAGE_TEXEL_BUFFER_ATOMIC_BIT) {
        flags |= format::BufferFeature::STORAGE_TEXEL_ATOMIC;
    }
    if features.intersects(vk::FORMAT_FEATURE_VERTEX_BUFFER_BIT) {
        flags |= format::BufferFeature::VERTEX;
    }

    flags
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
    let mut usage = vk::CommandBufferUsageFlags::empty();
    if flags.contains(command::CommandBufferFlags::ONE_TIME_SUBMIT) {
        usage |= vk::COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    }
    if flags.contains(command::CommandBufferFlags::RENDER_PASS_CONTINUE) {
        usage |= vk::COMMAND_BUFFER_USAGE_RENDER_PASS_CONTINUE_BIT;
    }
    if flags.contains(command::CommandBufferFlags::SIMULTANEOUS_USE) {
        usage |= vk::COMMAND_BUFFER_USAGE_SIMULTANEOUS_USE_BIT;
    }
    usage
}

pub fn map_command_buffer_level(level: command::RawLevel) -> vk::CommandBufferLevel {
    match level {
        command::RawLevel::Primary => vk::CommandBufferLevel::Primary,
        command::RawLevel::Secondary => vk::CommandBufferLevel::Secondary,
    }
}
