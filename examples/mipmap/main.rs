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

pub use gfx::format::{Rgba8, Depth};

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
    out: gfx::RenderTarget<Rgba8> = "o_Color",
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

fn make_texture<R, F>(factory: &mut F) -> gfx::handle::ShaderResourceView<R, [f32; 4]>
        where R: gfx::Resources, 
              F: gfx::Factory<R>
{
    let kind = gfx::tex::Kind::D2(4, 4, gfx::tex::AaMode::Single);
    let tex = factory.create_texture(kind, 3, gfx::SHADER_RESOURCE,
        Some(gfx::format::ChannelType::Unorm)).unwrap();

    factory.update_texture::<Rgba8>(&tex, &tex.get_info().to_image_info(0),
        gfx::cast_slice(&L0_DATA), None).unwrap();
    factory.update_texture::<Rgba8>(&tex, &tex.get_info().to_image_info(1),
        gfx::cast_slice(&L1_DATA), None).unwrap();
    factory.update_texture::<Rgba8>(&tex, &tex.get_info().to_image_info(2),
        gfx::cast_slice(&L2_DATA), None).unwrap();

    factory.view_texture_as_shader_resource::<Rgba8>(
        &tex, (0, 2), gfx::format::Swizzle::new()).unwrap()
}

pub fn main() {
    use gfx::traits::{Device, Factory, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_title("Mipmap example".to_string())
        .with_dimensions(800, 600);
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init::<Rgba8, Depth>(builder);
    let mut encoder = factory.create_encoder();

    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/120.glslv"),
        include_bytes!("shader/120.glslf"),
        gfx::state::CullFace::Nothing,
        pipe::new()
        ).unwrap();

    let vertex_data = [
        Vertex::new([ 0.0,  0.0], [ 0.0,  0.0]),
        Vertex::new([ 1.0,  0.0], [50.0,  0.0]),
        Vertex::new([ 1.0,  1.1], [50.0, 50.0]),

        Vertex::new([ 0.0,  0.0], [  0.0,   0.0]),
        Vertex::new([-1.0,  0.0], [800.0,   0.0]),
        Vertex::new([-1.0, -1.0], [800.0, 800.0]),
    ];
    let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

    let texture_view = make_texture(&mut factory);

    let sampler = factory.create_sampler(gfx::tex::SamplerInfo::new(
        gfx::tex::FilterMethod::Trilinear,
        gfx::tex::WrapMode::Tile,
    ));

    let data = pipe::Data {
        vbuf: vbuf,
        tex: (texture_view, sampler),
        out: main_color,
    };

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        encoder.reset();
        encoder.clear(&data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
