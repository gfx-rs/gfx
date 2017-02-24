// Copyright 2017 The Gfx-rs Developers.
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

extern crate env_logger;
extern crate gfx_corell;
#[cfg(all(target_os = "windows", not(feature = "vulkan")))]
extern crate gfx_device_dx12ll as back;
#[cfg(feature = "vulkan")]
extern crate gfx_device_vulkanll as back;

extern crate winit;

use gfx_corell::{format, pso, shade, state,
    Primitive, Instance, Adapter, Surface, SwapChain, QueueFamily, Factory, SubPass};
use gfx_corell::format::Formatted;

pub type ColorFormat = gfx_corell::format::Rgba8;

struct Vertex {
    a_Pos: [f32; 2],
    a_Color: [f32; 3],
}

#[cfg(any(feature = "vulkan", target_os = "windows"))]
fn main() {    env_logger::init().unwrap();
    let window = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("triangle (Low Level)".to_string())
        .build()
        .unwrap();

    // instantiate backend
    let instance = back::Instance::create();
    let physical_devices = instance.enumerate_adapters();
    let surface = instance.create_surface(&window);

    let queue_descs = physical_devices[0].get_queue_families().map(|family| { (family, family.num_queues()) });
    
    for device in &physical_devices {
        println!("{:?}", device.get_info());
    }

    // build a new device and associated command queues
    let (mut device, queues) = physical_devices[0].open(queue_descs);
    let mut swap_chain = surface.build_swapchain::<ColorFormat>(&queues[0]);

    #[cfg(all(target_os = "windows", not(feature = "vulkan")))]
    let shader_lib = device.create_shader_library(&[
            ("vs_main", include_bytes!("data/vs_main.o")),
            ("ps_main", include_bytes!("data/ps_main.o"))]
        ).expect("Error on creating shader lib");

    #[cfg(feature = "vulkan")]
    let shader_lib = device.create_shader_library(&[
            ("vs_main", include_bytes!("data/vs_main.spv")),
            ("ps_main", include_bytes!("data/ps_main.spv"))]
        ).expect("Error on creating shader lib");

    // dx12 runtime shader compilation
    /*
    let shader_lib = device.create_shader_library_from_hlsl(&[
                ("vs_main", shade::Stage::Vertex, include_bytes!("shader/triangle.hlsl")),
                ("ps_main", shade::Stage::Pixel, include_bytes!("shader/triangle.hlsl"))]
        ).expect("Error on creating shader lib");
    */

    let shader_entries = pso::GraphicsShaderSet {
        vertex_shader: "vs_main",
        hull_shader: None,
        domain_shader: None,
        geometry_shader: None,
        pixel_shader: Some("ps_main"),
    };

    let pipeline_signature = device.create_pipeline_signature();
    let render_pass = device.create_renderpass();

    //
    let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
        Primitive::TriangleList,
        state::Rasterizer::new_fill(),
        shader_entries);

    pipeline_desc.color_targets[0] = Some((ColorFormat::get_format(), state::MASK_ALL.into()));
    pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
        stride: std::mem::size_of::<Vertex>() as u8,
        rate: 0,
    });

    pipeline_desc.attributes.push((0, pso::Element {
        format: <format::Vec2<f32> as format::Formatted>::get_format(),
        offset: 0
    }));
    pipeline_desc.attributes.push((0, pso::Element {
        format: <format::Vec3<f32> as format::Formatted>::get_format(),
        offset: 8
    }));

    //
    let pipelines = device.create_graphics_pipelines(&[
        (&shader_lib, &pipeline_signature, SubPass { index: 0, main_pass: &render_pass }, &pipeline_desc)
    ]);

    println!("{:?}", pipelines);

    //
    'main: loop {
        for event in window.poll_events() {
            match event {
                winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                winit::Event::Closed => break 'main,
                _ => {},
            }
        }

        let frame = swap_chain.acquire_frame();

        // rendering

        // present frame
        swap_chain.present();
    }
}

#[cfg(not(any(feature = "vulkan", target_os = "windows")))]
fn main() {}
