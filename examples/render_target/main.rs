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

pub use gfx_app::ColorFormat;
pub use gfx::format::{Depth, Rgba8};

use cgmath::{Deg, Matrix4};
use gfx::{Bundle, texture, memory, format, handle, state};
use std::time::Instant;

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

gfx_defines! {
    vertex VertexRender {
        pos: [f32; 3] = "a_Pos",
    }

    constant LocalsRender {
        pos: [f32; 3] = "pos",
        far_plane: f32 = "farPlane",
    }
    
    constant Matrix {
        matrix: [[f32; 4]; 4] = "matrix",
    }

    pipeline pipe_render {
        vbuf: gfx::VertexBuffer<VertexRender> = (),
        locals: gfx::ConstantBuffer<LocalsRender> = "Locals",
        matrices: gfx::ConstantBuffer<Matrix> = "u_Matrices",
        out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

impl From<cgmath::Matrix4<f32>> for Matrix {
    fn from(m: cgmath::Matrix4<f32>) -> Self {
        Matrix { matrix: m.into() }
    }
}

impl Vertex {
    fn new(p: [f32; 2]) -> Vertex {
        Vertex { pos: p }
    }
}

impl VertexRender {
    fn new(p: [f32; 3]) -> VertexRender {
        VertexRender { pos: p }
    }
}

pub struct DepthFormat;

impl format::Formatted for DepthFormat {
    type Surface = format::D24_S8;
    type Channel = format::Unorm;
    type View = [f32; 4];

    fn get_format() -> format::Format {
        format::Format(format::SurfaceType::D24_S8, format::ChannelType::Unorm)
    }
}

fn create_cubemap<R, F, DF>(
    factory: &mut F,
    size: texture::Size,
) -> Result<
    (handle::ShaderResourceView<R, DF::View>, handle::DepthStencilView<R, DF>),
    gfx::CombinedError,
>
where
    R: gfx::Resources,
    F: gfx::Factory<R>,
    DF: format::DepthFormat + format::TextureFormat,
{
    // Get texture info
    let kind = texture::Kind::Cube(size);
    let levels = 1;
    let bind = gfx::DEPTH_STENCIL | gfx::SHADER_RESOURCE;
    let channel_type = <DF::Channel as format::ChannelTyped>::get_channel_type();

    // Create texture
    let texture = factory.create_texture(
        kind,
        levels,
        bind,
        memory::Usage::Data,
        Some(channel_type),
    )?;

    // View the texture as a shader resource
    let srv = factory.view_texture_as_shader_resource::<DF>(
        &texture,
        (0, 0),
        format::Swizzle::new(),
    )?;

    // View the texture as a depth stencil
    let dsv = factory.view_texture_as_depth_stencil_trivial(&texture)?;

    Ok((srv, dsv))
}

struct App<R: gfx::Resources> {
    bundle: Bundle<R, pipe::Data<R>>,
    bundle_render_cubemap: Bundle<R, pipe_render::Data<R>>,
    projection: Matrix4<f32>,
    proj_render: Matrix4<f32>,
    far_plane: f32,
    start_time: Instant,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(
        factory: &mut F,
        backend: gfx_app::shade::Backend,
        window_targets: gfx_app::WindowTargets<R>,
    ) -> Self {
        use gfx::traits::FactoryExt;

        let proj = cgmath::perspective(Deg(90.0f32), window_targets.aspect_ratio, 0.01, 100.0);

        let (bundle_skybox, dsv) = {
            let vs = gfx_app::shade::Source {
                glsl_150: include_bytes!("shader/cubemap_150.glslv"),
                hlsl_40: include_bytes!("data/vertex.fx"),
                ..gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: include_bytes!("shader/cubemap_150.glslf"),
                hlsl_40: include_bytes!("data/pixel.fx"),
                ..gfx_app::shade::Source::empty()
            };

            let vertex_data = [
                Vertex::new([-1.0, -1.0]),
                Vertex::new([3.0, -1.0]),
                Vertex::new([-1.0, 3.0]),
            ];
            let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, ());

            let (cubemap, dsv) = create_cubemap::<_, _, DepthFormat>(factory, 1024).unwrap();

            let sampler = factory.create_sampler_linear();

            let pso = factory
                .create_pipeline_simple(
                    vs.select(backend).unwrap(),
                    ps.select(backend).unwrap(),
                    pipe::new(),
                )
                .unwrap();

            let data = pipe::Data {
                vbuf: vbuf,
                cubemap: (cubemap, sampler),
                locals: factory.create_constant_buffer(1),
                out: window_targets.color,
            };

            (Bundle::new(slice, pso, data), dsv)
        };

        let bundle_render_cubemap = {
            let vs = gfx_app::shade::Source {
                glsl_150: include_bytes!("shader/render_150.glslv"),
                ..gfx_app::shade::Source::empty()
            };
            let gs = gfx_app::shade::Source {
                glsl_150: include_bytes!("shader/render_150.glslg"),
                ..gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: include_bytes!("shader/render_150.glslf"),
                ..gfx_app::shade::Source::empty()
            };

            let vertex_data = [
                VertexRender::new([0.0, 0.0, 1.0]),
                VertexRender::new([1.0, 1.0, 0.0]),
                VertexRender::new([1.0, -1.0, 0.0]),
            ];

            let (vbuf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data, ());

            let vs = factory
                .create_shader_vertex(&vs.select(backend).unwrap())
                .unwrap();
            let gs = factory
                .create_shader_geometry(&gs.select(backend).unwrap())
                .unwrap();
            let ps = factory
                .create_shader_pixel(&ps.select(backend).unwrap())
                .unwrap();
            let set = gfx::ShaderSet::Geometry(vs, gs, ps);

            let pso = factory
                .create_pipeline_state(
                    &set,
                    gfx::Primitive::TriangleList,
                    state::Rasterizer::new_fill(),
                    pipe_render::new(),
                )
                .unwrap();

            let data = pipe_render::Data {
                vbuf: vbuf,
                locals: factory.create_constant_buffer(1),
                matrices: factory.create_constant_buffer(6),
                out_depth: dsv,
            };

            Bundle::new(slice, pso, data)

        };

        let far_plane = 10.0;
        let proj_render = cgmath::perspective(
            cgmath::Deg(90.0),
            window_targets.aspect_ratio,
            0.1,
            far_plane,
        );

        App {
            bundle: bundle_skybox,
            bundle_render_cubemap,
            projection: proj,
            start_time: Instant::now(),
            proj_render: proj_render,
            far_plane,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        use cgmath::{Matrix4, Point3, SquareMatrix, Vector3, vec3};

        encoder.clear(&self.bundle.data.out, [0.3, 0.3, 0.3, 1.0]);
        self.bundle.encode(encoder);

        // Render to the render target
        {
            encoder.clear_depth(&self.bundle_render_cubemap.data.out_depth, 1.0);

            let far_plane = 10.0;
            let pos = Point3::new(0.0, 0.0, 0.0);

            let mut matrices: [Matrix; 6] =
                [
                    Matrix4::look_at(pos, pos + vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)).into(),
                    Matrix4::look_at(pos, pos + vec3(-1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)).into(),
                    Matrix4::look_at(pos, pos + vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)).into(),
                    Matrix4::look_at(pos, pos + vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, -1.0)).into(),
                    Matrix4::look_at(pos, pos + vec3(0.0, 0.0, 1.0), vec3(0.0, -1.0, 0.0)).into(),
                    Matrix4::look_at(pos, pos + vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0)).into(),
                ];

            for m in &mut matrices {
                m.matrix = (self.proj_render * Matrix4::from(m.matrix)).into();
            }

            let locals = LocalsRender {
                pos: pos.into(),
                far_plane,
            };

            encoder.update_constant_buffer(&self.bundle_render_cubemap.data.locals, &locals);
            encoder
                .update_buffer(&self.bundle_render_cubemap.data.matrices, &matrices, 0)
                .unwrap();
        }

        encoder.clear_depth(&self.bundle_render_cubemap.data.out_depth, 1.0);
        self.bundle_render_cubemap.encode(encoder);

        // Display the render target as a skybox
        {
            // Update camera position
            let elapsed = self.start_time.elapsed();
            let time = (elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1000_000_000.0) *
                0.75;
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
    }

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
        self.bundle.data.out = window_targets.color;
        self.projection =
            cgmath::perspective(Deg(90.0f32), window_targets.aspect_ratio, 0.01, 100.0);

        self.proj_render = cgmath::perspective(
            cgmath::Deg(90.0),
            window_targets.aspect_ratio,
            0.1,
            self.far_plane,
        );
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Render target example");
}
