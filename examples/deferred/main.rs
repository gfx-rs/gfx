// Copyright 2014 The Gfx-rs Developers.
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

#![feature(core, plugin)]
#![plugin(gfx_macros)]
#![feature(custom_attribute)]

extern crate cgmath;
extern crate gfx;
extern crate glfw;
extern crate time;
extern crate rand;
extern crate genmesh;
extern crate noise;

use rand::Rng;
use std::num::Float;
use cgmath::FixedArray;
use cgmath::{Matrix, Matrix4, Point3, Vector3, EuclideanVector};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceExt, TextureHandle, Plane, ToSlice, RawBufferHandle};
use gfx::batch::RefBatch;
use glfw::Context;
use genmesh::{Vertices, Triangulate};
use genmesh::generators::{SharedVertex, IndexedPolygon};
use time::precise_time_s;

use noise::{Seed, perlin2};

// Remember to also change the constants in the shaders
const NUM_LIGHTS: usize = 250;

#[vertex_format]
#[derive(Copy)]
struct TerrainVertex {
    #[name = "a_Pos"]
    pos: [f32; 3],
    #[name = "a_Normal"]
    normal: [f32; 3],
    #[name = "a_Color"]
    color: [f32; 3],
}

#[vertex_format]
#[derive(Copy)]
struct BlitVertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8; 3],
    #[as_float]
    #[name = "a_TexCoord"]
    tex_coord: [u8; 2],
}

#[vertex_format]
#[derive(Copy)]
struct CubeVertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8; 3],
}

#[shader_param]
struct TerrainParams {
    #[name = "u_Model"]
    model: [[f32; 4]; 4],
    #[name = "u_View"]
    view: [[f32; 4]; 4],
    #[name = "u_Proj"]
    proj: [[f32; 4]; 4],
    #[name = "u_CameraPos"]
    cam_pos: [f32; 3],
}

#[shader_param]
struct LightParams {
    #[name = "u_Transform"]
    transform: [[f32; 4]; 4],
    #[name = "u_LightPosBlock"]
    light_pos_buf: gfx::RawBufferHandle<gfx::GlResources>,
    #[name = "u_Radius"]
    radius: f32,
    #[name = "u_CameraPos"]
    cam_pos: [f32; 3],
    #[name = "u_FrameRes"]
    frame_res: [f32; 2],
    #[name = "u_TexPos"]
    tex_pos: gfx::shade::TextureParam,
    #[name = "u_TexNormal"]
    tex_normal: gfx::shade::TextureParam,
    #[name = "u_TexDiffuse"]
    tex_diffuse: gfx::shade::TextureParam,
}

#[shader_param]
struct EmitterParams {
    #[name = "u_Transform"]
    transform: [[f32; 4]; 4],
    #[name = "u_LightPosBlock"]
    light_pos_buf: gfx::RawBufferHandle<gfx::GlResources>,
    #[name = "u_Radius"]
    radius: f32,
}

#[shader_param]
struct BlitParams {
    #[name = "u_Tex"]
    tex: gfx::shade::TextureParam,
}

static TERRAIN_VERTEX_SRC: &'static [u8] = b"
    #version 120

    uniform mat4 u_Model;
    uniform mat4 u_View;
    uniform mat4 u_Proj;
    attribute vec3 a_Pos;
    attribute vec3 a_Normal;
    attribute vec3 a_Color;
    varying vec3 v_FragPos;
    varying vec3 v_Normal;
    varying vec3 v_Color;

    void main() {
        v_FragPos = (u_Model * vec4(a_Pos, 1.0)).xyz;
        v_Normal = a_Normal;
        v_Color = a_Color;
        gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
    }
";

static TERRAIN_FRAGMENT_SRC: &'static [u8] = b"
    #version 130

    varying vec3 v_FragPos;
    varying vec3 v_Normal;
    varying vec3 v_Color;

    void main() {
        vec3 n = normalize(v_Normal);

        gl_FragData[0] = vec4(v_FragPos, 0.0);
        gl_FragData[1] = vec4(n, 0.0);
        gl_FragData[2] = vec4(v_Color, 1.0);
    }
