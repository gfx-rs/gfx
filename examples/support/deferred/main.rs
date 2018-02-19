
// This is an example of deferred shading with gfx-rs.
//
// Two render targets are created: a geometry buffer and a result buffer.
//
// Rendering happens in two passes:
// First,  the terrain is rendered, writing position, normal and color to the geometry buffer.
// Second, the lights are rendered as cubes. each fragment reads from the geometry buffer,
//         light is applied, and the result is written to the result buffer.
//
// The result buffer is then displayed.
//
// Press 1-4 to show the immediate buffers. Press 0 to show the final result.

extern crate cgmath;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate gfx_support;
extern crate genmesh;
extern crate noise;
extern crate winit;

use gfx_support::{BackbufferView, ColorFormat};

#[cfg(feature="metal")]
use gfx::format::Depth32F as Depth;
#[cfg(not(feature="metal"))]
use gfx::format::Depth;

use cgmath::{Deg, Matrix4, Point3, SquareMatrix, Vector3};
use gfx::{Bundle, Device, GraphicsPoolExt, texture};
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{SharedVertex, IndexedPolygon};
use noise::{NoiseModule, Perlin};
use std::time::Instant;
use winit::WindowEvent;

// Remember to also change the constants in the shaders
const NUM_LIGHTS: usize = 250;
const LIGHT_RADIUS: f32 = 3.0;
const EMITTER_RADIUS: f32 = 0.2;
const TERRAIN_SCALE: [f32; 3] = [25.0, 25.0, 25.0];

pub type GFormat = [f32; 4];

