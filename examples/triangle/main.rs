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

gfx_vertex_struct!( Vertex {
    pos: [f32; 2] = "a_Pos",
    color: [f32; 3] = "a_Color",
});

gfx_pipeline_init!(PipeData PipeMeta PipeInit {
    vbuf: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    out: gfx::RenderTarget<gfx::format::Rgba8> = ("o_Color", gfx::state::MASK_ALL),
});

pub fn main() {
    use gfx::Device;
    use gfx::traits::{EncoderFactory, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_title("Triangle example".to_string());
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init_new::<gfx::format::Rgba8>(builder);
    let mut encoder = factory.create_encoder();

    let shaders = factory.create_shader_set(
        include_bytes!("triangle_150.glslv"),
        include_bytes!("triangle_150.glslf")
        ).unwrap();

    let pso = factory.create_pipeline_state(&shaders,
        gfx::Primitive::TriangleList,
        gfx::state::Rasterizer::new_fill(gfx::state::CullFace::Nothing),
        &PipeInit::new()
        ).unwrap();

    let vertex_data = [
        Vertex { pos: [ -0.5, -0.5 ], color: [1.0, 0.0, 0.0] },
        Vertex { pos: [  0.5, -0.5 ], color: [0.0, 1.0, 0.0] },
        Vertex { pos: [  0.0,  0.5 ], color: [0.0, 0.0, 1.0] },
    ];
    let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);
    let data = PipeData {
        vbuf: vbuf,
        out: main_color,
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

        encoder.reset();
        encoder.clear_target(&data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw_pipeline(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
