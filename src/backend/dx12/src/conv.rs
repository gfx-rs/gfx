use { validate_line_width };

use std::mem;
use spirv_cross::spirv;

use winapi::shared::basetsd::UINT8;
use winapi::shared::dxgiformat::*;
use winapi::shared::minwindef::{FALSE, INT, TRUE};
use winapi::um::d3d12::*;
use winapi::um::d3dcommon::*;

use hal::format::{Format, ImageFeature, SurfaceType};
use hal::{buffer, image, pso, Primitive};
use hal::pso::DescriptorSetLayoutBinding;

use native::descriptor::{DescriptorRange, DescriptorRangeType};
pub fn map_format(format: Format) -> Option<DXGI_FORMAT> {
    use hal::format::Format::*;

    // Handling packed formats according to the platform endianness.
    let reverse = unsafe { 1 == *(&1u32 as *const _ as *const u8) };
    let format = match format {
        Bgra4Unorm    if !reverse => DXGI_FORMAT_B4G4R4A4_UNORM,
        R5g6b5Unorm    if reverse => DXGI_FORMAT_B5G6R5_UNORM,
        B5g6r5Unorm   if !reverse => DXGI_FORMAT_B5G6R5_UNORM,
        B5g5r5a1Unorm if !reverse => DXGI_FORMAT_B5G5R5A1_UNORM,
        A1r5g5b5Unorm if reverse  => DXGI_FORMAT_B5G5R5A1_UNORM,
        R8Unorm => DXGI_FORMAT_R8_UNORM,
        R8Inorm => DXGI_FORMAT_R8_SNORM,
        R8Uint => DXGI_FORMAT_R8_UINT,
        R8Int => DXGI_FORMAT_R8_SINT,
        Rg8Unorm => DXGI_FORMAT_R8G8_UNORM,
        Rg8Inorm => DXGI_FORMAT_R8G8_SNORM,
        Rg8Uint => DXGI_FORMAT_R8G8_UINT,
        Rg8Int => DXGI_FORMAT_R8G8_SINT,
        Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        Rgba8Inorm => DXGI_FORMAT_R8G8B8A8_SNORM,
        Rgba8Uint => DXGI_FORMAT_R8G8B8A8_UINT,
        Rgba8Int => DXGI_FORMAT_R8G8B8A8_SINT,
        Rgba8Srgb => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        Bgra8Srgb => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        Abgr8Unorm if reverse => DXGI_FORMAT_R8G8B8A8_UNORM,
        Abgr8Inorm if reverse => DXGI_FORMAT_R8G8B8A8_SNORM,
        Abgr8Uint  if reverse => DXGI_FORMAT_R8G8B8A8_UINT,
        Abgr8Int   if reverse => DXGI_FORMAT_R8G8B8A8_SINT,
        Abgr8Srgb  if reverse => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        A2b10g10r10Unorm if reverse => DXGI_FORMAT_R10G10B10A2_UNORM,
        A2b10g10r10Uint  if reverse => DXGI_FORMAT_R10G10B10A2_UINT,
        R16Unorm => DXGI_FORMAT_R16_UNORM,
        R16Inorm => DXGI_FORMAT_R16_SNORM,
        R16Uint => DXGI_FORMAT_R16_UINT,
        R16Int => DXGI_FORMAT_R16_SINT,
        R16Float => DXGI_FORMAT_R16_FLOAT,
        Rg16Unorm => DXGI_FORMAT_R16G16_UNORM,
        Rg16Inorm => DXGI_FORMAT_R16G16_SNORM,
        Rg16Uint => DXGI_FORMAT_R16G16_UINT,
        Rg16Int => DXGI_FORMAT_R16G16_SINT,
        Rg16Float => DXGI_FORMAT_R16G16_FLOAT,
        Rgba16Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
        Rgba16Inorm => DXGI_FORMAT_R16G16B16A16_SNORM,
        Rgba16Uint => DXGI_FORMAT_R16G16B16A16_UINT,
        Rgba16Int => DXGI_FORMAT_R16G16B16A16_SINT,
        Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        R32Uint => DXGI_FORMAT_R32_UINT,
        R32Int => DXGI_FORMAT_R32_SINT,
        R32Float => DXGI_FORMAT_R32_FLOAT,
        Rg32Uint => DXGI_FORMAT_R32G32_UINT,
        Rg32Int => DXGI_FORMAT_R32G32_SINT,
        Rg32Float => DXGI_FORMAT_R32G32_FLOAT,
        Rgb32Uint => DXGI_FORMAT_R32G32B32_UINT,
        Rgb32Int => DXGI_FORMAT_R32G32B32_SINT,
        Rgb32Float => DXGI_FORMAT_R32G32B32_FLOAT,
        Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        Rgba32Int => DXGI_FORMAT_R32G32B32A32_SINT,
        Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        B10g11r11Ufloat if reverse => DXGI_FORMAT_R11G11B10_FLOAT,
        E5b9g9r9Ufloat  if reverse => DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        D16Unorm => DXGI_FORMAT_D16_UNORM,
        D24UnormS8Uint => DXGI_FORMAT_D24_UNORM_S8_UINT,
        X8D24Unorm if reverse => DXGI_FORMAT_D24_UNORM_S8_UINT,
        D32Float => DXGI_FORMAT_D32_FLOAT,
        D32FloatS8Uint => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        Bc1RgbUnorm => DXGI_FORMAT_BC1_UNORM,
        Bc1RgbSrgb => DXGI_FORMAT_BC1_UNORM_SRGB,
        Bc2Unorm => DXGI_FORMAT_BC2_UNORM,
        Bc2Srgb => DXGI_FORMAT_BC2_UNORM_SRGB,
        Bc3Unorm => DXGI_FORMAT_BC3_UNORM,
        Bc3Srgb => DXGI_FORMAT_BC3_UNORM_SRGB,
        Bc4Unorm => DXGI_FORMAT_BC4_UNORM,
        Bc4Inorm => DXGI_FORMAT_BC4_SNORM,
        Bc5Unorm => DXGI_FORMAT_BC5_UNORM,
        Bc5Inorm => DXGI_FORMAT_BC5_SNORM,
        Bc6hUfloat => DXGI_FORMAT_BC6H_UF16,
        Bc6hFloat => DXGI_FORMAT_BC6H_SF16,
        Bc7Unorm => DXGI_FORMAT_BC7_UNORM,
        Bc7Srgb => DXGI_FORMAT_BC7_UNORM_SRGB,

        _ => return None,
    };

    Some(format)
}

