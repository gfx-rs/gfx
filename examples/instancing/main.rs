// Copyright 2015 The gfx developers.
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

extern crate time;
extern crate rand;

extern crate glutin;
#[macro_use] extern crate gfx;
extern crate gfx_window_glutin;

use rand::Rng;

const QUAD_VERTICES: [Vertex; 4] = [
    Vertex { position: [-0.5,  0.5] },
    Vertex { position: [-0.5, -0.5] },
    Vertex { position: [ 0.5, -0.5] },
    Vertex { position: [ 0.5,  0.5] },
];

const QUAD_INDICES: [u8; 6] = [0, 1, 2, 2, 3, 0];

gfx_vertex_struct!(Vertex {
    position: [f32; 2] = "a_Position",
});

// color format: 0xRRGGBBAA
gfx_vertex_struct!(Instance {
    translate: [f32; 2] = "a_Translate",
    color: u32 = "a_Color",
});

gfx_pipeline_init!(PipeData PipeMeta PipeInit {
    vertex: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    instance: gfx::VertexBuffer<Instance> = gfx::PER_INSTANCE,
    scale: gfx::Global<f32> = "u_Scale",
    out: gfx::RenderTarget<gfx::format::Rgba8> = ("o_Color", gfx::state::MASK_ALL),
});

const MAX_INSTANCE_COUNT: usize = 2048;

fn main() {
    use gfx::traits::{Device, Factory, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_dimensions(800, 600)
        .with_title("Instancing example".to_string());
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init::<gfx::format::Rgba8>(builder);
    let mut encoder = factory.create_encoder();

    let pso = factory.create_pipeline_simple(
        include_bytes!("instancing_150.glslv"),
        include_bytes!("instancing_150.glslf"),
        gfx::state::CullFace::Back,
        PipeInit::new()
        ).unwrap();

    // we could use `factory.create_vertex_buffer_indexed` for the first two
    // but leaving the direct creation here for consistency.
    let quad_vertices = factory.create_buffer_static(&QUAD_VERTICES, gfx::BufferRole::Vertex);
    let quad_indices = factory.create_buffer_static(&QUAD_INDICES, gfx::BufferRole::Index);
    let quad_instances = factory.create_buffer_dynamic(MAX_INSTANCE_COUNT, gfx::BufferRole::Vertex);

    let instances_per_length: u32 = 32;
    println!("{} instances per length", instances_per_length);
    let instance_count = instances_per_length * instances_per_length;
    println!("{} instances", instance_count);
    assert!(instance_count as usize <= MAX_INSTANCE_COUNT);
    let size = 1.6 / instances_per_length as f32;
    println!("size: {}", size);
    let gap = 0.4 / (instances_per_length + 1) as f32;
    println!("gap: {}", gap);

    {
        let begin = -1. + gap + (size /2.);
        let mut translate = [begin, begin];
        let mut rng = rand::StdRng::new().unwrap();

        let length = instances_per_length as usize;
        let mut attributes = factory.map_buffer_writable(&quad_instances);
        for x in 0..length {
            for y in 0..length {
                let i = x*length + y;
                attributes.set(i, Instance {
                    translate: translate,
                    color: rng.next_u32()
                });
                translate[1] += size + gap;
            }
            translate[1] = begin;
            translate[0] += size + gap;
        }
    }

    let data = PipeData {
        vertex: quad_vertices,
        instance: quad_instances,
        scale: size,
        out: main_color,
    };
    let mut slice: gfx::Slice<_> = quad_indices.into();
    slice.instances = Some((instance_count, 0));

    'l: loop {
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'l,
                _ => {}
            }
        }

        encoder.reset();
        encoder.clear(&data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
