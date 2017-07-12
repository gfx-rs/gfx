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

use gfx::format::Formatted;
use gfx::traits::FactoryExt;
use gfx::{Adapter, CommandQueue, Factory, FrameSync, GraphicsPoolExt,
          Surface, SwapChain, SwapChainExt, WindowExt};

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
    let events_loop = glutin::EventsLoop::new();
    let builder = glutin::WindowBuilder::new()
        .with_title("Gamma example".to_string())
        .with_dimensions(1024, 768)
        .with_vsync();
    let win = gfx_window_glutin::build(builder, &events_loop, ColorFormat::get_format(), DepthFormat::get_format());
    let mut window = gfx_window_glutin::Window::new(window);

    let (mut surface, adapters) = window.get_surface_and_adapters();
    let queue_descs = adapters[0].get_queue_families().iter()
                                 .filter(|family| surface.supports_queue(&family) )
                                 .map(|family| { (family, 1) })
                                 .collect::<Vec<_>>();

    // Open device (factory and queues)
    let gfx::Device { mut factory, mut general_queues, mut graphics_queues, .. } = adapters[0].open(&queue_descs);
    let mut graphics_queue = if let Some(queue) = general_queues.first_mut() {
        queue.as_mut().into()
    } else if let Some(queue) = graphics_queues.first_mut() {
        queue.as_mut()
    } else {
        panic!("Unable to find a matching general or graphics queue.");
    };

    // Create swapchain
    let config = gfx::SwapchainConfig::new()
                    .with_color::<ColorFormat>();
    let mut swap_chain = surface.build_swapchain(config, &graphics_queue);
    let views = swap_chain.create_color_views(&mut factory);

    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/quad_150.glslv"),
        include_bytes!("shader/quad_150.glslf"),
        pipe::new()
    ).unwrap();
    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(&QUAD, &[0u16, 1, 2, 2, 3, 0] as &[u16]);
    let mut graphics_pool = graphics_queue.create_graphics_pool(1);
    let frame_semaphore = factory.create_semaphore();
    let draw_semaphore = factory.create_semaphore();

    let mut data = pipe::Data {
        vbuf: vertex_buffer,
        out: views[0].clone(),
    };

    let mut running = true;
    while running {
        events_loop.poll_events(|glutin::Event::WindowEvent{window_id: _, event}| {
            match event {
                glutin::WindowEvent::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape), _) |
                glutin::WindowEvent::Closed => running = false,
                glutin::WindowEvent::Resized(_width, _height) => {
                    // TODO
                },
                _ => {},
            }
        });

        graphics_pool.reset();

        // Get next frame
        let frame = swap_chain.acquire_frame(FrameSync::Semaphore(&frame_semaphore));
        data.out = views[frame.id()].clone();        

        // Draw a frame
        let mut encoder = graphics_pool.acquire_graphics_encoder();
        encoder.clear(&data.out, CLEAR_COLOR);
        encoder.draw(&slice, &pso, &data);
        encoder.synced_flush(&mut graphics_queue, &[&frame_semaphore], &[&draw_semaphore], None);
        swap_chain.present(&mut graphics_queue, &[&draw_semaphore]);
        graphics_queue.wait_idle();
        graphics_queue.cleanup();
    }
}
