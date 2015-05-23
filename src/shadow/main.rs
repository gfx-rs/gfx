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

gfx_parameters!( ForwardParams {
    u_Transform@ transform: [[f32; 4]; 4],
    u_NormalTransform@ normal_transform: [[f32; 3]; 3],
    u_Color@ color: [f32; 4],
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

fn create_plane<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
                -> (gfx::Mesh<R>, gfx::Slice<R>)
{
    let vertex_data = [
        Vertex::new([ 5, -5,  0], [0, 0, 1]),
        Vertex::new([ 5,  5,  0], [0, 0, 1]),
        Vertex::new([-5, -5,  0], [0, 0, 1]),
        Vertex::new([-5,  5,  0], [0, 0, 1]),
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
    mx_to_world: cgmath::Matrix4<f32>,
    batch_shadow: gfx::batch::OwnedBatch<ShadowParams<R>>,
    batch_forward: gfx::batch::OwnedBatch<ForwardParams<R>>,
}

struct Scene<R: gfx::Resources, S> {
    camera: Camera,
    lights: Vec<Light<S>>,
    entities: Vec<Entity<R>>,
}

//----------------------------------------

fn make_entity<R: gfx::Resources>(mesh: &gfx::Mesh<R>, slice: &gfx::Slice<R>,
               prog_fw: &gfx::handle::Program<R>, prog_sh: &gfx::handle::Program<R>,
               shadow: &gfx::shade::TextureParam<R>, transform: cgmath::Matrix4<f32>)
               -> Entity<R>
{
    use cgmath::FixedArray;
    Entity {
        mx_to_world: transform,
        batch_forward: {
            let data = ForwardParams {
                transform: cgmath::Matrix4::identity().into_fixed(),
                normal_transform: cgmath::Matrix3::identity().into_fixed(),
                color: [1.0, 1.0, 1.0, 1.0],
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
        depth: 5,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2DArray,
        format: gfx::tex::Format::DEPTH24,
    }).unwrap();

    let shadow_param = {
        let mut sinfo = gfx::tex::SamplerInfo::new(
            gfx::tex::FilterMethod::Bilinear,
            gfx::tex::WrapMode::Clamp
        );
        sinfo.comparison = gfx::tex::ComparisonMode::CompareRefToTexture(
            gfx::state::Comparison::Less
        );
        let sampler = factory.create_sampler(sinfo);
        (shadow_array.clone(), Some(sampler))
    };

    let (near, far) = (1f32, 20f32);

    struct EntityDesc {
        offset: cgmath::Vector3<f32>,
        angle: f32,
        scale: f32,
    }

    let cube_descs = vec![
        EntityDesc {
            offset: cgmath::vec3(-2.0, -2.0, 2.0),
            angle: 10.0,
            scale: 0.6,
        },
        EntityDesc {
            offset: cgmath::vec3(2.0, -2.0, 2.0),
            angle: 50.0,
            scale: 1.6,
        },
        EntityDesc {
            offset: cgmath::vec3(-2.0, 2.0, 2.0),
            angle: 140.0,
            scale: 1.1,
        },
        EntityDesc {
            offset: cgmath::vec3(2.0, 2.0, 2.0),
            angle: 210.0,
            scale: 0.9,
        },
    ];

    let mut entities: Vec<Entity<_>> = cube_descs.iter().map(|desc| {
        use cgmath::{EuclideanVector, Rotation3};
        let (mesh, slice) = create_cube(factory);
        make_entity(&mesh, &slice,
            &program_forward, &program_shadow, &shadow_param,
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
        let (mesh, slice) = create_plane(factory);
        make_entity(&mesh, &slice,
            &program_forward, &program_shadow, &shadow_param,
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

    struct LightDesc {
        pos: cgmath::Point3<f32>,
        color: gfx::ColorValue,
        fov: f32,
    }

    let light_descs = vec![
        LightDesc {
            pos: cgmath::Point3::new(-3.0, 10.0, 3.0),
            color: [1.0, 1.0, 1.0, 1.0],
            fov: 60.0,
        }
    ];

    let lights = light_descs.iter().enumerate().map(|(i, desc)| Light {
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

    Scene {
        camera: camera,
        lights: lights,
        entities: entities,
    }
}

//----------------------------------------

pub fn main() {
    use cgmath::{FixedArray, Matrix};

    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(
        glutin::WindowBuilder::new()
            .with_title("Multi-threaded shadow rendering example with gfx-rs".to_string())
            .with_dimensions(800, 600)
            .with_gl(glutin::GL_CORE)
            .with_depth_buffer(24)
            .build().unwrap()
    );

    let mut scene = create_scene(&device, &mut factory);

    'main: loop {
        // quit when Esc is pressed.
        for event in stream.out.window.poll_events() {
            use glutin::{Event, VirtualKeyCode};
            match event {
                Event::KeyboardInput(_, _, Some(VirtualKeyCode::Escape)) => break 'main,
                Event::Closed => break 'main,
                _ => {},
            }
        }

        stream.clear(gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });

        let mx_vp = {
            let mut proj = scene.camera.projection;
            proj.aspect = stream.get_aspect_ratio();
            let proj_mx: cgmath::Matrix4<_> = proj.into();
            proj_mx.mul_m(&scene.camera.mx_view)
        };
        for ent in scene.entities.iter_mut() {
            ent.batch_forward.param.transform = mx_vp.mul_m(&ent.mx_to_world).into_fixed();
            ent.batch_forward.param.normal_transform = {
                let m = &ent.mx_to_world;
                [[m.x.x, m.x.y, m.x.z],
                [m.y.x, m.y.y, m.y.z],
                [m.z.x, m.z.y, m.z.z]]
            };
            stream.draw(&ent.batch_forward).unwrap();
        }

        stream.present(&mut device);
    }
}
