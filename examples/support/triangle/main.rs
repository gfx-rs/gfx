// Copyright 2017 The Gfx-rs Developers.
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
#[deny(dead_code)]
#[macro_use]
extern crate gfx;
extern crate gfx_support;

use gfx::Bundle;
use gfx::GraphicsPoolExt;
use gfx_support::{Application, BackbufferView, ColorFormat};

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

// ----------------------------------------
struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    bundle: Bundle<B, pipe::Data<B::Resources>>,
}

impl<B: gfx::Backend> Application<B> for App<B> {
    fn new(device: &mut B::Device,
           _: &mut gfx::queue::GraphicsQueue<B>,
           backend: gfx_support::shade::Backend,
           window_targets: gfx_support::WindowTargets<B::Resources>)
           -> Self {
        use gfx::traits::DeviceExt;

        let pso = {
            let vs = gfx_support::shade::Source {
                glsl_120: include_bytes!("shader/triangle_120.glslv"),
                ..gfx_support::shade::Source::empty()
            };
            let ps = gfx_support::shade::Source {
                glsl_120: include_bytes!("shader/triangle_120.glslf"),
                ..gfx_support::shade::Source::empty()
            };

            device.create_pipeline_simple(vs.select(backend).unwrap(),
                                        ps.select(backend).unwrap(),
                                        pipe::new())
                .unwrap()
        };
        let (vertex_buffer, slice) = {
            const TRIANGLE: [Vertex; 3] = [Vertex {
                                               pos: [-0.5, -0.5],
                                               color: [1.0, 0.0, 0.0],
                                           },
                                           Vertex {
                                               pos: [0.5, -0.5],
                                               color: [0.0, 1.0, 0.0],
                                           },
                                           Vertex {
                                               pos: [0.0, 0.5],
                                               color: [0.0, 0.0, 1.0],
                                           }];
            device.create_vertex_buffer_with_slice(&TRIANGLE, ())
        };
        let data = pipe::Data {
            vbuf: vertex_buffer,
            out: window_targets.views[0].0.clone(),
        };

        App {
            views: window_targets.views,
            bundle: Bundle::new(slice, pso, data),
        }
    }

    fn render(&mut self,
              (_, sync): (gfx::Frame, &gfx_support::SyncPrimitives<B::Resources>),
              pool: &mut gfx::GraphicsCommandPool<B>,
              queue: &mut gfx::queue::GraphicsQueue<B>) {
        let mut encoder = pool.acquire_graphics_encoder();
        {
            const CLEAR_COLOR: [f32; 4] = [0.2, 0.1, 0.1, 1.0];
            encoder.clear(&self.bundle.data.out, CLEAR_COLOR);
        }
        self.bundle.encode(&mut encoder);
        encoder.synced_flush(queue, &[&sync.rendering], &[], Some(&sync.frame_fence))
            .expect("Could not flush encoder");
    }

    fn on_resize(&mut self, window_targets: gfx_support::WindowTargets<B::Resources>) {
        self.views = window_targets.views;
    }
}

pub fn main() {
    App::launch_simple("gfx_support Triangle example");
}
