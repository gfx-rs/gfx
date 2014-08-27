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

use super::super::state as s;
use super::super::target::Rect;
use super::gl;

pub fn bind_primitive(gl: &gl::Gl, p: s::Primitive) {
    gl.FrontFace(match p.front_face {
        s::Clockwise => gl::CW,
        s::CounterClockwise => gl::CCW,
    });

    let (gl_draw, gl_offset) = match p.method {
        s::Point => (gl::POINT, gl::POLYGON_OFFSET_POINT),
        s::Line(width) => {
            gl.LineWidth(width);
            (gl::LINE, gl::POLYGON_OFFSET_LINE)
        },
        s::Fill(cull) => {
            match cull {
                s::CullNothing => gl.Disable(gl::CULL_FACE),
                s::CullFront => {
                    gl.Enable(gl::CULL_FACE);
                    gl.CullFace(gl::FRONT);
                },
                s::CullBack => {
                    gl.Enable(gl::CULL_FACE);
                    gl.CullFace(gl::BACK);
                },
            }
            (gl::FILL, gl::POLYGON_OFFSET_FILL)
        },
    };

    gl.PolygonMode(gl::FRONT_AND_BACK, gl_draw);

    match p.offset {
        s::Offset(factor, units) => {
            gl.Enable(gl_offset);
            gl.PolygonOffset(factor, units as gl::types::GLfloat);
        },
        s::NoOffset => gl.Disable(gl_offset),
    }
}

pub fn bind_viewport(gl: &gl::Gl, rect: Rect) {
    gl.Viewport(
        rect.x as gl::types::GLint,
        rect.y as gl::types::GLint,
        rect.w as gl::types::GLint,
        rect.h as gl::types::GLint
    );
}

pub fn bind_scissor(gl: &gl::Gl, rect: Option<Rect>) {
    match rect {
        Some(r) => {
            gl.Enable(gl::SCISSOR_TEST);
            gl.Scissor(
                r.x as gl::types::GLint,
                r.y as gl::types::GLint,
                r.w as gl::types::GLint,
                r.h as gl::types::GLint
            );
        },
        None => gl.Disable(gl::SCISSOR_TEST),
    }
}

fn map_comparison(cmp: s::Comparison) -> gl::types::GLenum {
    match cmp {
        s::Never        => gl::NEVER,
        s::Less         => gl::LESS,
        s::LessEqual    => gl::LEQUAL,
        s::Equal        => gl::EQUAL,
        s::GreaterEqual => gl::GEQUAL,
        s::Greater      => gl::GREATER,
        s::NotEqual     => gl::NOTEQUAL,
        s::Always       => gl::ALWAYS,
    }
}

pub fn bind_depth(gl: &gl::Gl, depth: Option<s::Depth>) {
    match depth {
        Some(d) => {
            gl.Enable(gl::DEPTH_TEST);
            gl.DepthFunc(map_comparison(d.fun));
            gl.DepthMask(if d.write {gl::TRUE} else {gl::FALSE});
        },
        None => gl.Disable(gl::DEPTH_TEST),
    }
}

fn map_operation(op: s::StencilOp) -> gl::types::GLenum {
    match op {
        s::OpKeep          => gl::KEEP,
        s::OpZero          => gl::ZERO,
        s::OpReplace       => gl::REPLACE,
        s::OpIncrementClamp=> gl::INCR,
        s::OpIncrementWrap => gl::INCR_WRAP,
        s::OpDecrementClamp=> gl::DECR,
        s::OpDecrementWrap => gl::DECR_WRAP,
        s::OpInvert        => gl::INVERT,
    }
}

pub fn bind_stencil(gl: &gl::Gl, stencil: Option<s::Stencil>, cull: s::CullMode) {
    fn bind_side(gl: &gl::Gl, face: gl::types::GLenum, side: s::StencilSide) {
        gl.StencilFuncSeparate(face, map_comparison(side.fun),
            side.value as gl::types::GLint, side.mask_read as gl::types::GLuint);
        gl.StencilMaskSeparate(face, side.mask_write as gl::types::GLuint);
        gl.StencilOpSeparate(face, map_operation(side.op_fail),
            map_operation(side.op_depth_fail), map_operation(side.op_pass));
    }
    match stencil {
        Some(s) => {
            gl.Enable(gl::STENCIL_TEST);
            if cull != s::CullFront {
                bind_side(gl, gl::FRONT, s.front);
            }
            if cull != s::CullBack {
                bind_side(gl, gl::BACK, s.back);
            }
        }
        None => gl.Disable(gl::STENCIL_TEST),
    }
}


fn map_equation(eq: s::Equation) -> gl::types::GLenum {
    match eq {
        s::FuncAdd    => gl::FUNC_ADD,
        s::FuncSub    => gl::FUNC_SUBTRACT,
        s::FuncRevSub => gl::FUNC_REVERSE_SUBTRACT,
        s::FuncMin    => gl::MIN,
        s::FuncMax    => gl::MAX,
    }
}

fn map_factor(factor: s::Factor) -> gl::types::GLenum {
    match factor {
        s::Factor(s::Normal,  s::Zero)        => gl::ZERO,
        s::Factor(s::Inverse, s::Zero)        => gl::ONE,
        s::Factor(s::Normal,  s::SourceColor) => gl::SRC_COLOR,
        s::Factor(s::Inverse, s::SourceColor) => gl::ONE_MINUS_SRC_COLOR,
        s::Factor(s::Normal,  s::SourceAlpha) => gl::SRC_ALPHA,
        s::Factor(s::Inverse, s::SourceAlpha) => gl::ONE_MINUS_SRC_ALPHA,
        s::Factor(s::Normal,  s::DestColor)   => gl::DST_COLOR,
        s::Factor(s::Inverse, s::DestColor)   => gl::ONE_MINUS_DST_COLOR,
        s::Factor(s::Normal,  s::DestAlpha)   => gl::DST_ALPHA,
        s::Factor(s::Inverse, s::DestAlpha)   => gl::ONE_MINUS_DST_ALPHA,
        s::Factor(s::Normal,  s::ConstColor)  => gl::CONSTANT_COLOR,
        s::Factor(s::Inverse, s::ConstColor)  => gl::ONE_MINUS_CONSTANT_COLOR,
        s::Factor(s::Normal,  s::ConstAlpha)  => gl::CONSTANT_ALPHA,
        s::Factor(s::Inverse, s::ConstAlpha)  => gl::ONE_MINUS_CONSTANT_ALPHA,
        s::Factor(s::Normal,  s::SourceAlphaSaturated) => gl::SRC_ALPHA_SATURATE,
        _ => fail!("Unsupported blend factor: {}", factor),
    }
}

pub fn bind_blend(gl: &gl::Gl, blend: Option<s::Blend>) {
    match blend {
        Some(b) => {
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
            let [r, g, b, a] = b.value;
            gl.BlendColor(r, g, b, a);
        },
        None => gl.Disable(gl::BLEND),
    }
}

pub fn bind_color_mask(gl: &gl::Gl, mask: s::ColorMask) {
    gl.ColorMask(
        if (mask & s::Red  ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (mask & s::Green).is_empty() {gl::FALSE} else {gl::TRUE},
        if (mask & s::Blue ).is_empty() {gl::FALSE} else {gl::TRUE},
        if (mask & s::Alpha).is_empty() {gl::FALSE} else {gl::TRUE}
    );
}
