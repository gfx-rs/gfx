use core::{ColorSlot};
use core::state as s;
use core::state::{BlendValue, Comparison, CullFace, Equation,
                  Offset, RasterMethod, StencilOp, FrontFace};
use core::target::{ColorValue, Rect, Stencil};
use gl;
use smallvec::SmallVec;

pub fn bind_raster_method(gl: &gl::Gl, method: s::RasterMethod, offset: Option<s::Offset>) {
    let (gl_draw, gl_offset) = match method {
        RasterMethod::Point => (gl::POINT, gl::POLYGON_OFFSET_POINT),
        RasterMethod::Line(width) => {
            unsafe { gl.LineWidth(width as gl::types::GLfloat) };
            (gl::LINE, gl::POLYGON_OFFSET_LINE)
        },
        RasterMethod::Fill => (gl::FILL, gl::POLYGON_OFFSET_FILL),
    };

    unsafe { gl.PolygonMode(gl::FRONT_AND_BACK, gl_draw) };

    match offset {
        Some(Offset(factor, units)) => unsafe {
            gl.Enable(gl_offset);
            gl.PolygonOffset(factor as gl::types::GLfloat,
                             units as gl::types::GLfloat);
        },
        None => unsafe {
            gl.Disable(gl_offset)
        },
    }
}

pub fn bind_rasterizer(gl: &gl::Gl, r: &s::Rasterizer, is_embedded: bool) {
    unsafe {
        gl.FrontFace(match r.front_face {
            FrontFace::Clockwise => gl::CW,
            FrontFace::CounterClockwise => gl::CCW,
        })
    };

    match r.cull_face {
        CullFace::Nothing => unsafe { gl.Disable(gl::CULL_FACE) },
        CullFace::Front => { unsafe {
            gl.Enable(gl::CULL_FACE);
            gl.CullFace(gl::FRONT);
        }},
        CullFace::Back => { unsafe {
            gl.Enable(gl::CULL_FACE);
            gl.CullFace(gl::BACK);
        }}
    }

    if !is_embedded {
        bind_raster_method(gl, r.method, r.offset);
    }
    match r.samples {
        Some(_) => unsafe { gl.Enable(gl::MULTISAMPLE) },
        None => unsafe { gl.Disable(gl::MULTISAMPLE) },
    }
}

pub fn bind_draw_color_buffers(gl: &gl::Gl, num: usize) {
    let attachments: SmallVec<[gl::types::GLenum; 16]> =
        (0..num).map(|x| gl::COLOR_ATTACHMENT0 + x as u32).collect();
    unsafe { gl.DrawBuffers(num as gl::types::GLint, attachments.as_ptr()) };
}

pub fn map_comparison(cmp: Comparison) -> gl::types::GLenum {
    match cmp {
        Comparison::Never        => gl::NEVER,
        Comparison::Less         => gl::LESS,
        Comparison::LessEqual    => gl::LEQUAL,
        Comparison::Equal        => gl::EQUAL,
        Comparison::GreaterEqual => gl::GEQUAL,
        Comparison::Greater      => gl::GREATER,
        Comparison::NotEqual     => gl::NOTEQUAL,
        Comparison::Always       => gl::ALWAYS,
    }
}

pub fn bind_depth(gl: &gl::Gl, depth: &Option<s::Depth>) {
    match depth {
        &Some(ref d) => { unsafe {
            gl.Enable(gl::DEPTH_TEST);
            gl.DepthFunc(map_comparison(d.fun));
            gl.DepthMask(if d.write {gl::TRUE} else {gl::FALSE});
        }},
        &None => unsafe { gl.Disable(gl::DEPTH_TEST) },
    }
}

fn map_operation(op: StencilOp) -> gl::types::GLenum {
    match op {
        StencilOp::Keep          => gl::KEEP,
        StencilOp::Zero          => gl::ZERO,
        StencilOp::Replace       => gl::REPLACE,
        StencilOp::IncrementClamp=> gl::INCR,
        StencilOp::IncrementWrap => gl::INCR_WRAP,
        StencilOp::DecrementClamp=> gl::DECR,
        StencilOp::DecrementWrap => gl::DECR_WRAP,
        StencilOp::Invert        => gl::INVERT,
    }
}

