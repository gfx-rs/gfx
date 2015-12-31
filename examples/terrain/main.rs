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

extern crate cgmath;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use rand::Rng;
use cgmath::FixedArray;
use cgmath::{Matrix4, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::format::{DepthStencil, Rgba8};
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};
use time::precise_time_s;
use noise::{Seed, perlin2};


gfx_vertex_struct!( Vertex {
    pos: [f32; 3] = "a_Pos",
    color: [f32; 3] = "a_Color",
});

gfx_pipeline_init!( PipeData PipeMeta PipeInit {
    vbuf: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
    view: gfx::Global<[[f32; 4]; 4]> = "u_View",
    proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
    out_color: gfx::RenderTarget<Rgba8> = ("o_Color", gfx::state::MASK_ALL),
    out_depth: gfx::DepthTarget<DepthStencil> = gfx::state::Depth {
        fun: gfx::state::Comparison::LessEqual,
        write: true,
    },
});

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
    use gfx::traits::{Device, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_title("Terrain example".to_string());
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init_new::<gfx::format::Rgba8>(builder);
    let mut encoder = factory.create_encoder();

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

    let (vbuf, slice) = factory.create_vertex_buffer_indexed(&vertex_data, &index_data[..]);

    let pso = factory.create_pipeline_simple(
        include_bytes!("terrain_150.glslv"),
        include_bytes!("terrain_150.glslf"),
        gfx::state::CullFace::Back,
        &PipeInit::new()
        ).unwrap();

    let aspect_ratio = {
        let (w, h) = window.get_inner_size().unwrap();
        w as f32 / h as f32
    };

    let mut data = PipeData {
        vbuf: vbuf,
        model: Matrix4::identity().into_fixed(),
        view: Matrix4::identity().into_fixed(),
        proj: cgmath::perspective(
            cgmath::deg(60.0f32), aspect_ratio, 0.1, 1000.0
            ).into_fixed(),
        out_color: main_color,
        out_depth: main_depth,
    };

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
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

        encoder.reset();
        encoder.clear_target(&data.out_color, [0.3, 0.3, 0.3, 1.0]);
        encoder.clear_depth(&data.out_depth, 1.0);

        data.view = view.mat.into_fixed();
        encoder.draw_pipeline(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
