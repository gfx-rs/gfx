use core::format::{Format, SurfaceType};
use core::{buffer, memory, state, pso, Primitive};
use core::image::{self, FilterMethod, WrapMode};
use core::pso::DescriptorSetLayoutBinding;
use core::state::Comparison;
use std::fmt;
use winapi::*;


pub fn map_heap_properties(props: memory::Properties) -> D3D12_HEAP_PROPERTIES {
    //TODO: ensure the flags are valid
    if !props.contains(memory::CPU_VISIBLE) {
        D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        }
    } else {
        D3D12_HEAP_PROPERTIES {
            Type:  D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
            MemoryPoolPreference: D3D12_MEMORY_POOL_L0,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        }
    }
}

pub fn map_format(format: Format) -> Option<DXGI_FORMAT> {
    use core::format::SurfaceType::*;
    use core::format::ChannelType::*;
    Some(match format.0 {
        R4_G4 | R4_G4_B4_A4 | R5_G5_B5_A1 | R5_G6_B5 => return None,
        R8 => match format.1 {
            Int   => DXGI_FORMAT_R8_SINT,
            Uint  => DXGI_FORMAT_R8_UINT,
            Inorm => DXGI_FORMAT_R8_SNORM,
            Unorm => DXGI_FORMAT_R8_UNORM,
            _ => return None,
        },
        R8_G8 => match format.1 {
            Int   => DXGI_FORMAT_R8G8_SINT,
            Uint  => DXGI_FORMAT_R8G8_UINT,
            Inorm => DXGI_FORMAT_R8G8_SNORM,
            Unorm => DXGI_FORMAT_R8G8_UNORM,
            _ => return None,
        },
        R8_G8_B8_A8 => match format.1 {
            Int   => DXGI_FORMAT_R8G8B8A8_SINT,
            Uint  => DXGI_FORMAT_R8G8B8A8_UINT,
            Inorm => DXGI_FORMAT_R8G8B8A8_SNORM,
            Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
            Srgb  => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
            _ => return None,
        },
        R10_G10_B10_A2 => match format.1 {
            Uint  => DXGI_FORMAT_R10G10B10A2_UINT,
            Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
            _ => return None,
        },
        R11_G11_B10 => match format.1 {
            Float => DXGI_FORMAT_R11G11B10_FLOAT,
            _ => return None,
        },
        R16 => match format.1 {
            Int   => DXGI_FORMAT_R16_SINT,
            Uint  => DXGI_FORMAT_R16_UINT,
            Inorm => DXGI_FORMAT_R16_SNORM,
            Unorm => DXGI_FORMAT_R16_UNORM,
            Float => DXGI_FORMAT_R16_FLOAT,
            _ => return None,
        },
        R16_G16 => match format.1 {
            Int   => DXGI_FORMAT_R16G16_SINT,
            Uint  => DXGI_FORMAT_R16G16_UINT,
            Inorm => DXGI_FORMAT_R16G16_SNORM,
            Unorm => DXGI_FORMAT_R16G16_UNORM,
            Float => DXGI_FORMAT_R16G16_FLOAT,
            _ => return None,
        },
        R16_G16_B16 => return None,
        R16_G16_B16_A16 => match format.1 {
            Int   => DXGI_FORMAT_R16G16B16A16_SINT,
            Uint  => DXGI_FORMAT_R16G16B16A16_UINT,
            Inorm => DXGI_FORMAT_R16G16B16A16_SNORM,
            Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
            Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
            _ => return None,
        },
        R32 => match format.1 {
            Int   => DXGI_FORMAT_R32_SINT,
            Uint  => DXGI_FORMAT_R32_UINT,
            Float => DXGI_FORMAT_R32_FLOAT,
            _ => return None,
        },
        R32_G32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32_SINT,
            Uint  => DXGI_FORMAT_R32G32_UINT,
            Float => DXGI_FORMAT_R32G32_FLOAT,
            _ => return None,
        },
        R32_G32_B32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32B32_SINT,
            Uint  => DXGI_FORMAT_R32G32B32_UINT,
            Float => DXGI_FORMAT_R32G32B32_FLOAT,
            _ => return None,
        },
        R32_G32_B32_A32 => match format.1 {
            Int   => DXGI_FORMAT_R32G32B32A32_SINT,
            Uint  => DXGI_FORMAT_R32G32B32A32_UINT,
            Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
            _ => return None,
        },
        B8_G8_R8_A8 => match format.1 {
            Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
            Srgb => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
            _ => return None,
        },
        D16 => match format.1 {
            Unorm => DXGI_FORMAT_R16_UNORM,
            _ => return None,
        },
        D24 => match format.1 {
            Unorm => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
            _ => return None,
        },
        D24_S8 => match format.1 {
            Unorm => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
            Uint  => DXGI_FORMAT_X24_TYPELESS_G8_UINT,
            _ => return None,
        },
        D32 => match format.1 {
            Float => DXGI_FORMAT_R32_FLOAT,
            _ => return None,
        },
        D32_S8 => match format.1 {
            Float => DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS,
            Uint  => DXGI_FORMAT_X32_TYPELESS_G8X24_UINT,
            _ => return None,
        },
    })
}