gfx_defines!{
    constant LightInfo {
        pos: [f32; 4] = "pos",
    }

    vertex TerrainVertex {
        pos: [f32; 3] = "a_Pos",
        normal: [f32; 3] = "a_Normal",
        color: [f32; 3] = "a_Color",
    }

    vertex BlitVertex {
        pos_tex: [i8; 4] = "a_PosTexCoord",
    }

    vertex CubeVertex {
        pos: [i8; 4] = "a_Pos",
    }

    constant LightLocals {
        cam_pos_and_radius: [f32; 4] = "u_CamPosAndRadius",
    }

    constant TerrainLocals {
        model: [[f32; 4]; 4] = "u_Model",
        view: [[f32; 4]; 4] = "u_View",
        proj: [[f32; 4]; 4] = "u_Proj",
    }

    constant CubeLocals {
        transform: [[f32; 4]; 4] = "u_Transform",
        radius: f32 = "u_Radius",
    }

    pipeline light {
        vbuf: gfx::VertexBuffer<CubeVertex> = (),
        locals_vs: gfx::ConstantBuffer<CubeLocals> = "CubeLocals",
        locals_ps: gfx::ConstantBuffer<LightLocals> = "LightLocals",
        light_pos_buf: gfx::ConstantBuffer<LightInfo> = "LightPosBlock",
        tex_pos: gfx::TextureSampler<[f32; 4]> = "t_Position",
        tex_normal: gfx::TextureSampler<[f32; 4]> = "t_Normal",
        tex_diffuse: gfx::TextureSampler<[f32; 4]> = "t_Diffuse",
        out_color: gfx::BlendTarget<GFormat> =
            ("Target0", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
        out_depth: gfx::DepthTarget<Depth> =
            gfx::preset::depth::LESS_EQUAL_TEST,
    }

    pipeline terrain {
        vbuf: gfx::VertexBuffer<TerrainVertex> = (),
        locals: gfx::ConstantBuffer<TerrainLocals> = "TerrainLocals",
        //TODO: reconstruct the position from the depth instead of
        // storing it in the GBuffer
        out_position: gfx::RenderTarget<GFormat> = "Target0",
        out_normal: gfx::RenderTarget<GFormat> = "Target1",
        out_color: gfx::RenderTarget<GFormat> = "Target2",
        out_depth: gfx::DepthTarget<Depth> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }

    pipeline blit {
        vbuf: gfx::VertexBuffer<BlitVertex> = (),
        tex: gfx::TextureSampler<[f32; 4]> = "t_BlitTex",
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }

    pipeline emitter {
        vbuf: gfx::VertexBuffer<CubeVertex> = (),
        locals: gfx::ConstantBuffer<CubeLocals> = "CubeLocals",
        light_pos_buf: gfx::ConstantBuffer<LightInfo> = "LightPosBlock",
        out_color: gfx::BlendTarget<GFormat> =
            ("Target0", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
        out_depth: gfx::DepthTarget<Depth> =
            gfx::preset::depth::LESS_EQUAL_TEST,
    }
}

fn calculate_normal(perlin: &Perlin, x: f32, y: f32)-> [f32; 3] {
    use cgmath::InnerSpace;

    // determine sample points
    let s_x0 = x - 0.001;
    let s_x1 = x + 0.001;
    let s_y0 = y - 0.001;
    let s_y1 = y + 0.001;

    // calculate gradient in point
    let dzdx = (perlin.get([s_x1, y]) - perlin.get([s_x0, y]))/(s_x1 - s_x0);
    let dzdy = (perlin.get([x, s_y1]) - perlin.get([x, s_y0]))/(s_y1 - s_y0);

    // cross gradient vectors to get normal
    let normal = Vector3::new(1.0, 0.0, dzdx).cross(Vector3::new(0.0, 1.0, dzdy)).normalize();

    return normal.into();
}

fn calculate_color(height: f32) -> [f32; 3] {
    if height > 8.0 {
        [0.9, 0.9, 0.9] // white
    } else if height > 0.0 {
        [0.7, 0.7, 0.7] // grey
    } else if height > -5.0 {
        [0.2, 0.7, 0.2] // green
    } else {
        [0.2, 0.2, 0.7] // blue
    }
}

struct ViewPair<R: gfx::Resources, T: gfx::format::Formatted> {
    resource: gfx::handle::ShaderResourceView<R, T::View>,
    target: gfx::handle::RenderTargetView<R, T>,
}

// need a custom depth format in order to view SRV depth as float4
struct DepthFormat;
impl gfx::format::Formatted for DepthFormat {
    #[cfg(feature="metal")]
    type Surface = gfx::format::D32;
    #[cfg(not(feature="metal"))]
    type Surface = gfx::format::D24;

    type Channel = gfx::format::Unorm;
    type View = [f32; 4];

    fn get_format() -> gfx::format::Format {
        use gfx::format as f;
        f::Format(f::SurfaceType::D24, f::ChannelType::Unorm)
    }
}

fn create_g_buffer<R: gfx::Resources, D: gfx::Device<R>>(
                   width: texture::Size, height: texture::Size, device: &mut D)
                   -> (ViewPair<R, GFormat>, ViewPair<R, GFormat>, ViewPair<R, GFormat>,
                       gfx::handle::ShaderResourceView<R, [f32; 4]>, gfx::handle::DepthStencilView<R, Depth>)
{
    use gfx::format::ChannelSource;
    let pos = {
        let (_ , srv, rtv) = device.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let normal = {
        let (_ , srv, rtv) = device.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let diffuse = {
        let (_ , srv, rtv) = device.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let (tex, _srv, depth_rtv) = device.create_depth_stencil(width, height).unwrap();
    // ignoring the default SRV since we need to create a custom one with swizzling
    let swizzle = gfx::format::Swizzle(ChannelSource::X, ChannelSource::X, ChannelSource::X, ChannelSource::X);
    let depth_srv = device.view_texture_as_shader_resource::<DepthFormat>(&tex, (0,0), swizzle).unwrap();

    (pos, normal, diffuse, depth_srv, depth_rtv)
}


struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    terrain: Bundle<B, terrain::Data<B::Resources>>,
    blit: Bundle<B, blit::Data<B::Resources>>,
    light: Bundle<B, light::Data<B::Resources>>,
    emitter: Bundle<B, emitter::Data<B::Resources>>,
    intermediate: ViewPair<B::Resources, GFormat>,
    light_pos_vec: Vec<LightInfo>,
    perlin: Perlin,
    depth_resource: gfx::handle::ShaderResourceView<B::Resources, [f32; 4]>,
    debug_buf: Option<gfx::handle::ShaderResourceView<B::Resources, [f32; 4]>>,
    start_time: Instant,
}

impl<B: gfx::Backend> gfx_support::Application<B> for App<B> {
    fn new(device: &mut B::Device,
           _: &mut gfx::queue::GraphicsQueue<B>,
           backend: gfx_support::shade::Backend,
           window_targets: gfx_support::WindowTargets<B::Resources>) -> Self
    {
        use gfx::traits::DeviceExt;

        let (width, height, _, _) = window_targets.views[0].0.dimensions();
        let (gpos, gnormal, gdiffuse, depth_resource, depth_target) =
            create_g_buffer(width, height, device);
        let res = {
            let (_ , srv, rtv) = device.create_render_target(width, height).unwrap();
            ViewPair{ resource: srv, target: rtv }
        };

        let perlin = Perlin::new();

        let sampler = device.create_sampler(
            texture::SamplerInfo::new(texture::FilterMethod::Scale,
                                       texture::WrapMode::Clamp)
        );

        let terrain = {
            let plane = genmesh::generators::Plane::subdivide(256, 256);
            let vertex_data: Vec<TerrainVertex> = plane.shared_vertex_iter()
                .map(|genmesh::Vertex { pos, .. }| {
                    let (x, y) = (pos[0], pos[1]);
                    let h = TERRAIN_SCALE[2] * perlin.get([x, y]);
                    TerrainVertex {
                        pos: [TERRAIN_SCALE[0] * x, TERRAIN_SCALE[1] * y, h],
                        normal: calculate_normal(&perlin, x, y),
                        color: calculate_color(h),
                    }
                })
                .collect();

            let index_data: Vec<u32> = plane.indexed_polygon_iter()
                .triangulate()
                .vertices()
                .map(|i| i as u32)
                .collect();

            let (vbuf, slice) = device.create_vertex_buffer_with_slice(&vertex_data, &index_data[..]);

            let vs = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/terrain.glslv"),
                hlsl_40:  include_bytes!("data/terrain_vs.fx"),
                msl_11:   include_bytes!("shader/terrain_vertex.metal"),
                .. gfx_support::shade::Source::empty()
            };
            let ps = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/terrain.glslf"),
                hlsl_40:  include_bytes!("data/terrain_ps.fx"),
                msl_11:   include_bytes!("shader/terrain_frag.metal"),
                .. gfx_support::shade::Source::empty()
            };

            let pso = device.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                terrain::new()
                ).unwrap();

            let data = terrain::Data {
                vbuf: vbuf,
                locals: device.create_constant_buffer(1),
                out_position: gpos.target.clone(),
                out_normal: gnormal.target.clone(),
                out_color: gdiffuse.target.clone(),
                out_depth: depth_target.clone(),
            };

            Bundle::new(slice, pso, data)
        };

        let blit = {
            let vertex_data = [
                BlitVertex { pos_tex: [-3, -1, -1, 0] },
                BlitVertex { pos_tex: [ 1, -1,  1, 0] },
                BlitVertex { pos_tex: [ 1,  3,  1, 2] },
            ];

            let (vbuf, slice) = device.create_vertex_buffer_with_slice(&vertex_data, ());

            let vs = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/blit.glslv"),
                hlsl_40:  include_bytes!("data/blit_vs.fx"),
                msl_11:   include_bytes!("shader/blit_vertex.metal"),
                .. gfx_support::shade::Source::empty()
            };
            let ps = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/blit.glslf"),
                hlsl_40:  include_bytes!("data/blit_ps.fx"),
                msl_11:   include_bytes!("shader/blit_frag.metal"),
                .. gfx_support::shade::Source::empty()
            };

            let pso = device.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                blit::new()
                ).unwrap();

            let data = blit::Data {
                vbuf: vbuf,
                tex: (gpos.resource.clone(), sampler.clone()),
                out: window_targets.views[0].0.clone(),
            };

            Bundle::new(slice, pso, data)
        };

        let light_pos_buffer = device.create_constant_buffer(NUM_LIGHTS);

        let (light_vbuf, mut light_slice) = {
            let vertex_data = [
                // top (0, 0, 1)
                CubeVertex { pos: [-1, -1,  1, 1] },
                CubeVertex { pos: [ 1, -1,  1, 1] },
                CubeVertex { pos: [ 1,  1,  1, 1] },
                CubeVertex { pos: [-1,  1,  1, 1] },
                // bottom (0, 0, -1)
                CubeVertex { pos: [-1,  1, -1, 1] },
                CubeVertex { pos: [ 1,  1, -1, 1] },
                CubeVertex { pos: [ 1, -1, -1, 1] },
                CubeVertex { pos: [-1, -1, -1, 1] },
                // right (1, 0, 0)
                CubeVertex { pos: [ 1, -1, -1, 1] },
                CubeVertex { pos: [ 1,  1, -1, 1] },
                CubeVertex { pos: [ 1,  1,  1, 1] },
                CubeVertex { pos: [ 1, -1,  1, 1] },
                // left (-1, 0, 0)
                CubeVertex { pos: [-1, -1,  1, 1] },
                CubeVertex { pos: [-1,  1,  1, 1] },
                CubeVertex { pos: [-1,  1, -1, 1] },
                CubeVertex { pos: [-1, -1, -1, 1] },
                // front (0, 1, 0)
                CubeVertex { pos: [ 1,  1, -1, 1] },
                CubeVertex { pos: [-1,  1, -1, 1] },
                CubeVertex { pos: [-1,  1,  1, 1] },
                CubeVertex { pos: [ 1,  1,  1, 1] },
                // back (0, -1, 0)
                CubeVertex { pos: [ 1, -1,  1, 1] },
                CubeVertex { pos: [-1, -1,  1, 1] },
                CubeVertex { pos: [-1, -1, -1, 1] },
                CubeVertex { pos: [ 1, -1, -1, 1] },
            ];

            let index_data: &[u16] = &[
                 0,  1,  2,  2,  3,  0, // top
                 4,  5,  6,  6,  7,  4, // bottom
                 8,  9, 10, 10, 11,  8, // right
                12, 13, 14, 14, 15, 12, // left
                16, 17, 18, 18, 19, 16, // front
                20, 21, 22, 22, 23, 20, // back
            ];

            device.create_vertex_buffer_with_slice(&vertex_data, index_data)
        };
        light_slice.instances = Some((NUM_LIGHTS as gfx::InstanceCount, 0));

        let light = {
            let vs = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/light.glslv"),
                hlsl_40:  include_bytes!("data/light_vs.fx"),
                msl_11:   include_bytes!("shader/light_vertex.metal"),
                .. gfx_support::shade::Source::empty()
            };
            let ps = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/light.glslf"),
                hlsl_40:  include_bytes!("data/light_ps.fx"),
                msl_11:   include_bytes!("shader/light_frag.metal"),
                .. gfx_support::shade::Source::empty()
            };

            let pso = device.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                light::new()
                ).unwrap();

            let data = light::Data {
                vbuf: light_vbuf.clone(),
                locals_vs: device.create_constant_buffer(1),
                locals_ps: device.create_constant_buffer(1),
                light_pos_buf: light_pos_buffer.clone(),
                tex_pos: (gpos.resource.clone(), sampler.clone()),
                tex_normal: (gnormal.resource.clone(), sampler.clone()),
                tex_diffuse: (gdiffuse.resource.clone(), sampler.clone()),
                out_color: res.target.clone(),
                out_depth: depth_target.clone(),
            };

            Bundle::new(light_slice.clone(), pso, data)
        };

        let emitter = {
            let vs = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/emitter.glslv"),
                hlsl_40:  include_bytes!("data/emitter_vs.fx"),
                msl_11:   include_bytes!("shader/emitter_vertex.metal"),
                .. gfx_support::shade::Source::empty()
            };
            let ps = gfx_support::shade::Source {
                glsl_150: include_bytes!("shader/emitter.glslf"),
                hlsl_40:  include_bytes!("data/emitter_ps.fx"),
                msl_11:   include_bytes!("shader/emitter_frag.metal"),
                .. gfx_support::shade::Source::empty()
            };

            let pso = device.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                emitter::new()
                ).unwrap();

            let data = emitter::Data {
                vbuf: light_vbuf.clone(),
                locals: device.create_constant_buffer(1),
                light_pos_buf: light_pos_buffer.clone(),
                out_color: res.target.clone(),
                out_depth: depth_target.clone(),
            };

            Bundle::new(light_slice, pso, data)
        };

        App {
            views: window_targets.views,
            terrain,
            blit,
            light,
            emitter,
            intermediate: res,
            light_pos_vec: (0 ..NUM_LIGHTS).map(|_| {
                LightInfo{ pos: [0.0, 0.0, 0.0, 0.0] }
            }).collect(),
            perlin,
            depth_resource,
            debug_buf: None,
            start_time: Instant::now(),
        }
    }

    fn render(&mut self, (frame, sync): (gfx::Frame, &gfx_support::SyncPrimitives<B::Resources>),
              pool: &mut gfx::GraphicsCommandPool<B>, queue: &mut gfx::queue::GraphicsQueue<B>)
    {
        let elapsed = self.start_time.elapsed();
        let time = elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1000_000_000.0;

        // Update camera position
        let cam_pos = {
            // Slowly circle the center
            let x = (0.05*time).sin();
            let y = (0.05*time).cos();
            Point3::new(x * 32.0, y * 32.0, 16.0)
        };
        let view = Matrix4::look_at(
            cam_pos,
            Point3::new(0.0, 0.0, 0.0),
            Vector3::unit_z(),
        );
        let (width, height, _, _) = self.terrain.data.out_depth.dimensions();
        let aspect = width as f32 / height as f32;
        let proj = cgmath::perspective(Deg(60.0f32), aspect, 5.0, 100.0);
        let (cur_color, _) = self.views[frame.id()].clone();

        let mut encoder = pool.acquire_graphics_encoder();

        let terrain_locals = TerrainLocals {
            model: Matrix4::identity().into(),
            view: view.into(),
            proj: proj.into(),
        };
        encoder.update_constant_buffer(&self.terrain.data.locals, &terrain_locals);

        let light_locals = LightLocals {
            cam_pos_and_radius: [cam_pos.x, cam_pos.y, cam_pos.z,
                1.0 / (LIGHT_RADIUS * LIGHT_RADIUS)],
        };
        encoder.update_buffer(&self.light.data.locals_ps, &[light_locals], 0).unwrap();

        let mut cube_locals = CubeLocals {
            transform: (proj * view).into(),
            radius: LIGHT_RADIUS,
        };
        encoder.update_constant_buffer(&self.light.data.locals_vs, &cube_locals);
        cube_locals.radius = EMITTER_RADIUS;
        encoder.update_constant_buffer(&self.emitter.data.locals, &cube_locals);

        // Update light positions
        for (i, d) in self.light_pos_vec.iter_mut().enumerate() {
            let (x, y) = {
                let fi = i as f32;
                // Distribute lights nicely
                let r = 1.0 - (fi*fi) / ((NUM_LIGHTS*NUM_LIGHTS) as f32);
                (r * (0.2*time + i as f32).cos(), r * (0.2*time + i as f32).sin())
            };
            let h = self.perlin.get([x, y]);

            d.pos[0] = TERRAIN_SCALE[0] * x;
            d.pos[1] = TERRAIN_SCALE[1] * y;
            d.pos[2] = TERRAIN_SCALE[2] * h + 0.5;
        };
        encoder.update_buffer(&self.light.data.light_pos_buf, &self.light_pos_vec, 0).unwrap();

        encoder.clear_depth(&self.terrain.data.out_depth, 1.0);
        encoder.clear(&self.terrain.data.out_position, [0.0, 0.0, 0.0, 1.0]);
        encoder.clear(&self.terrain.data.out_normal, [0.0, 0.0, 0.0, 1.0]);
        encoder.clear(&self.terrain.data.out_color, [0.0, 0.0, 0.0, 1.0]);
        // Render the terrain to the geometry buffer
        self.terrain.encode(&mut encoder);

        let blit_tex = match self.debug_buf {
            Some(ref tex) => tex,   // Show one of the immediate buffers
            None => {
                encoder.clear(&self.intermediate.target, [0.0, 0.0, 0.0, 1.0]);
                // Apply lights
                self.light.encode(&mut encoder);
                // Draw light emitters
                self.emitter.encode(&mut encoder);
                &self.intermediate.resource
            }
        };
        self.blit.data.out = cur_color;
        self.blit.data.tex.0 = blit_tex.clone();
        // Show the result
        self.blit.encode(&mut encoder);
        encoder.synced_flush(queue, &[&sync.rendering], &[], Some(&sync.frame_fence))
               .expect("Could not flush encoder");
    }

    fn on(&mut self, event: WindowEvent) {
        if let WindowEvent::KeyboardInput {
            input: winit::KeyboardInput {
                virtual_keycode: Some(key),
                ..
            },
            .. } = event {
            use winit::VirtualKeyCode::*;
            match key {
                Key1 => self.debug_buf = Some(self.light.data.tex_pos.0.clone()),
                Key2 => self.debug_buf = Some(self.light.data.tex_normal.0.clone()),
                Key3 => self.debug_buf = Some(self.light.data.tex_diffuse.0.clone()),
                Key4 => self.debug_buf = Some(self.depth_resource.clone()),
                Key0 => self.debug_buf = None,
                _ => (),
            }
        }
    }

    fn on_resize_ext(&mut self, device: &mut B::Device, window_targets: gfx_support::WindowTargets<B::Resources>) {
        let (width, height, _, _) = window_targets.views[0].0.dimensions();

        let (gpos, gnormal, gdiffuse, depth_resource, depth_target) =
            create_g_buffer(width, height, device);
        self.intermediate = {
            let (_ , srv, rtv) = device.create_render_target(width, height).unwrap();
            ViewPair{ resource: srv, target: rtv }
        };
        self.views = window_targets.views;
        self.terrain.data.out_position = gpos.target.clone();
        self.terrain.data.out_normal = gnormal.target.clone();
        self.terrain.data.out_color = gdiffuse.target.clone();
        self.terrain.data.out_depth = depth_target.clone();

        self.blit.data.tex.0 = gpos.resource.clone();

        self.light.data.tex_pos.0 = gpos.resource.clone();
        self.light.data.tex_normal.0 = gnormal.resource.clone();
        self.light.data.tex_diffuse.0 = gdiffuse.resource.clone();
        self.light.data.out_color = self.intermediate.target.clone();
        self.light.data.out_depth = depth_target.clone();

        self.emitter.data.out_color = self.intermediate.target.clone();
        self.emitter.data.out_depth = depth_target.clone();

        self.depth_resource = depth_resource;
    }
}

pub fn main() {
    use gfx_support::Application;
    App::launch_simple("Deferred rendering example with gfx-rs");
}
