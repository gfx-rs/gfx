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

#![feature(plugin)]

extern crate cgmath;
extern crate gfx;
#[macro_use]
#[plugin]
extern crate gfx_macros;
extern crate glfw;

use cgmath::FixedArray;
use cgmath::{Matrix, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceHelper, ToSlice};
use gfx::batch;
use glfw::Context;

#[vertex_format]
#[derive(Copy)]
struct Vertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8; 3],

    #[as_float]
    #[name = "a_TexCoord"]
    tex_coord: [u8; 2],
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader. Its argument is the name of the type that will
// be generated to represent your the program. Search for `CubeBatch` below, to
// see how it's used.
#[shader_param(CubeBatch)]
struct Params {
    #[name = "u_Transform"]
    transform: [[f32; 4]; 4],

    #[name = "t_Color"]
    color: gfx::shade::TextureParam,
}

static VERTEX_SRC: gfx::ShaderSource<'static> = shaders! {
GLSL_120: b"
    #version 120

    attribute vec3 a_Pos;
    attribute vec2 a_TexCoord;
    varying vec2 v_TexCoord;

    uniform mat4 u_Transform;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
",
GLSL_150: b"
    #version 150 core

    in vec3 a_Pos;
    in vec2 a_TexCoord;
    out vec2 v_TexCoord;

    uniform mat4 u_Transform;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
",
};

static FRAGMENT_SRC: gfx::ShaderSource<'static> = shaders! {
GLSL_120: b"
    #version 120

    varying vec2 v_TexCoord;
    uniform sampler2D t_Color;

    void main() {
        vec4 tex = texture2D(t_Color, v_TexCoord);
        float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
        gl_FragColor = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
    }
",
GLSL_150: b"
    #version 150 core

    in vec2 v_TexCoord;
    out vec4 o_Color;

    uniform sampler2D t_Color;
    void main() {
        vec4 tex = texture(t_Color, v_TexCoord);
        float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
        o_Color = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
    }
",
};

//----------------------------------------

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenglForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenglProfile(glfw::OpenGlProfileHint::Core));

    let (window, events) = glfw
        .create_window(640, 480, "Cube example", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| window.get_proc_address(s));
    let mut renderer = device.create_renderer();
    let mut context = batch::Context::new();

    let vertex_data = [
        // top (0, 0, 1)
        Vertex { pos: [-1, -1,  1], tex_coord: [0, 0] },
        Vertex { pos: [ 1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 1] },
        // bottom (0, 0, -1)
        Vertex { pos: [-1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1, -1], tex_coord: [0, 0] },
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 1] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        // right (1, 0, 0)
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 0] },
        Vertex { pos: [ 1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        Vertex { pos: [ 1, -1,  1], tex_coord: [0, 1] },
        // left (-1, 0, 0)
        Vertex { pos: [-1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 0] },
        Vertex { pos: [-1,  1, -1], tex_coord: [0, 1] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        // front (0, 1, 0)
        Vertex { pos: [ 1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [-1,  1, -1], tex_coord: [0, 0] },
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 1] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        // back (0, -1, 0)
        Vertex { pos: [ 1, -1,  1], tex_coord: [0, 0] },
        Vertex { pos: [-1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 1] },
    ];

    let mesh = device.create_mesh(&vertex_data);

    let index_data: &[u8] = &[
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    let slice = device
        .create_buffer_static::<u8>(index_data)
        .to_slice(gfx::PrimitiveType::TriangleList);

    let texture_info = gfx::tex::TextureInfo {
        width: 1,
        height: 1,
        depth: 1,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2D,
        format: gfx::tex::RGBA8,
    };
    let image_info = texture_info.to_image_info();
    let texture = device.create_texture(texture_info).unwrap();
    device.update_texture(&texture, &image_info,
                          &[0x20u8, 0xA0u8, 0xC0u8, 0x00u8])
        .unwrap();

    let sampler = device.create_sampler(
        gfx::tex::SamplerInfo::new(gfx::tex::FilterMethod::Bilinear,
                                   gfx::tex::WrapMode::Clamp)
    );

    let program = device.link_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone())
                        .unwrap();
    let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

    let batch: CubeBatch = context.make_batch(&program, &mesh, slice, &state).unwrap();

    let view: AffineMatrix3<f32> = Transform::look_at(
        &Point3::new(1.5f32, -5.0, 3.0),
        &Point3::new(0f32, 0.0, 0.0),
        &Vector3::unit_z(),
    );
    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 10.0);

    let data = Params {
        transform: proj.mul_m(&view.mat).into_fixed(),
        color: (texture, Some(sampler)),
    };

    let clear_data = gfx::ClearData {
        color: [0.3, 0.3, 0.3, 1.0],
        depth: 1.0,
        stencil: 0,
    };

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(glfw::Key::Escape, _, glfw::Action::Press, _) =>
                    window.set_should_close(true),
                _ => {},
            }
        }

        renderer.clear(clear_data, gfx::COLOR | gfx::DEPTH, &frame);
        renderer.draw(&(&batch, &data, &context), &frame);
        device.submit(renderer.as_buffer());
        renderer.reset();

        window.swap_buffers();
    }
}
