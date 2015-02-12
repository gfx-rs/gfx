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

#![feature(core, plugin)]
#![plugin(gfx_macros)]

extern crate cgmath;
extern crate gfx;
extern crate glfw;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use std::fmt;
use rand::Rng;
use cgmath::FixedArray;
use cgmath::{Matrix4, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceExt, ToSlice};
use gfx::batch::RefBatch;
use glfw::Context;
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};
use time::precise_time_s;

use noise::{Seed, perlin2};

#[vertex_format]
#[derive(Copy)]
struct Vertex {
    #[name = "a_Pos"]
    pos: [f32; 3],

    #[name = "a_Color"]
    color: [f32; 3],
}

impl Clone for Vertex {
    fn clone(&self) -> Vertex {
        Vertex { pos: self.pos, color: self.color }
    }
}

impl fmt::Debug for Vertex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pos({}, {}, {})", self.pos[0], self.pos[1], self.pos[2])
    }
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader.
#[shader_param]
struct Params {
    #[name = "u_Model"]
    model: [[f32; 4]; 4],

    #[name = "u_View"]
    view: [[f32; 4]; 4],

    #[name = "u_Proj"]
    proj: [[f32; 4]; 4],
}

static VERTEX_SRC: &'static [u8] = b"
    #version 120

    attribute vec3 a_Pos;
    attribute vec3 a_Color;
    varying vec3 v_Color;

    uniform mat4 u_Model;
    uniform mat4 u_View;
    uniform mat4 u_Proj;

    void main() {
        v_Color = a_Color;
        gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
    }
";

static FRAGMENT_SRC: &'static [u8] = b"
    #version 120

    varying vec3 v_Color;

    void main() {
        gl_FragColor = vec4(v_Color, 1.0);
    }
";

fn calculate_color(height: f32) -> [f32; 3] {
    if height > 8.0 {
        [0.9, 0.9, 0.9] // white
    } else if height > 0.0 {
        [0.7, 0.7, 0.7] // greay
    } else if height > -5.0 {
        [0.2, 0.7, 0.2] // green
    } else {
        [0.2, 0.2, 0.7] // blue
    }
}

fn main() {
    use std::num::Float;

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
        .ok().expect("Failed to initialize glfs-rs");

    let (mut window, events) = glfw
        .create_window(800, 600, "Terrain example", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| window.get_proc_address(s));

    let rand_seed = rand::thread_rng().gen();
    let seed = Seed::new(rand_seed);
    let plane = Plane::subdivide(256, 256);
    let vertex_data: Vec<Vertex> = plane.shared_vertex_iter()
        .map(|(x, y)| {
            let h = perlin2(&seed, &[x, y]) * 32.0;
            Vertex {
                pos: [25.0 * x, 25.0 * y, h],
                color: calculate_color(h),
            }
        })
        .collect();

    let index_data: Vec<u32> = plane.indexed_polygon_iter()
        .triangulate()
        .vertices()
        .map(|i| i as u32)
        .collect();

    let slice = device
        .create_buffer_static::<u32>(index_data.as_slice())
        .to_slice(gfx::PrimitiveType::TriangleList);

    let mesh = device.create_mesh(vertex_data.as_slice());
    let program = device.link_program(VERTEX_SRC, FRAGMENT_SRC)
                        .ok().expect("Failed to link shaders.");
    let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

    let mut graphics = gfx::Graphics::new(device);
    let batch: RefBatch<Params> = graphics.make_batch(&program, &mesh, slice, &state)
                                          .ok().expect("Failed to make batch.");

    let aspect = w as f32 / h as f32;
    let mut data = Params {
        model: Matrix4::identity().into_fixed(),
        view: Matrix4::identity().into_fixed(),
        proj: cgmath::perspective(cgmath::deg(60.0f32), aspect,
                                  0.1, 1000.0).into_fixed(),
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

        let time = precise_time_s() as f32;
        let x = time.sin();
        let y = time.cos();
        let view: AffineMatrix3<f32> = Transform::look_at(
            &Point3::new(x * 32.0, y * 32.0, 16.0),
            &Point3::new(0.0, 0.0, 0.0),
            &Vector3::unit_z(),
        );
        data.view = view.mat.into_fixed();

        graphics.clear(clear_data, gfx::COLOR | gfx::DEPTH, &frame);
        graphics.draw(&batch, &data, &frame).unwrap();
        graphics.end_frame();

        window.swap_buffers();
    }
}
