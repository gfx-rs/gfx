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
extern crate gfx_app;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use rand::Rng;
use cgmath::{SquareMatrix, Matrix4, Point3, Vector3, EuclideanVector, deg};
use cgmath::{Transform, AffineMatrix3};
pub use gfx::format::Depth;
pub use gfx_app::ColorFormat;
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{SharedVertex, IndexedPolygon};
use time::precise_time_s;

use noise::{Seed, perlin2};

// Remember to also change the constants in the shaders
const NUM_LIGHTS: usize = 250;
const LIGHT_RADIUS: f32 = 3.0;
const EMITTER_RADIUS: f32 = 0.2;
const TERRAIN_SCALE: [f32; 3] = [25.0, 25.0, 25.0];

pub type GFormat = [f32; 4];

gfx_constant_struct!(LightInfo {
    pos: [f32; 4] = "pos",
});

gfx_vertex_struct!( TerrainVertex {
    pos: [f32; 3] = "a_Pos",
    normal: [f32; 3] = "a_Normal",
    color: [f32; 3] = "a_Color",
});

gfx_constant_struct!( TerrainLocals {
    model: [[f32; 4]; 4] = "u_Model",
    view: [[f32; 4]; 4] = "u_View",
    proj: [[f32; 4]; 4] = "u_Proj",
});

gfx_pipeline!( terrain {
    vbuf: gfx::VertexBuffer<TerrainVertex> = (),
    locals: gfx::ConstantBuffer<TerrainLocals> = "TerrainLocals",
    //TODO: reconstruct the position from the depth instead of
    // storing it in the GBuffer
    out_position: gfx::RenderTarget<GFormat> = "Target0",
    out_normal: gfx::RenderTarget<GFormat> = "Target1",
    out_color: gfx::RenderTarget<GFormat> = "Target2",
    out_depth: gfx::DepthTarget<Depth> =
        gfx::preset::depth::LESS_EQUAL_WRITE,
});

pub static TERRAIN_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    layout(std140)
    uniform TerrainLocals {
        mat4 u_Model;
        mat4 u_View;
        mat4 u_Proj;
    };
    in vec3 a_Pos;
    in vec3 a_Normal;
    in vec3 a_Color;
    out vec3 v_FragPos;
    out vec3 v_Normal;
    out vec3 v_Color;

    void main() {
        v_FragPos = (u_Model * vec4(a_Pos, 1.0)).xyz;
        v_Normal = mat3(u_Model) * a_Normal;
        v_Color = a_Color;
        gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
    }
";

pub static TERRAIN_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    in vec3 v_FragPos;
    in vec3 v_Normal;
    in vec3 v_Color;
    out vec4 o_Position;
    out vec4 o_Normal;
    out vec4 o_Color;

    void main() {
        vec3 n = normalize(v_Normal);

        o_Position = vec4(v_FragPos, 0.0);
        o_Normal = vec4(n, 0.0);
        o_Color = vec4(v_Color, 1.0);
    }
";

gfx_vertex_struct!( BlitVertex {
    pos: [i8; 2] = "a_Pos",
    tex_coord: [i8; 2] = "a_TexCoord",
});

gfx_pipeline!( blit {
    vbuf: gfx::VertexBuffer<BlitVertex> = (),
    tex: gfx::TextureSampler<[f32; 4]> = "t_BlitTex",
    out: gfx::RenderTarget<ColorFormat> = "Target0",
});

pub static BLIT_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    in ivec2 a_Pos;
    in ivec2 a_TexCoord;
    out vec2 v_TexCoord;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = vec4(a_Pos, 0.0, 1.0);
    }
";

pub static BLIT_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    uniform sampler2D t_BlitTex;
    in vec2 v_TexCoord;
    out vec4 o_Color;

    void main() {
        vec4 tex = texture(t_BlitTex, v_TexCoord);
        o_Color = tex;
    }
";

gfx_vertex_struct!( CubeVertex {
    pos: [i8; 4] = "a_Pos",
});

