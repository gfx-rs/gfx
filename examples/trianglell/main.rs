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
extern crate image;

use gfx_corell::{buffer, command, format, pass, pso, shade, state, target, 
    Device, CommandPool, GraphicsCommandPool, CommandQueue,
    GraphicsCommandBuffer, ProcessingCommandBuffer, TransferCommandBuffer, PrimaryCommandBuffer,
    Primitive, Instance, Adapter, Surface, SwapChain, QueueFamily, QueueSubmit, Factory, SubPass, FrameSync};
use gfx_corell::command::{RenderPassEncoder, RenderPassInlineEncoder};
use gfx_corell::format::Formatted;
use gfx_corell::memory::{self, ImageBarrier, ImageStateSrc, ImageStateDst, ImageLayout, ImageAccess};
use gfx_corell::factory::{DescriptorHeapType, DescriptorPoolDesc, DescriptorType,
    DescriptorSetLayoutBinding, DescriptorSetWrite, DescriptorWrite};

use std::io::Cursor;
use std::ops::Deref;
use gfx_corell::image as i;

pub type ColorFormat = gfx_corell::format::Srgba8;

#[derive(Debug, Clone, Copy)]
struct Vertex {
    a_Pos: [f32; 2],
    a_Uv: [f32; 3],
}

const TRIANGLE: [Vertex; 6] = [
    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0, 0.0] },
    Vertex { a_Pos: [  0.5, 0.33 ], a_Uv: [1.0, 1.0, 0.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0, 0.0] },

    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0, 0.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0, 0.0] },
    Vertex { a_Pos: [ -0.5,-0.33 ], a_Uv: [0.0, 0.0, 0.0] },
];

