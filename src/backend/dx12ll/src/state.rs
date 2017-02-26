// Copyright 2017 The Gfx-rs Developers.
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

use core::{pso, state};
use core::Primitive;
use core::MAX_COLOR_TARGETS;
use winapi::*;
use std::fmt;

pub fn map_rasterizer(rasterizer: &state::Rasterizer) -> D3D12_RASTERIZER_DESC {
    D3D12_RASTERIZER_DESC {
        FillMode: match rasterizer.method {
            state::RasterMethod::Point => {
                error!("Point rasterization is not supported");
                D3D12_FILL_MODE_WIREFRAME
            },
            state::RasterMethod::Line(_) => D3D12_FILL_MODE_WIREFRAME,
            state::RasterMethod::Fill => D3D12_FILL_MODE_SOLID,
        },
        CullMode: match rasterizer.cull_face {
            state::CullFace::Nothing => D3D12_CULL_MODE_NONE,
            state::CullFace::Front => D3D12_CULL_MODE_FRONT,
            state::CullFace::Back => D3D12_CULL_MODE_BACK,
        },
        FrontCounterClockwise: match rasterizer.front_face {
            state::FrontFace::Clockwise => FALSE,
            state::FrontFace::CounterClockwise => TRUE,
        },
        DepthBias: rasterizer.offset.map_or(0, |off| off.1 as INT),
        DepthBiasClamp: 16.0, // TODO: magic value?
        SlopeScaledDepthBias: rasterizer.offset.map_or(0.0, |off| off.0 as FLOAT),
        DepthClipEnable: TRUE,
        MultisampleEnable: if rasterizer.samples.is_some() { TRUE } else { FALSE },
        ForcedSampleCount: 0, // TODO
        AntialiasedLineEnable: FALSE,
        ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
    }
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

enum StencilSide {
    Front,
    Back,
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

pub fn map_primitive_topology(primitive: Primitive) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE{
    use core::Primitive::*;
    match primitive {
        PointList      => D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        LineList |
        LineStrip      => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        TriangleList |
        TriangleStrip  => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        PatchList(_)   => D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH,
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

pub fn map_render_targets(color_targets: &[Option<pso::ColorTargetDesc>; MAX_COLOR_TARGETS]) -> [D3D12_RENDER_TARGET_BLEND_DESC; 8] {
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

    for (mut target, desc) in targets.iter_mut().zip(color_targets.iter()) {
        let info = if let Some((_, ref info)) = *desc { info } else { continue };

        target.RenderTargetWriteMask = info.mask.bits() as UINT8;

        if let Some(ref b) = info.color {
            target.BlendEnable = TRUE;
            target.SrcBlend = map_blend_factor(b.source, false);
            target.DestBlend = map_blend_factor(b.destination, false);
            target.BlendOp = map_blend_op(b.equation);
        }
        if let Some(ref b) = info.alpha {
            target.BlendEnable = TRUE;
            target.SrcBlendAlpha = map_blend_factor(b.source, true);
            target.DestBlendAlpha = map_blend_factor(b.destination, true);
            target.BlendOpAlpha = map_blend_op(b.equation);
        }
    }

    targets
}
