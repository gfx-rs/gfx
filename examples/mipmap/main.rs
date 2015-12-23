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
    t_Tex@ tex: gfx::shade::TextureParam<R>,
});

// Larger red dots
const L0_DATA: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00,
    0x00, 0xc0, 0xc0, 0x00,
    0x00, 0xc0, 0xc0, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

// Uniform green
const L1_DATA: [u8; 4] = [
    0x10, 0x18,
    0x18, 0x18,
];

// Uniform blue
const L2_DATA: [u8; 1] = [ 0x02 ];


fn make_texture<R, F>(factory: &mut F) -> gfx::shade::TextureParam<R>
        where R: gfx::Resources, 
              F: gfx::Factory<R>
{
    let tex_info = gfx::tex::TextureInfo {
        width: 4,
        height: 4,
        depth: 1,
        levels: 3,
        kind: gfx::tex::Kind::D2(gfx::tex::AaMode::Single),
        format: gfx::tex::Format::SRGB8,
    };

    let l0_info = gfx::tex::ImageInfo {
        xoffset: 0,
        yoffset: 0,
        zoffset: 0,
        width: 4,
        height: 4,
        depth: 1,
        format: gfx::tex::Format::R3_G3_B2,
        mipmap: 0,
    };

    let l1_info = gfx::tex::ImageInfo {
        xoffset: 0,
        yoffset: 0,
        zoffset: 0,
        width: 2,
        height: 2,
        depth: 1,
        format: gfx::tex::Format::R3_G3_B2,
        mipmap: 1,
    };

    let l2_info = gfx::tex::ImageInfo {
        xoffset: 0,
        yoffset: 0,
        zoffset: 0,
        width: 1,
        height: 1,
        depth: 1,
        format: gfx::tex::Format::R3_G3_B2,
        mipmap: 2,
    };

    let tex = factory.create_texture(tex_info).unwrap();
    factory.update_texture(&tex, &l0_info, &L0_DATA, None).unwrap();
    factory.update_texture(&tex, &l1_info, &L1_DATA, None).unwrap();
    factory.update_texture(&tex, &l2_info, &L2_DATA, None).unwrap();

    let sampler_info = gfx::tex::SamplerInfo::new(
        gfx::tex::FilterMethod::Trilinear,
        gfx::tex::WrapMode::Tile,
    );

    let sampler = factory.create_sampler(sampler_info);

    (tex, Some(sampler))
}

pub fn main() {
    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(
        glutin::WindowBuilder::new()
            .with_title("Mipmap example".to_string())
            .with_dimensions(800, 600).build().unwrap()
    );

    let vertex_data = [
        Vertex::new([ 0.0,  0.0], [ 0.0,  0.0]),
        Vertex::new([ 1.0,  0.0], [50.0,  0.0]),
        Vertex::new([ 1.0,  1.1], [50.0, 50.0]),

        Vertex::new([ 0.0,  0.0], [  0.0,   0.0]),
        Vertex::new([-1.0,  0.0], [800.0,   0.0]),
        Vertex::new([-1.0, -1.0], [800.0, 800.0]),
    ];
    let mesh = factory.create_mesh(&vertex_data);

    let program = {
        let vs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/120.glslv")),
            .. gfx::ShaderSource::empty()
        };
        let fs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/120.glslf")),
            .. gfx::ShaderSource::empty()
        };
        factory.link_program_source(vs, fs).unwrap()
    };

    let texture = make_texture(&mut factory);

    let uniforms = Params {
        tex: texture,
        _r: std::marker::PhantomData,
    };
    let batch = gfx::batch::Full::new(mesh, program, uniforms).unwrap();

    'main: loop {
        // quit when Esc is pressed.
        for event in stream.out.window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
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
