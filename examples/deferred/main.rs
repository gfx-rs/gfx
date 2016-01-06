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
extern crate env_logger;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use rand::Rng;
use cgmath::FixedArray;
use cgmath::{Matrix, Matrix4, Point3, Vector3, EuclideanVector};
use cgmath::{Transform, AffineMatrix3};
pub use gfx::format::{Depth, I8Scaled, Rgba8};
use gfx::traits::{Device, Factory, FactoryExt};
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{SharedVertex, IndexedPolygon};
use time::precise_time_s;

use noise::{Seed, perlin2};

// Remember to also change the constants in the shaders
const NUM_LIGHTS: usize = 250;

pub type GFormat = [f32; 4];

gfx_constant_struct!(LightInfo {
    pos: [f32; 4],
});

gfx_vertex_struct!( TerrainVertex {
    pos: [f32; 3] = "a_Pos",
    normal: [f32; 3] = "a_Normal",
    color: [f32; 3] = "a_Color",
});

gfx_pipeline!( terrain {
    vbuf: gfx::VertexBuffer<TerrainVertex> = (),
    model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
    view: gfx::Global<[[f32; 4]; 4]> = "u_View",
    proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
    cam_pos: gfx::Global<[f32; 3]> = "u_CameraPos",
    out_position: gfx::RenderTarget<GFormat> = "o_Position",
    out_normal: gfx::RenderTarget<GFormat> = "o_Normal",
    out_color: gfx::RenderTarget<GFormat> = "o_Color",
    out_depth: gfx::DepthTarget<gfx::format::Depth> =
        gfx::preset::depth::LESS_EQUAL_WRITE,
});

pub static TERRAIN_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    uniform mat4 u_Model;
    uniform mat4 u_View;
    uniform mat4 u_Proj;
    in vec3 a_Pos;
    in vec3 a_Normal;
    in vec3 a_Color;
    out vec3 v_FragPos;
    out vec3 v_Normal;
    out vec3 v_Color;

    void main() {
        v_FragPos = (u_Model * vec4(a_Pos, 1.0)).xyz;
        v_Normal = a_Normal;
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
    pos: [I8Scaled; 3] = "a_Pos",
    tex_coord: [I8Scaled; 2] = "a_TexCoord",
});

gfx_pipeline!( blit {
    vbuf: gfx::VertexBuffer<BlitVertex> = (),
    tex: gfx::TextureSampler<GFormat> = "u_Tex",
    out: gfx::RenderTarget<Rgba8> = "o_Color",
});

pub static BLIT_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    in vec3 a_Pos;
    in vec2 a_TexCoord;
    out vec2 v_TexCoord;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = vec4(a_Pos, 1.0);
    }
";

pub static BLIT_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    uniform sampler2D u_Tex;
    in vec2 v_TexCoord;
    out vec4 o_Color;

    void main() {
        vec4 tex = texture(u_Tex, v_TexCoord);
        o_Color = tex;
    }
";

gfx_vertex_struct!( CubeVertex {
    pos: [I8Scaled; 3] = "a_Pos",
});

