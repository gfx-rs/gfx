// Copyright 2015 The gfx developers.
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

extern crate rand;
extern crate winit;

#[macro_use]
extern crate gfx;
extern crate gfx_app;

use rand::Rng;
use gfx::{Factory, GraphicsPoolExt};
use gfx_app::{BackbufferView, ColorFormat};

const QUAD_VERTICES: [Vertex; 4] = [
    Vertex { position: [-0.5, 0.5] },
    Vertex { position: [-0.5, -0.5] },
    Vertex { position: [0.5, -0.5] },
    Vertex { position: [0.5, 0.5] },
];

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

gfx_defines!{
    vertex Vertex {
        position: [f32; 2] = "a_Position",
    }

    // color format: 0xRRGGBBAA
    vertex Instance {
        translate: [f32; 2] = "a_Translate",
        color: u32 = "a_Color",
    }

    constant Locals {
        scale: f32 = "u_Scale",
    }

    pipeline pipe {
        vertex: gfx::VertexBuffer<Vertex> = (),
        instance: gfx::InstanceBuffer<Instance> = (),
        scale: gfx::Global<f32> = "u_Scale",
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

fn fill_instances(instances: &mut [Instance], instances_per_length: u32, size: f32) {
    let gap = 0.4 / (instances_per_length + 1) as f32;
    println!("gap: {}", gap);

    let begin = -1. + gap + (size / 2.);
    let mut translate = [begin, begin];
    let mut rng = rand::StdRng::new().unwrap();

    let length = instances_per_length as usize;
    for x in 0..length {
        for y in 0..length {
            let i = x * length + y;
            instances[i] = Instance {
                translate: translate,
                color: rng.next_u32(),
            };
            translate[1] += size + gap;
        }
        translate[1] = begin;
        translate[0] += size + gap;
    }
}

const MAX_INSTANCE_COUNT: usize = 2048;

struct App<B: gfx::Backend> {
    views: Vec<BackbufferView<B::Resources>>,
    pso: gfx::PipelineState<B::Resources, pipe::Meta>,
    data: pipe::Data<B::Resources>,
    slice: gfx::Slice<B::Resources>,
    upload: gfx::handle::Buffer<B::Resources, Instance>,
    uploading: bool, // TODO: not needed if we have the encoder everywhere
}

impl<B: gfx::Backend> gfx_app::Application<B> for App<B> {
    fn new(
        factory: &mut B::Factory,
        _: &mut gfx::queue::GraphicsQueue<B>,
        backend: gfx_app::shade::Backend,
        window_targets: gfx_app::WindowTargets<B::Resources>,
    ) -> Self {
        use gfx::traits::FactoryExt;

        let vs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/instancing_120.glslv"),
            glsl_150: include_bytes!("shader/instancing_150.glslv"),
            msl_11: include_bytes!("shader/instancing_vertex.metal"),
            hlsl_40: include_bytes!("data/vertex.fx"),
            ..gfx_app::shade::Source::empty()
        };
        let fs = gfx_app::shade::Source {
            glsl_120: include_bytes!("shader/instancing_120.glslf"),
            glsl_150: include_bytes!("shader/instancing_150.glslf"),
            msl_11: include_bytes!("shader/instancing_frag.metal"),
            hlsl_40: include_bytes!("data/pixel.fx"),
            ..gfx_app::shade::Source::empty()
        };

        let instances_per_length: u32 = 32;
        println!("{} instances per length", instances_per_length);
        let instance_count = instances_per_length * instances_per_length;
        println!("{} instances", instance_count);
        assert!(instance_count as usize <= MAX_INSTANCE_COUNT);
        let size = 1.6 / instances_per_length as f32;
        println!("size: {}", size);

        let upload = factory
            .create_upload_buffer(instance_count as usize)
            .unwrap();
        {
            let mut writer = factory.write_mapping(&upload).unwrap();
            fill_instances(&mut writer, instances_per_length, size);
        }

        let instances = factory
            .create_buffer(
                instance_count as usize,
                gfx::buffer::Role::Vertex,
                gfx::memory::Usage::Data,
                gfx::TRANSFER_DST,
            )
            .unwrap();


        let (quad_vertices, mut slice) =
            factory.create_vertex_buffer_with_slice(&QUAD_VERTICES, &QUAD_INDICES[..]);
        slice.instances = Some((instance_count, 0));
        let locals = Locals { scale: size };

        App {
            pso: factory
                .create_pipeline_simple(
                    vs.select(backend).unwrap(),
                    fs.select(backend).unwrap(),
                    pipe::new(),
                )
                .unwrap(),
            data: pipe::Data {
                vertex: quad_vertices,
                instance: instances,
                scale: size,
                locals: factory
                    .create_buffer_immutable(
                        &[locals],
                        gfx::buffer::Role::Constant,
                        gfx::Bind::empty(),
                    )
                    .unwrap(),
                out: window_targets.views[0].0.clone(),
            },
            slice: slice,
            upload: upload,
            uploading: true,
            views: window_targets.views,
        }
    }

    fn render(
        &mut self,
        (frame, sync): (gfx::Frame, &gfx_app::SyncPrimitives<B::Resources>),
        pool: &mut gfx::GraphicsCommandPool<B>,
        queue: &mut gfx::queue::GraphicsQueue<B>,
    ) {
        let mut encoder = pool.acquire_graphics_encoder();
        if self.uploading {
            encoder
                .copy_buffer(&self.upload, &self.data.instance, 0, 0, self.upload.len())
                .unwrap();
            self.uploading = false;
        }

        let (cur_color, _) = self.views[frame.id()].clone();
        self.data.out = cur_color;
        encoder.clear(&self.data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&self.slice, &self.pso, &self.data);
        encoder
            .synced_flush(queue, &[&sync.rendering], &[], Some(&sync.frame_fence))
            .expect("Coult not flush encoder");
    }

    // TODO: rerandomize instance data on event, needs factory

    fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<B::Resources>) {
        self.views = window_targets.views;
    }
}

pub fn main() {
    use gfx_app::Application;
    App::launch_simple("Instancing example");
}
