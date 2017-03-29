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

use gfx_corell::{buffer, command, format, pass, pso, state, target, 
    Device, CommandPool, GraphicsCommandPool, GraphicsCommandBuffer, ProcessingCommandBuffer, PrimaryCommandBuffer,
    Primitive, Instance, Adapter, Surface, SwapChain, QueueFamily, Factory, SubPass};
use gfx_corell::command::{RenderPassEncoder, RenderPassInlineEncoder};
use gfx_corell::format::Formatted;
use gfx_corell::memory::{self, ImageBarrier};

pub type ColorFormat = gfx_corell::format::Srgba8;

struct Vertex {
    a_Pos: [f32; 2],
    a_Color: [f32; 3],
}

#[cfg(any(feature = "vulkan", target_os = "windows"))]
fn main() {
    env_logger::init().unwrap();
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

    // Build a new device and associated command queues
    let Device { mut factory, mut general_queues, heap_types, .. } = physical_devices[0].open(queue_descs);
    let mut swap_chain = surface.build_swapchain::<ColorFormat>(&general_queues[0]);

    // Setup renderpass and pipeline
    #[cfg(all(target_os = "windows", not(feature = "vulkan")))]
    let shader_lib = factory.create_shader_library(&[
            ("vs_main", include_bytes!("data/vs_main.o")),
            ("ps_main", include_bytes!("data/ps_main.o"))]
        ).expect("Error on creating shader lib");

    #[cfg(feature = "vulkan")]
    let shader_lib = factory.create_shader_library(&[
            ("vs_main", include_bytes!("data/vs_main.spv")),
            ("ps_main", include_bytes!("data/ps_main.spv"))]
        ).expect("Error on creating shader lib");

    // dx12 runtime shader compilation
    /*
    let shader_lib = factory.create_shader_library_from_hlsl(&[
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

    let pipeline_layout = factory.create_pipeline_layout();

    let render_pass = {
        let attachment = pass::Attachment {
            format: ColorFormat::get_format(),
            load_op: pass::AttachmentLoadOp::Clear,
            store_op: pass::AttachmentStoreOp::Store,
            stencil_load_op: pass::AttachmentLoadOp::DontCare,
            stencil_store_op: pass::AttachmentStoreOp::DontCare,
            src_layout: memory::ImageLayout::Undefined, // TODO: maybe Option<_> here?
            dst_layout: memory::ImageLayout::Present,
        };

        let subpass = pass::SubpassDesc {
            color_attachments: &[(0, memory::ImageLayout::ColorAttachmentOptimal)],
        };

        let dependency = pass::SubpassDependency {
            src_pass: pass::SubpassRef::External,
            dst_pass: pass::SubpassRef::Pass(0),
            src_stage: pso::COLOR_ATTACHMENT_OUTPUT,
            dst_stage: pso::COLOR_ATTACHMENT_OUTPUT,
            src_access: memory::ImageAccess::empty(),
            dst_access: memory::COLOR_ATTACHMENT_READ | memory::COLOR_ATTACHMENT_WRITE,
        };

        factory.create_renderpass(&[attachment], &[subpass], &[dependency])
    };

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
    let pipelines = factory.create_graphics_pipelines(&[
        (&shader_lib, &pipeline_layout, SubPass { index: 0, main_pass: &render_pass }, &pipeline_desc)
    ]);

    println!("{:?}", pipelines);


    // Framebuffer and render target creation
    let frame_rtvs = swap_chain.get_images().iter().map(|image| {
        factory.view_image_as_render_target(&image, ColorFormat::get_format()).unwrap()
    }).collect::<Vec<_>>();

    let framebuffers = frame_rtvs.iter().map(|frame_rtv| {
        factory.create_framebuffer(&render_pass, &[&frame_rtv], &[], 1024, 768, 1)
    }).collect::<Vec<_>>();


    // Buffer allocations
    println!("Memory heaps: {:?}", heap_types);

    let heap = {
        let upload_heap = heap_types.iter().find(|&&heap_type| heap_type.properties.contains(memory::UPLOAD_HEAP)).unwrap();
        factory.create_heap(&heap_types[0], 1024)
    };

    let vertex_buffer = {
        let buffer = factory.create_buffer(3 * std::mem::size_of::<Vertex>() as u64, buffer::VERTEX).unwrap();
        println!("{:?}", buffer);
        let buffer_req = factory.get_buffer_requirements(&buffer);
        println!("buffer requirements: {:?}", buffer_req);

        factory.bind_buffer_memory(&heap, 0, buffer).unwrap()
    };

    // Rendering setup
    let viewport = target::Rect {
        x: 0, y: 0,
        w: 1024, h: 768,
    };
    let scissor = target::Rect {
        x: 0, y: 0,
        w: 1024, h: 768,
    };

    let mut graphics_pool = back::GraphicsCommandPool::from_queue(&mut general_queues[0], 16);

    //
    'main: loop {
        for event in window.poll_events() {
            match event {
                winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                winit::Event::Closed => break 'main,
                _ => {},
            }
        }

        graphics_pool.reset();

        let frame = swap_chain.acquire_frame();

        // Rendering
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            cmd_buffer.set_viewports(&[viewport]);
            cmd_buffer.set_scissors(&[scissor]);
            cmd_buffer.bind_graphics_pipeline(&pipelines[0].as_ref().unwrap());
            cmd_buffer.bind_vertex_buffers(pso::VertexBufferSet(vec![(&vertex_buffer, 0)]));

            {
                let mut encoder = back::RenderPassInlineEncoder::begin(
                    &mut cmd_buffer,
                    &render_pass,
                    &framebuffers[frame.id()],
                    target::Rect { x: 0, y: 0, w: 1024, h: 768 },
                    &[command::ClearValue::Color(command::ClearColor::Float([0.2, 0.2, 0.2, 1.0]))]);

                encoder.draw(0, 3, None);
            }

            // TODO: should transition to (_, Present) -> Present (for d3d12)
            
            cmd_buffer.finish()
        };

        general_queues[0].submit_graphics(&[submit]);

        // present frame
        swap_chain.present();
    }
}

#[cfg(not(any(feature = "vulkan", target_os = "windows")))]
fn main() {}
