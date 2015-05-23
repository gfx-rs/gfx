// Copyright 2015 The GFX developers.
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

extern crate cgmath;
extern crate genmesh;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::attrib::Floater;
use gfx::traits::*;

gfx_vertex!( Vertex {
    a_Pos@ pos: [Floater<i8>; 3],
    a_Normal@ normal: [Floater<i8>; 3],
});

impl Vertex {
    fn new(p: [i8; 3], n: [i8; 3]) -> Vertex {
        Vertex {
            pos: Floater::cast3(p),
            normal: Floater::cast3(n),
        }
    }
}

const MAX_LIGHTS: usize = 10;

#[derive(Clone, Copy, Debug)]
struct LightParam {
    pos: [f32; 4],
    color: [f32; 4],
    proj: [[f32; 4]; 4],
}

gfx_parameters!( ForwardParams {
    u_Transform@ transform: [[f32; 4]; 4],
    u_ModelTransform@ model_transform: [[f32; 4]; 4],
    u_NormalTransform@ normal_transform: [[f32; 3]; 3],
    u_Color@ color: [f32; 4],
    u_NumLights@ num_lights: i32,
    b_Lights@ light_buf: gfx::handle::RawBuffer<R>,
    t_Shadow@ shadow: gfx::shade::TextureParam<R>,
});

gfx_parameters!( ShadowParams {
    u_Transform@ transform: [[f32; 4]; 4],
});

//----------------------------------------

fn create_cube<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
               -> (gfx::Mesh<R>, gfx::Slice<R>)
{
    let vertex_data = [
        // top (0, 0, 1)
        Vertex::new([-1, -1,  1], [0, 0, 1]),
        Vertex::new([ 1, -1,  1], [0, 0, 1]),
        Vertex::new([ 1,  1,  1], [0, 0, 1]),
        Vertex::new([-1,  1,  1], [0, 0, 1]),
        // bottom (0, 0, -1)
        Vertex::new([-1,  1, -1], [0, 0, -1]),
        Vertex::new([ 1,  1, -1], [0, 0, -1]),
        Vertex::new([ 1, -1, -1], [0, 0, -1]),
        Vertex::new([-1, -1, -1], [0, 0, -1]),
        // right (1, 0, 0)
        Vertex::new([ 1, -1, -1], [1, 0, 0]),
        Vertex::new([ 1,  1, -1], [1, 0, 0]),
        Vertex::new([ 1,  1,  1], [1, 0, 0]),
        Vertex::new([ 1, -1,  1], [1, 0, 0]),
        // left (-1, 0, 0)
        Vertex::new([-1, -1,  1], [-1, 0, 0]),
        Vertex::new([-1,  1,  1], [-1, 0, 0]),
        Vertex::new([-1,  1, -1], [-1, 0, 0]),
        Vertex::new([-1, -1, -1], [-1, 0, 0]),
        // front (0, 1, 0)
        Vertex::new([ 1,  1, -1], [0, 1, 0]),
        Vertex::new([-1,  1, -1], [0, 1, 0]),
        Vertex::new([-1,  1,  1], [0, 1, 0]),
        Vertex::new([ 1,  1,  1], [0, 1, 0]),
        // back (0, -1, 0)
        Vertex::new([ 1, -1,  1], [0, -1, 0]),
        Vertex::new([-1, -1,  1], [0, -1, 0]),
        Vertex::new([-1, -1, -1], [0, -1, 0]),
        Vertex::new([ 1, -1, -1], [0, -1, 0]),
    ];

    let mesh = factory.create_mesh(&vertex_data);

    let index_data: &[u8] = &[
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    let slice = index_data.to_slice(factory, gfx::PrimitiveType::TriangleList);

    (mesh, slice)
}

fn create_plane<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F, size: i8)
                -> (gfx::Mesh<R>, gfx::Slice<R>)
{
    let vertex_data = [
        Vertex::new([ size, -size,  0], [0, 0, 1]),
        Vertex::new([ size,  size,  0], [0, 0, 1]),
        Vertex::new([-size, -size,  0], [0, 0, 1]),
        Vertex::new([-size,  size,  0], [0, 0, 1]),
    ];

    let mesh = factory.create_mesh(&vertex_data);
    let slice = mesh.to_slice(gfx::PrimitiveType::TriangleStrip);

    (mesh, slice)
}