gfx_pipeline!( light {
    vbuf: gfx::VertexBuffer<CubeVertex> = (),
    transform: gfx::Global<[[f32; 4]; 4]> = "u_Transform",
    light_pos_buf: gfx::ConstantBuffer<LightInfo> = "u_LightPosBlock",
    radius: gfx::Global<f32> = "u_Radius",
    cam_pos: gfx::Global<[f32; 3]> = "u_CameraPos",
    frame_res: gfx::Global<[f32; 2]> = "u_FrameRes",
    tex_pos: gfx::TextureSampler<GFormat> = "u_TexPos",
    tex_normal: gfx::TextureSampler<GFormat> = "u_TexNormal",
    tex_diffuse: gfx::TextureSampler<GFormat> = "u_TexDiffuse",
    out_color: gfx::BlendTarget<GFormat> =
        ("o_Color", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
    out_depth: gfx::DepthTarget<gfx::format::Depth> =
        gfx::preset::depth::LESS_EQUAL_TEST,
});

pub static LIGHT_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    uniform mat4 u_Transform;
    uniform float u_Radius;
    in vec3 a_Pos;
    out vec3 v_LightPos;

    const int NUM_LIGHTS = 250;
    layout(std140)
    uniform u_LightPosBlock {
        vec4 offs[NUM_LIGHTS];
    };

    void main() {
        v_LightPos = offs[gl_InstanceID].xyz;
        gl_Position = u_Transform * vec4(u_Radius * a_Pos + offs[gl_InstanceID].xyz, 1.0);
    }
";

pub static LIGHT_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    uniform float u_Radius;
    uniform vec3 u_CameraPos;
    uniform vec2 u_FrameRes;
    uniform sampler2D u_TexPos;
    uniform sampler2D u_TexNormal;
    uniform sampler2D u_TexDiffuse;
    in vec3 v_LightPos;
    out vec4 o_Color;

    void main() {
        vec2 texCoord = gl_FragCoord.xy / u_FrameRes;
        vec3 pos     = texture(u_TexPos,     texCoord).xyz;
        vec3 normal  = texture(u_TexNormal,  texCoord).xyz;
        vec3 diffuse = texture(u_TexDiffuse, texCoord).xyz;

        vec3 light    = v_LightPos;
        vec3 to_light = normalize(light - pos);
        vec3 to_cam   = normalize(u_CameraPos - pos);

        vec3 n = normalize(normal);
        float s = pow(max(0.0, dot(to_cam, reflect(-to_light, n))), 20.0);
        float d = max(0.0, dot(n, to_light));

        float dist_sq = dot(light - pos, light - pos);
        float scale = max(0.0, 1.0-dist_sq/(u_Radius*u_Radius));

        vec3 res_color = d*vec3(diffuse) + vec3(s);

        o_Color = vec4(scale*res_color, 1.0);
    }
";

gfx_pipeline!( emitter {
    vbuf: gfx::VertexBuffer<CubeVertex> = (),
    transform: gfx::Global<[[f32; 4]; 4]> = "u_Transform",
    light_pos_buf: gfx::ConstantBuffer<LightInfo> = "u_LightPosBlock",
    radius: gfx::Global<f32> = "u_Radius",
    out_color: gfx::BlendTarget<GFormat> =
        ("o_Color", gfx::state::MASK_ALL, gfx::preset::blend::ADD),
    out_depth: gfx::DepthTarget<gfx::format::Depth> =
        gfx::preset::depth::LESS_EQUAL_TEST,
});

pub static EMITTER_VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    uniform mat4 u_Transform;
    uniform float u_Radius;
    in vec3 a_Pos;

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
    let normal = Vector3::new(1.0, 0.0, dzdx).cross(&Vector3::new(0.0, 1.0, dzdy)).normalize();

    return normal.into_fixed();
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

struct ViewPair<R: gfx::Resources, T> {
    resource: gfx::handle::ShaderResourceView<R, T>,
    target: gfx::handle::RenderTargetView<R, T>,
}

fn create_g_buffer<R: gfx::Resources, F: Factory<R>>(
                   width: gfx::tex::Size, height: gfx::tex::Size, factory: &mut F)
                   -> (ViewPair<R, GFormat>, ViewPair<R, GFormat>, ViewPair<R, GFormat>,
                       gfx::handle::ShaderResourceView<R, Depth>, gfx::handle::DepthStencilView<R, Depth>)
{
    let pos = {
        let (_ , srv, rtv) = factory.create_render_target(width, height, false).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let normal = {
        let (_ , srv, rtv) = factory.create_render_target(width, height, false).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let diffuse = {
        let (_ , srv, rtv) = factory.create_render_target(width, height, false).unwrap();
        ViewPair{ resource: srv, target: rtv }
    };
    let (_, depth_srv, depth_rtv) = factory.create_depth_stencil(width, height).unwrap();

    (pos, normal, diffuse, depth_srv, depth_rtv)
}

pub fn main() {
    env_logger::init().unwrap();
    let (window, mut device, mut factory, main_color, _) =
        gfx_window_glutin::init::<Rgba8>(glutin::WindowBuilder::new()
            .with_title("Deferred rendering example with gfx-rs".to_string())
            .with_dimensions(800, 600)
            .with_gl(glutin::GL_CORE)
    );

    let (w, h) = {
        let (w, h) = window.get_inner_size().unwrap();
        (w as gfx::tex::Size, h as gfx::tex::Size)
    };
    let (gpos, gnormal, gdiffuse, depth_resource, depth_target) =
        create_g_buffer(w, h, &mut factory);
    let res = {
        let (_ , srv, rtv) = factory.create_render_target(w, h, false).unwrap();
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

    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(60.0f32), aspect, 5.0, 100.0);

    let terrain_scale = Vector3::new(25.0, 25.0, 25.0);
    let (terrain_pso, mut terrain_data, terrain_slice) = {
        let plane = genmesh::generators::Plane::subdivide(256, 256);
        let vertex_data: Vec<TerrainVertex> = plane.shared_vertex_iter()
            .map(|(x, y)| {
                let h = terrain_scale.z * perlin2(&seed, &[x, y]);
                TerrainVertex {
                    pos: [terrain_scale.x * x, terrain_scale.y * y, h],
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

        let pso = factory.create_pipeline_simple(
            TERRAIN_VERTEX_SRC, TERRAIN_FRAGMENT_SRC,
            gfx::state::CullFace::Back, terrain::new()
            ).unwrap();

        let data = terrain::Data {
            vbuf: vbuf,
            model: Matrix4::identity().into_fixed(),
            view: Matrix4::identity().into_fixed(),
            proj: proj.into_fixed(),
            cam_pos: Vector3::new(0.0, 0.0, 0.0).into_fixed(),
            out_position: gpos.target.clone(),
            out_normal: gnormal.target.clone(),
            out_color: gdiffuse.target.clone(),
            out_depth: depth_target.clone(),
        };

        (pso, data, slice)
    };

    let (blit_pso, mut blit_data, blit_slice) = {
        let vertex_data = [
            BlitVertex { pos: I8Scaled::cast3([-1, -1, 0]), tex_coord: I8Scaled::cast2([0, 0]) },
            BlitVertex { pos: I8Scaled::cast3([ 1, -1, 0]), tex_coord: I8Scaled::cast2([1, 0]) },
            BlitVertex { pos: I8Scaled::cast3([ 1,  1, 0]), tex_coord: I8Scaled::cast2([1, 1]) },
            BlitVertex { pos: I8Scaled::cast3([-1, -1, 0]), tex_coord: I8Scaled::cast2([0, 0]) },
            BlitVertex { pos: I8Scaled::cast3([ 1,  1, 0]), tex_coord: I8Scaled::cast2([1, 1]) },
            BlitVertex { pos: I8Scaled::cast3([-1,  1, 0]), tex_coord: I8Scaled::cast2([0, 1]) },
        ];

        let (vbuf, slice) = factory.create_vertex_buffer(&vertex_data);

        let pso = factory.create_pipeline_simple(
            BLIT_VERTEX_SRC, BLIT_FRAGMENT_SRC,
            gfx::state::CullFace::Nothing, blit::new()
            ).unwrap();

        let data = blit::Data {
            vbuf: vbuf,
            tex: (gpos.resource.clone(), sampler.clone()),
            out: main_color.clone(),
        };

        (pso, data, slice)
    };

    let light_pos_buffer = factory.create_constant_buffer(NUM_LIGHTS);

    let (light_vbuf, mut light_slice) = {
        let vertex_data = [
            // top (0, 0, 1)
            CubeVertex { pos: I8Scaled::cast3([-1, -1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1, -1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1,  1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([-1,  1,  1]) },
            // bottom (0, 0, -1)
            CubeVertex { pos: I8Scaled::cast3([-1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1, -1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([-1, -1, -1]) },
            // right (1, 0, 0)
            CubeVertex { pos: I8Scaled::cast3([ 1, -1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1,  1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1, -1,  1]) },
            // left (-1, 0, 0)
            CubeVertex { pos: I8Scaled::cast3([-1, -1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([-1,  1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([-1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([-1, -1, -1]) },
            // front (0, 1, 0)
            CubeVertex { pos: I8Scaled::cast3([ 1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([-1,  1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([-1,  1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1,  1,  1]) },
            // back (0, -1, 0)
            CubeVertex { pos: I8Scaled::cast3([ 1, -1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([-1, -1,  1]) },
            CubeVertex { pos: I8Scaled::cast3([-1, -1, -1]) },
            CubeVertex { pos: I8Scaled::cast3([ 1, -1, -1]) },
        ];

        let index_data: &[u8] = &[
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

    let (light_pso, mut light_data) = {
        let pso = factory.create_pipeline_simple(
            LIGHT_VERTEX_SRC, LIGHT_FRAGMENT_SRC,
            gfx::state::CullFace::Back, light::new()
            ).unwrap();

        let data = light::Data {
            vbuf: light_vbuf.clone(),
            transform: Matrix4::identity().into_fixed(),
            light_pos_buf: light_pos_buffer.clone(),
            radius: 3.0,
            cam_pos: Vector3::new(0.0, 0.0, 0.0).into_fixed(),
            frame_res: [w as f32, h as f32],
            tex_pos: (gpos.resource.clone(), sampler.clone()),
            tex_normal: (gnormal.resource.clone(), sampler.clone()),
            tex_diffuse: (gdiffuse.resource.clone(), sampler.clone()),
            out_color: res.target.clone(),
            out_depth: depth_target.clone(),
        };

        (pso, data)
    };

    let (emitter_pso, mut emitter_data) = {
        let pso = factory.create_pipeline_simple(
            EMITTER_VERTEX_SRC, EMITTER_FRAGMENT_SRC,
            gfx::state::CullFace::Back, emitter::new()
            ).unwrap();

        let data = emitter::Data {
            vbuf: light_vbuf.clone(),
            transform: Matrix4::identity().into_fixed(),
            light_pos_buf: light_pos_buffer.clone(),
            radius: 0.2,
            out_color: res.target.clone(),
            out_depth: depth_target.clone(),
        };

        (pso, data)
    };

    let mut debug_buf: Option<gfx::handle::ShaderResourceView<_, GFormat>> = None;

    let mut light_pos_vec: Vec<LightInfo> = (0 ..NUM_LIGHTS).map(|_| {
        LightInfo{ pos: [0.0, 0.0, 0.0, 0.0] }
    }).collect();

    let mut encoder = factory.create_encoder();

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            use glutin::{Event, VirtualKeyCode};
            match event {
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key1)) =>
                    debug_buf = Some(gpos.resource.clone()),
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key2)) =>
                    debug_buf = Some(gnormal.resource.clone()),
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key3)) =>
                    debug_buf = Some(gdiffuse.resource.clone()),
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key4)) => {
                    use gfx::core::factory::Phantom; //hack
                    debug_buf = Some(Phantom::new(depth_resource.raw().clone()))
                },
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Key0)) =>
                    debug_buf = None,
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Escape)) |
                Event::Closed => break 'main,
                _ => {},
            }
        }

        let time = precise_time_s() as f32;

        // Update camera position
        {
            let cam_pos = {
                // Slowly circle the center
                let x = (0.05*time).sin();
                let y = (0.05*time).cos();
                Point3::new(x * 32.0, y * 32.0, 16.0)
            };
            let view: AffineMatrix3<f32> = Transform::look_at(
                &cam_pos,
                &Point3::new(0.0, 0.0, 0.0),
                &Vector3::unit_z(),
            );
            terrain_data.view = view.mat.into_fixed();
            terrain_data.cam_pos = cam_pos.into_fixed();

            light_data.transform = proj.mul_m(&view.mat).into_fixed();
            light_data.cam_pos = cam_pos.into_fixed();

            emitter_data.transform = proj.mul_m(&view.mat).into_fixed();
        }

        // Update light positions
        for (i, d) in light_pos_vec.iter_mut().enumerate() {
            let (x, y) = {
                let fi = i as f32;
                // Distribute lights nicely
                let r = 1.0 - (fi*fi) / ((NUM_LIGHTS*NUM_LIGHTS) as f32);
                (r * (0.2*time + i as f32).cos(), r * (0.2*time + i as f32).sin())
            };
            let h = perlin2(&seed, &[x, y]);

            d.pos[0] = terrain_scale.x * x;
            d.pos[1] = terrain_scale.y * y;
            d.pos[2] = terrain_scale.z * h + 0.5;
        };
        factory.update_buffer(&light_pos_buffer, &light_pos_vec, 0)
               .unwrap();

        encoder.reset();
        encoder.clear_depth(&depth_target, 1.0);
        encoder.clear(&gpos.target, [0.0, 0.0, 0.0, 1.0]);
        encoder.clear(&gnormal.target, [0.0, 0.0, 0.0, 1.0]);
        encoder.clear(&gdiffuse.target, [0.0, 0.0, 0.0, 1.0]);
        // Render the terrain to the geometry buffer
        encoder.draw(&terrain_slice, &terrain_pso, &terrain_data);

        let blit_tex = match debug_buf {
            Some(ref tex) => tex,   // Show one of the immediate buffers
            None => {
                encoder.clear(&res.target, [0.0, 0.0, 0.0, 1.0]);
                // Apply lights
                encoder.draw(&light_slice, &light_pso, &light_data);
                // Draw light emitters
                encoder.draw(&light_slice, &emitter_pso, &emitter_data);

                &res.resource
            }
        };
        blit_data.tex = (blit_tex.clone(), sampler.clone());
        // Show the result
        encoder.clear(&main_color, [0.0, 0.0, 0.0, 1.0]);
        encoder.draw(&blit_slice, &blit_pso, &blit_data);

        device.submit(encoder.as_buffer());
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
