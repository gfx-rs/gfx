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
extern crate gfx_device_gl;

use std::marker::PhantomData;
use rand::Rng;
use glutin::WindowBuilder;
use gfx::device::{Factory, BufferRole, PrimitiveType};
use gfx::extra::stream::Stream;
use gfx::extra::factory::FactoryExt;
use gfx::render::mesh::{Mesh, ToSlice};
use gfx::render::batch;

const QUAD_VERTICES: [Vertex; 4] = [
    Vertex { position: [-0.5,  0.5] },
    Vertex { position: [-0.5, -0.5] },
    Vertex { position: [ 0.5, -0.5] },
    Vertex { position: [ 0.5,  0.5] },
];

const QUAD_INDICES: [u8; 6] = [0, 1, 2, 2, 3, 0];

gfx_vertex! {
    Vertex {
        a_Position@ position: [f32; 2],
    }
}

// color format: 0xRRGGBBAA
gfx_vertex! {
    Attributes {
        a_Translate@ translate: [f32; 2],
        a_Color@ color: u32,
    }
}

gfx_parameters! {
    Params {
        u_Scale@ scale: f32,
    }
}

const MAX_INSTANCE_COUNT: usize = 2048;

fn main() {
    let window = WindowBuilder::new().with_dimensions(800, 600)
                                     .with_title("Instancing".to_string())
                                     .build().unwrap();
    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(window);


    let quad_vertices = factory.create_buffer_static(&QUAD_VERTICES, BufferRole::Vertex);
    let quad_indices = factory.create_buffer_static(&QUAD_INDICES, BufferRole::Index);
    let quad_vertices_count = QUAD_VERTICES.len() as u32;

    let attributes = factory.create_buffer_dynamic(MAX_INSTANCE_COUNT, BufferRole::Vertex);

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
        let mut attributes = factory.map_buffer_writable(&attributes);
        for x in 0..length {
            for y in 0..length {
                let i = x*length + y;
                attributes.set(i, Attributes {
                    translate: translate,
                    color: rng.next_u32()
                });
                translate[1] += size + gap;
            }
            translate[1] = begin;
            translate[0] += size + gap;
        }
    }

    let mesh = Mesh::from_format_instanced(quad_vertices, quad_vertices_count, attributes);

    let program = {
        let vs = gfx::ShaderSource {
            glsl_150: Some(include_bytes!("instancing_150.glslv")),
            glsl_120: Some(include_bytes!("instancing_120.glslv")),
            .. gfx::ShaderSource::empty()
        };
        let fs = gfx::ShaderSource {
            glsl_150: Some(include_bytes!("instancing_150.glslf")),
            glsl_120: Some(include_bytes!("instancing_120.glslf")),
            .. gfx::ShaderSource::empty()
        };
        factory.link_program_source(vs, fs).unwrap()
    };

    let params = Params {
        scale: size,
        _r: PhantomData
    };

    let mut batch = batch::Full::new(mesh, program, params).unwrap();
    batch.slice = quad_indices.to_slice(PrimitiveType::TriangleList);

    'l: loop {
        for event in stream.out.window.poll_events() {
            match event {
                glutin::Event::Closed => break 'l,
                _ => {}
            }
        }

        stream.clear(gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });
        stream.draw_instanced(&batch, instance_count, 0).unwrap();
        stream.present(&mut device);
    }
}