//----------------------------------------

struct Camera {
    mx_view: cgmath::Matrix4<f32>,
    projection: cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>,
}

struct Light<S> {
    position: cgmath::Point3<f32>,
    mx_view: cgmath::Matrix4<f32>,
    projection: cgmath::Perspective<f32>,
    color: gfx::ColorValue,
    stream: S,
}

struct Entity<R: gfx::Resources> {
    dynamic: bool,
    mx_to_world: cgmath::Matrix4<f32>,
    batch_shadow: gfx::batch::OwnedBatch<ShadowParams<R>>,
    batch_forward: gfx::batch::OwnedBatch<ForwardParams<R>>,
}

struct Scene<R: gfx::Resources, S> {
    camera: Camera,
    lights: Vec<Light<S>>,
    entities: Vec<Entity<R>>,
    light_buf: gfx::handle::Buffer<R, LightParam>,
}

//----------------------------------------

fn make_entity<R: gfx::Resources>(dynamic: bool, mesh: &gfx::Mesh<R>, slice: &gfx::Slice<R>,
               prog_fw: &gfx::handle::Program<R>, prog_sh: &gfx::handle::Program<R>,
               num_lights: usize, light_buf: &gfx::handle::Buffer<R, LightParam>,
               shadow: &gfx::shade::TextureParam<R>, transform: cgmath::Matrix4<f32>)
               -> Entity<R>
{
    use cgmath::FixedArray;
    Entity {
        dynamic: dynamic,
        mx_to_world: transform,
        batch_forward: {
            let data = ForwardParams {
                transform: cgmath::Matrix4::identity().into_fixed(),
                model_transform: cgmath::Matrix4::identity().into_fixed(),
                normal_transform: cgmath::Matrix3::identity().into_fixed(),
                color: [1.0, 1.0, 1.0, 1.0],
                num_lights: num_lights as i32,
                light_buf: light_buf.raw().clone(),
                shadow: shadow.clone(),
                _r: std::marker::PhantomData,
            };
            let mut batch = gfx::batch::OwnedBatch::new(
                mesh.clone(), prog_fw.clone(), data).unwrap();
            batch.slice = slice.clone();
            batch.state = batch.state.depth(gfx::state::Comparison::LessEqual, true);
            batch
        },
        batch_shadow: {
            let data = ShadowParams {
                transform: cgmath::Matrix4::identity().into_fixed(),
                _r: std::marker::PhantomData,
            };
            let mut batch = gfx::batch::OwnedBatch::new(
                mesh.clone(), prog_sh.clone(), data).unwrap();
            batch.slice = slice.clone();
            batch.state = batch.state.depth(gfx::state::Comparison::LessEqual, true);
            batch.state.primitive.offset = Some(gfx::state::Offset(2.0, 2));
            batch
        },
    }
}

