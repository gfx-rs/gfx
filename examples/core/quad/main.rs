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
extern crate gfx_core as core;
#[cfg(feature = "dx12")]
extern crate gfx_device_dx12 as back;
#[cfg(feature = "vulkan")]
extern crate gfx_device_vulkan as back;
#[cfg(feature = "vulkan")]
extern crate gfx_window_vulkan as win;
#[cfg(feature = "metal")]
extern crate gfx_device_metal as back;

extern crate winit;
extern crate image;

use core::{buffer, command, device as d, image as i, memory as m, pass, pso, shade, state};
use core::{Adapter, Device, QueueFamily, SwapChain, WindowExt};
use core::{DescriptorPool, Gpu, FrameSync, Primitive, SubPass, Surface, SwapchainConfig};
use core::format::{Formatted, Srgba8 as ColorFormat, Vec2};
use core::queue::Submission;
use core::target::Rect;

use std::io::Cursor;


const VS: &str = "vs_main";
const PS: &str = "ps_main";

#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
struct Vertex {
    a_Pos: [f32; 2],
    a_Uv: [f32; 2],
}

const QUAD: [Vertex; 6] = [
    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5, 0.33 ], a_Uv: [1.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },

    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },
    Vertex { a_Pos: [ -0.5,-0.33 ], a_Uv: [0.0, 0.0] },
];

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
fn main() {
    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("quad".to_string())
        .build(&events_loop)
        .unwrap();
    let window_size = window.get_inner_size_pixels().unwrap();
    let pixel_width = window_size.0 as u16;
    let pixel_height = window_size.1 as u16;

    // instantiate backend
    let mut vk_window = win::Window(window);
    let (mut surface, adapters) = vk_window.get_surface_and_adapters();
    for adapter in &adapters {
        println!("{:?}", adapter.get_info());
    }
    let adapter = &adapters[0];
    let queue_descs = adapter.get_queue_families()
        .iter()
        .map(|&(ref family, qtype)| (family, qtype, family.num_queues()) )
        .collect::<Vec<_>>();

    // Build a new device and associated command queues
    let Gpu { mut device, mut general_queues, heap_types, .. } = adapter.open(&queue_descs);
    let mut queue = general_queues.remove(0);
    let swap_config = SwapchainConfig::new()
        .with_color::<ColorFormat>();
    let mut swap_chain = surface.build_swapchain(swap_config, &queue);

    // Setup renderpass and pipeline
    // dx12 runtime shader compilation
    #[cfg(feature = "dx12")]
    let shader_lib = device.create_shader_library_from_source(&[
            (VS, shade::Stage::Vertex, include_bytes!("shader/quad.hlsl")),
            (PS, shade::Stage::Pixel, include_bytes!("shader/quad.hlsl")),
        ]).expect("Error on creating shader lib");
    #[cfg(feature = "vulkan")]
    let shader_lib = device.create_shader_library(&[
            (VS, include_bytes!("data/vs_main.spv")),
            (PS, include_bytes!("data/ps_main.spv")),
        ]).expect("Error on creating shader lib");
    #[cfg(all(feature = "metal", feature = "metal_argument_buffer"))]
    let shader_lib = device.create_shader_library_from_source(
            include_str!("shader/quad_indirect.metal"),
            back::LanguageVersion::new(2, 0),
        ).expect("Error on creating shader lib");
    #[cfg(all(feature = "metal", not(feature = "metal_argument_buffer")))]
    let shader_lib = device.create_shader_library_from_source(
            include_str!("shader/quad.metal"),
            back::LanguageVersion::new(1, 1),
        ).expect("Error on creating shader lib");

    let shader_entries = pso::GraphicsShaderSet {
        vertex_shader: VS,
        hull_shader: None,
        domain_shader: None,
        geometry_shader: None,
        pixel_shader: Some(PS),
    };

    let set0_layout = device.create_descriptor_set_layout(&[
            pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::SampledImage,
                count: 1,
                stage_flags: shade::STAGE_PIXEL,
            }
        ],
    );

    let set1_layout = device.create_descriptor_set_layout(&[
            pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::Sampler,
                count: 1,
                stage_flags: shade::STAGE_PIXEL,
            }
        ],
    );

    let pipeline_layout = device.create_pipeline_layout(&[&set0_layout, &set1_layout]);

    let render_pass = {
        let attachment = pass::Attachment {
            format: ColorFormat::get_format(),
            load_op: pass::AttachmentLoadOp::Clear,
            store_op: pass::AttachmentStoreOp::Store,
            stencil_load_op: pass::AttachmentLoadOp::DontCare,
            stencil_store_op: pass::AttachmentStoreOp::DontCare,
            src_layout: i::ImageLayout::Undefined, // TODO: maybe Option<_> here?
            dst_layout: i::ImageLayout::Present,
        };

        let subpass = pass::SubpassDesc {
            color_attachments: &[(0, i::ImageLayout::ColorAttachmentOptimal)],
        };

        let dependency = pass::SubpassDependency {
            src_pass: pass::SubpassRef::External,
            dst_pass: pass::SubpassRef::Pass(0),
            src_stage: pso::COLOR_ATTACHMENT_OUTPUT,
            dst_stage: pso::COLOR_ATTACHMENT_OUTPUT,
            src_access: i::Access::empty(),
            dst_access: i::COLOR_ATTACHMENT_READ | i::COLOR_ATTACHMENT_WRITE,
        };

        device.create_renderpass(&[attachment], &[subpass], &[dependency])
    };

    //
    let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
        Primitive::TriangleList,
        pso::Rasterizer::new_fill(),
        shader_entries,
    );
    pipeline_desc.blender.targets.push(pso::ColorInfo {
        mask: state::MASK_ALL,
        color: Some(state::BlendChannel {
            equation: state::Equation::Add,
            source: state::Factor::ZeroPlus(state::BlendValue::SourceAlpha),
            destination: state::Factor::OneMinus(state::BlendValue::SourceAlpha),
        }),
        alpha: Some(state::BlendChannel {
            equation: state::Equation::Add,
            source: state::Factor::One,
            destination: state::Factor::One,
        }),
    });
    pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
        stride: std::mem::size_of::<Vertex>() as u32,
        rate: 0,
    });

    pipeline_desc.attributes.push(pso::AttributeDesc {
        location: 1,
        binding: 0,
        element: pso::Element {
            format: <Vec2<f32> as Formatted>::get_format(),
            offset: 0,
        },
    });
    pipeline_desc.attributes.push(pso::AttributeDesc {
        location: 0,
        binding: 0,
        element: pso::Element {
            format: <Vec2<f32> as Formatted>::get_format(),
            offset: 8
        },
    });

    //
    let pipelines = device.create_graphics_pipelines(&[
        (&shader_lib, &pipeline_layout, SubPass { index: 0, main_pass: &render_pass }, &pipeline_desc)
    ]);

    println!("pipelines: {:?}", pipelines);

    // Descriptors
    let mut srv_pool = device.create_descriptor_pool(
        1, // sets
        &[pso::DescriptorRangeDesc { ty: pso::DescriptorType::SampledImage, count: 1 }],
    );
    let set0 = srv_pool.allocate_sets(&[&set0_layout]);

    let mut sampler_pool = device.create_descriptor_pool(
        1, // sets
        &[pso::DescriptorRangeDesc { ty: pso::DescriptorType::Sampler, count: 1 }],
    );
    let set1 = sampler_pool.allocate_sets(&[&set1_layout]);

    // Framebuffer and render target creation
    let frame_rtvs = swap_chain.get_backbuffers().iter().map(|bb| {
        device.view_image_as_render_target(&bb.color, ColorFormat::get_format(), (0..1, 0..1)).unwrap()
    }).collect::<Vec<_>>();

    let framebuffers = frame_rtvs.iter().map(|frame_rtv| {
        device.create_framebuffer(&render_pass, &[&frame_rtv], &[], pixel_width as u32, pixel_height as u32, 1)
    }).collect::<Vec<_>>();


    let upload_heap =
        heap_types.iter().find(|&&heap_type| {
            heap_type.properties.contains(m::CPU_VISIBLE | m::COHERENT)
        })
        .unwrap();

    // Buffer allocations
    println!("Memory heaps: {:?}", heap_types);

    let heap = device.create_heap(upload_heap, d::ResourceHeapType::Buffers, 1024).unwrap();
    let buffer_stride = std::mem::size_of::<Vertex>() as u64;
    let buffer_len = QUAD.len() as u64 * buffer_stride;

    let vertex_buffer = {
        let buffer = device.create_buffer(buffer_len, buffer_stride, buffer::VERTEX).unwrap();
        println!("{:?}", buffer);
        device.bind_buffer_memory(&heap, 0, buffer).unwrap()
    };

    // TODO: check transitions: read/write mapping and vertex buffer read
    device.write_mapping::<Vertex>(&vertex_buffer, 0, buffer_len)
          .unwrap()
          .copy_from_slice(&QUAD);

    // Image
    let img_data = include_bytes!("data/logo.png");

    let img = image::load(Cursor::new(&img_data[..]), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = i::Kind::D2(width as i::Size, height as i::Size, i::AaMode::Single);
    let row_alignment_mask = device.get_limits().min_buffer_copy_pitch_alignment as u32 - 1;
    let image_stride = 4usize;
    let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
    let upload_size = (height * row_pitch) as u64;
    println!("upload row pitch {}, total size {}", row_pitch, upload_size);

    let image_upload_heap = device.create_heap(upload_heap, d::ResourceHeapType::Buffers, upload_size).unwrap();
    let image_upload_buffer = {
        let buffer = device.create_buffer(upload_size, image_stride as u64, buffer::TRANSFER_SRC).unwrap();
        device.bind_buffer_memory(&image_upload_heap, 0, buffer).unwrap()
    };

    // copy image data into staging buffer
    {
        let mut mapping = device.write_mapping::<u8>(&image_upload_buffer, 0, upload_size).unwrap();
        for y in 0 .. height as usize {
            let row = &(*img)[y*(width as usize)*image_stride .. (y+1)*(width as usize)*image_stride];
            let dest_base = y * row_pitch as usize;
            mapping[dest_base .. dest_base + row.len()].copy_from_slice(row);
        }
    }

    let image = device.create_image(kind, 1, ColorFormat::get_format(), i::TRANSFER_DST | i::SAMPLED).unwrap(); // TODO: usage
    println!("{:?}", image);
    let image_req = device.get_image_requirements(&image);

    let device_heap = heap_types.iter().find(|&&heap_type| heap_type.properties.contains(m::DEVICE_LOCAL)).unwrap();
    let image_heap = device.create_heap(device_heap, d::ResourceHeapType::Images, image_req.size).unwrap();

    let image_logo = device.bind_image_memory(&image_heap, 0, image).unwrap();
    let image_srv = device.view_image_as_shader_resource(&image_logo, ColorFormat::get_format()).unwrap();

    let sampler = device.create_sampler(
        i::SamplerInfo::new(
            i::FilterMethod::Bilinear,
            i::WrapMode::Clamp,
        )
    );

    device.update_descriptor_sets(&[
        pso::DescriptorSetWrite {
            set: &set0[0],
            binding: 0,
            array_offset: 0,
            write: pso::DescriptorWrite::SampledImage(vec![(&image_srv, i::ImageLayout::Undefined)]),
        },
        pso::DescriptorSetWrite {
            set: &set1[0],
            binding: 0,
            array_offset: 0,
            write: pso::DescriptorWrite::Sampler(vec![&sampler]),
        },
    ]);

    // Rendering setup
    let viewport = core::Viewport {
        x: 0, y: 0,
        w: pixel_width, h: pixel_height,
        near: 0.0, far: 1.0,
    };
    let scissor = Rect {
        x: 0, y: 0,
        w: pixel_width, h: pixel_height,
    };

    let mut frame_semaphore = device.create_semaphore();
    let mut frame_fence = device.create_fence(false); // TODO: remove
    let mut graphics_pool = queue.create_graphics_pool(16);

    // copy buffer to texture
    {
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            let image_barrier = m::Barrier::Image {
                state_src: (i::Access::empty(), i::ImageLayout::Undefined),
                state_dst: (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                target: &image_logo,
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(&[image_barrier]);

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                i::ImageLayout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_pitch: row_pitch,
                    buffer_slice_pitch: row_pitch * (height as u32),
                    image_aspect: i::ASPECT_COLOR,
                    image_subresource: (0, 0..1),
                    image_offset: command::Offset { x: 0, y: 0, z: 0 },
                    image_extent: command::Extent { width, height, depth: 1 },
                }]);

            let image_barrier = m::Barrier::Image {
                state_src: (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                state_dst: (i::SHADER_READ, i::ImageLayout::ShaderReadOnlyOptimal),
                target: &image_logo,
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(&[image_barrier]);

            cmd_buffer.finish()
        };

        let submission = Submission::new()
            .submit(&[submit]);
        queue.submit(submission, Some(&mut frame_fence));

        device.wait_for_fences(&[&frame_fence], d::WaitFor::All, !0);
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

        device.reset_fences(&[&frame_fence]);
        graphics_pool.reset();
        let frame = swap_chain.acquire_frame(FrameSync::Semaphore(&mut frame_semaphore));

        // Rendering
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            let rtv = &swap_chain.get_backbuffers()[frame.id()].color;
            let rtv_target_barrier = m::Barrier::Image {
                state_src: (i::Access::empty(), i::ImageLayout::Undefined),
                state_dst: (i::COLOR_ATTACHMENT_WRITE, i::ImageLayout::ColorAttachmentOptimal),
                target: rtv,
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(&[rtv_target_barrier]);

            cmd_buffer.set_viewports(&[viewport]);
            cmd_buffer.set_scissors(&[scissor]);
            cmd_buffer.bind_graphics_pipeline(&pipelines[0].as_ref().unwrap());
            cmd_buffer.bind_vertex_buffers(pso::VertexBufferSet(vec![(&vertex_buffer, 0)]));
            cmd_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, &[&set0[0], &set1[0]]); //TODO

            {
                let mut encoder = cmd_buffer.begin_renderpass_inline(
                    &render_pass,
                    &framebuffers[frame.id()],
                    Rect { x: 0, y: 0, w: pixel_width, h: pixel_height },
                    &[command::ClearValue::Color(command::ClearColor::Float([0.8, 0.8, 0.8, 1.0]))],
                );
                encoder.draw(0, 6, None);
            }

            let rtv_present_barrier = m::Barrier::Image {
                state_src: (i::COLOR_ATTACHMENT_WRITE, i::ImageLayout::ColorAttachmentOptimal),
                state_dst: (i::Access::empty(), i::ImageLayout::Present),
                target: rtv,
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(&[rtv_present_barrier]);

            cmd_buffer.finish()
        };

        let submission = Submission::new()
            .wait_on(&[(&mut frame_semaphore, pso::BOTTOM_OF_PIPE)])
            .submit(&[submit]);
        queue.submit(submission, Some(&mut frame_fence));

        // TODO: replace with semaphore
        device.wait_for_fences(&[&frame_fence], d::WaitFor::All, !0);

        // present frame
        swap_chain.present(&mut queue, &[]);
    }

    // cleanup!
    device.destroy_descriptor_pool(srv_pool);
    device.destroy_descriptor_pool(sampler_pool);
    device.destroy_descriptor_set_layout(set0_layout);
    device.destroy_descriptor_set_layout(set1_layout);
    device.destroy_shader_lib(shader_lib);
    device.destroy_pipeline_layout(pipeline_layout);
    device.destroy_renderpass(render_pass);
    device.destroy_heap(heap);
    device.destroy_heap(image_heap);
    device.destroy_heap(image_upload_heap);
    device.destroy_buffer(vertex_buffer);
    device.destroy_buffer(image_upload_buffer);
    device.destroy_image(image_logo);
    device.destroy_shader_resource_view(image_srv);
    device.destroy_sampler(sampler);
    device.destroy_fence(frame_fence);
    device.destroy_semaphore(frame_semaphore);
    for pipeline in pipelines {
        if let Ok(pipeline) = pipeline {
            device.destroy_graphics_pipeline(pipeline);
        }
    }
    for framebuffer in framebuffers {
        device.destroy_framebuffer(framebuffer);
    }
    for rtv in frame_rtvs {
        device.destroy_render_target_view(rtv);
    }
}

#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal")))]
fn main() {
    println!("You need to enable the native API feature (vulkan/metal) in order to test the LL");
}