pub fn map_format_dsv(surface: SurfaceType) -> Option<DXGI_FORMAT> {
    Some(match surface {
        SurfaceType::D16    => DXGI_FORMAT_D16_UNORM,
        SurfaceType::D24    => DXGI_FORMAT_D24_UNORM_S8_UINT,
        SurfaceType::D24_S8 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        SurfaceType::D32    => DXGI_FORMAT_D32_FLOAT,
        SurfaceType::D32_S8 => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        _ => return None,
    })
}

pub fn map_topology_type(primitive: Primitive) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
    use core::Primitive::*;
    match primitive {
        PointList  => D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        LineList |
        LineStrip |
        LineListAdjacency |
        LineStripAdjacency => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        TriangleList |
        TriangleStrip |
        TriangleListAdjacency |
        TriangleStripAdjacency  => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        PatchList(_)   => D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH,
    }
}

pub fn map_topology(primitive: Primitive) -> D3D12_PRIMITIVE_TOPOLOGY {
    match primitive {
        Primitive::PointList              => D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        Primitive::LineList               => D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        Primitive::LineListAdjacency      => D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
        Primitive::LineStrip              => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
        Primitive::LineStripAdjacency     => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
        Primitive::TriangleList           => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        Primitive::TriangleListAdjacency  => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        Primitive::TriangleStrip          => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Primitive::TriangleStripAdjacency => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Primitive::PatchList(num) => { assert!(num != 0);
            D3D_PRIMITIVE_TOPOLOGY(D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 + (num as u32) - 1)
        },
    }
}

