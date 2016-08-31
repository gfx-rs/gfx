// Copyright 2014 The Gfx-rs Developers.
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

use core::{MAX_COLOR_TARGETS, ColorSlot};
use core::state as s;
use core::state::{BlendValue, Comparison, CullFace, Equation,
                  Offset, RasterMethod, StencilOp, FrontFace};
use core::target::{ColorValue, Rect, Stencil};
use gl;


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

pub fn bind_draw_color_buffers(gl: &gl::Gl, mask: usize) {
    let attachments = [
        gl::COLOR_ATTACHMENT0,  gl::COLOR_ATTACHMENT1,  gl::COLOR_ATTACHMENT2,
        gl::COLOR_ATTACHMENT3,  gl::COLOR_ATTACHMENT4,  gl::COLOR_ATTACHMENT5,
        gl::COLOR_ATTACHMENT6,  gl::COLOR_ATTACHMENT7,  gl::COLOR_ATTACHMENT8,
        gl::COLOR_ATTACHMENT9,  gl::COLOR_ATTACHMENT10, gl::COLOR_ATTACHMENT11,
        gl::COLOR_ATTACHMENT12, gl::COLOR_ATTACHMENT13, gl::COLOR_ATTACHMENT14,
        gl::COLOR_ATTACHMENT15];
    let mut targets = [0; MAX_COLOR_TARGETS];
    let mut count = 0;
    let mut i = 0;
    while mask >> i != 0 {
        if mask & (1<<i) != 0 {
            targets[count] = attachments[i];
            count += 1;
        }
        i += 1;
    }
    unsafe { gl.DrawBuffers(count as gl::types::GLint, targets.as_ptr()) };
}

pub fn bind_viewport(gl: &gl::Gl, rect: Rect) {
    unsafe { gl.Viewport(
        rect.x as gl::types::GLint,
        rect.y as gl::types::GLint,
        rect.w as gl::types::GLint,
        rect.h as gl::types::GLint
    )};
}

pub fn bind_scissor(gl: &gl::Gl, rect: Option<Rect>) {
    match rect {
        Some(r) => { unsafe {
            gl.Enable(gl::SCISSOR_TEST);
            gl.Scissor(
                r.x as gl::types::GLint,
                r.y as gl::types::GLint,
                r.w as gl::types::GLint,
                r.h as gl::types::GLint
            );
        }},
        None => unsafe { gl.Disable(gl::SCISSOR_TEST) },
    }
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