pub fn map_format_dsv(surface: SurfaceType) -> Option<DXGI_FORMAT> {
    Some(match surface {
        SurfaceType::D16    => DXGI_FORMAT_D16_UNORM,
        SurfaceType::X8D24 |
        SurfaceType::D24_S8 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        SurfaceType::D32    => DXGI_FORMAT_D32_FLOAT,
        SurfaceType::D32_S8 => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        _ => return None,
    })
}

pub fn map_topology_type(primitive: Primitive) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
    use hal::Primitive::*;
    match primitive {
        PointList  => D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        LineList |
        LineStrip |
        LineListAdjacency |
        LineStripAdjacency => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        TriangleList |
        TriangleStrip |
        TriangleListAdjacency |
        TriangleStripAdjacency => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        PatchList(_) => D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH,
    }
}

pub fn map_topology(primitive: Primitive) -> D3D12_PRIMITIVE_TOPOLOGY {
    use hal::Primitive::*;
    match primitive {
        PointList              => D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        LineList               => D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        LineListAdjacency      => D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
        LineStrip              => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
        LineStripAdjacency     => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
        TriangleList           => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        TriangleListAdjacency  => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        TriangleStrip          => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        TriangleStripAdjacency => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        PatchList(num) => { assert!(num != 0);
            D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST + (num as u32) - 1
        },
    }
}

