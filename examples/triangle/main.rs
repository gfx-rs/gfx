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
#![cfg_attr(target_os = "emscripten", allow(unused_mut))] // this is annoying...

#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::traits::FactoryExt;
use gfx::Device;
use glutin::{window::WindowBuilder, event_loop::{ControlFlow, EventLoop}, event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent}};

pub type ColorFormat = gfx::format::Srgba8;
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

const TRIANGLE: [Vertex; 3] = [
    Vertex { pos: [ -0.5, -0.5 ], color: [1.0, 0.0, 0.0] },
    Vertex { pos: [  0.5, -0.5 ], color: [0.0, 1.0, 0.0] },
    Vertex { pos: [  0.0,  0.5 ], color: [0.0, 0.0, 1.0] }
];

const CLEAR_COLOR: [f32; 4] = [0.1, 0.2, 0.3, 1.0];

pub fn main() {
    let event_loop = EventLoop::new();
    let window_config = WindowBuilder::new()
        .with_title("Triangle example".to_string())
        .with_inner_size(glutin::dpi::PhysicalSize::new(1024, 768));

    let (api, version, vs_code, fs_code) = if cfg!(target_os = "emscripten") {
        (
            glutin::Api::WebGl, (2, 0),
            include_bytes!("shader/triangle_300_es.glslv").to_vec(),
            include_bytes!("shader/triangle_300_es.glslf").to_vec(),
        )
    } else {
        (
            glutin::Api::OpenGl, (3, 2),
            include_bytes!("shader/triangle_150_core.glslv").to_vec(),
            include_bytes!("shader/triangle_150_core.glslf").to_vec(),
        )
    };

    let context = glutin::ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(api, version))
        .with_vsync(true);
    let (window_ctx, mut device, mut factory, main_color, mut main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat, _>(window_config, context, &event_loop)
            .expect("Failed to create window");
    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());

    let pso = factory.create_pipeline_simple(&vs_code, &fs_code, pipe::new())
        .unwrap();
    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(&TRIANGLE, ());
    let mut data = pipe::Data {
        vbuf: vertex_buffer,
        out: main_color
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::LoopDestroyed => return,
            Event::MainEventsCleared => window_ctx.window().request_redraw(),
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical) => {
                    window_ctx.resize(physical);
                    gfx_window_glutin::update_views(&window_ctx, &mut data.out, &mut main_depth);
                }
                WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                    input: KeyboardInput {
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::RedrawRequested(_) => {
                // draw a frame
                encoder.clear(&data.out, CLEAR_COLOR);
                encoder.draw(&slice, &pso, &data);
                encoder.flush(&mut device);
                window_ctx.swap_buffers().unwrap();
                device.cleanup();
            }
            _ => (),
        }
    });
}
