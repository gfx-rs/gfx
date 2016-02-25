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
extern crate gfx_app;


gfx_vertex_struct!( Vertex {
    pos: [f32; 2] = "a_Pos",
    color: [f32; 3] = "a_Color",
});

gfx_pipeline!(pipe {
    vbuf: gfx::VertexBuffer<Vertex> = (),
    out: gfx::RenderTarget<gfx::format::Srgb8> = "Target0",
});

struct App<R: gfx::Resources> {
    pso: gfx::PipelineState<R, pipe::Meta>,
    data: pipe::Data<R>,
    slice: gfx::Slice<R>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(mut factory: F, init: gfx_app::Init<R>) -> Self {
        use gfx::traits::FactoryExt;
        let pso = factory.create_pipeline_simple(
            //include_bytes!("triangle_150.glslv"),
            //include_bytes!("triangle_150.glslf"),
            include_bytes!("data/vertex.fx"),
            include_bytes!("data/pixel.fx"),
            gfx::state::CullFace::Nothing,
            pipe::new()
            ).unwrap();

        let vertex_data = [
            Vertex { pos: [ -0.5, -0.5 ], color: [1.0, 0.0, 0.0] },
            Vertex { pos: [  0.5, -0.5 ], color: [0.0, 1.0, 0.0] },
            Vertex { pos: [  0.0,  0.5 ], color: [0.0, 0.0, 1.0] },
        ];
        let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);
        App {
            pso: pso,
            data: pipe::Data {
                vbuf: vbuf,
                out: init.color,
            },
            slice: slice,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        encoder.clear(&self.data.out, [0.1, 0.2, 0.3]);
        encoder.draw(&self.slice, &self.pso, &self.data);
    }
}

pub fn main() {
    <App<_> as gfx_app::ApplicationD3D11>::launch("Triangle example", gfx_app::Config {
        size: (800, 600),
    });
}