pub fn map_rasterizer(rasterizer: &pso::Rasterizer) -> D3D12_RASTERIZER_DESC {
    D3D12_RASTERIZER_DESC {
        FillMode: match rasterizer.polgyon_mode {
            state::RasterMethod::Point => {
                error!("Point rasterization is not supported");
                D3D12_FILL_MODE_WIREFRAME
            },
            state::RasterMethod::Line(_) => D3D12_FILL_MODE_WIREFRAME,
            state::RasterMethod::Fill => D3D12_FILL_MODE_SOLID,
        },
        CullMode: match rasterizer.cull_mode {
            state::CullFace::Nothing => D3D12_CULL_MODE_NONE,
            state::CullFace::Front => D3D12_CULL_MODE_FRONT,
            state::CullFace::Back => D3D12_CULL_MODE_BACK,
        },
        FrontCounterClockwise: match rasterizer.front_face {
            state::FrontFace::Clockwise => FALSE,
            state::FrontFace::CounterClockwise => TRUE,
        },
        DepthBias: rasterizer.depth_bias.map_or(0, |bias| bias.const_factor as INT),
        DepthBiasClamp: rasterizer.depth_bias.map_or(0.0, |bias| bias.clamp),
        SlopeScaledDepthBias: rasterizer.depth_bias.map_or(0.0, |bias| bias.slope_factor),
        DepthClipEnable: if rasterizer.depth_clamping { TRUE } else { FALSE },
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

fn map_blend_factor(factor: state::Factor, scalar: bool) -> D3D12_BLEND {
    use core::state::BlendValue::*;
    use core::state::Factor::*;
    match factor {
        Zero => D3D12_BLEND_ZERO,
        One => D3D12_BLEND_ONE,
        SourceAlphaSaturated => D3D12_BLEND_SRC_ALPHA_SAT,
        ZeroPlus(SourceColor) if !scalar => D3D12_BLEND_SRC_COLOR,
        ZeroPlus(SourceAlpha) => D3D12_BLEND_SRC_ALPHA,
        ZeroPlus(DestColor) if !scalar => D3D12_BLEND_DEST_COLOR,
        ZeroPlus(DestAlpha) => D3D12_BLEND_DEST_ALPHA,
        ZeroPlus(ConstColor) if !scalar => D3D12_BLEND_BLEND_FACTOR,
        ZeroPlus(ConstAlpha) => D3D12_BLEND_BLEND_FACTOR,
        OneMinus(SourceColor) if !scalar => D3D12_BLEND_INV_SRC_COLOR,
        OneMinus(SourceAlpha) => D3D12_BLEND_INV_SRC_ALPHA,
        OneMinus(DestColor) if !scalar => D3D12_BLEND_INV_DEST_COLOR,
        OneMinus(DestAlpha) => D3D12_BLEND_INV_DEST_ALPHA,
        OneMinus(ConstColor) if !scalar => D3D12_BLEND_INV_BLEND_FACTOR,
        OneMinus(ConstAlpha) => D3D12_BLEND_INV_BLEND_FACTOR,
        _ => {
            error!("Invalid blend factor requested for {}: {:?}",
                if scalar {"alpha"} else {"color"}, factor);
            D3D12_BLEND_ZERO
        }
    }
}

fn map_blend_op(equation: state::Equation) -> D3D12_BLEND_OP {
    use core::state::Equation::*;
    match equation {
        Add => D3D12_BLEND_OP_ADD,
        Sub => D3D12_BLEND_OP_SUBTRACT,
        RevSub => D3D12_BLEND_OP_REV_SUBTRACT,
        Min => D3D12_BLEND_OP_MIN,
        Max => D3D12_BLEND_OP_MAX,
    }
}


pub fn map_render_targets(
    color_targets: &[pso::ColorInfo],
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

    for (target, desc) in targets.iter_mut().zip(color_targets.iter()) {
        target.RenderTargetWriteMask = desc.mask.bits() as UINT8;

        if let Some(ref b) = desc.color {
            target.BlendEnable = TRUE;
            target.SrcBlend = map_blend_factor(b.source, false);
            target.DestBlend = map_blend_factor(b.destination, false);
            target.BlendOp = map_blend_op(b.equation);
        }
        if let Some(ref b) = desc.alpha {
            target.BlendEnable = TRUE;
            target.SrcBlendAlpha = map_blend_factor(b.source, true);
            target.DestBlendAlpha = map_blend_factor(b.destination, true);
            target.BlendOpAlpha = map_blend_op(b.equation);
        }
    }

    targets
}

pub fn map_depth_stencil(dsi: &pso::DepthStencilInfo) -> D3D12_DEPTH_STENCIL_DESC {
    D3D12_DEPTH_STENCIL_DESC {
        DepthEnable: if dsi.depth.is_some() { TRUE } else { FALSE },
        DepthWriteMask: D3D12_DEPTH_WRITE_MASK(match dsi.depth {
            Some(ref d) if d.write => 1,
            _ => 0,
        }),
        DepthFunc: match dsi.depth {
            Some(ref d) => map_comparison(d.fun),
            None => D3D12_COMPARISON_FUNC_NEVER,
        },
        StencilEnable: if dsi.front.is_some() || dsi.back.is_some() { TRUE } else { FALSE },
        StencilReadMask: map_stencil_mask(dsi, StencilAccess::Read, |s| (s.mask_read as UINT8)),
        StencilWriteMask: map_stencil_mask(dsi, StencilAccess::Write, |s| (s.mask_write as UINT8)),
        FrontFace: map_stencil_side(&dsi.front),
        BackFace: map_stencil_side(&dsi.back),
    }
}

fn map_comparison(func: state::Comparison) -> D3D12_COMPARISON_FUNC {
    match func {
        state::Comparison::Never => D3D12_COMPARISON_FUNC_NEVER,
        state::Comparison::Less => D3D12_COMPARISON_FUNC_LESS,
        state::Comparison::LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
        state::Comparison::Equal => D3D12_COMPARISON_FUNC_EQUAL,
        state::Comparison::GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        state::Comparison::Greater => D3D12_COMPARISON_FUNC_GREATER,
        state::Comparison::NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
        state::Comparison::Always => D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

fn map_stencil_op(op: state::StencilOp) -> D3D12_STENCIL_OP {
    use core::state::StencilOp::*;
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

fn map_stencil_side(side: &Option<state::StencilSide>) -> D3D12_DEPTH_STENCILOP_DESC {
    let side = side.unwrap_or_default();
    D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: map_stencil_op(side.op_fail),
        StencilDepthFailOp: map_stencil_op(side.op_depth_fail),
        StencilPassOp: map_stencil_op(side.op_pass),
        StencilFunc: map_comparison(side.fun),
    }
}

#[derive(Copy, Clone, Debug)]
enum StencilAccess {
    Read,
    Write,
}

impl fmt::Display for StencilAccess {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            StencilAccess::Read => "read",
            StencilAccess::Write  => "write",
        })
    }
}

fn map_stencil_mask<F>(dsi: &pso::DepthStencilInfo, access: StencilAccess, accessor: F) -> UINT8
    where F: Fn(&state::StencilSide) -> UINT8 {
    match (dsi.front, dsi.back) {
        (Some(ref front), Some(ref back)) if accessor(front) != accessor(back) => {
            error!("Different {} masks on stencil front ({}) and back ({}) are not supported",
                access, accessor(front), accessor(back));
            accessor(front)
        },
        (Some(ref front), _) => accessor(front),
        (_, Some(ref back)) => accessor(back),
        (None, None) => 0,
    }
}

pub fn map_wrap(wrap: WrapMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    match wrap {
        WrapMode::Tile   => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        WrapMode::Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        WrapMode::Clamp  => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        WrapMode::Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
    }
}

pub enum FilterOp {
    Product,
    Comparison,
    //Maximum, TODO
    //Minimum, TODO
}

pub fn map_filter(filter: FilterMethod, op: FilterOp) -> D3D12_FILTER {
    use core::image::FilterMethod::*;
    match op {
        FilterOp::Product => match filter {
            Scale          => D3D12_FILTER_MIN_MAG_MIP_POINT,
            Mipmap         => D3D12_FILTER_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D12_FILTER_ANISOTROPIC,
        },
        FilterOp::Comparison => match filter {
            Scale          => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
            Mipmap         => D3D12_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR,
            Bilinear       => D3D12_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT,
            Trilinear      => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
            Anisotropic(_) => D3D12_FILTER_COMPARISON_ANISOTROPIC,
        },
    }
}

pub fn map_function(fun: Comparison) -> D3D12_COMPARISON_FUNC {
    match fun {
        Comparison::Never => D3D12_COMPARISON_FUNC_NEVER,
        Comparison::Less => D3D12_COMPARISON_FUNC_LESS,
        Comparison::LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
        Comparison::Equal => D3D12_COMPARISON_FUNC_EQUAL,
        Comparison::GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        Comparison::Greater => D3D12_COMPARISON_FUNC_GREATER,
        Comparison::NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
        Comparison::Always => D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

pub fn map_buffer_resource_state(access: buffer::Access) -> D3D12_RESOURCE_STATES {
    // Mutable states
    if access.contains(buffer::SHADER_WRITE) {
        return D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    }
    if access.contains(buffer::TRANSFER_WRITE) {
        // Resolve not relevant for buffers.
        return D3D12_RESOURCE_STATE_COPY_DEST;
    }

    // Read-only states
    let mut state = D3D12_RESOURCE_STATE_COMMON;

    if access.contains(buffer::TRANSFER_READ) {
        state = state | D3D12_RESOURCE_STATE_COPY_SOURCE | D3D12_RESOURCE_STATE_RESOLVE_DEST;
    }
    if access.contains(buffer::INDEX_BUFFER_READ) {
        state = state | D3D12_RESOURCE_STATE_INDEX_BUFFER;
    }
    if access.contains(buffer::VERTEX_BUFFER_READ) || access.contains(buffer::CONSTANT_BUFFER_READ) {
        state = state | D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER;
    }
    if access.contains(buffer::INDIRECT_COMMAND_READ) {
        state = state | D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT;
    }
    if access.contains(buffer::SHADER_READ) {
        // SHADER_READ only allows SRV access
        state = state | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    }

    state
}

pub fn map_image_resource_state(access: image::Access, layout: image::ImageLayout) -> D3D12_RESOURCE_STATES {
    // `D3D12_RESOURCE_STATE_PRESENT` is the same as COMMON (general state)
    if layout == image::ImageLayout::Present {
        return D3D12_RESOURCE_STATE_PRESENT;
    }

    // Mutable states
    if access.contains(image::SHADER_WRITE) {
        return D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    }
    if access.contains(image::DEPTH_STENCIL_ATTACHMENT_WRITE) {
        return D3D12_RESOURCE_STATE_DEPTH_WRITE;
    }
    if access.contains(image::COLOR_ATTACHMENT_READ) || access.contains(image::COLOR_ATTACHMENT_WRITE) {
        return D3D12_RESOURCE_STATE_RENDER_TARGET;
    }

    // `TRANSFER_WRITE` requires special handling as it requires RESOLVE_DEST | COPY_DEST
    // but only 1 write-only allowed. We do the required translation before the commands.
    // We currently assume that `COPY_DEST` is more common state than out of renderpass resolves.
    // Resolve operations need to insert a barrier before and after the command to transition from and
    // into `COPY_DEST` to have a consistent state for srcAccess.
    if access.contains(image::TRANSFER_WRITE) {
        return D3D12_RESOURCE_STATE_COPY_DEST;
    }

    // Read-only states
    let mut state = D3D12_RESOURCE_STATE_COMMON;

    if access.contains(image::TRANSFER_READ) {
        state = state | D3D12_RESOURCE_STATE_COPY_SOURCE | D3D12_RESOURCE_STATE_RESOLVE_DEST;
    }
    if access.contains(image::INPUT_ATTACHMENT_READ) {
        state = state | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    }
    if access.contains(image::DEPTH_STENCIL_ATTACHMENT_READ) {
        state = state | D3D12_RESOURCE_STATE_DEPTH_READ;
    }
    if access.contains(image::SHADER_READ) {
        // SHADER_READ only allows SRV access
        // Already handled the `SHADER_WRITE` write case above.
        assert!(!access.contains(image::SHADER_WRITE));
        state = state | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    }

    state
}

pub fn map_descriptor_range(bind: &DescriptorSetLayoutBinding, register_space: u32) -> D3D12_DESCRIPTOR_RANGE {
    D3D12_DESCRIPTOR_RANGE {
        RangeType: match bind.ty {
            pso::DescriptorType::Sampler => D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
            pso::DescriptorType::SampledImage => D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            pso::DescriptorType::StorageBuffer |
            pso::DescriptorType::StorageImage => D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
            pso::DescriptorType::UniformBuffer => D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
            _ => panic!("unsupported binding type {:?}", bind.ty)
        },
        NumDescriptors: bind.count as _,
        BaseShaderRegister: bind.binding as _,
        RegisterSpace: register_space,
        OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
    }
}

pub fn map_buffer_flags(usage: buffer::Usage) -> D3D12_RESOURCE_FLAGS {
    let mut flags = D3D12_RESOURCE_FLAG_NONE;

    if usage.contains(buffer::STORAGE) {
        flags = flags | D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }

    flags
}

pub fn map_image_flags(usage: image::Usage) -> D3D12_RESOURCE_FLAGS {
    let mut flags = D3D12_RESOURCE_FLAG_NONE;

    if usage.contains(image::COLOR_ATTACHMENT) {
        flags = flags | D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if usage.contains(image::DEPTH_STENCIL_ATTACHMENT) {
        flags = flags | D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    if usage.contains(image::STORAGE) {
        flags = flags | D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    if usage.contains(image::DEPTH_STENCIL_ATTACHMENT) && !usage.contains(image::SAMPLED) {
        flags = flags | D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE;
    }

    flags
}
