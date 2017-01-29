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

extern crate cgmath;
#[macro_use]
extern crate gfx;
extern crate gfx_app;

pub use gfx_app::{ColorFormat, DepthFormat};
use gfx::{Bundle, texture};

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

struct App<R: gfx::Resources>{
    bundle: Bundle<R, pipe::Data<R>>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(factory: &mut F, backend: gfx_app::shade::Backend, window_targets: gfx_app::WindowTargets<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/triangle_120.glslv"),
            glsl_150: include_bytes!("shader/triangle_150.glslv"),
            msl_11: include_bytes!("shader/triangle_vertex.metal"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/triangle_120.glslf"),
            glsl_150: include_bytes!("shader/triangle_150.glslf"),
            msl_11: include_bytes!("shader/triangle_frag.metal"),
            .. gfx_app::shade::Source::empty()
        };

        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&TRIANGLE, ());

        let pso = factory.create_pipeline_simple(
            vs.select(backend).unwrap(),
            ps.select(backend).unwrap(),
            pipe::new()
        ).unwrap();

        let data = pipe::Data {
            vbuf: vbuf,
            out: window_targets.color,
        };

        App {
            bundle: Bundle::new(slice, pso, data),
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        encoder.clear(&self.bundle.data.out, CLEAR_COLOR);
        self.bundle.encode(encoder);
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.bundle.data.out = window_targets.color;
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Triangle example");
}