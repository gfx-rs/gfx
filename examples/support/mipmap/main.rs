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
extern crate gfx_support;

use gfx::{texture, Device, GraphicsPoolExt};
use gfx_support::{BackbufferView, ColorFormat};

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        uv: [f32; 2] = "a_Uv",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        tex: gfx::TextureSampler<[f32; 4]> = "t_Tex",
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

impl Vertex {
    fn new(p: [f32; 2], u: [f32; 2]) -> Vertex {
        Vertex {
            pos: p,
            uv: u,
        }
    }
}

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


struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    pso: gfx::PipelineState<B::Resources, pipe::Meta>,
    data: pipe::Data<B::Resources>,
    slice: gfx::Slice<B::Resources>,
}

impl<B: gfx::Backend> gfx_support::Application<B> for App<B> {
    fn new(device: &mut B::Device,
           _: &mut gfx::queue::GraphicsQueue<B>,
           backend: gfx_support::shade::Backend,
           window_targets: gfx_support::WindowTargets<B::Resources>) -> Self
    {
        use gfx::traits::DeviceExt;

        let vs = gfx_support::shade::Source {
            glsl_120: include_bytes!("shader/120.glslv"),
            glsl_150: include_bytes!("shader/150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_support::shade::Source::empty()
        };
        let fs = gfx_support::shade::Source {
            glsl_120: include_bytes!("shader/120.glslf"),
            glsl_150: include_bytes!("shader/150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_support::shade::Source::empty()
        };

        let vertex_data = [
            Vertex::new([ 0.0,  0.0], [ 0.0,  0.0]),
            Vertex::new([ 1.0,  0.0], [50.0,  0.0]),
            Vertex::new([ 1.0,  1.1], [50.0, 50.0]),

            Vertex::new([ 0.0,  0.0], [  0.0,   0.0]),
            Vertex::new([-1.0,  0.0], [800.0,   0.0]),
            Vertex::new([-1.0, -1.0], [800.0, 800.0]),
        ];
        let (vbuf, slice) = device.create_vertex_buffer_with_slice(&vertex_data, ());

        let (_, texture_view) = device.create_texture_immutable::<ColorFormat>(
            texture::Kind::D2(4, 4, texture::AaMode::Single),
            &[&L0_DATA, &L1_DATA, &L2_DATA]
            ).unwrap();

        let sampler = device.create_sampler(texture::SamplerInfo::new(
            texture::FilterMethod::Trilinear,
            texture::WrapMode::Tile,
        ));

        let out_color = window_targets.views[0].0.clone();

        App {
            views: window_targets.views,
            pso: device.create_pipeline_simple(
                vs.select(backend).unwrap(),
                fs.select(backend).unwrap(),
                pipe::new()
                ).unwrap(),
            data: pipe::Data {
                vbuf,
                tex: (texture_view, sampler),
                out: out_color,
            },
            slice,
        }
    }

    fn render(&mut self, (frame, sync): (gfx::Frame, &gfx_support::SyncPrimitives<B::Resources>),
              pool: &mut gfx::GraphicsCommandPool<B>, queue: &mut gfx::queue::GraphicsQueue<B>)
    {
        let (cur_color, _) = self.views[frame.id()].clone();
        self.data.out = cur_color;

        let mut encoder = pool.acquire_graphics_encoder();
        encoder.clear(&self.data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&self.slice, &self.pso, &self.data);
        encoder.synced_flush(queue, &[&sync.rendering], &[], Some(&sync.frame_fence))
               .expect("Could not flush encoder");
    }

    fn on_resize(&mut self, window_targets: gfx_support::WindowTargets<B::Resources>) {
        self.views = window_targets.views;
    }
}

pub fn main() {
    use gfx_support::Application;
    App::launch_simple("Mipmap example");
}
