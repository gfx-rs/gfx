// Copyright 2014 The Gfx-rs Developers.
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
extern crate image;

use std::io::Cursor;
use std::time::Instant;
use gfx::format::{Rgba8, Depth};
use gfx_app::{BackbufferView, ColorFormat};
use gfx::{Bundle, GraphicsPoolExt};

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        uv: [f32; 2] = "a_Uv",
    }

    constant Locals {
        offsets: [f32; 2] = "u_Offsets",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        color: gfx::TextureSampler<[f32; 4]> = "t_Color",
        flow: gfx::TextureSampler<[f32; 4]> = "t_Flow",
        noise: gfx::TextureSampler<[f32; 4]> = "t_Noise",
        offset0: gfx::Global<f32> = "f_Offset0",
        offset1: gfx::Global<f32> = "f_Offset1",
        locals: gfx::ConstantBuffer<Locals> = "Locals",
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

fn load_texture<R, F>(factory: &mut F, data: &[u8])
                -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R> {
    use gfx::texture as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_immutable_u8::<Rgba8>(kind, &[&img]).unwrap();
    Ok(view)
}

struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    bundle: Bundle<B, pipe::Data<B::Resources>>,
    cycles: [f32; 2],
    time_start: Instant,
}

impl<B: gfx::Backend> gfx_app::Application<B> for App<B> {
    fn new(factory: &mut B::Factory,
           backend: gfx_app::shade::Backend,
           window_targets: gfx_app::WindowTargets<B::Resources>) -> Self
    {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/flowmap_120.glslv"),
            glsl_150: include_bytes!("shader/flowmap_150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            msl_11:   include_bytes!("shader/flowmap_vertex.metal"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/flowmap_120.glslf"),
            glsl_150: include_bytes!("shader/flowmap_150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            msl_11:   include_bytes!("shader/flowmap_frag.metal"),
            .. gfx_app::shade::Source::empty()
        };

        let vertex_data = [
            Vertex::new([-1.0, -1.0], [0.0, 0.0]),
            Vertex::new([ 1.0, -1.0], [1.0, 0.0]),
            Vertex::new([ 1.0,  1.0], [1.0, 1.0]),

            Vertex::new([-1.0, -1.0], [0.0, 0.0]),
            Vertex::new([ 1.0,  1.0], [1.0, 1.0]),
            Vertex::new([-1.0,  1.0], [0.0, 1.0]),
        ];

        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, ());

        let water_texture = load_texture(factory, &include_bytes!("image/water.png")[..]).unwrap();
        let flow_texture  = load_texture(factory, &include_bytes!("image/flow.png")[..]).unwrap();
        let noise_texture = load_texture(factory, &include_bytes!("image/noise.png")[..]).unwrap();
        let sampler = factory.create_sampler_linear();

        let pso = factory.create_pipeline_simple(
            vs.select(backend).unwrap(),
            ps.select(backend).unwrap(),
            pipe::new()
            ).unwrap();

        let data = pipe::Data {
            vbuf: vbuf,
            color: (water_texture, sampler.clone()),
            flow: (flow_texture, sampler.clone()),
            noise: (noise_texture, sampler.clone()),
            offset0: 0.0,
            offset1: 0.0,
            locals: factory.create_constant_buffer(1),
            out: window_targets.views[0].0.clone(),
        };

        App {
            views: window_targets.views,
            bundle: Bundle::new(slice, pso, data),
            cycles: [0.0, 0.5],
            time_start: Instant::now(),
        }
    }

    fn render<Gp>(&mut self, (frame, semaphore): (gfx::Frame, &<B::Resources as gfx::Resources>::Semaphore),
                  pool: &mut Gp, queue: &mut gfx::queue::GraphicsQueueMut<B>)
        where Gp: gfx::GraphicsCommandPool<B>
    {
        let delta = self.time_start.elapsed();
        self.time_start = Instant::now();
        let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1000_000_000.0;

        // since we sample our diffuse texture twice we need to lerp between
        // them to get a smooth transition (shouldn't even be noticable).
        // They start half a cycle apart (0.5) and is later used to calculate
        // the interpolation amount via `2.0 * abs(cycle0 - .5f)`
        self.cycles[0] += 0.25 * delta;
        if self.cycles[0] > 1.0 {
            self.cycles[0] -= 1.0;
        }
        self.cycles[1] += 0.25 * delta;
        if self.cycles[1] > 1.0 {
            self.cycles[1] -= 1.0;
        }

        let (cur_color, _) = self.views[frame.id()].clone();
        self.bundle.data.out = cur_color;

        let mut encoder = pool.acquire_graphics_encoder();
        self.bundle.data.offset0 = self.cycles[0];
        self.bundle.data.offset1 = self.cycles[1];
        let locals = Locals { offsets: self.cycles };
        encoder.update_constant_buffer(&self.bundle.data.locals, &locals);

        encoder.clear(&self.bundle.data.out, [0.3, 0.3, 0.3, 1.0]);
        self.bundle.encode(&mut encoder);
        encoder.flush(queue);
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<B::Resources>) {
        self.views = window_targets.views;
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Flowmap example");
}