pub fn map_rasterizer(rasterizer: &pso::Rasterizer) -> D3D12_RASTERIZER_DESC {
    use hal::pso::PolygonMode::*;
    use hal::pso::FrontFace::*;

    let bias = match rasterizer.depth_bias { //TODO: support dynamic depth bias
        Some(pso::State::Static(db)) => db,
        Some(_) | None => pso::DepthBias::default(),
    };

    D3D12_RASTERIZER_DESC {
        FillMode: match rasterizer.polygon_mode {
            Point => {
                error!("Point rasterization is not supported");
                D3D12_FILL_MODE_WIREFRAME
            },
            Line(width) => {
                validate_line_width(width);
                D3D12_FILL_MODE_WIREFRAME
            },
            Fill => D3D12_FILL_MODE_SOLID,
        },
        CullMode: match rasterizer.cull_face {
            pso::Face::NONE => D3D12_CULL_MODE_NONE,
            pso::Face::FRONT => D3D12_CULL_MODE_FRONT,
            pso::Face::BACK => D3D12_CULL_MODE_BACK,
            _ => panic!("Culling both front and back faces is not supported"),
        },
        FrontCounterClockwise: match rasterizer.front_face {
            Clockwise => FALSE,
            CounterClockwise => TRUE,
        },
        DepthBias: bias.const_factor as INT,
        DepthBiasClamp: bias.clamp,
        SlopeScaledDepthBias: bias.slope_factor,
        DepthClipEnable: !rasterizer.depth_clamping as _,
        MultisampleEnable: FALSE, // TODO: currently not supported
        ForcedSampleCount: 0, // TODO: currently not supported
        AntialiasedLineEnable: FALSE, // TODO: currently not supported
        ConservativeRaster: if rasterizer.conservative { // TODO: check support
            D3D12_CONSERVATIVE_RASTERIZATION_MODE_ON
        } else {
            D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF
        },
    }
}

fn map_factor(factor: pso::Factor) -> D3D12_BLEND {
    use hal::pso::Factor::*;
    match factor {
        Zero => D3D12_BLEND_ZERO,
        One => D3D12_BLEND_ONE,
        SrcColor => D3D12_BLEND_SRC_COLOR,
        OneMinusSrcColor => D3D12_BLEND_INV_SRC_COLOR,
        DstColor => D3D12_BLEND_DEST_COLOR,
        OneMinusDstColor => D3D12_BLEND_INV_DEST_COLOR,
        SrcAlpha => D3D12_BLEND_SRC_ALPHA,
        OneMinusSrcAlpha => D3D12_BLEND_INV_SRC_ALPHA,
        DstAlpha => D3D12_BLEND_DEST_ALPHA,
        OneMinusDstAlpha => D3D12_BLEND_INV_DEST_ALPHA,
        ConstColor | ConstAlpha => D3D12_BLEND_BLEND_FACTOR,
        OneMinusConstColor | OneMinusConstAlpha => D3D12_BLEND_INV_BLEND_FACTOR,
        SrcAlphaSaturate => D3D12_BLEND_SRC_ALPHA_SAT,
        Src1Color => D3D12_BLEND_SRC1_COLOR,
        OneMinusSrc1Color => D3D12_BLEND_INV_SRC1_COLOR,
        Src1Alpha => D3D12_BLEND_SRC1_ALPHA,
        OneMinusSrc1Alpha => D3D12_BLEND_INV_SRC1_ALPHA,
    }
}

fn map_blend_op(operation: pso::BlendOp) -> (D3D12_BLEND_OP, D3D12_BLEND, D3D12_BLEND) {
    use hal::pso::BlendOp::*;
    match operation {
        Add    { src, dst } => (D3D12_BLEND_OP_ADD,          map_factor(src), map_factor(dst)),
        Sub    { src, dst } => (D3D12_BLEND_OP_SUBTRACT,     map_factor(src), map_factor(dst)),
        RevSub { src, dst } => (D3D12_BLEND_OP_REV_SUBTRACT, map_factor(src), map_factor(dst)),
        Min => (D3D12_BLEND_OP_MIN, D3D12_BLEND_ZERO, D3D12_BLEND_ZERO),
        Max => (D3D12_BLEND_OP_MAX, D3D12_BLEND_ZERO, D3D12_BLEND_ZERO),
    }
}


