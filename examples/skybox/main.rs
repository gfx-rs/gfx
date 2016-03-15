// Copyright 2016 The Gfx-rs Developers.
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
extern crate gfx_app;
extern crate cgmath;

extern crate image;

use std::io::Cursor;
pub use gfx::format::{Srgba8, Depth, Rgba8};
use gfx::tex::{CubeFace, Kind, ImageInfoCommon};

gfx_vertex_struct!( Vertex {
    pos: [f32; 2] = "a_Pos",
});

impl Vertex {
    fn new(p: [f32; 2]) -> Vertex {
        Vertex {
            pos: p,
        }
    }
}

gfx_constant_struct!( Locals {
    inv_proj: [[f32; 4]; 4] = "u_InvProj",
    view: [[f32; 4]; 4] = "u_WorldToCamera",
});

gfx_pipeline!( pipe {
    vbuf: gfx::VertexBuffer<Vertex> = (),
    cubemap: gfx::TextureSampler<[f32; 4]> = "t_Cubemap",
    locals: gfx::ConstantBuffer<Locals> = "Locals",
    out: gfx::RenderTarget<Srgba8> = "Target0",
});

struct CubemapData<'a> {
    up: &'a [u8],
    down: &'a [u8],
    front: &'a [u8],
    back: &'a [u8],
    right: &'a [u8],
    left: &'a [u8],
}

impl<'a> CubemapData<'a> {
    fn as_array(self) -> [(CubeFace, &'a [u8]); 6] {
        [(CubeFace::PosY, self.up),
         (CubeFace::NegY, self.down),
         (CubeFace::PosZ, self.front),
         (CubeFace::NegZ, self.back),
         (CubeFace::PosX, self.right),
         (CubeFace::NegX, self.left)]
    }
}

fn load_cubemap<R, F>(factory: &mut F, data: CubemapData) -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R> {

    let mut cube_tex = None;

    for &(face, img) in data.as_array().iter() {
        let img = image::load(Cursor::new(img), image::JPEG).unwrap().to_rgba();

        let (width, height) = img.dimensions();
        assert_eq!(width, height);

        match cube_tex {
            Some(_) => {},
            None => {
                cube_tex = Some(factory.create_texture(
                        Kind::Cube(width as u16),
                        1,
                        gfx::SHADER_RESOURCE,
                        gfx::Usage::GpuOnly,
                        Some(gfx::format::ChannelType::Unorm)
                ).unwrap())
            }
        }

        let img_info = ImageInfoCommon {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: width as u16,
            height: height as u16,
            format: (),
            depth: 1,
            mipmap: 0
        };

        if let Some(ref ctex) = cube_tex {
            factory.update_texture::<Rgba8>(&ctex, &img_info, gfx::cast_slice(&img), Some(face)).unwrap();
        }
    };

    Ok(factory.view_texture_as_shader_resource::<Rgba8>(&cube_tex.unwrap(), (0, 0), gfx::format::Swizzle::new()).unwrap())
}

struct App<R: gfx::Resources>{
    bundle: pipe::Bundle<R>,
    projection: cgmath::Matrix4<f32>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(mut factory: F, init: gfx_app::Init<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/cubemap_150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_150: include_bytes!("shader/cubemap_150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_app::shade::Source::empty()
        };

        let vertex_data = [
            Vertex::new([-1.0, -1.0]),
            Vertex::new([ 3.0, -1.0]),
            Vertex::new([-1.0,  3.0])
        ];
        let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

        let cubemap = load_cubemap(&mut factory, CubemapData {
            up: &include_bytes!("image/posy.jpg")[..],
            down: &include_bytes!("image/negy.jpg")[..],
            front: &include_bytes!("image/posz.jpg")[..],
            back: &include_bytes!("image/negz.jpg")[..],
            right: &include_bytes!("image/posx.jpg")[..],
            left: &include_bytes!("image/negx.jpg")[..],
        }).unwrap();

        let sampler = factory.create_sampler_linear();

        let proj = cgmath::perspective(cgmath::deg(60.0f32), init.aspect_ratio, 0.01, 100.0);

        let pso = factory.create_pipeline_simple(
            vs.select(init.backend).unwrap(),
            ps.select(init.backend).unwrap(),
            gfx::state::CullFace::Nothing,
            pipe::new()
        ).unwrap();

        let data = pipe::Data {
            vbuf: vbuf,
            cubemap: (cubemap, sampler),
            locals: factory.create_constant_buffer(1),
            out: init.color,
        };

        App {
            bundle: pipe::bundle(slice, pso, data),
            projection: proj,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        {
            use cgmath::{AffineMatrix3, SquareMatrix, Transform, Vector3, Point3};
            // Update camera position
            let time = time::precise_time_s() as f32 * 0.25;
            let x = time.sin();
            let z = time.cos();

            let view: AffineMatrix3<f32> = Transform::look_at(
                Point3::new(x, x / 2.0, z),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::unit_y(),
            );

            let locals = Locals {
                inv_proj: self.projection.invert().unwrap().into(),
                view: view.mat.into(),
            };
            encoder.update_constant_buffer(&self.bundle.data.locals, &locals);
        }

        encoder.clear(&self.bundle.data.out, [0.3, 0.3, 0.3, 1.0]);
        self.bundle.encode(encoder);
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_default("Skybox example");
}
