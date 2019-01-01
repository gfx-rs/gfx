#![allow(dead_code)] //TODO: remove

use crate::hal::pso;
use crate::hal::ColorSlot;
use crate::{gl, GlContainer};
use glow::Context;
use smallvec::SmallVec;

pub(crate) fn bind_draw_color_buffers(gl: &GlContainer, num: usize) {
    let attachments: SmallVec<[gl::types::GLenum; 16]> =
        (0..num).map(|x| gl::COLOR_ATTACHMENT0 + x as u32).collect();
    unsafe { gl.draw_buffers(num as gl::types::GLint, attachments.as_ptr()) };
}

pub fn map_comparison(cmp: pso::Comparison) -> glow::Func {
    use crate::hal::pso::Comparison::*;
    use glow::Func as G;
    match cmp {
        Never => G::Never,
        Less => G::Less,
        LessEqual => G::LessEqual,
        Equal => G::Equal,
        GreaterEqual => G::GreaterEqual,
        Greater => G::Greater,
        NotEqual => G::NotEqual,
        Always => G::Always,
    }
}

fn map_operation(op: pso::StencilOp) -> glow::StencilOp {
    use crate::hal::pso::StencilOp::*;
    use glow::StencilOp as SO;
    match op {
        Keep => SO::Keep,
        Zero => SO::Zero,
        Replace => SO::Replace,
        IncrementClamp => SO::Increment,
        IncrementWrap => SO::IncrementWrap,
        DecrementClamp => SO::Decrement,
        DecrementWrap => SO::DecrementWrap,
        Invert => SO::Invert,
    }
}

pub(crate) fn bind_stencil(
    gl: &GlContainer,
    stencil: &pso::StencilTest,
    (ref_front, ref_back): (pso::StencilValue, pso::StencilValue),
    cull: Option<pso::Face>,
) {
    fn bind_side(
        gl: &GlContainer,
        face: glow::Face,
        side: &pso::StencilFace,
        ref_value: pso::StencilValue,
    ) {
        unsafe {
            let mr = match side.mask_read {
                pso::State::Static(v) => v,
                pso::State::Dynamic => !0,
            };
            let mw = match side.mask_write {
                pso::State::Static(v) => v,
                pso::State::Dynamic => !0,
            };
            gl.stencil_func_separate(face, map_comparison(side.fun), ref_value as _, mr);
            gl.stencil_mask_separate(face, mw);
            gl.stencil_op_separate(
                face,
                map_operation(side.op_fail),
                map_operation(side.op_depth_fail),
                map_operation(side.op_pass),
            );
        }
    }
    match *stencil {
        pso::StencilTest::On { ref front, ref back } => {
            unsafe { gl.enable(glow::Parameter::StencilTest) };
            if let Some(cf) = cull {
                if !cf.contains(pso::Face::FRONT) {
                    bind_side(gl, glow::Face::Front, front, ref_front);
                }
                if !cf.contains(pso::Face::BACK) {
                    bind_side(gl, glow::Face::Back, back, ref_back);
                }
            }
        }
        pso::StencilTest::Off => unsafe {
            gl.disable(glow::Parameter::StencilTest);
        },
    }
}

fn map_factor(factor: pso::Factor) -> glow::BlendFactor {
    use crate::hal::pso::Factor::*;
    use glow::BlendFactor as BF;
    match factor {
        Zero => BF::Zero,
        One => BF::One,
        SrcColor => BF::SrcColor,
        OneMinusSrcColor => BF::OneMinusSrcColor,
        DstColor => BF::DstColor,
        OneMinusDstColor => BF::OneMinusDstColor,
        SrcAlpha => BF::SrcAlpha,
        OneMinusSrcAlpha => BF::OneMinusSrcAlpha,
        DstAlpha => BF::DstAlpha,
        OneMinusDstAlpha => BF::OneMinusDstAlpha,
        ConstColor => BF::ConstantColor,
        OneMinusConstColor => BF::OneMinusConstantColor,
        ConstAlpha => BF::ConstantAlpha,
        OneMinusConstAlpha => BF::OneMinusConstantAlpha,
        SrcAlphaSaturate => BF::SrcAlphaSaturate,
        Src1Color => BF::Src1Color,
        OneMinusSrc1Color => BF::OneMinusSrc1Color,
        Src1Alpha => BF::Src1Alpha,
        OneMinusSrc1Alpha => BF::OneMinusSrc1Alpha,
    }
}

fn map_blend_op(operation: pso::BlendOp) -> (glow::BlendMode, glow::BlendFactor, glow::BlendFactor) {
    use glow::BlendMode as BM;
    use glow::BlendFactor as BF;
    match operation {
        pso::BlendOp::Add { src, dst } => (BM::FuncAdd, map_factor(src), map_factor(dst)),
        pso::BlendOp::Sub { src, dst } => (BM::FuncSubtract, map_factor(src), map_factor(dst)),
        pso::BlendOp::RevSub { src, dst } => {
            (BM::FuncReverseSubtract, map_factor(src), map_factor(dst))
        }
        pso::BlendOp::Min => (BM::Min, BF::Zero, BF::Zero),
        pso::BlendOp::Max => (BM::Max, BF::Zero, BF::Zero),
    }
}

pub(crate) fn bind_blend(gl: &GlContainer, desc: &pso::ColorBlendDesc) {
    use crate::hal::pso::ColorMask as Cm;

    match desc.1 {
        pso::BlendState::On { color, alpha } => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(alpha);
            gl.enable(glow::Parameter::Blend);
            gl.blend_equation_separate(color_eq, alpha_eq);
            gl.blend_func_separate(color_src, color_dst, alpha_src, alpha_dst);
        },
        pso::BlendState::Off => unsafe {
            gl.disable(glow::Parameter::Blend);
        },
    };

    unsafe {
        gl.color_mask(
            desc.0.contains(Cm::RED) as _,
            desc.0.contains(Cm::GREEN) as _,
            desc.0.contains(Cm::BLUE) as _,
            desc.0.contains(Cm::ALPHA) as _,
        );
    }
}

pub(crate) fn bind_blend_slot(gl: &GlContainer, slot: ColorSlot, desc: &pso::ColorBlendDesc) {
    use crate::hal::pso::ColorMask as Cm;

    match desc.1 {
        pso::BlendState::On { color, alpha } => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(alpha);
            //Note: using ARB functions as they are more compatible
            gl.enable_i(glow::Parameter::Blend, slot as _);
            gl.blend_equation_separate_i(slot as _, color_eq, alpha_eq);
            gl.blend_func_separate_i(slot as _, color_src, color_dst, alpha_src, alpha_dst);
        },
        pso::BlendState::Off => unsafe {
            gl.disable_i(glow::Parameter::Blend, slot as _);
        },
    };

    unsafe {
        gl.color_mask_i(slot as _,
            desc.0.contains(Cm::RED) as _,
            desc.0.contains(Cm::GREEN) as _,
            desc.0.contains(Cm::BLUE) as _,
            desc.0.contains(Cm::ALPHA) as _,
        );
    }
}

pub(crate) fn unlock_color_mask(gl: &GlContainer) {
    unsafe { gl.color_mask(true, true, true, true) };
}

pub(crate) fn set_blend_color(gl: &GlContainer, color: pso::ColorValue) {
    unsafe { gl.blend_color(color[0], color[1], color[2], color[3]) };
}