fn create_scene<D, F>(_: &D, factory: &mut F)
                -> Scene<D::Resources, gfx::OwnedStream<D, gfx::Plane<D::Resources>>> where
    D: gfx::Device,
    F: gfx::Factory<D::Resources> + gfx::traits::StreamFactory<D>,
{
    let program_forward = factory.link_program(
        include_bytes!("shader/forward_150.glslv"),
        include_bytes!("shader/forward_150.glslf"),
    ).unwrap();
    let program_shadow = factory.link_program(
        include_bytes!("shader/shadow_150.glslv"),
        include_bytes!("shader/shadow_150.glslf"),
    ).unwrap();

    let shadow_array = factory.create_texture(gfx::tex::TextureInfo {
        width: 512,
        height: 512,
        depth: MAX_LIGHTS as gfx::tex::Size,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2DArray,
        format: gfx::tex::Format::DEPTH24,
    }).unwrap();

    let (near, far) = (1f32, 20f32);

    let light_buf = factory.create_buffer_dynamic::<LightParam>(
        MAX_LIGHTS, gfx::BufferRole::Uniform);

    struct LightDesc {
        pos: cgmath::Point3<f32>,
        color: gfx::ColorValue,
        fov: f32,
    }

    let light_descs = vec![
        LightDesc {
            pos: cgmath::Point3::new(7.0, -5.0, 10.0),
            color: [0.5, 1.0, 0.5, 1.0],
            fov: 60.0,
        },
        LightDesc {
            pos: cgmath::Point3::new(-5.0, 7.0, 10.0),
            color: [1.0, 0.5, 0.5, 1.0],
            fov: 45.0,
        },
    ];

    let lights: Vec<_> = light_descs.iter().enumerate().map(|(i, desc)| Light {
        position: desc.pos.clone(),
        mx_view: cgmath::Matrix4::look_at(
            &desc.pos,
            &cgmath::Point3::new(0.0, 0.0, 0.0),
            &cgmath::Vector3::unit_z(),
        ),
        projection: cgmath::PerspectiveFov {
            fovy: cgmath::deg(desc.fov),
            aspect: 1.0,
            near: near,
            far: far,
        }.to_perspective(),
        color: desc.color.clone(),
        stream: factory.create_stream(
            gfx::Plane::Texture(
                shadow_array.clone(),
                0,
                Some(i as gfx::Layer)
            ),
        ),
    }).collect();

    let light_params: Vec<_> = lights.iter().map(|light| LightParam {
        pos: [light.position.x, light.position.y, light.position.z, 1.0],
        color: light.color,
        proj: {
            use cgmath::{FixedArray, Matrix, Matrix4};
            let mx_proj: Matrix4<_> = light.projection.into();
            mx_proj.mul_m(&light.mx_view).into_fixed()
        },
    }).collect();
    factory.update_buffer(&light_buf, &light_params, 0).unwrap();

    let shadow_param = {
        let mut sinfo = gfx::tex::SamplerInfo::new(
            gfx::tex::FilterMethod::Bilinear,
            gfx::tex::WrapMode::Clamp
        );
        sinfo.comparison = gfx::tex::ComparisonMode::CompareRefToTexture(
            gfx::state::Comparison::LessEqual
        );
        let sampler = factory.create_sampler(sinfo);
        (shadow_array.clone(), Some(sampler))
    };

    struct CubeDesc {
        offset: cgmath::Vector3<f32>,
        angle: f32,
        scale: f32,
    }

    let cube_descs = vec![
        CubeDesc {
            offset: cgmath::vec3(-2.0, -2.0, 2.0),
            angle: 10.0,
            scale: 0.7,
        },
        CubeDesc {
            offset: cgmath::vec3(2.0, -2.0, 2.0),
            angle: 50.0,
            scale: 1.3,
        },
        CubeDesc {
            offset: cgmath::vec3(-2.0, 2.0, 2.0),
            angle: 140.0,
            scale: 1.1,
        },
        CubeDesc {
            offset: cgmath::vec3(2.0, 2.0, 2.0),
            angle: 210.0,
            scale: 0.9,
        },
    ];

    let mut entities: Vec<_> = cube_descs.iter().map(|desc| {
        use cgmath::{EuclideanVector, Rotation3};
        let (mesh, slice) = create_cube(factory);
        make_entity(true, &mesh, &slice,
            &program_forward, &program_shadow,
            lights.len(), &light_buf, &shadow_param,
            cgmath::Decomposed {
                disp: desc.offset.clone(),
                rot: cgmath::Quaternion::from_axis_angle(
                    &desc.offset.normalize(),
                    cgmath::deg(desc.angle).into(),
                ),
                scale: desc.scale,
            }.into(),
        )
    }).collect();
    entities.push({
        let (mesh, slice) = create_plane(factory, 7);
        make_entity(false, &mesh, &slice,
            &program_forward, &program_shadow,
            lights.len(), &light_buf, &shadow_param,
            cgmath::Matrix4::identity())
    });

    let camera = Camera {
        mx_view: cgmath::Matrix4::look_at(
            &cgmath::Point3::new(3.0f32, -10.0, 6.0),
            &cgmath::Point3::new(0f32, 0.0, 0.0),
            &cgmath::Vector3::unit_z(),
        ),
        projection: cgmath::PerspectiveFov {
            fovy: cgmath::deg(45.0f32),
            aspect: 1.0,
            near: near,
            far: far,
        },
    };

    Scene {
        camera: camera,
        lights: lights,
        entities: entities,
        light_buf: light_buf.clone(),
    }
}

