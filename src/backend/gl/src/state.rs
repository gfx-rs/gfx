#![allow(dead_code)] //TODO: remove

use hal::{ColorSlot};
use hal::pso;
use {gl, GlContainer};
use smallvec::SmallVec;

pub(crate) fn bind_polygon_mode(gl: &GlContainer, mode: pso::PolygonMode, bias: Option<pso::DepthBias>) {
    use hal::pso::PolygonMode::*;

    let (gl_draw, gl_offset) = match mode {
        Point => (gl::POINT, gl::POLYGON_OFFSET_POINT),
        Line(width) => {
            unsafe { gl.LineWidth(width) };
            (gl::LINE, gl::POLYGON_OFFSET_LINE)
        },
        Fill => (gl::FILL, gl::POLYGON_OFFSET_FILL),
    };

    unsafe { gl.PolygonMode(gl::FRONT_AND_BACK, gl_draw) };

    match bias {
        Some(bias) => unsafe {
            gl.Enable(gl_offset);
            gl.PolygonOffset(bias.slope_factor as _, bias.const_factor as _);
        },
        None => unsafe {
            gl.Disable(gl_offset)
        },
    }
}

pub(crate) fn bind_rasterizer(gl: &GlContainer, r: &pso::Rasterizer, is_embedded: bool) {
    use hal::pso::FrontFace::*;

    unsafe {
        gl.FrontFace(match r.front_face {
            Clockwise => gl::CW,
            CounterClockwise => gl::CCW,
        })
    };

    if !r.cull_face.is_empty() {
        unsafe {
            gl.Enable(gl::CULL_FACE);
            gl.CullFace(match r.cull_face {
                pso::Face::FRONT => gl::FRONT,
                pso::Face::BACK => gl::BACK,
                _ => gl::FRONT_AND_BACK,
            });
        }
    } else {
        unsafe {
            gl.Disable(gl::CULL_FACE);
        }
    }

    if !is_embedded {
        bind_polygon_mode(gl, r.polygon_mode, r.depth_bias);
        match false { //TODO
            true => unsafe { gl.Enable(gl::MULTISAMPLE) },
            false => unsafe { gl.Disable(gl::MULTISAMPLE) },
        }
    }
}

pub(crate) fn bind_draw_color_buffers(gl: &GlContainer, num: usize) {
    let attachments: SmallVec<[gl::types::GLenum; 16]> =
        (0..num).map(|x| gl::COLOR_ATTACHMENT0 + x as u32).collect();
    unsafe { gl.DrawBuffers(num as gl::types::GLint, attachments.as_ptr()) };
}

pub(crate) fn map_comparison(cmp: pso::Comparison) -> gl::types::GLenum {
    use hal::pso::Comparison::*;
    match cmp {
        Never        => gl::NEVER,
        Less         => gl::LESS,
        LessEqual    => gl::LEQUAL,
        Equal        => gl::EQUAL,
        GreaterEqual => gl::GEQUAL,
        Greater      => gl::GREATER,
        NotEqual     => gl::NOTEQUAL,
        Always       => gl::ALWAYS,
    }
}

pub(crate) fn bind_depth(gl: &GlContainer, depth: &pso::DepthTest) {
    match *depth {
        pso::DepthTest::On { fun, write } => unsafe {
            gl.Enable(gl::DEPTH_TEST);
            gl.DepthFunc(map_comparison(fun));
            gl.DepthMask(write as _);
        },
        pso::DepthTest::Off => unsafe {
            gl.Disable(gl::DEPTH_TEST);
        },
    }
}

fn map_operation(op: pso::StencilOp) -> gl::types::GLenum {
    use hal::pso::StencilOp::*;
    match op {
        Keep          => gl::KEEP,
        Zero          => gl::ZERO,
        Replace       => gl::REPLACE,
        IncrementClamp=> gl::INCR,
        IncrementWrap => gl::INCR_WRAP,
        DecrementClamp=> gl::DECR,
        DecrementWrap => gl::DECR_WRAP,
        Invert        => gl::INVERT,
    }
}

pub(crate) fn bind_stencil(
    gl: &GlContainer,
    stencil: &pso::StencilTest,
    (ref_front, ref_back): (pso::StencilValue, pso::StencilValue),
    cull: Option<pso::Face>,
) {
    fn bind_side(gl: &GlContainer, face: gl::types::GLenum, side: &pso::StencilFace, ref_value: pso::StencilValue) {
        unsafe {
            let mr = match side.mask_read {
                pso::State::Static(v) => v,
                pso::State::Dynamic => !0,
            };
            let mw = match side.mask_write {
                pso::State::Static(v) => v,
                pso::State::Dynamic => !0,
            };
            gl.StencilFuncSeparate(face, map_comparison(side.fun), ref_value as _, mr);
            gl.StencilMaskSeparate(face, mw);
            gl.StencilOpSeparate(face, map_operation(side.op_fail), map_operation(side.op_depth_fail), map_operation(side.op_pass));
        }
    }
    match *stencil {
        pso::StencilTest::On { ref front, ref back } => {
            unsafe { gl.Enable(gl::STENCIL_TEST) };
            if let Some(cf) = cull {
                if !cf.contains(pso::Face::FRONT) {
                    bind_side(gl, gl::FRONT, front, ref_front);
                }
                if !cf.contains(pso::Face::BACK) {
                    bind_side(gl, gl::BACK, back, ref_back);
                }
            }
        }
        pso::StencilTest::Off => unsafe {
            gl.Disable(gl::STENCIL_TEST);
        },
    }
}

