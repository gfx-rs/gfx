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

#[macro_use]
extern crate gfx;
extern crate gfx_app;
extern crate cgmath;
extern crate image;

pub use gfx_app::ColorFormat;
pub use gfx::format::{Depth, Rgba8};

use cgmath::{Deg, Matrix4};
use gfx::{Bundle, texture};
use std::io::Cursor;
use std::time::{Instant};

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
    }

    constant Locals {
        inv_proj: [[f32; 4]; 4] = "u_InvProj",
        view: [[f32; 4]; 4] = "u_WorldToCamera",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        cubemap: gfx::TextureSampler<[f32; 4]> = "t_Cubemap",
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

impl Vertex {
    fn new(p: [f32; 2]) -> Vertex {
        Vertex {
            pos: p,
        }
    }
}

struct CubemapData<'a> {
    up: &'a [u8],
    down: &'a [u8],
    front: &'a [u8],
    back: &'a [u8],
    right: &'a [u8],
    left: &'a [u8],
}

impl<'a> CubemapData<'a> {
    fn as_array(self) -> [&'a [u8]; 6] {
        [self.right, self.left, self.up, self.down, self.front, self.back]
    }
}

fn load_cubemap<R, F>(factory: &mut F, data: CubemapData) -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String>
        where R: gfx::Resources, F: gfx::Factory<R>
{
    let images = data.as_array().iter().map(|data| {
        image::load(Cursor::new(data), image::JPEG).unwrap().to_rgba()
    }).collect::<Vec<_>>();
    let data: [&[u8]; 6] = [&images[0], &images[1], &images[2], &images[3], &images[4], &images[5]];
    let kind = texture::Kind::Cube(images[0].dimensions().0 as u16);
    match factory.create_texture_immutable_u8::<Rgba8>(kind, &data) {
        Ok((_, view)) => Ok(view),
        Err(_) => Err("Unable to create an immutable cubemap texture".to_owned()),
    }
}

struct App<R: gfx::Resources>{
    bundle: Bundle<R, pipe::Data<R>>,
    projection: Matrix4<f32>,
    start_time: Instant,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(factory: &mut F, backend: gfx_app::shade::Backend,
           window_targets: gfx_app::WindowTargets<R>) -> Self {
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
        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, ());

        let cubemap = load_cubemap(factory, CubemapData {
            up: &include_bytes!("image/posy.jpg")[..],
            down: &include_bytes!("image/negy.jpg")[..],
            front: &include_bytes!("image/posz.jpg")[..],
            back: &include_bytes!("image/negz.jpg")[..],
            right: &include_bytes!("image/posx.jpg")[..],
            left: &include_bytes!("image/negx.jpg")[..],
        }).unwrap();

        let sampler = factory.create_sampler_linear();

        let proj = cgmath::perspective(Deg(60.0f32), window_targets.aspect_ratio, 0.01, 100.0);

        let pso = factory.create_pipeline_simple(
            vs.select(backend).unwrap(),
            ps.select(backend).unwrap(),
            pipe::new()
        ).unwrap();

        let data = pipe::Data {
            vbuf: vbuf,
            cubemap: (cubemap, sampler),
            locals: factory.create_constant_buffer(1),
            out: window_targets.color,
        };

        App {
            bundle: Bundle::new(slice, pso, data),
            projection: proj,
            start_time: Instant::now(),
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        {
            use cgmath::{Matrix4, Point3, SquareMatrix, Vector3};

            // Update camera position
            let elapsed = self.start_time.elapsed();
            let time = (elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1000_000_000.0) * 0.25;
            let x = time.sin();
            let z = time.cos();

            let view = Matrix4::look_at(
                Point3::new(x, x / 2.0, z),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::unit_y(),
            );

            let locals = Locals {
                inv_proj: self.projection.invert().unwrap().into(),
                view: view.into(),
            };
            encoder.update_constant_buffer(&self.bundle.data.locals, &locals);
        }

        encoder.clear(&self.bundle.data.out, [0.3, 0.3, 0.3, 1.0]);
        self.bundle.encode(encoder);
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.bundle.data.out = window_targets.color;
        self.projection = cgmath::perspective(Deg(60.0f32), window_targets.aspect_ratio, 0.01, 100.0);
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Skybox example");
}
