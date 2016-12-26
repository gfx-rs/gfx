// Copyright 2015 The Gfx-rs Developers.
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

#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::traits::FactoryExt;
use gfx::Device;

pub type ColorFormat = gfx::format::Rgba8;
pub type DepthFormat = gfx::format::DepthStencil;

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        color: [f32; 3] = "a_Color",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

const QUAD: [Vertex; 4] = [
    Vertex { pos: [ -0.5,  0.5 ], color: [0.0, 0.0, 0.0] },
    Vertex { pos: [ -0.5, -0.5 ], color: [0.0, 0.0, 0.0] },
    Vertex { pos: [  0.5, -0.5 ], color: [1.0, 1.0, 1.0] },
    Vertex { pos: [  0.5,  0.5 ], color: [1.0, 1.0, 1.0] }
];

const CLEAR_COLOR: [f32; 4] = [0.5, 0.5, 0.5, 1.0];

pub fn main() {
    let builder = glutin::WindowBuilder::new()
        .with_title("Gamma example".to_string())
        .with_dimensions(1024, 768)
        .with_vsync();
    let (window, mut device, mut factory, main_color, _main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
    let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();
    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/quad_150.glslv"),
        include_bytes!("shader/quad_150.glslf"),
        pipe::new()
    ).unwrap();
    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(&QUAD, &[0u16, 1, 2, 2, 3, 0] as &[u16]);
    let data = pipe::Data {
        vbuf: vertex_buffer,
        out: main_color
    };

    'main: loop {
        // loop over events
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }
        // draw a frame
        encoder.clear(&data.out, CLEAR_COLOR);
        encoder.draw(&slice, &pso, &data);
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
