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
    out: gfx::RenderTarget<gfx::format::Srgba8> = "Target0",
});

struct App<R: gfx::Resources> {
    pso: gfx::PipelineState<R, pipe::Meta>,
    data: pipe::Data<R>,
    slice: gfx::Slice<R>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(mut factory: F, init: gfx_app::Init<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/triangle_120.glslv"),
            glsl_150: include_bytes!("shader/triangle_150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let fs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/triangle_120.glslf"),
            glsl_150: include_bytes!("shader/triangle_150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_app::shade::Source::empty()
        };

        let vertex_data = [
            Vertex { pos: [ -0.5, -0.5 ], color: [1.0, 0.0, 0.0] },
            Vertex { pos: [  0.5, -0.5 ], color: [0.0, 1.0, 0.0] },
            Vertex { pos: [  0.0,  0.5 ], color: [0.0, 0.0, 1.0] },
        ];
        let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

        App {
            pso: factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                fs.select(init.backend).unwrap(),
                gfx::state::CullFace::Nothing,
                pipe::new()
                ).unwrap(),
            data: pipe::Data {
                vbuf: vbuf,
                out: init.color,
            },
            slice: slice,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        encoder.clear(&self.data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&self.slice, &self.pso, &self.data);
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_default("Triangle example");
}
