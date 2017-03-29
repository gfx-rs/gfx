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

use core::Primitive;
use core::state;
use ash::vk;

pub fn map_topology(prim: Primitive) -> vk::PrimitiveTopology {
    match prim {
        Primitive::PointList     => vk::PrimitiveTopology::PointList,
        Primitive::LineList      => vk::PrimitiveTopology::LineList,
        Primitive::LineStrip     => vk::PrimitiveTopology::LineStrip,
        Primitive::TriangleList  => vk::PrimitiveTopology::TriangleList,
        Primitive::TriangleStrip => vk::PrimitiveTopology::TriangleStrip,
        Primitive::PatchList(_)  => vk::PrimitiveTopology::PatchList,
    }
}

pub fn map_polygon_mode(rm: state::RasterMethod) -> (vk::PolygonMode, f32) {
    match rm {
        state::RasterMethod::Point   => (vk::PolygonMode::Point, 1.0),
        state::RasterMethod::Line(w) => (vk::PolygonMode::Line, w as f32),
        state::RasterMethod::Fill    => (vk::PolygonMode::Fill, 1.0),
    }
}

pub fn map_cull_mode(cf: state::CullFace) -> vk::CullModeFlags {
    match cf {
        state::CullFace::Nothing => vk::CULL_MODE_NONE,
        state::CullFace::Front   => vk::CULL_MODE_FRONT_BIT,
        state::CullFace::Back    => vk::CULL_MODE_BACK_BIT,
    }
}

pub fn map_front_face(ff: state::FrontFace) -> vk::FrontFace {
    match ff {
        state::FrontFace::Clockwise        => vk::FrontFace::Clockwise,
        state::FrontFace::CounterClockwise => vk::FrontFace::CounterClockwise,
    }
}

pub fn map_comparison(fun: state::Comparison) -> vk::CompareOp {
    use core::state::Comparison::*;
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

pub fn map_stencil_op(op: state::StencilOp) -> vk::StencilOp {
    use core::state::StencilOp::*;
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

pub fn map_stencil_side(side: &state::StencilSide) -> vk::StencilOpState {
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

pub fn map_blend_factor(factor: state::Factor, scalar: bool) -> vk::BlendFactor {
    use core::state::BlendValue::*;
    use core::state::Factor::*;
    match factor {
        Zero => vk::BlendFactor::Zero,
        One => vk::BlendFactor::One,
        SourceAlphaSaturated => vk::BlendFactor::SrcAlphaSaturate,
        ZeroPlus(SourceColor) if !scalar => vk::BlendFactor::SrcColor,
        ZeroPlus(SourceAlpha) => vk::BlendFactor::SrcAlpha,
        ZeroPlus(DestColor) if !scalar => vk::BlendFactor::DstColor,
        ZeroPlus(DestAlpha) => vk::BlendFactor::DstAlpha,
        ZeroPlus(ConstColor) if !scalar => vk::BlendFactor::ConstantColor,
        ZeroPlus(ConstAlpha) => vk::BlendFactor::ConstantAlpha,
        OneMinus(SourceColor) if !scalar => vk::BlendFactor::OneMinusSrcColor,
        OneMinus(SourceAlpha) => vk::BlendFactor::OneMinusSrcAlpha,
        OneMinus(DestColor) if !scalar => vk::BlendFactor::OneMinusDstColor,
        OneMinus(DestAlpha) => vk::BlendFactor::OneMinusDstAlpha,
        OneMinus(ConstColor) if !scalar => vk::BlendFactor::OneMinusConstantColor,
        OneMinus(ConstAlpha) => vk::BlendFactor::OneMinusConstantAlpha,
        _ => {
            error!("Invalid blend factor requested for {}: {:?}",
                if scalar {"alpha"} else {"color"}, factor);
            vk::BlendFactor::Zero
        }
    }
}

pub fn map_blend_op(equation: state::Equation) -> vk::BlendOp {
    use core::state::Equation::*;
    match equation {
        Add => vk::BlendOp::Add,
        Sub => vk::BlendOp::Subtract,
        RevSub => vk::BlendOp::ReverseSubtract,
        Min => vk::BlendOp::Min,
        Max => vk::BlendOp::Max,
    }
}
