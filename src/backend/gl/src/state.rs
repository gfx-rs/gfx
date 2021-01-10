use crate::{ColorSlot, GlContainer};
use glow::HasContext;
use hal::pso;

pub fn map_comparison(cmp: pso::Comparison) -> u32 {
    use hal::pso::Comparison::*;
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

#[allow(dead_code)]
fn map_operation(op: pso::StencilOp) -> u32 {
    use hal::pso::StencilOp::*;
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

#[allow(dead_code)]
pub(crate) fn bind_stencil(gl: &GlContainer, stencil: &Option<pso::StencilTest>, cull: pso::Face) {
    fn bind_side(
        gl: &GlContainer,
        face: u32,
        side: &pso::StencilFace,
        read_mask: pso::StencilValue,
        ref_value: pso::StencilValue,
    ) {
        unsafe {
            gl.stencil_func_separate(face, map_comparison(side.fun), ref_value as _, read_mask);
            gl.stencil_op_separate(
                face,
                map_operation(side.op_fail),
                map_operation(side.op_depth_fail),
                map_operation(side.op_pass),
            );
        }
    }
    match *stencil {
        Some(ref stencil) => {
            unsafe { gl.enable(glow::STENCIL_TEST) };
            let read_masks = stencil.read_masks.static_or(pso::Sided::new(!0));
            let ref_values = stencil.reference_values.static_or(pso::Sided::new(0));
            if !cull.contains(pso::Face::FRONT) {
                bind_side(
                    gl,
                    glow::FRONT,
                    &stencil.faces.front,
                    read_masks.front,
                    ref_values.front,
                );
                if let pso::State::Static(values) = stencil.write_masks {
                    unsafe {
                        gl.stencil_mask_separate(glow::FRONT, values.front);
                    }
                }
            }
            if !cull.contains(pso::Face::BACK) {
                bind_side(
                    gl,
                    glow::BACK,
                    &stencil.faces.back,
                    read_masks.back,
                    ref_values.back,
                );
                if let pso::State::Static(values) = stencil.write_masks {
                    unsafe {
                        gl.stencil_mask_separate(glow::BACK, values.back);
                    }
                }
            }
        }
        None => unsafe {
            gl.disable(glow::STENCIL_TEST);
        },
    }
}

fn map_factor(factor: pso::Factor) -> u32 {
    use hal::pso::Factor::*;
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
        pso::BlendOp::RevSub { src, dst } => (
            glow::FUNC_REVERSE_SUBTRACT,
            map_factor(src),
            map_factor(dst),
        ),
        pso::BlendOp::Min => (glow::MIN, glow::ZERO, glow::ZERO),
        pso::BlendOp::Max => (glow::MAX, glow::ZERO, glow::ZERO),
    }
}

pub(crate) fn set_blend(gl: &GlContainer, blend: &Option<pso::BlendState>) {
    match blend {
        Some(ref blend) => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(blend.color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(blend.alpha);
            gl.enable(glow::BLEND);
            gl.blend_equation_separate(color_eq, alpha_eq);
            gl.blend_func_separate(color_src, color_dst, alpha_src, alpha_dst);
        },
        None => unsafe {
            gl.disable(glow::BLEND);
        },
    };
}

pub(crate) fn set_blend_slot(
    gl: &GlContainer,
    slot: ColorSlot,
    blend: &Option<pso::BlendState>,
    features: &hal::Features,
) {
    if !features.contains(hal::Features::INDEPENDENT_BLENDING) {
        warn!("independent blending is not supported");
        return;
    }

    match blend {
        Some(ref blend) => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(blend.color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(blend.alpha);
            gl.enable_draw_buffer(glow::BLEND, slot as _);
            gl.blend_equation_separate_draw_buffer(slot as _, color_eq, alpha_eq);
            gl.blend_func_separate_draw_buffer(
                slot as _, color_src, color_dst, alpha_src, alpha_dst,
            );
        },
        None => unsafe {
            gl.disable_draw_buffer(glow::BLEND, slot as _);
        },
    };
}

pub(crate) fn _unlock_color_mask(gl: &GlContainer) {
    unsafe { gl.color_mask(true, true, true, true) };
}

pub(crate) fn set_blend_color(gl: &GlContainer, color: pso::ColorValue) {
    unsafe { gl.blend_color(color[0], color[1], color[2], color[3]) };
}
