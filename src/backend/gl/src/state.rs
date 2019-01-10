#![allow(dead_code)] //TODO: remove

use crate::hal::{pso, ColorSlot};
use crate::GlContainer;
use glow::Context;
use smallvec::SmallVec;

pub(crate) fn bind_draw_color_buffers(gl: &GlContainer, num: usize) {
    let attachments: SmallVec<[u32; 16]> =
        (0..num).map(|x| glow::COLOR_ATTACHMENT0 + x as u32).collect();
    unsafe { gl.draw_buffers(&attachments) };
}

pub fn map_comparison(cmp: pso::Comparison) -> u32 {
    use crate::hal::pso::Comparison::*;
    match cmp {
        Never => glow::NEVER,
        Less => glow::LESS,
        LessEqual => glow::LEQUAL,
        Equal => glow::EQUAL,
        GreaterEqual => glow::GEQUAL,
        Greater => glow::GREATER,
        NotEqual => glow::NOTEQUAL,
        Always => glow::ALWAYS,
    }
}

fn map_operation(op: pso::StencilOp) -> glow::StencilOp {
    use crate::hal::pso::StencilOp::*;
    match op {
        Keep => glow::KEEP,
        Zero => glow::ZERO,
        Replace => glow::REPLACE,
        IncrementClamp => glow::INCR,
        IncrementWrap => glow::INCR_WRAP,
        DecrementClamp => glow::DECR,
        DecrementWrap => glow::DECR_WRAP,
        Invert => glow::INVERT,
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
        face: u32,
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
            unsafe { gl.enable(glow::STENCIL_TEST) };
            if let Some(cf) = cull {
                if !cf.contains(pso::Face::FRONT) {
                    bind_side(gl, glow::FRONT, front, ref_front);
                }
                if !cf.contains(pso::Face::BACK) {
                    bind_side(gl, glow::BACK, back, ref_back);
                }
            }
        }
        pso::StencilTest::Off => unsafe {
            gl.disable(glow::STENCIL_TEST);
        },
    }
}

fn map_factor(factor: pso::Factor) -> u32 {
    use crate::hal::pso::Factor::*;
    match factor {
        Zero => glow::ZERO,
        One => glow::ONE,
        SrcColor => glow::SRC_COLOR,
        OneMinusSrcColor => glow::ONE_MINUS_SRC_COLOR,
        DstColor => glow::DST_COLOR,
        OneMinusDstColor => glow::ONE_MINUS_DST_COLOR,
        SrcAlpha => glow::SRC_ALPHA,
        OneMinusSrcAlpha => glow::ONE_MINUS_SRC_ALPHA,
        DstAlpha => glow::DST_ALPHA,
        OneMinusDstAlpha => glow::ONE_MINUS_DST_ALPHA,
        ConstColor => glow::CONSTANT_COLOR,
        OneMinusConstColor => glow::ONE_MINUS_CONSTANT_COLOR,
        ConstAlpha => glow::CONSTANT_ALPHA,
        OneMinusConstAlpha => glow::ONE_MINUS_CONSTANT_ALPHA,
        SrcAlphaSaturate => glow::SRC_ALPHA_SATURATE,
        Src1Color => glow::SRC1_COLOR,
        OneMinusSrc1Color => glow::ONE_MINUS_SRC1_COLOR,
        Src1Alpha => glow::SRC1_ALPHA,
        OneMinusSrc1Alpha => glow::ONE_MINUS_SRC1_ALPHA,
    }
}

fn map_blend_op(operation: pso::BlendOp) -> (u32, u32, u32) {
    match operation {
        pso::BlendOp::Add { src, dst } => (glow::FUNC_ADD, map_factor(src), map_factor(dst)),
        pso::BlendOp::Sub { src, dst } => (glow::FUNC_SUBTRACT, map_factor(src), map_factor(dst)),
        pso::BlendOp::RevSub { src, dst } => {
            (glow::FUNC_REVERSE_SUBTRACT, map_factor(src), map_factor(dst))
        }
        pso::BlendOp::Min => (glow::MIN, glow::ZERO, glow::ZERO),
        pso::BlendOp::Max => (glow::MAX, glow::ZERO, glow::ZERO),
    }
}

pub(crate) fn bind_blend(gl: &GlContainer, desc: &pso::ColorBlendDesc) {
    use crate::hal::pso::ColorMask as Cm;

    match desc.1 {
        pso::BlendState::On { color, alpha } => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(alpha);
            gl.enable(glow::BLEND);
            gl.blend_equation_separate(color_eq, alpha_eq);
            gl.blend_func_separate(color_src, color_dst, alpha_src, alpha_dst);
        },
        pso::BlendState::Off => unsafe {
            gl.disable(glow::BLEND);
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
            #[cfg(not(target_arch = "wasm32"))] // TODO
            {
                gl.enable_draw_buffer(glow::BLEND, slot as _);
                gl.blend_equation_separate_draw_buffer(slot as _, color_eq, alpha_eq);
                gl.blend_func_separate_draw_buffer(slot as _, color_src, color_dst, alpha_src, alpha_dst);
            }
        },
        pso::BlendState::Off => unsafe {
            gl.disable_draw_buffer(glow::BLEND, slot as _);
        },
    };

    #[cfg(not(target_arch = "wasm32"))] // TODO
    unsafe {
        gl.color_mask_draw_buffer(
            slot as _,
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