";

static BLIT_VERTEX_SRC: &'static [u8] = b"
    #version 120

    attribute vec3 a_Pos;
    attribute vec2 a_TexCoord;
    varying vec2 v_TexCoord;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = vec4(a_Pos, 1.0);
    }
";

static BLIT_FRAGMENT_SRC: &'static [u8] = b"
    #version 120

    uniform sampler2D u_Tex;
    varying vec2 v_TexCoord;

    void main() {
        vec4 tex = texture2D(u_Tex, v_TexCoord);
        gl_FragColor = tex;
    }
";

static LIGHT_VERTEX_SRC: &'static [u8] = b"
    #version 140
    #extension GL_EXT_draw_instanced : enable
    #extension GL_ARB_uniform_buffer_object : enable

    uniform mat4 u_Transform;
    uniform float u_Radius;
    attribute vec3 a_Pos;
    varying vec3 v_LightPos;

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

static LIGHT_FRAGMENT_SRC: &'static [u8] = b"
    #version 120

    uniform float u_Radius;
    uniform vec3 u_CameraPos;
    uniform vec2 u_FrameRes;
    uniform sampler2D u_TexPos;
    uniform sampler2D u_TexNormal;
    uniform sampler2D u_TexDiffuse;
    varying vec3 v_LightPos;

    void main() {
        vec2 texCoord = gl_FragCoord.xy / u_FrameRes;
        vec3 pos     = texture2D(u_TexPos,     texCoord).xyz;
        vec3 normal  = texture2D(u_TexNormal,  texCoord).xyz;
        vec3 diffuse = texture2D(u_TexDiffuse, texCoord).xyz;

        vec3 light    = v_LightPos;
        vec3 to_light = normalize(light - pos);
        vec3 to_cam   = normalize(u_CameraPos - pos);

        vec3 n = normalize(normal);
        float s = pow(max(0.0, dot(to_cam, reflect(-to_light, n))), 20.0);
        float d = max(0.0, dot(n, to_light));

        float dist_sq = dot(light - pos, light - pos);
        float scale = max(0.0, 1.0-dist_sq/(u_Radius*u_Radius));

        vec3 res_color = d*vec3(diffuse) + vec3(s);

        gl_FragColor = vec4(scale*res_color, 1.0);
    }
";

static EMITTER_VERTEX_SRC: &'static [u8] = b"
    #version 140
    #extension GL_EXT_draw_instanced : enable
    #extension GL_ARB_uniform_buffer_object : enable

    uniform mat4 u_Transform;
    uniform float u_Radius;
    attribute vec3 a_Pos;

    const int NUM_LIGHTS = 250;
    layout(std140)
    uniform u_LightPosBlock {
        vec4 offs[NUM_LIGHTS];
    };

    void main() {
        gl_Position = u_Transform * vec4(u_Radius * a_Pos + offs[gl_InstanceID].xyz, 1.0);
    }
";