pub fn map_render_targets(
    color_targets: &[pso::ColorBlendDesc],
) -> [D3D12_RENDER_TARGET_BLEND_DESC; 8] {
    let dummy_target = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: FALSE,
        LogicOpEnable: FALSE,
        SrcBlend: D3D12_BLEND_ZERO,
        DestBlend: D3D12_BLEND_ZERO,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ZERO,
        DestBlendAlpha: D3D12_BLEND_ZERO,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_CLEAR,
        RenderTargetWriteMask: 0,
    };
    let mut targets = [dummy_target; 8];

    for (target, &pso::ColorBlendDesc(mask, blend)) in targets.iter_mut().zip(color_targets.iter()) {
        target.RenderTargetWriteMask = mask.bits() as UINT8;
        if let pso::BlendState::On { color, alpha } = blend {
            let (color_op, color_src, color_dst) = map_blend_op(color);
            let (alpha_op, alpha_src, alpha_dst) = map_blend_op(alpha);
            target.BlendEnable = TRUE;
            target.BlendOp = color_op;
            target.SrcBlend = color_src;
            target.DestBlend = color_dst;
            target.BlendOpAlpha = alpha_op;
            target.SrcBlendAlpha = alpha_src;
            target.DestBlendAlpha = alpha_dst;
        }
    }

    targets
}

pub fn map_depth_stencil(dsi: &pso::DepthStencilDesc) -> D3D12_DEPTH_STENCIL_DESC {
    let (depth_on, depth_write, depth_func) = match dsi.depth {
        pso::DepthTest::On { fun, write } => (TRUE, write, map_comparison(fun)),
        pso::DepthTest::Off => unsafe { mem::zeroed() },
    };

    let (stencil_on, front, back, read_mask, write_mask) = match dsi.stencil {
        pso::StencilTest::On { ref front, ref back } => {
            if front.mask_read != back.mask_read || front.mask_write != back.mask_write {
                error!("Different masks on stencil front ({:?}) and back ({:?}) are not supported", front, back);
            }
            (TRUE, map_stencil_side(front), map_stencil_side(back), front.mask_read, front.mask_write)
        },
        pso::StencilTest::Off => unsafe { mem::zeroed() },
    };

    D3D12_DEPTH_STENCIL_DESC {
        DepthEnable: depth_on,
        DepthWriteMask: if depth_write {D3D12_DEPTH_WRITE_MASK_ALL} else {D3D12_DEPTH_WRITE_MASK_ZERO},
        DepthFunc: depth_func,
        StencilEnable: stencil_on,
        StencilReadMask: match read_mask {
            pso::State::Static(rm) => rm as _,
            pso::State::Dynamic => !0,
        },
        StencilWriteMask: match write_mask {
            pso::State::Static(wm) => wm as _,
            pso::State::Dynamic => !0,
        },
        FrontFace: front,
        BackFace: back,
    }
}

