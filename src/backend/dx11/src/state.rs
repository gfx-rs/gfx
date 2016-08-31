// Copyright 2016 The Gfx-rs Developers.
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

use std::ptr;
use winapi::*;
use core::{pso, state};
use data::map_function;

pub fn make_rasterizer(device: *mut ID3D11Device, rast: &state::Rasterizer, use_scissor: bool)
                       -> *const ID3D11RasterizerState {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: match rast.method {
            state::RasterMethod::Point => {
                error!("Point rasterization is not supported");
                D3D11_FILL_WIREFRAME
            },
            state::RasterMethod::Line(_) => D3D11_FILL_WIREFRAME,
            state::RasterMethod::Fill => D3D11_FILL_SOLID,
        },
        CullMode: match rast.cull_face {
            state::CullFace::Nothing => D3D11_CULL_NONE,
            state::CullFace::Front => D3D11_CULL_FRONT,
            state::CullFace::Back => D3D11_CULL_BACK,
        },
        FrontCounterClockwise: match rast.front_face {
            state::FrontFace::Clockwise => FALSE,
            state::FrontFace::CounterClockwise => TRUE,
        },
        DepthBias: match rast.offset {
            Some(ref o) => o.1 as INT,
            None => 0,
        },
        DepthBiasClamp: 16.0,
        SlopeScaledDepthBias: match rast.offset {
            Some(ref o) => o.0 as FLOAT,
            None => 0.0,
        },
        DepthClipEnable: TRUE,
        ScissorEnable: if use_scissor {TRUE} else {FALSE},
        MultisampleEnable: match rast.samples {
            Some(_) => TRUE,
            None => FALSE,
        },
        AntialiasedLineEnable: FALSE,
    };

    let mut handle = ptr::null_mut();
    let hr = unsafe {
        (*device).CreateRasterizerState(&desc, &mut handle)
    };
    if !SUCCEEDED(hr) {
        error!("Failed to create rasterizer state {:?}, descriptor {:#?}, code {:x}", rast, desc, hr);
    }
    handle as *const _
}

fn map_stencil_op(oper: state::StencilOp) -> D3D11_STENCIL_OP {
    use core::state::StencilOp::*;
    match oper {
        Keep => D3D11_STENCIL_OP_KEEP,
        Zero => D3D11_STENCIL_OP_ZERO,
        Replace => D3D11_STENCIL_OP_REPLACE,
        IncrementClamp => D3D11_STENCIL_OP_INCR_SAT,
        IncrementWrap => D3D11_STENCIL_OP_INCR,
        DecrementClamp => D3D11_STENCIL_OP_DECR_SAT,
        DecrementWrap => D3D11_STENCIL_OP_DECR,
        Invert => D3D11_STENCIL_OP_INVERT,
    }
}

fn map_stencil_side(side_: &Option<state::StencilSide>) -> D3D11_DEPTH_STENCILOP_DESC {
    let side = side_.unwrap_or_default();
    D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: map_stencil_op(side.op_fail),
        StencilDepthFailOp: map_stencil_op(side.op_depth_fail),
        StencilPassOp: map_stencil_op(side.op_pass),
        StencilFunc: map_function(side.fun),
    }
}

fn map_stencil_mask<F>(dsi: &pso::DepthStencilInfo, name: &str, accessor: F) -> UINT8
    where F: Fn(&state::StencilSide) -> UINT8 {
    match (dsi.front, dsi.back) {
        (Some(ref front), Some(ref back)) if accessor(front) != accessor(back) => {
            error!("Different {} masks on stencil front ({}) and back ({}) are not supported",
                name, accessor(front), accessor(back));
            accessor(front)
        },
        (Some(ref front), _) => accessor(front),
        (_, Some(ref back)) => accessor(back),
        (None, None) => 0,
    }
}

pub fn make_depth_stencil(device: *mut ID3D11Device, dsi: &pso::DepthStencilInfo)
                          -> *const ID3D11DepthStencilState {
    let desc = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: if dsi.depth.is_some() {TRUE} else {FALSE},
        DepthWriteMask: D3D11_DEPTH_WRITE_MASK(match dsi.depth {
            Some(ref d) if d.write => 1,
            _ => 0,
        }),
        DepthFunc: match dsi.depth {
            Some(ref d) => map_function(d.fun),
            None => D3D11_COMPARISON_NEVER,
        },
        StencilEnable: if dsi.front.is_some() || dsi.back.is_some() {TRUE} else {FALSE},
        StencilReadMask: map_stencil_mask(dsi, "read", |s| (s.mask_read as UINT8)),
        StencilWriteMask: map_stencil_mask(dsi, "write", |s| (s.mask_write as UINT8)),
        FrontFace: map_stencil_side(&dsi.front),
        BackFace: map_stencil_side(&dsi.back),
    };

    let mut handle = ptr::null_mut();
    let hr = unsafe {
        (*device).CreateDepthStencilState(&desc, &mut handle)
    };
    if !SUCCEEDED(hr) {
        error!("Failed to create depth-stencil state {:?}, descriptor {:#?}, error {:x}", dsi, desc, hr);
    }
    handle as *const _
}

