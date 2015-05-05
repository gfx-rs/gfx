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

#![feature(plugin, custom_attribute)]
#![plugin(gfx_macros)]

extern crate cgmath;
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use std::fmt;
use rand::Rng;
use cgmath::FixedArray;
use cgmath::{Matrix4, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::traits::*;
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};
use time::precise_time_s;

use noise::{Seed, perlin2};

#[vertex_format]
#[derive(Clone, Copy)]
struct Vertex {
    #[name = "a_Pos"]
    pos: [f32; 3],

    #[name = "a_Color"]
    color: [f32; 3],
}

impl fmt::Debug for Vertex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pos({}, {}, {})", self.pos[0], self.pos[1], self.pos[2])
    }
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader.
#[shader_param]
struct Params<R: gfx::Resources> {
    #[name = "u_Model"]
    model: [[f32; 4]; 4],

    #[name = "u_View"]
    view: [[f32; 4]; 4],

    #[name = "u_Proj"]
    proj: [[f32; 4]; 4],

    _dummy: std::marker::PhantomData<R>,
}

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

pub fn main() {
    let mut canvas = gfx_window_glutin::init(glutin::Window::new().unwrap())
                                       .into_canvas();
    canvas.output.window.set_title("Terrain example");

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

    let slice = canvas.factory
        .create_buffer_index::<u32>(&index_data)
        .to_slice(gfx::PrimitiveType::TriangleList);

    let mesh = canvas.factory.create_mesh(&vertex_data);
    let program = {
        let vs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("terrain_120.glslv")),
            glsl_150: Some(include_bytes!("terrain_150.glslv")),
            .. gfx::ShaderSource::empty()
        };
        let fs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("terrain_120.glslf")),
            glsl_150: Some(include_bytes!("terrain_150.glslf")),
            .. gfx::ShaderSource::empty()
        };
        canvas.factory.link_program_source(vs, fs, &canvas.device.get_capabilities())
                      .unwrap()
    };

    let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

    let data = Params {
        model: Matrix4::identity().into_fixed(),
        view: Matrix4::identity().into_fixed(),
        proj: cgmath::perspective(cgmath::deg(60.0f32), 
                                  canvas.get_aspect_ratio(),
                                  0.1, 1000.0
                                  ).into_fixed(),
        _dummy: std::marker::PhantomData,
    };
    let mut context = gfx::batch::Context::new();
    let mut batch = context.make_batch(&program, data, &mesh, slice, &state)
                           .unwrap();

    'main: loop {
        // quit when Esc is pressed.
        for event in canvas.output.window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
                glutin::Event::Closed => break 'main,
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
        batch.params.view = view.mat.into_fixed();

        canvas.clear(gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });
        canvas.draw(&(&batch, &context)).unwrap();
        canvas.present();
    }
}