//----------------------------------------

pub fn main() {
    use cgmath::{EuclideanVector, FixedArray, Matrix, Rotation3, Vector};

    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(
        glutin::WindowBuilder::new()
            .with_title("Multi-threaded shadow rendering example with gfx-rs".to_string())
            .with_dimensions(800, 600)
            .with_gl(glutin::GL_CORE)
            .with_depth_buffer(24)
            .build().unwrap()
    );
    let _ = stream.out.set_gamma(gfx::Gamma::Convert); // enable srgb

    let mut scene = create_scene(&device, &mut factory);
    let mut last_mouse: (i32, i32) = (0, 0);

    'main: loop {
        for event in stream.out.window.poll_events() {
            use glutin::{Event, VirtualKeyCode};
            match event {
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Escape)) => break 'main,
                Event::MouseMoved(cur) => if cur != last_mouse {
                    let axis = cgmath::vec3(
                        (cur.0 - last_mouse.0) as f32,
                        0.0,
                        (cur.1 - last_mouse.1) as f32,
                    );
                    let len = axis.length();
                    for ent in scene.entities.iter_mut() {
                        if !ent.dynamic {
                            continue
                        }
                        let rot = cgmath::Decomposed {
                            disp: cgmath::vec3(0.0, 0.0, 0.0),
                            rot: cgmath::Quaternion::from_axis_angle(
                                &axis.mul_s(1.0 / len),
                                cgmath::deg(len * 0.3).into(),
                            ),
                            scale: 1.0,
                        }.into();
                        ent.mx_to_world = ent.mx_to_world.mul_m(&rot);
                    }
                    last_mouse = cur;
                },
                Event::Closed => break 'main,
                _ => {},
            }
        }

        // fill up shadow map for each light
        for light in scene.lights.iter_mut() {
            // clear
            light.stream.clear(gfx::ClearData {
                color: [0.0; 4],
                depth: 1.0,
                stencil: 0,
            });
            // fill
            for ent in scene.entities.iter_mut() {
                let batch = &mut ent.batch_shadow; //TODO: clone
                batch.param.transform = {
                    let mx_proj: cgmath::Matrix4<_> = light.projection.into();
                    let mx_view = mx_proj.mul_m(&light.mx_view);
                    let mvp = mx_view.mul_m(&ent.mx_to_world);
                    mvp.into_fixed()
                };
                light.stream.draw(batch).unwrap();
            }
            // submit
            light.stream.flush(&mut device);
        }

        // draw entities with forward pass
        stream.clear(gfx::ClearData {
            color: [0.1, 0.2, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });

        let mx_vp = {
            let mut proj = scene.camera.projection;
            proj.aspect = stream.get_aspect_ratio();
            let mx_proj: cgmath::Matrix4<_> = proj.into();
            mx_proj.mul_m(&scene.camera.mx_view)
        };
        for ent in scene.entities.iter_mut() {
            let batch = &mut ent.batch_forward;
            batch.param.transform = mx_vp.mul_m(&ent.mx_to_world).into_fixed();
            batch.param.model_transform = ent.mx_to_world.into_fixed();
            batch.param.normal_transform = {
                let m = &ent.mx_to_world;
                [[m.x.x, m.x.y, m.x.z],
                [m.y.x, m.y.y, m.y.z],
                [m.z.x, m.z.y, m.z.z]]
            };
            stream.draw(batch).unwrap();
        }

        // done
        stream.present(&mut device);
    }
}