fn map_factor(factor: pso::Factor) -> gl::types::GLenum {
    use hal::pso::Factor::*;
    match factor {
        Zero => gl::ZERO,
        One => gl::ONE,
        SrcColor => gl::SRC_COLOR,
        OneMinusSrcColor => gl::ONE_MINUS_SRC_COLOR,
        DstColor => gl::DST_COLOR,
        OneMinusDstColor => gl::ONE_MINUS_DST_COLOR,
        SrcAlpha => gl::SRC_ALPHA,
        OneMinusSrcAlpha => gl::ONE_MINUS_SRC_ALPHA,
        DstAlpha => gl::DST_ALPHA,
        OneMinusDstAlpha => gl::ONE_MINUS_DST_ALPHA,
        ConstColor => gl::CONSTANT_COLOR,
        OneMinusConstColor => gl::ONE_MINUS_CONSTANT_COLOR,
        ConstAlpha => gl::CONSTANT_ALPHA,
        OneMinusConstAlpha => gl::ONE_MINUS_CONSTANT_ALPHA,
        SrcAlphaSaturate => gl::SRC_ALPHA_SATURATE,
        Src1Color => gl::SRC1_COLOR,
        OneMinusSrc1Color => gl::ONE_MINUS_SRC1_COLOR,
        Src1Alpha => gl::SRC1_ALPHA,
        OneMinusSrc1Alpha => gl::ONE_MINUS_SRC1_ALPHA,
    }
}

fn map_blend_op(operation: pso::BlendOp) -> (gl::types::GLenum, gl::types::GLenum, gl::types::GLenum) {
    match operation {
        pso::BlendOp::Add { src, dst }    => (gl::FUNC_ADD,              map_factor(src), map_factor(dst)),
        pso::BlendOp::Sub { src, dst }    => (gl::FUNC_SUBTRACT,         map_factor(src), map_factor(dst)),
        pso::BlendOp::RevSub { src, dst } => (gl::FUNC_REVERSE_SUBTRACT, map_factor(src), map_factor(dst)),
        pso::BlendOp::Min => (gl::MIN, gl::ZERO, gl::ZERO),
        pso::BlendOp::Max => (gl::MAX, gl::ZERO, gl::ZERO),
    }
}

pub(crate) fn bind_blend(gl: &GlContainer, desc: &pso::ColorBlendDesc) {
    use hal::pso::ColorMask as Cm;

    match desc.1 {
        pso::BlendState::On { color, alpha } => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(alpha);
            gl.Enable(gl::BLEND);
            gl.BlendEquationSeparate(color_eq, alpha_eq);
            gl.BlendFuncSeparate(color_src, color_dst, alpha_src, alpha_dst);
        },
        pso::BlendState::Off => unsafe {
            gl.Disable(gl::BLEND);
        },
    };

    unsafe { gl.ColorMask(
        desc.0.contains(Cm::RED) as _,
        desc.0.contains(Cm::GREEN) as _,
        desc.0.contains(Cm::BLUE) as _,
        desc.0.contains(Cm::ALPHA) as _,
    )};
}

pub(crate) fn bind_blend_slot(gl: &GlContainer, slot: ColorSlot, desc: &pso::ColorBlendDesc) {
    use hal::pso::ColorMask as Cm;

    match desc.1 {
        pso::BlendState::On { color, alpha } => unsafe {
            let (color_eq, color_src, color_dst) = map_blend_op(color);
            let (alpha_eq, alpha_src, alpha_dst) = map_blend_op(alpha);
            //Note: using ARB functions as they are more compatible
            gl.Enablei(gl::BLEND, slot as _);
            gl.BlendEquationSeparateiARB(slot as _, color_eq, alpha_eq);
            gl.BlendFuncSeparateiARB(slot as _, color_src, color_dst, alpha_src, alpha_dst);
        },
        pso::BlendState::Off => unsafe {
            gl.Disablei(gl::BLEND, slot as _);
        },
    };

    unsafe { gl.ColorMaski(slot as _,
        desc.0.contains(Cm::RED) as _,
        desc.0.contains(Cm::GREEN) as _,
        desc.0.contains(Cm::BLUE) as _,
        desc.0.contains(Cm::ALPHA) as _,
    )};
}

pub(crate) fn unlock_color_mask(gl: &GlContainer) {
    unsafe { gl.ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE) };
}

pub(crate) fn set_blend_color(gl: &GlContainer, color: pso::ColorValue) {
    unsafe {
        gl.BlendColor(color[0], color[1], color[2], color[3])
    };
}
