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

extern crate time;

#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

extern crate image;

use std::io::Cursor;
pub use gfx::format::Rgba8;

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

gfx_pipeline!( pipe {
    vbuf: gfx::VertexBuffer<Vertex> = (),
    color: gfx::TextureSampler<[f32; 4]> = "t_Color",
    flow: gfx::TextureSampler<[f32; 4]> = "t_Flow",
    noise: gfx::TextureSampler<[f32; 4]> = "t_Noise",
    offset0: gfx::Global<f32> = "f_Offset0",
    offset1: gfx::Global<f32> = "f_Offset1",
    out: gfx::RenderTarget<Rgba8> = "o_Color",
});

fn load_texture<R, F>(factory: &mut F, data: &[u8])
                -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R> {
    use gfx::tex as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_const::<Rgba8>(kind, gfx::cast_slice(&img), false).unwrap();
    Ok(view)
}

pub fn main() {
    use time::precise_time_s;
    use gfx::traits::{Device, FactoryExt};

    let builder = glutin::WindowBuilder::new()
            .with_title("Flowmap example".to_string())
            .with_dimensions(800, 600);
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init::<Rgba8>(builder);
    let mut encoder = factory.create_encoder();

    let vertex_data = [
        Vertex::new([-1.0, -1.0], [0.0, 0.0]),
        Vertex::new([ 1.0, -1.0], [1.0, 0.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 1.0]),

        Vertex::new([-1.0, -1.0], [0.0, 0.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 1.0]),
        Vertex::new([-1.0,  1.0], [0.0, 1.0]),
    ];

    let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

    let water_texture = load_texture(&mut factory, &include_bytes!("image/water.png")[..]).unwrap();
    let flow_texture  = load_texture(&mut factory, &include_bytes!("image/flow.png")[..]).unwrap();
    let noise_texture = load_texture(&mut factory, &include_bytes!("image/noise.png")[..]).unwrap();
    let sampler = factory.create_sampler_linear();

    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/flowmap_150.glslv"),
        include_bytes!("shader/flowmap_150.glslf"),
        gfx::state::CullFace::Nothing,
        pipe::new()
        ).unwrap();

    let mut data = pipe::Data {
        vbuf: vbuf,
        color: (water_texture, sampler.clone()),
        flow: (flow_texture, sampler.clone()),
        noise: (noise_texture, sampler.clone()),
        offset0: 0f32,
        offset1: 0.5f32,
        out: main_color,
    };

    let mut cycle0 = 0.0f32;
    let mut cycle1 = 0.5f32;

    let mut time_start = precise_time_s();
    let mut time_end;
    'main: loop {
        time_end = time_start;
        time_start = precise_time_s();

        let delta = (time_start - time_end) as f32;

        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        // since we sample our diffuse texture twice we need to lerp between
        // them to get a smooth transition (shouldn't even be noticable).

        // they start half a cycle apart (0.5) and is later used to calculate
        // the interpolation amount via `2.0 * abs(cycle0 - .5f)`
        cycle0 += 0.25f32 * delta;
        if cycle0 > 1f32 {
            cycle0 -= 1f32;
        }

        cycle1 += 0.25f32 * delta;
        if cycle1 > 1f32 {
            cycle1 -= 1f32;
        }

        encoder.reset();
        encoder.clear(&data.out, [0.3, 0.3, 0.3, 1.0]);

        data.offset0 = cycle0;
        data.offset1 = cycle1;
        encoder.draw(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
