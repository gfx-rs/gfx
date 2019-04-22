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
extern crate image;

use gfx::traits::{Factory, FactoryExt};
use gfx::Device;
use gfx::memory::Typed;
use gfx::format::{Formatted, SurfaceTyped};

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

type SurfaceData = <<ColorFormat as Formatted>::Surface as SurfaceTyped>::DataType;

pub fn main() {
    let mut events_loop = glutin::EventsLoop::new();
    let window_builder = glutin::WindowBuilder::new()
        .with_title("Gamma example".to_string())
        .with_dimensions((1024, 768).into());

    let (api, version, vs_code, fs_code) = if cfg!(target_os = "emscripten") {
        (
            glutin::Api::WebGl, (2, 0),
            include_bytes!("shader/quad_300_es.glslv").to_vec(),
            include_bytes!("shader/quad_300_es.glslf").to_vec(),
        )
    } else {
        (
            glutin::Api::OpenGl, (3, 2),
            include_bytes!("shader/quad_150_core.glslv").to_vec(),
            include_bytes!("shader/quad_150_core.glslf").to_vec(),
        )
    };

    let context = glutin::ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(api, version))
        .with_vsync(true);
    let (window, mut device, mut factory, main_color, mut main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(window_builder, context, &events_loop)
            .expect("Failed to create window");
    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());
    let pso = factory.create_pipeline_simple(&vs_code, &fs_code, pipe::new())
        .unwrap();
    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(&QUAD, &[0u16, 1, 2, 2, 3, 0] as &[u16]);
    let mut data = pipe::Data {
        vbuf: vertex_buffer,
        out: main_color
    };

    let mut screenshot = false;
    let (w, h, _, _) = data.out.get_dimensions();
    let mut download = factory.create_download_buffer::<SurfaceData>(w as usize * h as usize)
        .unwrap();

    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            use glutin::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested |
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => running = false,
                    WindowEvent::Resized(size) => {
                        window.resize(size.to_physical(window.window().get_hidpi_factor()));
                        gfx_window_glutin::update_views(&window, &mut data.out, &mut main_depth);
                        download = factory
                            .create_download_buffer(
                                size.width.round() as usize * size.height.round() as usize,
                            )
                            .unwrap();
                    },
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::S),
                            state: ElementState::Released,
                            ..
                        },
                        ..
                    } => screenshot = true,
                _ => (),
                }
            }

            if screenshot {
                println!("taking screenshot");
                let (w, h, _, _) = data.out.get_dimensions();
                encoder.copy_texture_to_buffer_raw(
                    data.out.raw().get_texture(),
                    None,
                    gfx::texture::RawImageInfo {
                        xoffset: 0,
                        yoffset: 0,
                        zoffset: 0,
                        width: w,
                        height: h,
                        depth: 0,
                        format: ColorFormat::get_format(),
                        mipmap: 0,
                    },
                    download.raw(),
                    0
                ).unwrap();
                encoder.flush(&mut device);

                let path = "screen.png";
                println!("saving screenshot to {}", path);
                let reader = factory.read_mapping(&download).unwrap();
                // intermediary buffer only to avoid casting
                let mut data = Vec::with_capacity(w as usize * h as usize * 4);
                for pixel in reader.iter() {
                    data.extend(pixel);
                }
                image::save_buffer(path, &data, w as u32, h as u32, image::ColorType::RGBA(8))
                    .unwrap();

                println!("done!");
                screenshot = false;
            }
        });
        // draw a frame
        encoder.clear(&data.out, CLEAR_COLOR);
        encoder.draw(&slice, &pso, &data);
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