pub fn bind_stencil(gl: &gl::Gl, stencil: &Option<s::Stencil>, refs: (Stencil, Stencil), cull: s::CullFace) {
    fn bind_side(gl: &gl::Gl, face: gl::types::GLenum, side: s::StencilSide, ref_value: Stencil) { unsafe {
        gl.StencilFuncSeparate(face, map_comparison(side.fun),
            ref_value as gl::types::GLint, side.mask_read as gl::types::GLuint);
        gl.StencilMaskSeparate(face, side.mask_write as gl::types::GLuint);
        gl.StencilOpSeparate(face, map_operation(side.op_fail),
            map_operation(side.op_depth_fail), map_operation(side.op_pass));
    }}
    match stencil {
        &Some(ref s) => {
            unsafe { gl.Enable(gl::STENCIL_TEST) };
            if cull != CullFace::Front {
                bind_side(gl, gl::FRONT, s.front, refs.0);
            }
            if cull != CullFace::Back {
                bind_side(gl, gl::BACK, s.back, refs.1);
            }
        }
        &None => unsafe { gl.Disable(gl::STENCIL_TEST) },
    }
}


fn map_equation(eq: Equation) -> gl::types::GLenum {
    match eq {
        Equation::Add    => gl::FUNC_ADD,
        Equation::Sub    => gl::FUNC_SUBTRACT,
        Equation::RevSub => gl::FUNC_REVERSE_SUBTRACT,
        Equation::Min    => gl::MIN,
        Equation::Max    => gl::MAX,
    }
}

fn map_factor(factor: s::Factor) -> gl::types::GLenum {
    match factor {
        s::Factor::Zero                              => gl::ZERO,
        s::Factor::One                               => gl::ONE,
        s::Factor::ZeroPlus(BlendValue::SourceColor) => gl::SRC_COLOR,
        s::Factor::OneMinus(BlendValue::SourceColor) => gl::ONE_MINUS_SRC_COLOR,
        s::Factor::ZeroPlus(BlendValue::SourceAlpha) => gl::SRC_ALPHA,
        s::Factor::OneMinus(BlendValue::SourceAlpha) => gl::ONE_MINUS_SRC_ALPHA,
        s::Factor::ZeroPlus(BlendValue::DestColor)   => gl::DST_COLOR,
        s::Factor::OneMinus(BlendValue::DestColor)   => gl::ONE_MINUS_DST_COLOR,
        s::Factor::ZeroPlus(BlendValue::DestAlpha)   => gl::DST_ALPHA,
        s::Factor::OneMinus(BlendValue::DestAlpha)   => gl::ONE_MINUS_DST_ALPHA,
        s::Factor::ZeroPlus(BlendValue::ConstColor)  => gl::CONSTANT_COLOR,
        s::Factor::OneMinus(BlendValue::ConstColor)  => gl::ONE_MINUS_CONSTANT_COLOR,
        s::Factor::ZeroPlus(BlendValue::ConstAlpha)  => gl::CONSTANT_ALPHA,
        s::Factor::OneMinus(BlendValue::ConstAlpha)  => gl::ONE_MINUS_CONSTANT_ALPHA,
        s::Factor::SourceAlphaSaturated => gl::SRC_ALPHA_SATURATE,
    }
}

pub fn bind_blend(gl: &gl::Gl, color: s::Color) {
    match color.blend {
        Some(b) => unsafe {
            gl.Enable(gl::BLEND);
            gl.BlendEquationSeparate(
                map_equation(b.color.equation),
                map_equation(b.alpha.equation)
            );
            gl.BlendFuncSeparate(
                map_factor(b.color.source),
                map_factor(b.color.destination),
                map_factor(b.alpha.source),
                map_factor(b.alpha.destination)
            );
        },
        None => unsafe {
            gl.Disable(gl::BLEND);
        },
    };
    unsafe { gl.ColorMask(
        if (color.mask & s::RED  ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::GREEN).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::BLUE ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::ALPHA).is_empty() {gl::FALSE} else {gl::TRUE}
    )};
}

pub fn bind_blend_slot(gl: &gl::Gl, slot: ColorSlot, color: s::Color) {
    let buf = slot as gl::types::GLuint;
    match color.blend {
        Some(b) => unsafe {
            //Note: using ARB functions as they are more compatible
            gl.Enablei(gl::BLEND, buf);
            gl.BlendEquationSeparateiARB(buf,
                map_equation(b.color.equation),
                map_equation(b.alpha.equation)
            );
            gl.BlendFuncSeparateiARB(buf,
                map_factor(b.color.source),
                map_factor(b.color.destination),
                map_factor(b.alpha.source),
                map_factor(b.alpha.destination)
            );
        },
        None => unsafe {
            gl.Disablei(gl::BLEND, buf);
        },
    };
    unsafe { gl.ColorMaski(buf,
        if (color.mask & s::RED  ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::GREEN).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::BLUE ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (color.mask & s::ALPHA).is_empty() {gl::FALSE} else {gl::TRUE}
    )};
}

pub fn unlock_color_mask(gl: &gl::Gl) {
    unsafe { gl.ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE) };
}

pub fn set_blend_color(gl: &gl::Gl, color: ColorValue) {
    unsafe {
        gl.BlendColor(color[0], color[1], color[2], color[3])
    };
}
