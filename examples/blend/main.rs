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
extern crate gfx_app;
extern crate image;
extern crate winit;

pub use gfx_app::ColorFormat;
pub use gfx::format::{Rgba8, DepthStencil};
use gfx::Bundle;

gfx_defines!{
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        uv: [f32; 2] = "a_Uv",
    }

    constant Locals {
        blend: i32 = "u_Blend",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        lena: gfx::TextureSampler<[f32; 4]> = "t_Lena",
        tint: gfx::TextureSampler<[f32; 4]> = "t_Tint",
        blend: gfx::Global<i32> = "i_Blend",
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
                -> Result<gfx::handle::ShaderResourceView<R, [f32; 4]>, String> where
                R: gfx::Resources, F: gfx::Factory<R> {
    use std::io::Cursor;
    use gfx::texture as t;
    let img = image::load(Cursor::new(data), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = t::Kind::D2(width as t::Size, height as t::Size, t::AaMode::Single);
    let (_, view) = factory.create_texture_immutable_u8::<Rgba8>(kind, &[&img]).unwrap();
    Ok(view)
}

const BLENDS: [&'static str; 9] = [
    "Screen",
    "Dodge",
    "Burn",
    "Overlay",
    "Multiply",
    "Add",
    "Divide",
    "Grain Extract",
    "Grain Merge",
];

struct App<R: gfx::Resources>{
    bundle: Bundle<R, pipe::Data<R>>,
    id: u8,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(factory: &mut F, backend: gfx_app::shade::Backend,
           window_targets: gfx_app::WindowTargets<R>) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/blend_120.glslv"),
            glsl_150: include_bytes!("shader/blend_150.glslv"),
            hlsl_40:  include_bytes!("data/vertex.fx"),
            .. gfx_app::shade::Source::empty()
        };
        let ps = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/blend_120.glslf"),
            glsl_150: include_bytes!("shader/blend_150.glslf"),
            hlsl_40:  include_bytes!("data/pixel.fx"),
            .. gfx_app::shade::Source::empty()
        };

        // fullscreen quad
        let vertex_data = [
            Vertex::new([-1.0, -1.0], [0.0, 1.0]),
            Vertex::new([ 1.0, -1.0], [1.0, 1.0]),
            Vertex::new([ 1.0,  1.0], [1.0, 0.0]),

            Vertex::new([-1.0, -1.0], [0.0, 1.0]),
            Vertex::new([ 1.0,  1.0], [1.0, 0.0]),
            Vertex::new([-1.0,  1.0], [0.0, 0.0]),
        ];
        let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, ());

        let lena_texture = load_texture(factory, &include_bytes!("image/lena.png")[..]).unwrap();
        let tint_texture = load_texture(factory, &include_bytes!("image/tint.png")[..]).unwrap();
        let sampler = factory.create_sampler_linear();

        let pso = factory.create_pipeline_simple(
            vs.select(backend).unwrap(),
            ps.select(backend).unwrap(),
            pipe::new()
        ).unwrap();

        // we pass a integer to our shader to show what blending function we want
        // it to use. normally you'd have a shader program per technique, but for
        // the sake of simplicity we'll just branch on it inside the shader.

        // each index correspond to a conditional branch inside the shader
        println!("Using '{}' blend equation", BLENDS[0]);
        let cbuf = factory.create_constant_buffer(1);

        let data = pipe::Data {
            vbuf: vbuf,
            lena: (lena_texture, sampler.clone()),
            tint: (tint_texture, sampler),
            blend: 0,
            locals: cbuf,
            out: window_targets.color,
        };

        App {
            bundle: Bundle::new(slice, pso, data),
            id: 0,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        self.bundle.data.blend = (self.id as i32).into();
        let locals = Locals { blend: self.id as i32 };
        encoder.update_constant_buffer(&self.bundle.data.locals, &locals);
        encoder.clear(&self.bundle.data.out, [0.0; 4]);
        self.bundle.encode(encoder);
    }

    fn on(&mut self, event: winit::WindowEvent) {
        if let winit::WindowEvent::KeyboardInput {
                input: winit::KeyboardInput {
                    state: winit::ElementState::Pressed,
                    virtual_keycode: Some(winit::VirtualKeyCode::B),
                    ..
                },
                .. } = event {
            self.id += 1;
            if self.id as usize >= BLENDS.len() {
                self.id = 0;
            }
            println!("Using '{}' blend equation", BLENDS[self.id as usize]);
        }
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.bundle.data.out = window_targets.color;
    }
}

pub fn main() {
    use gfx_app::Application;
    let wb = winit::WindowBuilder::new().with_title("Blending example");
    App::launch_default(wb);
}
