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

pub use gfx::format::Depth;
pub use gfx_app::ColorFormat;

gfx_vertex_struct!( Vertex {
    pos: [f32; 2] = "a_Pos",
    uv: [f32; 2] = "a_Uv",
});

impl Vertex {
    fn new(p: [f32; 2], u: [f32; 2]) -> Vertex {
        Vertex {
            pos: p,
            uv: u,
        }
    }
}

gfx_pipeline!(pipe {
    vbuf: gfx::VertexBuffer<Vertex> = (),
    tex: gfx::TextureSampler<[f32; 4]> = "t_Tex",
    out: gfx::RenderTarget<ColorFormat> = "Target0",
});

// Larger red dots
const L0_DATA: [[u8; 4]; 16] = [
    [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ],
    [ 0x00, 0x00, 0x00, 0x00 ], [ 0xc0, 0x00, 0x00, 0x00 ], [ 0xc0, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ],
    [ 0x00, 0x00, 0x00, 0x00 ], [ 0xc0, 0x00, 0x00, 0x00 ], [ 0xc0, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ],
    [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ], [ 0x00, 0x00, 0x00, 0x00 ],
];

// Uniform green
const L1_DATA: [[u8; 4]; 4] = [
    [ 0x00, 0xc0, 0x00, 0x00 ], [ 0x00, 0xc0, 0x00, 0x00 ],
    [ 0x00, 0xc0, 0x00, 0x00 ], [ 0x00, 0xc0, 0x00, 0x00 ],
];

// Uniform blue
const L2_DATA: [[u8; 4]; 1] = [ [ 0x00, 0x00, 0xc0, 0x00 ] ];


struct App<R: gfx::Resources> {
    pso: gfx::PipelineState<R, pipe::Meta>,
    data: pipe::Data<R>,
    slice: gfx::Slice<R>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(mut factory: F, init: gfx_app::Init<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/120.glslv"),
            glsl_150: include_bytes!("shader/150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let fs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/120.glslf"),
            glsl_150: include_bytes!("shader/150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_app::shade::Source::empty()
        };

        let vertex_data = [
            Vertex::new([ 0.0,  0.0], [ 0.0,  0.0]),
            Vertex::new([ 1.0,  0.0], [50.0,  0.0]),
            Vertex::new([ 1.0,  1.1], [50.0, 50.0]),

            Vertex::new([ 0.0,  0.0], [  0.0,   0.0]),
            Vertex::new([-1.0,  0.0], [800.0,   0.0]),
            Vertex::new([-1.0, -1.0], [800.0, 800.0]),
        ];
        let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

        let (_, texture_view) = factory.create_texture_const::<ColorFormat>(
            gfx::tex::Kind::D2(4, 4, gfx::tex::AaMode::Single),
            &[&L0_DATA, &L1_DATA, &L2_DATA]
            ).unwrap();

        let sampler = factory.create_sampler(gfx::tex::SamplerInfo::new(
            gfx::tex::FilterMethod::Trilinear,
            gfx::tex::WrapMode::Tile,
        ));

        App {
            pso: factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                fs.select(init.backend).unwrap(),
                gfx::state::CullFace::Nothing,
                pipe::new()
                ).unwrap(),
            data: pipe::Data {
                vbuf: vbuf,
                tex: (texture_view, sampler),
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
    App::launch_default("Mipmap example");
}
