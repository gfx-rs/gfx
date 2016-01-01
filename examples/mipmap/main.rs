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

use gfx::format::{Rgba8};

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

gfx_pipeline_init!( PipeData PipeMeta PipeInit {
    vbuf: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    tex: gfx::TextureSampler<Rgba8> = "t_Tex",
    out: gfx::RenderTarget<Rgba8> = ("o_Color", gfx::state::MASK_ALL),
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

fn make_texture<R, F>(factory: &mut F) -> gfx::handle::ShaderResourceView<R, Rgba8>
        where R: gfx::Resources, 
              F: gfx::Factory<R>
{
    /*
    let tex_info = gfx::tex::TextureInfo {
        kind: gfx::tex::Kind::D2(4, 4, gfx::tex::AaMode::Single),
        levels: 3,
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
    };*/

    let kind = gfx::tex::Kind::D2(2, 2, gfx::tex::AaMode::Single);
    //TODO: proper update
    //let tex = factory.create_new_texture(kind, gfx::SHADER_RESOURCE, 3,
    //    Some(gfx::format::ChannelType::UintNormalized)
    //    ).unwrap();
    //factory.update_texture(&tex, &l0_info, &L0_DATA, None).unwrap();
    //factory.update_texture(&tex, &l1_info, &L1_DATA, None).unwrap();
    //factory.update_texture(&tex, &l2_info, &L2_DATA, None).unwrap();

    //factory.view_texture_as_shader_resource(&tex, (0, 2)).unwrap()

    let (_, view) = factory.create_texture_const(kind, gfx::cast_slice(&L0_DATA), true)
                           .unwrap();
    view
}

pub fn main() {
    use gfx::traits::{Device, Factory, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_title("Mipmap example".to_string())
        .with_dimensions(800, 600);
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init_new::<gfx::format::Rgba8>(builder);
    let mut encoder = factory.create_encoder();

    let pso = factory.create_pipeline_simple(
        include_bytes!("shader/120.glslv"),
        include_bytes!("shader/120.glslf"),
        gfx::state::CullFace::Nothing,
        &PipeInit::new()
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

    let data = PipeData {
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
        encoder.draw_pipeline(&slice, &pso, &data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