gfx_constant_struct!( CubeLocals {
    transform: [[f32; 4]; 4] = "u_Transform",
    radius: f32 = "u_Radius",
});

gfx_constant_struct!( LightLocals {
    cam_pos_and_radius: [f32; 4] = "u_CameraPosAndRadius",
});

gfx_pipeline!( light {
    vbuf: gfx::VertexBuffer<CubeVertex> = (),
    locals_vs: gfx::ConstantBuffer<CubeLocals> = "CubeLocals",
    locals_ps: gfx::ConstantBuffer<LightLocals> = "LightLocals",
    light_pos_buf: gfx::ConstantBuffer<LightInfo> = "u_LightPosBlock",
    tex_pos: gfx::TextureSampler<[f32; 4]> = "t_Position",
    tex_normal: gfx::TextureSampler<[f32; 4]> = "t_Normal",
    tex_diffuse: gfx::TextureSampler<[f32; 4]> = "t_Diffuse",
    out_color: gfx::BlendTarget<GFormat> =
        ("Target0", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
    out_depth: gfx::DepthTarget<Depth> =
        gfx::preset::depth::LESS_EQUAL_TEST,
});

pub static LIGHT_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    in ivec3 a_Pos;
    out vec3 v_LightPos;

    layout(std140)
    uniform CubeLocals {
        mat4 u_Transform;
        float u_Radius;
    };

    const int NUM_LIGHTS = 250;
    layout(std140)
    uniform u_LightPosBlock {
        vec4 offs[NUM_LIGHTS];
    };

    void main() {
        v_LightPos = offs[gl_InstanceID].xyz;
        gl_Position = u_Transform * vec4(u_Radius * a_Pos + v_LightPos, 1.0);
    }
";

pub static LIGHT_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    layout(std140)
    uniform LightLocals {
        vec4 u_CameraPosAndRadius;
    };
    uniform sampler2D t_Position;
    uniform sampler2D t_Normal;
    uniform sampler2D t_Diffuse;
    in vec3 v_LightPos;
    out vec4 o_Color;

    void main() {
        ivec2 itc = ivec2(gl_FragCoord.xy);
        vec3 pos     = texelFetch(t_Position, itc, 0).xyz;
        vec3 normal  = texelFetch(t_Normal,   itc, 0).xyz;
        vec3 diffuse = texelFetch(t_Diffuse,  itc, 0).xyz;

        vec3 light    = v_LightPos;
        vec3 to_light = normalize(light - pos);
        vec3 to_cam   = normalize(u_CameraPosAndRadius.xyz - pos);

        vec3 n = normalize(normal);
        float s = pow(max(0.0, dot(to_cam, reflect(-to_light, n))), 20.0);
        float d = max(0.0, dot(n, to_light));

        float dist_sq = dot(light - pos, light - pos);
        float scale = max(0.0, 1.0 - dist_sq * u_CameraPosAndRadius.w);

        vec3 res_color = d * diffuse + vec3(s);

        o_Color = vec4(scale*res_color, 1.0);
    }
";

gfx_pipeline!( emitter {
    vbuf: gfx::VertexBuffer<CubeVertex> = (),
    locals: gfx::ConstantBuffer<CubeLocals> = "CubeLocals",
    light_pos_buf: gfx::ConstantBuffer<LightInfo> = "u_LightPosBlock",
    out_color: gfx::BlendTarget<GFormat> =
        ("Target0", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
    out_depth: gfx::DepthTarget<Depth> =
        gfx::preset::depth::LESS_EQUAL_TEST,
});

pub static EMITTER_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    in ivec3 a_Pos;

    layout(std140)
    uniform CubeLocals {
        mat4 u_Transform;
        float u_Radius;
    };

    const int NUM_LIGHTS = 250;
    layout(std140)
    uniform u_LightPosBlock {
        vec4 offs[NUM_LIGHTS];
    };

    void main() {
        gl_Position = u_Transform * vec4(u_Radius * a_Pos + offs[gl_InstanceID].xyz, 1.0);
    }
";

pub static EMITTER_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    out vec4 o_Color;

    void main() {
        o_Color = vec4(1.0, 1.0, 1.0, 1.0);
    }
";

fn calculate_normal(seed: &Seed, x: f32, y: f32)-> [f32; 3] {
    // determine sample points
    let s_x0 = x - 0.001;
    let s_x1 = x + 0.001;
    let s_y0 = y - 0.001;
    let s_y1 = y + 0.001;

    // calculate gradient in point
    let dzdx = (perlin2(seed, &[s_x1, y]) - perlin2(seed, &[s_x0, y]))/(s_x1 - s_x0);
    let dzdy = (perlin2(seed, &[x, s_y1]) - perlin2(seed, &[x, s_y0]))/(s_y1 - s_y0);

    // cross gradient vectors to get normal
    let normal = Vector3::new(1.0, 0.0, dzdx).cross(Vector3::new(0.0, 1.0, dzdy)).normalize();

    return normal.into();
}

fn calculate_color(height: f32) -> [f32; 3] {
    if height > 8.0 {
        [0.9, 0.9, 0.9] // white
    } else if height > 0.0 {
        [0.7, 0.7, 0.7] // greay
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
    type Surface = gfx::format::D24;
    type Channel = gfx::format::Unorm;
    type View = [f32; 4];

    fn get_format() -> gfx::format::Format {
        use gfx::format as f;
        f::Format(f::SurfaceType::D24, f::ChannelType::Unorm)
    }
}

fn create_g_buffer<R: gfx::Resources, F: gfx::Factory<R>>(
                   width: gfx::tex::Size, height: gfx::tex::Size, factory: &mut F)
                   -> (ViewPair<R, GFormat>, ViewPair<R, GFormat>, ViewPair<R, GFormat>,
                       gfx::handle::ShaderResourceView<R, [f32; 4]>, gfx::handle::DepthStencilView<R, Depth>)
{
    use gfx::format::ChannelSource;
    let pos = {
        let (_ , srv, rtv) = factory.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let normal = {
        let (_ , srv, rtv) = factory.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let diffuse = {
        let (_ , srv, rtv) = factory.create_render_target(width, height).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let (tex, _srv, depth_rtv) = factory.create_depth_stencil(width, height).unwrap();
    // ignoring the default SRV since we need to create a custom one with swizzling
    let swizzle = gfx::format::Swizzle(ChannelSource::X, ChannelSource::X, ChannelSource::X, ChannelSource::X);
    let depth_srv = factory.view_texture_as_shader_resource::<DepthFormat>(&tex, (0,0), swizzle).unwrap();

    (pos, normal, diffuse, depth_srv, depth_rtv)
}


struct App<R: gfx::Resources> {
    terrain: terrain::Bundle<R>,
    blit: blit::Bundle<R>,
    light: light::Bundle<R>,
    emitter: emitter::Bundle<R>,
    intermediate: ViewPair<R, GFormat>,
    light_pos_vec: Vec<LightInfo>,
    seed: Seed,
    debug_buf: Option<gfx::handle::ShaderResourceView<R, [f32; 4]>>,
}

impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
    fn new<F: gfx::Factory<R>>(mut factory: F, init: gfx_app::Init<R>) -> Self {
        use gfx::traits::FactoryExt;

        let (width, height, _, _) = init.color.get_dimensions();
        let (gpos, gnormal, gdiffuse, _depth_resource, depth_target) =
            create_g_buffer(width, height, &mut factory);
        let res = {
            let (_ , srv, rtv) = factory.create_render_target(width, height).unwrap();
            ViewPair{ resource: srv, target: rtv }
        };

        let seed = {
            let rand_seed = rand::thread_rng().gen();
            Seed::new(rand_seed)
        };

        let sampler = factory.create_sampler(
            gfx::tex::SamplerInfo::new(gfx::tex::FilterMethod::Scale,
                                       gfx::tex::WrapMode::Clamp)
        );

        let terrain = {
            let plane = genmesh::generators::Plane::subdivide(256, 256);
            let vertex_data: Vec<TerrainVertex> = plane.shared_vertex_iter()
                .map(|(x, y)| {
                    let h = TERRAIN_SCALE[2] * perlin2(&seed, &[x, y]);
                    TerrainVertex {
                        pos: [TERRAIN_SCALE[0] * x, TERRAIN_SCALE[1] * y, h],
                        normal: calculate_normal(&seed, x, y),
                        color: calculate_color(h),
                    }
                })
                .collect();

            let index_data: Vec<u32> = plane.indexed_polygon_iter()
                .triangulate()
                .vertices()
                .map(|i| i as u32)
                .collect();

            let (vbuf, slice) = factory.create_vertex_buffer_indexed(&vertex_data, &index_data[..]);

            let vs = gfx_app::shade::Source {
                glsl_150: TERRAIN_VERTEX_SRC,
                hlsl_40:  include_bytes!("data/terrain_vs.fx"),
                .. gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: TERRAIN_FRAGMENT_SRC,
                hlsl_40:  include_bytes!("data/terrain_ps.fx"),
                .. gfx_app::shade::Source::empty()
            };

            let pso = factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                ps.select(init.backend).unwrap(),
                gfx::state::CullFace::Back, terrain::new()
                ).unwrap();

            let data = terrain::Data {
                vbuf: vbuf,
                locals: factory.create_constant_buffer(1),
                out_position: gpos.target.clone(),
                out_normal: gnormal.target.clone(),
                out_color: gdiffuse.target.clone(),
                out_depth: depth_target.clone(),
            };

            terrain::bundle(slice, pso, data)
        };

        let blit = {
            let vertex_data = [
                BlitVertex { pos: [-3, -1], tex_coord: [-1, 0] },
                BlitVertex { pos: [ 1, -1], tex_coord: [1, 0] },
                BlitVertex { pos: [ 1,  3], tex_coord: [1, 2] },
            ];

            let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

            let vs = gfx_app::shade::Source {
                glsl_150: BLIT_VERTEX_SRC,
                hlsl_40:  include_bytes!("data/blit_vs.fx"),
                .. gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: BLIT_FRAGMENT_SRC,
                hlsl_40:  include_bytes!("data/blit_ps.fx"),
                .. gfx_app::shade::Source::empty()
            };

            let pso = factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                ps.select(init.backend).unwrap(),
                gfx::state::CullFace::Nothing, blit::new()
                ).unwrap();

            let data = blit::Data {
                vbuf: vbuf,
                tex: (gpos.resource.clone(), sampler.clone()),
                out: init.color,
            };

            blit::bundle(slice, pso, data)
        };

        let light_pos_buffer = factory.create_constant_buffer(NUM_LIGHTS);

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

            factory.create_vertex_buffer_indexed(&vertex_data, index_data)
        };
        light_slice.instances = Some((NUM_LIGHTS as gfx::InstanceCount, 0));

        let light = {
            let vs = gfx_app::shade::Source {
                glsl_150: LIGHT_VERTEX_SRC,
                hlsl_40:  include_bytes!("data/light_vs.fx"),
                .. gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: LIGHT_FRAGMENT_SRC,
                hlsl_40:  include_bytes!("data/light_ps.fx"),
                .. gfx_app::shade::Source::empty()
            };

            let pso = factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                ps.select(init.backend).unwrap(),
                gfx::state::CullFace::Back, light::new()
                ).unwrap();

            let data = light::Data {
                vbuf: light_vbuf.clone(),
                locals_vs: factory.create_constant_buffer(1),
                locals_ps: factory.create_constant_buffer(1),
                light_pos_buf: light_pos_buffer.clone(),
                tex_pos: (gpos.resource.clone(), sampler.clone()),
                tex_normal: (gnormal.resource.clone(), sampler.clone()),
                tex_diffuse: (gdiffuse.resource.clone(), sampler.clone()),
                out_color: res.target.clone(),
                out_depth: depth_target.clone(),
            };

            light::bundle(light_slice.clone(), pso, data)
        };

        let emitter = {
            let vs = gfx_app::shade::Source {
                glsl_150: EMITTER_VERTEX_SRC,
                hlsl_40:  include_bytes!("data/emitter_vs.fx"),
                .. gfx_app::shade::Source::empty()
            };
            let ps = gfx_app::shade::Source {
                glsl_150: EMITTER_FRAGMENT_SRC,
                hlsl_40:  include_bytes!("data/emitter_ps.fx"),
                .. gfx_app::shade::Source::empty()
            };

            let pso = factory.create_pipeline_simple(
                vs.select(init.backend).unwrap(),
                ps.select(init.backend).unwrap(),
                gfx::state::CullFace::Back, emitter::new()
                ).unwrap();

            let data = emitter::Data {
                vbuf: light_vbuf.clone(),
                locals: factory.create_constant_buffer(1),
                light_pos_buf: light_pos_buffer.clone(),
                out_color: res.target.clone(),
                out_depth: depth_target.clone(),
            };

            emitter::bundle(light_slice, pso, data)
        };

        App {
            terrain: terrain,
            blit: blit,
            light: light,
            emitter: emitter,
            intermediate: res,
            light_pos_vec: (0 ..NUM_LIGHTS).map(|_| {
                LightInfo{ pos: [0.0, 0.0, 0.0, 0.0] }
            }).collect(),
            seed: seed,
            debug_buf: None,
        }
    }

    /*fn update(&mut self) {
        Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key1)) =>
            debug_buf = Some(gpos.resource.clone()),
        Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key2)) =>
            debug_buf = Some(gnormal.resource.clone()),
        Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key3)) =>
            debug_buf = Some(gdiffuse.resource.clone()),
        Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key4)) =>
            debug_buf = Some(depth_resource.clone()),
        Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key0)) =>
            debug_buf = None,
    }*/

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        let time = precise_time_s() as f32;

        // Update camera position
        let cam_pos = {
            // Slowly circle the center
            let x = (0.05*time).sin();
            let y = (0.05*time).cos();
            Point3::new(x * 32.0, y * 32.0, 16.0)
        };
        let view: AffineMatrix3<f32> = Transform::look_at(
            cam_pos,
            Point3::new(0.0, 0.0, 0.0),
            Vector3::unit_z(),
        );
        let (width, height, _, _) = self.terrain.data.out_depth.get_dimensions();
        let aspect = width as f32 / height as f32;
        let proj = cgmath::perspective(deg(60.0f32), aspect, 5.0, 100.0);

        let terrain_locals = TerrainLocals {
            model: Matrix4::identity().into(),
            view: view.mat.into(),
            proj: proj.into(),
        };
        encoder.update_constant_buffer(&self.terrain.data.locals, &terrain_locals);

        let light_locals = LightLocals {
            cam_pos_and_radius: [cam_pos.x, cam_pos.y, cam_pos.z,
                1.0 / (LIGHT_RADIUS * LIGHT_RADIUS)],
        };
        encoder.update_buffer(&self.light.data.locals_ps, &[light_locals], 0).unwrap();

        let mut cube_locals = CubeLocals {
            transform: (proj * view.mat).into(),
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
            let h = perlin2(&self.seed, &[x, y]);

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
        self.terrain.encode(encoder);

        let blit_tex = match self.debug_buf {
            Some(ref tex) => tex,   // Show one of the immediate buffers
            None => {
                encoder.clear(&self.intermediate.target, [0.0, 0.0, 0.0, 1.0]);
                // Apply lights
                self.light.encode(encoder);
                // Draw light emitters
                self.emitter.encode(encoder);
                &self.intermediate.resource
            }
        };
        self.blit.data.tex.0 = blit_tex.clone();
        // Show the result
        self.blit.encode(encoder);
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_default("Deferred rendering example with gfx-rs");
}
