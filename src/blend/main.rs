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
extern crate gfx_window_glutin;
extern crate glutin;

extern crate image;

use std::io::Cursor;
use gfx::traits::{Factory, Stream, FactoryExt};

gfx_vertex!( Vertex {
    a_Pos@ pos: [f32; 2],
    a_Uv@ uv: [f32; 2],
});

impl Vertex {
    fn new(p: [f32; 2], u: [f32; 2]) -> Vertex {
        Vertex {
            pos: p,
            uv: u,
        }
    }
}

gfx_parameters!( Params {
    t_Lena@ lena: gfx::shade::TextureParam<R>,
    t_Tint@ tint: gfx::shade::TextureParam<R>,
    i_Blend@ blend: i32,
});

fn load_texture<R, F>(factory: &mut F, data: &[u8]) -> Result<gfx::handle::Texture<R>, String>
        where R: gfx::Resources, F: gfx::device::Factory<R> {
    let img = image::load(Cursor::new(data), image::PNG).unwrap();

    let (fmt, img) = match img {
        image::DynamicImage::ImageRgba8(img) => (gfx::tex::RGBA8, img),
        img =>                                  (gfx::tex::RGBA8, img.to_rgba())
    };
    let (width, height) = img.dimensions();
    let tex_info = gfx::tex::TextureInfo {
        width: width as u16,
        height: height as u16,
        depth: 1,
        levels: 1,
        kind: gfx::tex::Kind::D2,
        format: fmt
    };

    Ok(factory.create_texture_static(tex_info, &img).unwrap())
}

pub fn main() {
    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(
        glutin::WindowBuilder::new()
            .with_title("Blending example".to_string())
            .with_dimensions(800, 600).build().unwrap()
    );

    // fullscreen quad
    let vertex_data = [
        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([ 1.0, -1.0], [1.0, 1.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 0.0]),

        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([ 1.0,  1.0], [1.0, 0.0]),
        Vertex::new([-1.0,  1.0], [0.0, 0.0]),
    ];
    let mesh = factory.create_mesh(&vertex_data);

    let lena_texture = load_texture(&mut factory, &include_bytes!("image/lena.png")[..]).unwrap();
    let tint_texture = load_texture(&mut factory, &include_bytes!("image/tint.png")[..]).unwrap();

    let program = {
        let vs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/blend_120.glslv")),
            glsl_150: Some(include_bytes!("shader/blend_150.glslv")),
            .. gfx::ShaderSource::empty()
        };
        let fs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/blend_120.glslf")),
            glsl_150: Some(include_bytes!("shader/blend_150.glslf")),
            .. gfx::ShaderSource::empty()
        };
        factory.link_program_source(vs, fs).unwrap()
    };

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

    let uniforms = Params {
        lena: (lena_texture, None),
        tint: (tint_texture, None),
        blend: blend_func.0,
        _r: std::marker::PhantomData,
    };
    let mut batch = gfx::batch::Full::new(mesh, program, uniforms).unwrap();

    'main: loop {
        // quit when Esc is pressed.
        for event in stream.out.window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
                glutin::Event::KeyboardInput(glutin::ElementState::Pressed, _, Some(glutin::VirtualKeyCode::B)) => {
                    let blend_func = blends_cycle.next().unwrap();

                    println!("Using '{}' blend equation", blend_func.1);
                    batch.params.blend = blend_func.0;
                },
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        stream.clear(gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });

        stream.draw(&batch).unwrap();
        stream.present(&mut device);
    }
}
