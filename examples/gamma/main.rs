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
#[cfg(not(target_os = "windows"))]
extern crate gfx_window_glutin;
#[cfg(target_os = "windows")]
extern crate gfx_window_dxgi;
#[cfg(target_os = "windows")]
extern crate gfx_device_dx11;
extern crate glutin;
extern crate image;

use gfx::traits::{Factory, FactoryExt};
use gfx::Device;
use gfx::memory::Typed;
use gfx::format::{Formatted, SurfaceTyped};
#[cfg(not(target_os = "windows"))]
use glutin::GlContext;

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

#[cfg(target_os = "windows")]
struct Backend {
    window: gfx_window_dxgi::Window,
}
#[cfg(not(target_os = "windows"))]
struct Backend<R: gfx::Resources> {
    window: glutin::GlWindow,
    dsv: gfx::handle::DepthStencilView<R, DepthFormat>,
}

pub fn main() {
    let mut events_loop = glutin::EventsLoop::new();
    let window_builder = glutin::WindowBuilder::new()
        .with_title("Gamma example".to_string())
        .with_dimensions(1024, 768);

    let (mut backend, mut device, mut factory, pso, main_color) = match () {
        #[cfg(not(target_os = "windows"))]
        _ => {
            let context = glutin::ContextBuilder::new()
                .with_vsync(true);
            let (window, device, mut factory, main_color, main_depth) =
                gfx_window_glutin::init::<ColorFormat, DepthFormat>(window_builder, context, &events_loop);
            let pso = factory.create_pipeline_simple(
                include_bytes!("shader/quad_150.glslv"),
                include_bytes!("shader/quad_150.glslf"),
                pipe::new()
            ).unwrap();
            (Backend { window, dsv: main_depth }, device, factory, pso, main_color)
        }
        #[cfg(target_os = "windows")]
        _ => {
            let (window, device, mut factory, main_color) =
                gfx_window_dxgi::init::<ColorFormat>(window_builder, &events_loop).unwrap();
            let pso = factory.create_pipeline_simple(
                include_bytes!("data/vertex.fx"),
                include_bytes!("data/pixel.fx"),
                pipe::new()
            ).unwrap();
            (Backend { window }, device, factory, pso, main_color)
        }
    };

    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(&QUAD, &[0u16, 1, 2, 2, 3, 0] as &[u16]);
    let mut data = pipe::Data {
        vbuf: vertex_buffer,
        out: main_color
    };

    let mut screenshot = false;
    let (w, h, _, _) = data.out.get_dimensions();
    let mut download = factory.create_download_buffer::<SurfaceData>(w as usize * h as usize)
        .unwrap();

    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());
    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                match event {
                    glutin::WindowEvent::KeyboardInput {
                        input: glutin::KeyboardInput {
                            virtual_keycode: Some(glutin::VirtualKeyCode::Escape),
                            .. },
                        ..
                    } | glutin::WindowEvent::Closed => running = false,
                    glutin::WindowEvent::Resized(width, height) => {
                        #[cfg(not(target_os = "windows"))]
                        {
                            backend.window.resize(width, height);
                            gfx_window_glutin::update_views(&mut backend.window, &mut data.out, &mut backend.dsv);
                        }
                        #[cfg(target_os = "windows")]
                        {
                            gfx_window_dxgi::update_view(&mut backend.window, &mut factory, &mut device, width as _, height as _, &mut data.out);
                        }
                        download = factory.create_download_buffer(width as usize * height as usize).unwrap();
                    },
                    glutin::WindowEvent::KeyboardInput {
                        input: glutin::KeyboardInput {
                            virtual_keycode: Some(glutin::VirtualKeyCode::S),
                            state: glutin::ElementState::Released,
                            ..
                        },
                        ..
                    } => screenshot = true,
                    _ => (),
                }
            }
        });

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

        // draw a frame
        encoder.clear(&data.out, CLEAR_COLOR);
        encoder.draw(&slice, &pso, &data);
        encoder.flush(&mut device);
        #[cfg(not(target_os = "windows"))]
        {
            backend.window.swap_buffers().unwrap();
        }
        #[cfg(target_os = "windows")]
        {
            backend.window.swap_buffers(1);
        }
        device.cleanup();
    }
}
