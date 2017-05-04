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
#[macro_use]
extern crate gfx;
extern crate gfx_app;
extern crate winit;

pub use gfx::format::{DepthStencil};
pub use gfx_app::{ColorFormat, DepthFormat};

#[cfg(feature="metal")]
pub use gfx::format::Depth32F as Depth;
#[cfg(not(feature="metal"))]
pub use gfx::format::Depth;

use cgmath::{Deg, Matrix4, Point3, Vector3};
use std::sync::{Arc, RwLock};

// Section-1: vertex formats and shader parameters

gfx_defines!{
    vertex Vertex {
        pos: [i8; 4] = "a_Pos",
        normal: [i8; 4] = "a_Normal",
    }

    constant ForwardVsLocals {
        transform: [[f32; 4]; 4] = "u_Transform",
        model_transform: [[f32; 4]; 4] = "u_ModelTransform",
    }

    constant ForwardPsLocals {
        color: [f32; 4] = "u_Color",
        num_lights: i32 = "u_NumLights",
        _padding: [i32; 3] = "",
    }

    constant ShadowLocals {
        transform: [[f32; 4]; 4] = "u_Transform",
    }

    constant LightParam {
        pos: [f32; 4] = "pos",
        color: [f32; 4] = "color",
        proj: [[f32; 4]; 4] = "proj",
    }

    pipeline forward {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        vs_locals: gfx::ConstantBuffer<ForwardVsLocals> = "VsLocals",
        ps_locals: gfx::ConstantBuffer<ForwardPsLocals> = "PsLocals",
        light_buf: gfx::ConstantBuffer<LightParam> = "b_Lights",
        shadow: gfx::TextureSampler<f32> = "t_Shadow",
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }

    pipeline shadow {
        vbuf: gfx::VertexBuffer<Vertex> = (),
        locals: gfx::ConstantBuffer<ShadowLocals> = "Locals",
        out: gfx::DepthTarget<Depth> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

impl Vertex {
    fn new(p: [i8; 3], n: [i8; 3]) -> Vertex {
        Vertex {
            pos: [p[0], p[1], p[2], 1],
            normal: [n[0], n[1], n[2], 0],
        }
    }
}

const MAX_LIGHTS: usize = 10;

//----------------------------------------
// Section-2: simple primitives generation
//TODO: replace by genmesh

fn create_cube<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
               -> (gfx::handle::Buffer<R, Vertex>, gfx::Slice<R>)
{
    use gfx::traits::FactoryExt;
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

    let index_data: &[u16] = &[
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    factory.create_vertex_buffer_with_slice(&vertex_data, index_data)
}

fn create_plane<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F, size: i8)
                -> (gfx::handle::Buffer<R, Vertex>, gfx::Slice<R>)
{
    use gfx::traits::FactoryExt;
    let vertex_data = [
        Vertex::new([ size, -size,  0], [0, 0, 1]),
        Vertex::new([ size,  size,  0], [0, 0, 1]),
        Vertex::new([-size, -size,  0], [0, 0, 1]),
        Vertex::new([-size,  size,  0], [0, 0, 1]),
    ];

    let index_data: &[u16] = &[
        0, 1, 2,
        2, 1, 3
    ];

    factory.create_vertex_buffer_with_slice(&vertex_data, index_data)
}

//----------------------------------------
// Section-3: scene definitions

struct Camera {
    mx_view: Matrix4<f32>,
    projection: cgmath::PerspectiveFov<f32>,
}

struct Light<R: gfx::Resources, C: gfx::CommandBuffer<R>> {
    position: Point3<f32>,
    mx_view: Matrix4<f32>,
    projection: cgmath::Perspective<f32>,
    color: gfx::ColorValue,
    shadow: gfx::handle::DepthStencilView<R, Depth>,
    encoder: gfx::Encoder<R, C>,
}

struct Entity<R: gfx::Resources> {
    dynamic: bool,
    mx_to_world: Matrix4<f32>,
    batch_shadow: shadow::Data<R>,
    batch_forward: forward::Data<R>,
    slice: gfx::Slice<R>,
}

struct Share<R: gfx::Resources> {
    entities: Vec<Entity<R>>,
    shadow_pso: gfx::PipelineState<R, shadow::Meta>,
}

struct Scene<R: gfx::Resources, C: gfx::CommandBuffer<R>> {
    camera: Camera,
    lights: Vec<Light<R, C>>,
    light_dirty: bool,
    light_buf: gfx::handle::Buffer<R, LightParam>,
    share: Arc<RwLock<Share<R>>>,
}

//----------------------------------------
// Section-4: scene construction routines

/// Create a full scene
fn create_scene<R, F>(factory: &mut F,
                out_color: gfx::handle::RenderTargetView<R, ColorFormat>,
                out_depth: gfx::handle::DepthStencilView<R, DepthFormat>,
                shadow_pso: gfx::PipelineState<R, shadow::Meta>)
                -> Scene<R, F::CommandBuffer> where
    R: gfx::Resources,
    F: gfx_app::Factory<R>,
{
    use cgmath::{InnerSpace, SquareMatrix};
    use gfx::traits::FactoryExt;

    // create shadows
    let (shadow_tex, shadow_resource) = {
        use gfx::texture as t;
        let kind = t::Kind::D2Array(512, 512, MAX_LIGHTS as gfx::Layer, t::AaMode::Single);
        let bind = gfx::SHADER_RESOURCE | gfx::DEPTH_STENCIL;
        let cty = gfx::format::ChannelType::Unorm;
        let tex = factory.create_texture(kind, 1, bind, gfx::memory::Usage::Data, Some(cty)).unwrap();
        let resource = factory.view_texture_as_shader_resource::<Depth>(
            &tex, (0, 0), gfx::format::Swizzle::new()).unwrap();
        (tex, resource)
    };
    let shadow_sampler = {
        use gfx::texture as t;
        let mut sinfo = t::SamplerInfo::new(
            t::FilterMethod::Bilinear,
            t::WrapMode::Clamp
        );
        sinfo.comparison = Some(gfx::state::Comparison::LessEqual);
        factory.create_sampler(sinfo)
    };

    // create lights
    struct LightDesc {
        pos: Point3<f32>,
        color: gfx::ColorValue,
        fov: f32,
    }

    let light_descs = vec![
        LightDesc {
            pos: Point3::new(7.0, -5.0, 10.0),
            color: [0.5, 1.0, 0.5, 1.0],
            fov: 60.0,
        },
        LightDesc {
            pos: Point3::new(-5.0, 7.0, 10.0),
            color: [1.0, 0.5, 0.5, 1.0],
            fov: 45.0,
        },
    ];

    let (near, far) = (1f32, 20f32);
    let lights: Vec<_> = light_descs.iter().enumerate().map(|(i, desc)| Light {
        position: desc.pos.clone(),
        mx_view: Matrix4::look_at(
            desc.pos,
            Point3::new(0.0, 0.0, 0.0),
            Vector3::unit_z(),
        ),
        projection: cgmath::PerspectiveFov {
            fovy: Deg(desc.fov).into(),
            aspect: 1.0,
            near: near,
            far: far,
        }.to_perspective(),
        color: desc.color.clone(),
        shadow: factory.view_texture_as_depth_stencil(
            &shadow_tex, 0, Some(i as gfx::Layer), gfx::texture::DepthStencilFlags::empty(),
            ).unwrap(),
        encoder: factory.create_encoder(),
    }).collect();
    let light_buf = factory.create_constant_buffer(MAX_LIGHTS);

    // create entities
    struct CubeDesc {
        offset: Vector3<f32>,
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

    let (cube_buf, cube_slice) = create_cube(factory);
    let locals = ForwardPsLocals {
        color: [1.0, 1.0, 1.0, 1.0],
        num_lights: lights.len() as i32,
        _padding: [0i32; 3]
    };

    let mut fw_data = forward::Data {
        vbuf: cube_buf.clone(),
        vs_locals: factory.create_constant_buffer(1),
        ps_locals: factory.create_buffer_immutable(&[locals],
            gfx::buffer::Role::Constant, gfx::Bind::empty()
            ).unwrap(),
        light_buf: light_buf.clone(),
        shadow: (shadow_resource, shadow_sampler),
        out_color: out_color,
        out_depth: out_depth,
    };

    let mut sh_data = shadow::Data {
        vbuf: cube_buf,
        locals: factory.create_constant_buffer(1),
        // the output here is temporary, will be overwritten for every light source
        out: factory.view_texture_as_depth_stencil(&shadow_tex, 0, None,
            gfx::texture::DepthStencilFlags::empty()).unwrap(),
    };

    let mut entities: Vec<_> = cube_descs.iter().map(|desc| {
        use cgmath::{Decomposed, Quaternion, Rotation3};
        let transform = Decomposed {
            disp: desc.offset.clone(),
            rot: Quaternion::from_axis_angle(
                desc.offset.normalize(),
                Deg(desc.angle),
            ),
            scale: desc.scale,
        }.into();
        Entity {
            dynamic: true,
            mx_to_world: transform,
            batch_forward: fw_data.clone(),
            batch_shadow: sh_data.clone(),
            slice: cube_slice.clone(),
        }
    }).collect();

    let (plane_buf, plane_slice) = create_plane(factory, 7);
    fw_data.vbuf = plane_buf.clone();
    sh_data.vbuf = plane_buf;

    entities.push(Entity {
        dynamic: false,
        mx_to_world: Matrix4::identity(),
        batch_forward: fw_data,
        batch_shadow: sh_data,
        slice: plane_slice,
    });

    // create camera
    let camera = Camera {
        mx_view: Matrix4::look_at(
            Point3::new(3.0f32, -10.0, 6.0),
            Point3::new(0f32, 0.0, 0.0),
            Vector3::unit_z(),
        ),
        projection: cgmath::PerspectiveFov {
            fovy: Deg(45.0f32).into(),
            aspect: 1.0,
            near: near,
            far: far,
        },
    };

    let share = Share {
        shadow_pso: shadow_pso,
        entities: entities,
    };

    Scene {
        camera: camera,
        lights: lights,
        light_dirty: true,
        light_buf: light_buf,
        share: Arc::new(RwLock::new(share)),
    }
}

//----------------------------------------
// Section-5: application

struct App<R: gfx::Resources, C: gfx::CommandBuffer<R>> {
    window_targets: gfx_app::WindowTargets<R>,
    is_parallel: bool,
    forward_pso: gfx::PipelineState<R, forward::Meta>,
    encoder: gfx::Encoder<R, C>,
    scene: Scene<R, C>,
}

impl<R: gfx::Resources, C: gfx::CommandBuffer<R>> App<R, C> {
    fn rotate(&mut self, axis: Vector3<f32>) {
        use cgmath::{Decomposed, InnerSpace, Quaternion, Rotation3};
        let len = axis.magnitude();
        for ent in self.scene.share.write().unwrap().entities.iter_mut() {
            if !ent.dynamic {
                continue
            }
            // rotate all cubes around the axis
            let rot = Decomposed {
                scale: 1.0,
                rot: Quaternion::from_axis_angle(
                    axis * (1.0 / len),
                    Deg(len * 0.3),
                ),
                disp: cgmath::vec3(0.0, 0.0, 0.0)
            };
            ent.mx_to_world = ent.mx_to_world * Matrix4::from(rot);
        }
    }
}

// Note: these 'static and Sync bounds are unfortunate...
// We need to figure out how to make it less painful.
impl<R, C> gfx_app::ApplicationBase<R, C> for App<R, C> where
    R: gfx::Resources + 'static,
    C: gfx::CommandBuffer<R> + Send + 'static,
{
    fn new<F>(factory: &mut F, backend: gfx_app::shade::Backend, window_targets: gfx_app::WindowTargets<R>) -> Self
    where F: gfx_app::Factory<R, CommandBuffer=C>,
    {
        use std::env;
        use gfx::traits::FactoryExt;
        use gfx_app::shade::Source;

        let mut is_parallel = true;
        for arg in env::args().skip(1) {
            if arg == "single" {
                is_parallel = false;
            }
        }
        println!("Running in {}-threaded mode",
            if is_parallel {"multi"} else {"single"},
        );

        let forward_pso = {
            let vs = Source {
                glsl_150: include_bytes!("shader/forward_150.glslv"),
                hlsl_41:  include_bytes!("data/forward_vs.fx"),
                msl_11:   include_bytes!("shader/forward_vertex.metal"),
                .. Source::empty()
            };
            let ps = Source {
                glsl_150: include_bytes!("shader/forward_150.glslf"),
                hlsl_41:  include_bytes!("data/forward_ps.fx"),
                msl_11:   include_bytes!("shader/forward_frag.metal"),
                .. Source::empty()
            };
            factory.create_pipeline_simple(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap(),
                forward::new()
                ).unwrap()
        };

        let shadow_pso = {
            let vs = Source {
                glsl_150: include_bytes!("shader/shadow_150.glslv"),
                hlsl_41:  include_bytes!("data/shadow_vs.fx"),
                msl_11:   include_bytes!("shader/shadow_vertex.metal"),
                .. Source::empty()
            };
            let ps = Source {
                glsl_150: include_bytes!("shader/shadow_150.glslf"),
                hlsl_41:  include_bytes!("data/shadow_ps.fx"),
                msl_11:   b"\n",
                .. Source::empty()
            };
            let set = factory.create_shader_set(
                vs.select(backend).unwrap(),
                ps.select(backend).unwrap()
                ).unwrap();
            factory.create_pipeline_state(&set,
                gfx::Primitive::TriangleList,
                gfx::state::Rasterizer::new_fill()
                                       .with_cull_back()
                                       .with_offset(2.0, 1),
                shadow::new()
                ).unwrap()
        };

        let scene = create_scene(factory,
            window_targets.color.clone(),
            window_targets.depth.clone(),
            shadow_pso);

        App {
            window_targets: window_targets,
            is_parallel: is_parallel,
            forward_pso: forward_pso,
            encoder: factory.create_encoder(),
            scene: scene,
        }
    }

    fn render<D>(&mut self, device: &mut D) where
        D: gfx::Device<Resources=R, CommandBuffer=C>
    {
        self.rotate(cgmath::vec3(0.0, 0.0, 1.0));
        if self.scene.light_dirty {
            // init light parameters
            let light_params: Vec<_> = self.scene.lights.iter().map(|light| LightParam {
                pos: [light.position.x, light.position.y, light.position.z, 1.0],
                color: light.color,
                proj: {
                    let mx_proj = Matrix4::from(light.projection);
                    (mx_proj * light.mx_view).into()
                },
            }).collect();
            self.encoder.update_buffer(&self.scene.light_buf, &light_params, 0).unwrap();
            self.scene.light_dirty = false;
        }

        // fill up shadow map for each light
        if self.is_parallel {
            use std::thread;
            use std::sync::mpsc;

            let (sender_orig, receiver) = mpsc::channel();
            let num = self.scene.lights.len();
            // run parallel threads
            let _threads: Vec<_> = (0..num).map(|_| {
                // move the light into the thread scope
                let mut light = self.scene.lights.swap_remove(0);
                let share = self.scene.share.clone();
                let sender = sender_orig.clone();
                thread::spawn(move || {
                    // clear
                    light.encoder.clear_depth(&light.shadow, 1.0);
                    // fill
                    let subshare = share.read().unwrap();
                    for ent in subshare.entities.iter() {
                        let mut batch = ent.batch_shadow.clone();
                        batch.out = light.shadow.clone();
                        let locals = ShadowLocals{
                            transform: {
                                let mx_proj: Matrix4<_> = light.projection.into();
                                let mx_view = mx_proj * light.mx_view;
                                let mvp = mx_view * ent.mx_to_world;
                                mvp.into()
                            },
                        };
                        light.encoder.update_constant_buffer(&batch.locals, &locals);
                        light.encoder.draw(&ent.slice, &subshare.shadow_pso, &batch);
                    }
                    sender.send(light).unwrap();
                })
            }).collect();
            // wait for the results and execute them
            // put the lights back into the scene
            for _ in 0..num {
                let mut light = receiver.recv().unwrap();
                light.encoder.flush(device);
                self.scene.lights.push(light);
            }
        } else {
            for light in self.scene.lights.iter_mut() {
                // clear
                self.encoder.clear_depth(&light.shadow, 1.0);
                // fill
                let subshare = self.scene.share.read().unwrap();
                for ent in subshare.entities.iter() {
                    let mut batch = ent.batch_shadow.clone();
                    batch.out = light.shadow.clone();
                    let locals = ShadowLocals{
                        transform: {
                            let mx_proj: Matrix4<_> = light.projection.into();
                            let mx_view = mx_proj * light.mx_view;
                            let mvp = mx_view * ent.mx_to_world;
                            mvp.into()
                        },
                    };
                    self.encoder.update_constant_buffer(&batch.locals, &locals);
                    self.encoder.draw(&ent.slice, &subshare.shadow_pso, &batch);
                }
            }
        }

        // draw entities with forward pass
        self.encoder.clear(&self.window_targets.color, [0.1, 0.2, 0.3, 1.0]);
        self.encoder.clear_depth(&self.window_targets.depth, 1.0);

        let mx_vp = {
            let mut proj = self.scene.camera.projection;
            proj.aspect = self.window_targets.aspect_ratio;
            let mx_proj: Matrix4<_> = proj.into();
            mx_proj * self.scene.camera.mx_view
        };

        for ent in self.scene.share.write().unwrap().entities.iter_mut() {
            let batch = &ent.batch_forward;
            let locals = ForwardVsLocals {
                transform: (mx_vp * ent.mx_to_world).into(),
                model_transform: ent.mx_to_world.into(),
            };
            self.encoder.update_constant_buffer(&batch.vs_locals, &locals);
            self.encoder.draw(&ent.slice, &self.forward_pso, batch);
        }

        self.encoder.flush(device);
    }

    fn get_exit_key() -> Option<winit::VirtualKeyCode> {
        Some(winit::VirtualKeyCode::Escape)
    }

    fn on(&mut self, event: winit::WindowEvent) {
        match event {
            _ => () //TODO
        }
    }

    fn on_resize<F>(&mut self, _factory: &mut F, window_targets: gfx_app::WindowTargets<R>)
    where F: gfx_app::Factory<R, CommandBuffer=C>
    {
        for ent in self.scene.share.write().unwrap().entities.iter_mut() {
            ent.batch_forward.out_color = window_targets.color.clone();
            ent.batch_forward.out_depth = window_targets.depth.clone();
        }
        self.window_targets = window_targets;
    }
}

//----------------------------------------
// Section-6: main entry point

pub fn main() {
    let wb = winit::WindowBuilder::new().with_title(
        "Multi-threaded shadow rendering example with gfx-rs");
    gfx_app::launch_gl3::<App<_, _>>(wb);
}