pub fn map_comparison(func: pso::Comparison) -> D3D12_COMPARISON_FUNC {
    use hal::pso::Comparison::*;
    match func {
        Never => D3D12_COMPARISON_FUNC_NEVER,
        Less => D3D12_COMPARISON_FUNC_LESS,
        LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
        Equal => D3D12_COMPARISON_FUNC_EQUAL,
        GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        Greater => D3D12_COMPARISON_FUNC_GREATER,
        NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
        Always => D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

fn map_stencil_op(op: pso::StencilOp) -> D3D12_STENCIL_OP {
    use hal::pso::StencilOp::*;
    match op {
        Keep => D3D12_STENCIL_OP_KEEP,
        Zero => D3D12_STENCIL_OP_ZERO,
        Replace => D3D12_STENCIL_OP_REPLACE,
        IncrementClamp => D3D12_STENCIL_OP_INCR_SAT,
        IncrementWrap => D3D12_STENCIL_OP_INCR,
        DecrementClamp => D3D12_STENCIL_OP_DECR_SAT,
        DecrementWrap => D3D12_STENCIL_OP_DECR,
        Invert => D3D12_STENCIL_OP_INVERT,
    }
}

fn map_stencil_side(side: &pso::StencilFace) -> D3D12_DEPTH_STENCILOP_DESC {
    D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: map_stencil_op(side.op_fail),
        StencilDepthFailOp: map_stencil_op(side.op_depth_fail),
        StencilPassOp: map_stencil_op(side.op_pass),
        StencilFunc: map_comparison(side.fun),
    }
}

pub fn map_wrap(wrap: image::WrapMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    use hal::image::WrapMode::*;
    match wrap {
        Tile   => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        Clamp  => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
    }
}

fn map_filter_type(filter: image::Filter) -> D3D12_FILTER_TYPE {
    match filter {
        image::Filter::Nearest => D3D12_FILTER_TYPE_POINT,
        image::Filter::Linear => D3D12_FILTER_TYPE_LINEAR,
    }
}

fn map_anisotropic(anisotropic: image::Anisotropic) -> D3D12_FILTER {
    match anisotropic {
        image::Anisotropic::On(_) => D3D12_FILTER_ANISOTROPIC,
        image::Anisotropic::Off => 0,
    }
}

pub fn map_filter(
    mag_filter: image::Filter,
    min_filter: image::Filter,
    mip_filter: image::Filter,
    reduction: D3D12_FILTER_REDUCTION_TYPE,
    anisotropic: image::Anisotropic,
) -> D3D12_FILTER {
    let mag = map_filter_type(mag_filter);
    let min = map_filter_type(min_filter);
    let mip = map_filter_type(mip_filter);

    (min & D3D12_FILTER_TYPE_MASK) << D3D12_MIN_FILTER_SHIFT |
    (mag & D3D12_FILTER_TYPE_MASK) << D3D12_MAG_FILTER_SHIFT |
    (mip & D3D12_FILTER_TYPE_MASK) << D3D12_MIP_FILTER_SHIFT |
    (reduction & D3D12_FILTER_REDUCTION_TYPE_MASK) << D3D12_FILTER_REDUCTION_TYPE_SHIFT |
    map_anisotropic(anisotropic)
}

pub fn map_buffer_resource_state(access: buffer::Access) -> D3D12_RESOURCE_STATES {
    use self::buffer::Access;
    // Mutable states
    if access.contains(Access::SHADER_WRITE) {
        return D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    }
    if access.contains(Access::TRANSFER_WRITE) {
        // Resolve not relevant for buffers.
        return D3D12_RESOURCE_STATE_COPY_DEST;
    }

    // Read-only states
    let mut state = D3D12_RESOURCE_STATE_COMMON;

    if access.contains(Access::TRANSFER_READ) {
        state |= D3D12_RESOURCE_STATE_COPY_SOURCE;
    }
    if access.contains(Access::INDEX_BUFFER_READ) {
        state |= D3D12_RESOURCE_STATE_INDEX_BUFFER;
    }
    if access.contains(Access::VERTEX_BUFFER_READ) || access.contains(Access::CONSTANT_BUFFER_READ) {
        state |= D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER;
    }
    if access.contains(Access::INDIRECT_COMMAND_READ) {
        state |= D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT;
    }
    if access.contains(Access::SHADER_READ) {
        // SHADER_READ only allows SRV access
        state |= D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    }

    state
}

pub fn map_image_resource_state(access: image::Access, layout: image::Layout) -> D3D12_RESOURCE_STATES {
    use self::image::Access;
    // `D3D12_RESOURCE_STATE_PRESENT` is the same as COMMON (general state)
    if layout == image::Layout::Present {
        return D3D12_RESOURCE_STATE_PRESENT;
    }

    // Mutable states
    if access.contains(Access::SHADER_WRITE) {
        return D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    }
    if access.contains(Access::DEPTH_STENCIL_ATTACHMENT_WRITE) {
        return D3D12_RESOURCE_STATE_DEPTH_WRITE;
    }
    if access.contains(Access::COLOR_ATTACHMENT_READ) || access.contains(Access::COLOR_ATTACHMENT_WRITE) {
        return D3D12_RESOURCE_STATE_RENDER_TARGET;
    }

    // `TRANSFER_WRITE` requires special handling as it requires RESOLVE_DEST | COPY_DEST
    // but only 1 write-only allowed. We do the required translation before the commands.
    // We currently assume that `COPY_DEST` is more common state than out of renderpass resolves.
    // Resolve operations need to insert a barrier before and after the command to transition from and
    // into `COPY_DEST` to have a consistent state for srcAccess.
    if access.contains(Access::TRANSFER_WRITE) {
        return D3D12_RESOURCE_STATE_COPY_DEST;
    }

    // Read-only states
    let mut state = D3D12_RESOURCE_STATE_COMMON;

    if access.contains(Access::TRANSFER_READ) {
        state |= D3D12_RESOURCE_STATE_COPY_SOURCE;
    }
    if access.contains(Access::INPUT_ATTACHMENT_READ) {
        state |= D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    }
    if access.contains(Access::DEPTH_STENCIL_ATTACHMENT_READ) {
        state |= D3D12_RESOURCE_STATE_DEPTH_READ;
    }
    if access.contains(Access::SHADER_READ) {
        // SHADER_READ only allows SRV access
        // Already handled the `SHADER_WRITE` write case above.
        assert!(!access.contains(Access::SHADER_WRITE));
        state |= D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    }

    state
}

pub fn map_descriptor_range(
    bind: &DescriptorSetLayoutBinding,
    register_space: u32,
    sampler: bool,
) -> DescriptorRange {
    DescriptorRange::new(
        match bind.ty {
            pso::DescriptorType::Sampler => DescriptorRangeType::Sampler,
            pso::DescriptorType::SampledImage
            | pso::DescriptorType::InputAttachment
            | pso::DescriptorType::UniformTexelBuffer => DescriptorRangeType::SRV,
            pso::DescriptorType::StorageBuffer
            | pso::DescriptorType::StorageBufferDynamic
            | pso::DescriptorType::StorageTexelBuffer
            | pso::DescriptorType::StorageImage => DescriptorRangeType::UAV,
            pso::DescriptorType::UniformBuffer | pso::DescriptorType::UniformBufferDynamic => {
                DescriptorRangeType::CBV
            }
            pso::DescriptorType::CombinedImageSampler => if sampler {
                DescriptorRangeType::Sampler
            } else {
                DescriptorRangeType::SRV
            },
        },
        bind.count as _,
        bind.binding as _,
        register_space,
        D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
    )
}

pub fn map_buffer_flags(usage: buffer::Usage) -> D3D12_RESOURCE_FLAGS {
    let mut flags = D3D12_RESOURCE_FLAG_NONE;

    // TRANSFER_DST also used for clearing buffers, which is implemented via UAV clears.
    if usage.contains(buffer::Usage::STORAGE) || usage.contains(buffer::Usage::TRANSFER_DST) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }

    flags
}

