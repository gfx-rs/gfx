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

extern crate image;

pub use gfx::format::{Rgba8, Srgba8, DepthStencil};
use gfx::traits::{Device, Factory, FactoryExt};

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
    lena: gfx::TextureSampler<[f32; 4]> = "t_Lena",
    tint: gfx::TextureSampler<[f32; 4]> = "t_Tint",
    blend: gfx::Global<i32> = "i_Blend",
    out: gfx::RenderTarget<Srgba8> = "o_Color",
});

fn load_texture<R, F>(factory: &mut F, data: &[u8])
                -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String> where
                R: gfx::Resources, F: gfx::Factory<R> {
    use std::io::Cursor;
    use gfx::tex as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_const::<Rgba8>(kind, gfx::cast_slice(&img), false).unwrap();
    Ok(view)
}

pub fn main() {
    let builder = glutin::WindowBuilder::new()
            .with_title("Blending example".to_string())
            .with_dimensions(800, 600);
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init::<Srgba8, DepthStencil>(builder);
    let mut encoder = factory.create_encoder();

    // fullscreen quad
    let vertex_data = [
        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([ 1.0, -1.0], [1.0, 1.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 0.0]),

        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 0.0]),
        Vertex::new([-1.0,  1.0], [0.0, 0.0]),
    ];
    let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

    let lena_texture = load_texture(&mut factory, &include_bytes!("image/lena.png")[..]).unwrap();
    let tint_texture = load_texture(&mut factory, &include_bytes!("image/tint.png")[..]).unwrap();
    let sampler = factory.create_sampler_linear();

    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/blend_150.glslv"),
        include_bytes!("shader/blend_150.glslf"),
        gfx::state::CullFace::Nothing,
        pipe::new()
        ).unwrap();

    // we pass a integer to our shader to show what blending function we want
    // it to use. normally you'd have a shader program per technique, but for
    // the sake of simplicity we'll just branch on it inside the shader.

    // each index correspond to a conditional branch inside the shader
    let blends = [
        (0, "Screen"),
        (1, "Dodge"),
        (2, "Burn"),
        (3, "Overlay"),
        (4, "Multiply"),
        (5, "Add"),
        (6, "Divide"),
        (7, "Grain Extract"),
        (8, "Grain Merge")
    ];
    let mut blends_cycle = blends.iter().cycle();
    let blend_func = blends_cycle.next().unwrap();

    println!("Using '{}' blend equation", blend_func.1);

    let mut data = pipe::Data {
        vbuf: vbuf,
        lena: (lena_texture, sampler.clone()),
        tint: (tint_texture, sampler),
        blend: blend_func.0,
        out: main_color,
    };

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(glutin::ElementState::Pressed, _, Some(glutin::VirtualKeyCode::B)) => {
                    let blend_func = blends_cycle.next().unwrap();
                    println!("Using '{}' blend equation", blend_func.1);
                    data.blend = blend_func.0;
                },
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        encoder.reset();
        encoder.clear(&data.out, [0.0; 4]);
        encoder.draw(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