fn map_blend_factor(factor: state::Factor, scalar: bool) -> D3D11_BLEND {
    use core::state::BlendValue::*;
    use core::state::Factor::*;
    match factor {
        Zero => D3D11_BLEND_ZERO,
        One => D3D11_BLEND_ONE,
        SourceAlphaSaturated => D3D11_BLEND_SRC_ALPHA_SAT,
        ZeroPlus(SourceColor) if !scalar => D3D11_BLEND_SRC_COLOR,
        ZeroPlus(SourceAlpha) => D3D11_BLEND_SRC_ALPHA,
        ZeroPlus(DestColor) if !scalar => D3D11_BLEND_DEST_COLOR,
        ZeroPlus(DestAlpha) => D3D11_BLEND_DEST_ALPHA,
        ZeroPlus(ConstColor) if !scalar => D3D11_BLEND_BLEND_FACTOR,
        ZeroPlus(ConstAlpha) => D3D11_BLEND_BLEND_FACTOR,
        OneMinus(SourceColor) if !scalar => D3D11_BLEND_INV_SRC_COLOR,
        OneMinus(SourceAlpha) => D3D11_BLEND_INV_SRC_ALPHA,
        OneMinus(DestColor) if !scalar => D3D11_BLEND_INV_DEST_COLOR,
        OneMinus(DestAlpha) => D3D11_BLEND_INV_DEST_ALPHA,
        OneMinus(ConstColor) if !scalar => D3D11_BLEND_INV_BLEND_FACTOR,
        OneMinus(ConstAlpha) => D3D11_BLEND_INV_BLEND_FACTOR,
        _ => {
            error!("Invalid blend factor requested for {}: {:?}",
                if scalar {"alpha"} else {"color"}, factor);
            D3D11_BLEND_ZERO
        }
    }
}

fn map_blend_op(equation: state::Equation) -> D3D11_BLEND_OP {
    use core::state::Equation::*;
    match equation {
        Add => D3D11_BLEND_OP_ADD,
        Sub => D3D11_BLEND_OP_SUBTRACT,
        RevSub => D3D11_BLEND_OP_REV_SUBTRACT,
        Min => D3D11_BLEND_OP_MIN,
        Max => D3D11_BLEND_OP_MAX,
    }
}

pub fn make_blend(device: *mut ID3D11Device, targets: &[Option<pso::ColorTargetDesc>])
                  -> *const ID3D11BlendState {
    let dummy_target = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: FALSE,
        SrcBlend: D3D11_BLEND_ZERO,
        DestBlend: D3D11_BLEND_ONE,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ZERO,
        DestBlendAlpha: D3D11_BLEND_ONE,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: 0xF,
    };
    let mut desc = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: FALSE, //TODO
        IndependentBlendEnable: match targets[1..].iter().find(|t| t.is_some()) {
            Some(_) => TRUE,
            None => FALSE,
        },
        RenderTarget: [dummy_target; 8],
    };

    for (out, odesc) in desc.RenderTarget.iter_mut().zip(targets.iter()) {
        let info = match odesc {
            &Some(ref d) => &d.1,
            &None => continue,
        };
        out.RenderTargetWriteMask = info.mask.bits() as UINT8;
        if let Some(ref b) = info.color {
            out.BlendEnable = TRUE;
            out.SrcBlend = map_blend_factor(b.source, false);
            out.DestBlend = map_blend_factor(b.destination, false);
            out.BlendOp = map_blend_op(b.equation);
        }
        if let Some(ref b) = info.alpha {
            out.BlendEnable = TRUE;
            out.SrcBlendAlpha = map_blend_factor(b.source, true);
            out.DestBlendAlpha = map_blend_factor(b.destination, true);
            out.BlendOpAlpha = map_blend_op(b.equation);
        }
    }

    let mut handle = ptr::null_mut();
    let hr = unsafe {
        (*device).CreateBlendState(&desc, &mut handle)
    };
    if !SUCCEEDED(hr) {
        error!("Failed to create blend state {:?}, descriptor {:#?}, error {:x}", targets, desc, hr);
    }
    handle as *const _
}