#[cfg(any(feature = "vulkan", target_os = "windows"))]
fn main() {
    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("triangle (Low Level)".to_string())
        .build(&events_loop)
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

    let set0_layout = factory.create_descriptor_set_layout(&[
            DescriptorSetLayoutBinding {
                binding: 0,
                ty: DescriptorType::SampledImage,
                count: 1,
                stage_flags: shade::STAGE_PIXEL,
            }
        ]);

    let set1_layout = factory.create_descriptor_set_layout(&[
            DescriptorSetLayoutBinding {
                binding: 0,
                ty: DescriptorType::Sampler,
                count: 1,
                stage_flags: shade::STAGE_PIXEL,
            }
        ]);

    let pipeline_layout = factory.create_pipeline_layout(&[&set0_layout, &set1_layout]);

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

    pipeline_desc.color_targets[0] = Some((
        ColorFormat::get_format(),
        state::Blend {
            color: state::BlendChannel {
                equation: state::Equation::Add,
                source: state::Factor::ZeroPlus(state::BlendValue::SourceAlpha),
                destination: state::Factor::OneMinus(state::BlendValue::SourceAlpha),
            },
            alpha: state::BlendChannel {
                equation: state::Equation::Add,
                source: state::Factor::One,
                destination: state::Factor::One,
            },
        }.into()
    ));
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

    // Descriptors
    let heap_srv = factory.create_descriptor_heap(DescriptorHeapType::SrvCbvUav, 16);
    let mut srv_pool = factory.create_descriptor_set_pool(
        &heap_srv,
        1, // sets
        0, // offset
        &[DescriptorPoolDesc { ty: DescriptorType::SampledImage, count: 1 }],
    );

    let set0 = factory.create_descriptor_sets(&mut srv_pool, &[&set0_layout]);

    let heap_sampler = factory.create_descriptor_heap(DescriptorHeapType::Sampler, 16);
    let mut sampler_pool = factory.create_descriptor_set_pool(
        &heap_sampler,
        1, // sets
        0, // offset
        &[DescriptorPoolDesc { ty: DescriptorType::Sampler, count: 1 }],
    );

    let set1 = factory.create_descriptor_sets(&mut sampler_pool, &[&set1_layout]);

    // Framebuffer and render target creation
    let frame_rtvs = swap_chain.get_images().iter().map(|image| {
        factory.view_image_as_render_target(&image, ColorFormat::get_format()).unwrap()
    }).collect::<Vec<_>>();

    let framebuffers = frame_rtvs.iter().map(|frame_rtv| {
        factory.create_framebuffer(&render_pass, &[&frame_rtv], &[], 1024, 768, 1)
    }).collect::<Vec<_>>();


    let upload_heap =
        heap_types.iter().find(|&&heap_type| {
            heap_type.properties.contains(memory::CPU_VISIBLE | memory::COHERENT)
        })
        .unwrap();

    // Buffer allocations
    println!("Memory heaps: {:?}", heap_types);

    let heap = factory.create_heap(upload_heap, 1024);

    let vertex_buffer = {
        let buffer = factory.create_buffer(TRIANGLE.len() as u64 * std::mem::size_of::<Vertex>() as u64, buffer::VERTEX).unwrap();
        println!("{:?}", buffer);
        let buffer_req = factory.get_buffer_requirements(&buffer);
        println!("buffer requirements: {:?}", buffer_req);

        factory.bind_buffer_memory(&heap, 0, buffer).unwrap()
    };

    // TODO: check transitions: read/write mapping and vertex buffer read

    {
        let mut mapping = factory.write_mapping::<Vertex>(&vertex_buffer, 0, TRIANGLE.len() as u64).unwrap();
        mapping.copy_from_slice(&TRIANGLE);
    }

    // Image
    let img_data = include_bytes!("data/logo.png");

    let img = image::load(Cursor::new(&img_data[..]), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = i::Kind::D2(width as i::Size, height as i::Size, i::AaMode::Single);

    let image_upload_heap = factory.create_heap(upload_heap, img.len() as u64);
    let image_upload_buffer = {
        let buffer = factory.create_buffer(img.len() as u64, buffer::TRANSFER_SRC).unwrap();
        let buffer_req = factory.get_buffer_requirements(&buffer);
        factory.bind_buffer_memory(&image_upload_heap, 0, buffer).unwrap()
    };

    // copy image data into staging buffer
    {

        let mut mapping = factory.write_mapping::<u8>(&image_upload_buffer, 0, img.len() as u64).unwrap();
        mapping.copy_from_slice(img.deref());
    }

    let image = factory.create_image(kind, 1, gfx_corell::format::Srgba8::get_format(), i::TRANSFER_DST | i::SAMPLED).unwrap(); // TODO: usage
    let image_req = factory.get_image_requirements(&image);

    println!("image requirements: {:?}", image_req);

    let device_heap = heap_types.iter().find(|&&heap_type| heap_type.properties.contains(memory::DEVICE_LOCAL)).unwrap();
    let image_heap = factory.create_heap(device_heap, image_req.size);

    let image_logo = factory.bind_image_memory(&image_heap, 0, image).unwrap();
    let image_srv = factory.view_image_as_shader_resource(&image_logo, gfx_corell::format::Srgba8::get_format()).unwrap();

    let sampler = factory.create_sampler(i::SamplerInfo::new(
                                            i::FilterMethod::Bilinear,
                                            i::WrapMode::Clamp,
                                        ));

    factory.update_descriptor_sets(&[
        DescriptorSetWrite {
            set: &set0[0],
            binding: 0,
            array_offset: 0,
            write: DescriptorWrite::SampledImage(vec![(&image_srv, memory::ImageLayout::Undefined)]),
        },
        DescriptorSetWrite {
            set: &set1[0],
            binding: 0,
            array_offset: 0,
            write: DescriptorWrite::Sampler(vec![&sampler]),
        },
    ]);

    // Rendering setup
    let viewport = target::Rect {
        x: 0, y: 0,
        w: 1024, h: 768,
    };
    let scissor = target::Rect {
        x: 0, y: 0,
        w: 1024, h: 768,
    };

    let mut frame_semaphore = factory.create_semaphore();
    let mut graphics_pool = back::GraphicsCommandPool::from_queue(&mut general_queues[0], 16);

    // copy buffer to texture
    {
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            let image_barrier = ImageBarrier {
                state_src: ImageStateSrc::State(ImageAccess::empty(), ImageLayout::Undefined),
                state_dst: ImageStateDst::State(memory::TRANSFER_WRITE, ImageLayout::TransferDstOptimal),
                image: &image_logo,
            };
            cmd_buffer.pipeline_barrier(&[], &[], &[image_barrier]);

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                memory::ImageLayout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    image_mip_level: 0,
                    image_base_layer: 0,
                    image_layers: 1,
                    image_offset: command::Offset { x: 0, y: 0, z: 0 },
                    image_extent: command::Extent { width: width, height: height, depth: 1 },
                }]);

            let image_barrier = ImageBarrier {
                state_src: ImageStateSrc::State(memory::TRANSFER_WRITE, ImageLayout::TransferDstOptimal),
                state_dst: ImageStateDst::State(memory::SHADER_READ, ImageLayout::ShaderReadOnlyOptimal),
                image: &image_logo,
            };
            cmd_buffer.pipeline_barrier(&[], &[], &[image_barrier]);

            cmd_buffer.finish()
        };

        general_queues[0].submit_graphics(
            &[
                QueueSubmit {
                    cmd_buffers: &[submit],
                    wait_semaphores: &[],
                    signal_semaphores: &[],
                }
            ],
            None,
        );
    }
    
    //
    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                            .. },
                        ..
                    } | winit::WindowEvent::Closed => running = false,
                    _ => (),
                }
            }
        });

        general_queues[0].wait_idle(); // SLOW!

        graphics_pool.reset();
        let frame = swap_chain.acquire_frame(FrameSync::Semaphore(&mut frame_semaphore));

        // Rendering
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            cmd_buffer.set_viewports(&[viewport]);
            cmd_buffer.set_scissors(&[scissor]);
            cmd_buffer.bind_graphics_pipeline(&pipelines[0].as_ref().unwrap());
            cmd_buffer.bind_vertex_buffers(pso::VertexBufferSet(vec![(&vertex_buffer, 0)]));
            cmd_buffer.bind_descriptor_heaps(Some(&heap_srv), Some(&heap_sampler));
            cmd_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, &[&set0[0], &set1[0]]);

            {
                let mut encoder = back::RenderPassInlineEncoder::begin(
                    &mut cmd_buffer,
                    &render_pass,
                    &framebuffers[frame.id()],
                    target::Rect { x: 0, y: 0, w: 1024, h: 768 },
                    &[command::ClearValue::Color(command::ClearColor::Float([0.8, 0.8, 0.8, 1.0]))]);

                encoder.draw(0, 6, None);
            }

            // TODO: should transition to (_, Present) -> Present (for d3d12)
            
            cmd_buffer.finish()
        };

        general_queues[0].submit_graphics(
            &[
                QueueSubmit {
                    cmd_buffers: &[submit],
                    wait_semaphores: &[(&mut frame_semaphore, pso::BOTTOM_OF_PIPE)],
                    signal_semaphores: &[],
                }
            ],
            None,
        );

        general_queues[0].wait_idle(); // TODO: replace with semaphore

        // present frame
        swap_chain.present();
    }

    // cleanup!
    factory.destroy_descriptor_heap(heap_srv);
    factory.destroy_descriptor_heap(heap_sampler);
    factory.destroy_descriptor_set_pool(srv_pool);
    factory.destroy_descriptor_set_pool(sampler_pool);
    factory.destroy_descriptor_set_layout(set0_layout);
    factory.destroy_descriptor_set_layout(set1_layout);
    factory.destroy_shader_lib(shader_lib);
    factory.destroy_pipeline_layout(pipeline_layout);
    factory.destroy_renderpass(render_pass);
    factory.destroy_heap(heap);
    factory.destroy_heap(image_heap);
    factory.destroy_heap(image_upload_heap);
    factory.destroy_buffer(vertex_buffer);
    factory.destroy_buffer(image_upload_buffer);
    factory.destroy_image(image_logo);
    factory.destroy_shader_resource_view(image_srv);
    factory.destroy_sampler(sampler);
    factory.destroy_semaphore(frame_semaphore);
    for pipeline in pipelines {
        if let Ok(pipeline) = pipeline {
            factory.destroy_graphics_pipeline(pipeline);
        }
    }
    for framebuffer in framebuffers {
        factory.destroy_framebuffer(framebuffer);
    }
    for rtv in frame_rtvs {
        factory.destroy_render_target_view(rtv);
    }
}

#[cfg(not(any(feature = "vulkan", target_os = "windows")))]
fn main() {}