static EMITTER_FRAGMENT_SRC: &'static [u8] = b"
    #version 120

    void main() {
        gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
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

fn create_g_buffer(width: u16, height: u16, device: &mut gfx::GlDevice)
        -> (gfx::Frame, TextureHandle<gfx::GlResources>, TextureHandle<gfx::GlResources>,
            TextureHandle<gfx::GlResources>, TextureHandle<gfx::GlResources>) {
    let mut frame = gfx::Frame::new(width, height);

    let texture_info_float = gfx::tex::TextureInfo {
        width: width,
        height: height,
        depth: 1,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2D,
        format: gfx::tex::Format::Float(gfx::tex::Components::RGBA, gfx::attrib::FloatSize::F32),
    };
    let texture_info_depth = gfx::tex::TextureInfo {
        width: width,
        height: height,
        depth: 1,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2D,
        format: gfx::tex::Format::DEPTH24STENCIL8,
    };
    let texture_pos     = device.create_texture(texture_info_float)
                                .ok().expect("failed to create texture.");
    let texture_normal  = device.create_texture(texture_info_float)
                                .ok().expect("failed to create texture.");
    let texture_diffuse = device.create_texture(texture_info_float)
                                .ok().expect("failed to create texture.");
    let texture_depth   = device.create_texture(texture_info_depth)
                                .ok().expect("failed to create texture.");

    frame.colors.push(Plane::Texture(texture_pos,     0, None));
    frame.colors.push(Plane::Texture(texture_normal,  0, None));
    frame.colors.push(Plane::Texture(texture_diffuse, 0, None));
    frame.depth = Some(Plane::Texture(texture_depth, 0, None));

    (frame, texture_pos, texture_normal, texture_diffuse, texture_depth)
}

fn create_res_buffer(width: u16, height: u16, device: &mut gfx::GlDevice, texture_depth: TextureHandle<gfx::GlResources>)
        -> (gfx::Frame, TextureHandle<gfx::GlResources>, TextureHandle<gfx::GlResources>) {
    let mut frame = gfx::Frame::new(width, height);

    let texture_info_float = gfx::tex::TextureInfo {
        width: width,
        height: height,
        depth: 1,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2D,
        format: gfx::tex::Format::Float(gfx::tex::Components::RGBA, gfx::attrib::FloatSize::F32),
    };

    let texture_frame = device.create_texture(texture_info_float)
                              .ok().expect("failed to create texture.");
;

    frame.colors.push(Plane::Texture(texture_frame, 0, None));
    frame.depth = Some(Plane::Texture(texture_depth, 0, None));

    (frame, texture_frame, texture_depth)
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
                        .ok().expect("Failed to initialize glfw.");

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenglForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenglProfile(glfw::OpenGlProfileHint::Core));

    let (mut window, events) = glfw
        .create_window(800, 600, "", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| window.get_proc_address(s));
    let mut renderer = device.create_renderer();
    let mut context = gfx::batch::Context::new();

    let (g_buffer, texture_pos, texture_normal, texture_diffuse, texture_depth)  = create_g_buffer(w as u16, h as u16, &mut device);
    let (res_buffer, texture_frame, _)  = create_res_buffer(w as u16, h as u16, &mut device, texture_depth);

    let seed = {
        let rand_seed = rand::thread_rng().gen();
        Seed::new(rand_seed)
    };

    let terrain_scale = Vector3::new(25.0, 25.0, 25.0);
    let terrain_batch: RefBatch<TerrainParams> = {
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

        let mesh = device.create_mesh(vertex_data.as_slice());

        let slice = device
            .create_buffer_static::<u32>(index_data.as_slice())
            .to_slice(gfx::PrimitiveType::TriangleList);

        let program = device.link_program(TERRAIN_VERTEX_SRC, TERRAIN_FRAGMENT_SRC)
                            .ok().expect("Failed to link program");
        let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

        context.make_batch(&program, &mesh, slice, &state)
               .ok().expect("Failed to match back")
    };

    let blit_batch: RefBatch<BlitParams> = {
        let vertex_data = [
            BlitVertex { pos: [-1, -1, 0], tex_coord: [0, 0] },
            BlitVertex { pos: [ 1, -1, 0], tex_coord: [1, 0] },
            BlitVertex { pos: [ 1,  1, 0], tex_coord: [1, 1] },
            BlitVertex { pos: [-1, -1, 0], tex_coord: [0, 0] },
            BlitVertex { pos: [ 1,  1, 0], tex_coord: [1, 1] },
            BlitVertex { pos: [-1,  1, 0], tex_coord: [0, 1] },
        ];
        let mesh = device.create_mesh(&vertex_data);
        let slice = mesh.to_slice(gfx::PrimitiveType::TriangleList);

        let program = device.link_program(BLIT_VERTEX_SRC, BLIT_FRAGMENT_SRC)
                            .ok().expect("Failed to link program");
        let state = gfx::DrawState::new();

        context.make_batch(&program, &mesh, slice, &state)
               .ok().expect("Failed to create batch")
    };

    let (light_batch, emitter_batch) = {
        let vertex_data = [
            // top (0, 0, 1)
            CubeVertex { pos: [-1, -1,  1] },
            CubeVertex { pos: [ 1, -1,  1] },
            CubeVertex { pos: [ 1,  1,  1] },
            CubeVertex { pos: [-1,  1,  1] },
            // bottom (0, 0, -1)
            CubeVertex { pos: [-1,  1, -1] },
            CubeVertex { pos: [ 1,  1, -1] },
            CubeVertex { pos: [ 1, -1, -1] },
            CubeVertex { pos: [-1, -1, -1] },
            // right (1, 0, 0)
            CubeVertex { pos: [ 1, -1, -1] },
            CubeVertex { pos: [ 1,  1, -1] },
            CubeVertex { pos: [ 1,  1,  1] },
            CubeVertex { pos: [ 1, -1,  1] },
            // left (-1, 0, 0)
            CubeVertex { pos: [-1, -1,  1] },
            CubeVertex { pos: [-1,  1,  1] },
            CubeVertex { pos: [-1,  1, -1] },
            CubeVertex { pos: [-1, -1, -1] },
            // front (0, 1, 0)
            CubeVertex { pos: [ 1,  1, -1] },
            CubeVertex { pos: [-1,  1, -1] },
            CubeVertex { pos: [-1,  1,  1] },
            CubeVertex { pos: [ 1,  1,  1] },
            // back (0, -1, 0)
            CubeVertex { pos: [ 1, -1,  1] },
            CubeVertex { pos: [-1, -1,  1] },
            CubeVertex { pos: [-1, -1, -1] },
            CubeVertex { pos: [ 1, -1, -1] },
        ];

        let index_data: &[u8] = &[
             0,  1,  2,  2,  3,  0, // top
             4,  5,  6,  6,  7,  4, // bottom
             8,  9, 10, 10, 11,  8, // right
            12, 13, 14, 14, 15, 12, // left
            16, 17, 18, 18, 19, 16, // front
            20, 21, 22, 22, 23, 20, // back
        ];

        let mesh = device.create_mesh(&vertex_data);
        let slice = device
            .create_buffer_static::<u8>(index_data)
            .to_slice(gfx::PrimitiveType::TriangleList);

        let state = gfx::DrawState::new()
            .depth(gfx::state::Comparison::LessEqual, false)
            .blend(gfx::BlendPreset::Additive);

        let light_batch: RefBatch<LightParams> = {
            let program = device.link_program(LIGHT_VERTEX_SRC, LIGHT_FRAGMENT_SRC)
                                .ok().expect("Failed to link program.");

            context.make_batch(&program, &mesh, slice, &state)
                   .ok().expect("Failed to create batch")
        };

        let emitter_batch: RefBatch<EmitterParams> = {
            let program = device.link_program(EMITTER_VERTEX_SRC, EMITTER_FRAGMENT_SRC)
                                .ok().expect("Failed to link program.");

            context.make_batch(&program, &mesh, slice, &state)
                   .ok().expect("Failed to create batch")
        };

        (light_batch, emitter_batch)
    };

    let clear_data = gfx::ClearData {
        color: [0.0, 0.0, 0.0, 1.0],
        depth: 1.0,
        stencil: 0,
    };

    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(60.0f32), aspect, 5.0, 100.0);

    let mut terrain_data = TerrainParams {
        model: Matrix4::identity().into_fixed(),
        view: Matrix4::identity().into_fixed(),
        proj: proj.into_fixed(),
        cam_pos: Vector3::new(0.0, 0.0, 0.0).into_fixed(),
    };

    let sampler = device.create_sampler(
        gfx::tex::SamplerInfo::new(gfx::tex::FilterMethod::Scale,
                                   gfx::tex::WrapMode::Clamp)
    );


    let light_pos_buffer = device.create_buffer::<[f32; 4]>(NUM_LIGHTS, gfx::BufferUsage::Stream);

    let mut light_data = LightParams {
        transform: Matrix4::identity().into_fixed(),
        light_pos_buf: light_pos_buffer.raw(),
        radius: 3.0,
        cam_pos: Vector3::new(0.0, 0.0, 0.0).into_fixed(),
        frame_res: [w as f32, h as f32],
        tex_pos: (texture_pos, Some(sampler)),
        tex_normal: (texture_normal, Some(sampler)),
        tex_diffuse: (texture_diffuse, Some(sampler)),
    };


    let mut emitter_data = EmitterParams {
        transform: Matrix4::identity().into_fixed(),
        light_pos_buf: light_pos_buffer.raw(),
        radius: 0.2,
    };

    let mut blit_data = BlitParams {
        tex: (texture_pos, Some(sampler)),
    };

    let mut debug_buf: Option<TextureHandle<gfx::GlResources>> = None;

    let mut light_pos_vec: Vec<[f32; 4]> = (0 ..NUM_LIGHTS).map(|_| {
        [0.0, 0.0, 0.0, 0.0]
    }).collect();

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(glfw::Key::Escape, _, glfw::Action::Press, _) =>
                    window.set_should_close(true),
                glfw::WindowEvent::Key(glfw::Key::Num1, _, glfw::Action::Press, _) =>
                    debug_buf = Some(texture_pos),
                glfw::WindowEvent::Key(glfw::Key::Num2, _, glfw::Action::Press, _) =>
                    debug_buf = Some(texture_normal),
                glfw::WindowEvent::Key(glfw::Key::Num3, _, glfw::Action::Press, _) =>
                    debug_buf = Some(texture_diffuse),
                glfw::WindowEvent::Key(glfw::Key::Num4, _, glfw::Action::Press, _) =>
                    debug_buf = Some(texture_depth),
                glfw::WindowEvent::Key(glfw::Key::Num0, _, glfw::Action::Press, _) =>
                    debug_buf = None,
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
        for (i, p) in light_pos_vec.iter_mut().enumerate() {
            let (x, y) = {
                let fi = i as f32;
                // Distribute lights nicely
                let r = 1.0 - (fi*fi) / ((NUM_LIGHTS*NUM_LIGHTS) as f32);
                (r * (0.2*time + i as f32).cos(), r * (0.2*time + i as f32).sin())
            };
            let h = perlin2(&seed, &[x, y]);

            p[0] = terrain_scale.x * x;
            p[1] = terrain_scale.y * y;
            p[2] = terrain_scale.z * h + 0.5;
        };
        device.update_buffer(light_pos_buffer, light_pos_vec.as_slice(), 0);

        // Render the terrain to the geometry buffer
        renderer.clear(clear_data, gfx::COLOR|gfx::DEPTH, &g_buffer);
        renderer.draw(
            &(&terrain_batch, &terrain_data, &context),
            &g_buffer)
            .unwrap();

        match debug_buf {
            Some(tex) => {
                // Show one of the immediate buffers
                blit_data.tex = (tex, Some(sampler));
                renderer.clear(clear_data, gfx::COLOR | gfx::DEPTH, &frame);
                renderer.draw(
                    &(&blit_batch, &blit_data, &context),
                    &frame)
                    .unwrap();
            },
            None => {
                renderer.clear(clear_data, gfx::COLOR, &res_buffer);

                // Apply light
                renderer.draw_instanced(
                    &(&light_batch, &light_data, &context),
                    NUM_LIGHTS as u32, 0, &res_buffer)
                    .unwrap();
                // Draw light emitters
                renderer.draw_instanced(
                    &(&emitter_batch, &emitter_data, &context),
                    NUM_LIGHTS as u32, 0, &res_buffer)
                    .unwrap();

                // Show the result
                renderer.clear(clear_data, gfx::COLOR | gfx::DEPTH, &frame);
                blit_data.tex = (texture_frame, Some(sampler));
                renderer.draw(
                    &(&blit_batch, &blit_data, &context),
                    &frame)
                    .unwrap();
            }
        }
        device.submit(renderer.as_buffer());
        renderer.reset();

        window.swap_buffers();
    }
}
