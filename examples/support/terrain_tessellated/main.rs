extern crate cgmath;
extern crate genmesh;
#[macro_use]
extern crate gfx;
extern crate gfx_support;
extern crate noise;

use gfx_support::{BackbufferView, ColorFormat, DepthFormat};
use gfx::GraphicsPoolExt;

use cgmath::{Deg, Matrix4, Point3, SquareMatrix, Vector3};
use genmesh::Vertices;
use genmesh::generators::{Plane, SharedVertex, IndexedPolygon};
use noise::{NoiseModule, Perlin};
use std::time::{Instant};

gfx_defines!{
    vertex Vertex {
        pos: [f32; 3] = "a_Pos",
        color: [f32; 3] = "a_Color",
    }

    constant Locals {
        model: [[f32; 4]; 4] = "u_Model",
        view: [[f32; 4]; 4] = "u_View",
        proj: [[f32; 4]; 4] = "u_Proj",
    }

    pipeline pipe {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
        view: gfx::Global<[[f32; 4]; 4]> = "u_View",
        proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

fn calculate_color(height: f32) -> [f32; 3] {
    if height > 8.0 {
        [0.9, 0.9, 0.9] // white
    } else if height > 0.0 {
        [0.7, 0.7, 0.7] // gray
    } else if height > -5.0 {
        [0.2, 0.7, 0.2] // green
    } else {
        [0.2, 0.2, 0.7] // blue
    }
}

struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    pso: gfx::PipelineState<B::Resources, pipe::Meta>,
    data: pipe::Data<B::Resources>,
    slice: gfx::Slice<B::Resources>,
    start_time: Instant,
}

impl<B: gfx::Backend> gfx_support::Application<B> for App<B> {
    fn new(device: &mut B::Device,
           _: &mut gfx::queue::GraphicsQueue<B>,
           backend: gfx_support::shade::Backend,
           window_targets: gfx_support::WindowTargets<B::Resources>) -> Self
    {
        use gfx::traits::DeviceExt;

        let vs = gfx_support::shade::Source {
            glsl_150: include_bytes!("shader/terrain.glslv"), //v = vertex
            hlsl_40:  include_bytes!("data/vertex.fx"),
            msl_11: include_bytes!("shader/terrain_vertex.metal"),
            vulkan:   include_bytes!("data/vert.spv"),
            .. gfx_support::shade::Source::empty()
        };
        let hs = gfx_support::shade::Source {
            glsl_150: include_bytes!("shader/terrain.glslc"), //c = tess control
            hlsl_50:  include_bytes!("data/hull.fx"),
            msl_11: include_bytes!("shader/terrain_frag.metal"),
            vulkan:   include_bytes!("data/frag.spv"),
            .. gfx_support::shade::Source::empty()
        };
        let ds = gfx_support::shade::Source {
            glsl_150: include_bytes!("shader/terrain.glsle"), //e = tess evaluation
            hlsl_50:  include_bytes!("data/domain.fx"),
            msl_11: include_bytes!("shader/terrain_frag.metal"),
            vulkan:   include_bytes!("data/frag.spv"),
            .. gfx_support::shade::Source::empty()
        };
        let ps = gfx_support::shade::Source {
            glsl_150: include_bytes!("shader/terrain.glslf"), //f = fragment
            hlsl_40:  include_bytes!("data/pixel.fx"),
            msl_11: include_bytes!("shader/terrain_frag.metal"),
            vulkan:   include_bytes!("data/frag.spv"),
            .. gfx_support::shade::Source::empty()
        };

        let perlin = Perlin::new();
        let plane = Plane::subdivide(16, 16);
        let vertex_data: Vec<Vertex> = plane.shared_vertex_iter()
            .map(|genmesh::Vertex { pos, .. }| {
                let (x, y) = (pos[0], pos[1]);
                let h = perlin.get([x, y]) * 32.0;
                Vertex {
                    pos: [25.0 * x, 25.0 * y, h],
                    color: calculate_color(h),
                }
            })
            .collect();

        let index_data: Vec<u32> = plane.indexed_polygon_iter()
            //.triangulate() NO NEED THESE ARE QUAD PATCHES
            .vertices()
            .map(|i| i as u32)
            .collect();

        let (vbuf, slice) = device.create_vertex_buffer_with_slice(&vertex_data, &index_data[..]);
        let set = device.create_shader_set_tessellation(
            &vs.select(backend).unwrap(),
            &hs.select(backend).unwrap(),
            &ds.select(backend).unwrap(),
            &ps.select(backend).unwrap()
        ).unwrap();
        let mut fillmode = gfx::state::Rasterizer::new_fill();
        fillmode.method = gfx::state::RasterMethod::Line(1);
        let (out_color, out_depth) = window_targets.views[0].clone();
        App {
            views: window_targets.views,
            pso: device.create_pipeline_state(&set,
                gfx::Primitive::PatchList(4), fillmode, pipe::new()).unwrap(),
            data: pipe::Data {
                vbuf,
                locals: device.create_constant_buffer(1),
                model: Matrix4::identity().into(),
                view: Matrix4::identity().into(),
                proj: cgmath::perspective(
                    Deg(60.0f32), window_targets.aspect_ratio, 0.1, 1000.0
                    ).into(),
                out_color,
                out_depth,
            },
            slice,
            start_time: Instant::now(),
        }
    }

    fn render(&mut self, (frame, sync): (gfx::Frame, &gfx_support::SyncPrimitives<B::Resources>),
              pool: &mut gfx::GraphicsCommandPool<B>, queue: &mut gfx::queue::GraphicsQueue<B>)
    {
        let elapsed = self.start_time.elapsed();
        let time = elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1000_000_000.0;
        let x = time.sin();
        let y = time.cos();
        let view = Matrix4::look_at(
            Point3::new(x * 32.0, y * 32.0, 16.0),
            Point3::new(0.0, 0.0, 0.0),
            Vector3::unit_z(),
        );

        self.data.view = view.into();
        let locals = Locals {
            model: self.data.model,
            view: self.data.view,
            proj: self.data.proj,
        };

        let (cur_color, cur_depth) = self.views[frame.id()].clone();
        self.data.out_color = cur_color;
        self.data.out_depth = cur_depth;

        let mut encoder = pool.acquire_graphics_encoder();
        encoder.update_buffer(&self.data.locals, &[locals], 0).unwrap();
        encoder.clear(&self.data.out_color, [0.3, 0.3, 0.3, 1.0]);
        encoder.clear_depth(&self.data.out_depth, 1.0);
        encoder.draw(&self.slice, &self.pso, &self.data);
        encoder.synced_flush(queue, &[&sync.rendering], &[], Some(&sync.frame_fence))
               .expect("Could not flush encoder");
    }

    fn on_resize(&mut self, window_targets: gfx_support::WindowTargets<B::Resources>) {
        self.views = window_targets.views;
        self.data.proj = cgmath::perspective(
                Deg(60.0f32), window_targets.aspect_ratio, 0.1, 1000.0
            ).into();
    }
}

pub fn main() {
    use gfx_support::Application;
    App::launch_simple("Terrain tessellation example");
}