pub fn map_image_flags(usage: image::Usage, features: ImageFeature) -> D3D12_RESOURCE_FLAGS {
    use self::image::Usage;
    let mut flags = D3D12_RESOURCE_FLAG_NONE;

    // Blit operations implemented via a graphics pipeline
    if usage.contains(Usage::COLOR_ATTACHMENT) {
        debug_assert!(features.contains(ImageFeature::COLOR_ATTACHMENT));
        flags |= D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if usage.contains(Usage::DEPTH_STENCIL_ATTACHMENT) {
        debug_assert!(features.contains(ImageFeature::DEPTH_STENCIL_ATTACHMENT));
        flags |= D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    if usage.contains(Usage::TRANSFER_DST) {
        if features.contains(ImageFeature::COLOR_ATTACHMENT) {
            flags |= D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
        };
        if features.contains(ImageFeature::DEPTH_STENCIL_ATTACHMENT) {
            flags |= D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL
        };
    }
    if usage.contains(Usage::STORAGE) {
        debug_assert!(features.contains(ImageFeature::STORAGE));
        flags |= D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    if !features.contains(ImageFeature::SAMPLED) {
        flags |= D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE;
    }

    flags
}

pub fn map_execution_model(model: spirv::ExecutionModel) -> pso::Stage {
    match model {
        spirv::ExecutionModel::Vertex => pso::Stage::Vertex,
        spirv::ExecutionModel::Fragment => pso::Stage::Fragment,
        spirv::ExecutionModel::Geometry => pso::Stage::Geometry,
        spirv::ExecutionModel::GlCompute => pso::Stage::Compute,
        spirv::ExecutionModel::TessellationControl => pso::Stage::Hull,
        spirv::ExecutionModel::TessellationEvaluation => pso::Stage::Domain,
        spirv::ExecutionModel::Kernel => panic!("Kernel is not a valid execution model."),
    }
}

pub fn map_stage(stage: pso::Stage) -> spirv::ExecutionModel {
    match stage {
        pso::Stage::Vertex => spirv::ExecutionModel::Vertex,
        pso::Stage::Fragment => spirv::ExecutionModel::Fragment,
        pso::Stage::Geometry => spirv::ExecutionModel::Geometry,
        pso::Stage::Compute => spirv::ExecutionModel::GlCompute,
        pso::Stage::Hull => spirv::ExecutionModel::TessellationControl,
        pso::Stage::Domain => spirv::ExecutionModel::TessellationEvaluation,
    }
}
