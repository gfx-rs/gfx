extern crate env_logger;
extern crate gfx;
extern crate gfx_core as core;
extern crate gfx_backend_vulkan as back;

extern crate winit;
extern crate image;

use core::{command, device as d, image as i, memory as m, pass, pso, pool, state};
use core::{Adapter, Device, Instance};
use core::{DescriptorPool, Primitive};
use core::format::{Formatted, Srgba8 as ColorFormat, Vec2};
use core::pass::Subpass;
use core::queue::Submission;
use core::target::Rect;
use gfx::allocators::StackAllocator as Allocator;

use std::io::Cursor;

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
    let instance = back::Instance::create("gfx-rs quad", 1);
    let surface = instance.create_surface(&window);
    let adapters = instance.enumerate_adapters();
    for adapter in &adapters {
        println!("{:?}", adapter.get_info());
    }
    let adapter = &adapters[0];

    type Context<C> = gfx::Context<back::Backend, C>;
    let (mut context, backbuffers) =
        Context::init_graphics::<ColorFormat>(surface, adapter);

    let mut render_device = (*context.ref_device()).clone();
    let device: &mut back::Device = render_device.mut_raw();

    // Setup renderpass and pipeline
    let vs_module = device.create_shader_module(include_bytes!("data/vs_main.spv")).unwrap();
    let fs_module = device.create_shader_module(include_bytes!("data/ps_main.spv")).unwrap();

    let set0_layout = device.create_descriptor_set_layout(&[
            pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::SampledImage,
                count: 1,
                stage_flags: pso::STAGE_FRAGMENT,
            }
        ],
    );

    let set1_layout = device.create_descriptor_set_layout(&[
            pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::Sampler,
                count: 1,
                stage_flags: pso::STAGE_FRAGMENT,
            }
        ],
    );

    let pipeline_layout = device.create_pipeline_layout(&[&set0_layout, &set1_layout]);

    let render_pass = {
        let attachment = pass::Attachment {
            format: ColorFormat::get_format(),
            ops: pass::AttachmentOps::new(pass::AttachmentLoadOp::Clear, pass::AttachmentStoreOp::Store),
            stencil_ops: pass::AttachmentOps::DONT_CARE,
            layouts: i::ImageLayout::Undefined .. i::ImageLayout::Present,
        };

        let subpass = pass::SubpassDesc {
            color_attachments: &[(0, i::ImageLayout::ColorAttachmentOptimal)],
        };

        let dependency = pass::SubpassDependency {
            passes: pass::SubpassRef::External .. pass::SubpassRef::Pass(0),
            stages: pso::COLOR_ATTACHMENT_OUTPUT .. pso::COLOR_ATTACHMENT_OUTPUT,
            accesses: i::Access::empty() .. (i::COLOR_ATTACHMENT_READ | i::COLOR_ATTACHMENT_WRITE),
        };

        device.create_renderpass(&[attachment], &[subpass], &[dependency])
    };

    //
    let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
        Primitive::TriangleList,
        pso::Rasterizer::new_fill(),
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
        location: 0,
        binding: 0,
        element: pso::Element {
            format: <Vec2<f32> as Formatted>::get_format(),
            offset: 0,
        },
    });
    pipeline_desc.attributes.push(pso::AttributeDesc {
        location: 1,
        binding: 0,
        element: pso::Element {
            format: <Vec2<f32> as Formatted>::get_format(),
            offset: 8
        },
    });

    //
    let pipelines = {
        let shader_entries = pso::GraphicsShaderSet {
            vertex: pso::EntryPoint { entry: "main", module: &vs_module },
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(pso::EntryPoint { entry: "main", module: &fs_module },),
        };
        let subpass = Subpass { index: 0, main_pass: &render_pass };
        device.create_graphics_pipelines(&[
            (shader_entries, &pipeline_layout, subpass, &pipeline_desc)
        ])
    };

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

    // Framebuffer creation
    let framebuffers = backbuffers.iter().map(|backbuffer| {
        let frame_rtv = backbuffer.color.resource();
        let extent = d::Extent { width: pixel_width as _, height: pixel_height as _, depth: 1 };
        device.create_framebuffer(&render_pass, &[frame_rtv], &[], extent)
    }).collect::<Vec<_>>();

    let mut upload = Allocator::new(
        gfx::memory::Usage::Upload,
        &context.ref_device());
    let mut data = Allocator::new(
        gfx::memory::Usage::Data,
        &context.ref_device());
    println!("Memory types: {:?}", context.ref_device().heap_types());
    println!("Memory heaps: {:?}", context.ref_device().memory_heaps());

    let vertex_count = QUAD.len() as u64;
    let vertex_buffer = context.mut_device().create_buffer::<Vertex, _>(
        &mut upload,
        gfx::buffer::VERTEX,
        vertex_count
    ).unwrap();

    context.mut_device()
        .write_mapping(&vertex_buffer, 0..vertex_count)
        .unwrap()
        .copy_from_slice(&QUAD);

    let img_data = include_bytes!("../../core/quad/data/logo.png");
    let img = image::load(Cursor::new(&img_data[..]), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = i::Kind::D2(width as i::Size, height as i::Size, i::AaMode::Single);
    let row_alignment_mask = device.get_limits().min_buffer_copy_pitch_alignment as u32 - 1;
    let image_stride = 4usize;
    let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
    let upload_size = (height * row_pitch) as u64;
    println!("upload row pitch {}, total size {}", row_pitch, upload_size);

    let image_upload_buffer = context.mut_device().create_buffer_raw(
        &mut upload,
        gfx::buffer::TRANSFER_SRC,
        upload_size,
        image_stride as u64
    ).unwrap();

    println!("copy image data into staging buffer");

    if let Ok(mut image_data) = context.mut_device()
        .write_mapping(&image_upload_buffer, 0..upload_size)
    {
        for y in 0 .. height as usize {
            let row = &(*img)[y*(width as usize)*image_stride .. (y+1)*(width as usize)*image_stride];
            let dest_base = y * row_pitch as usize;
            image_data[dest_base .. dest_base + row.len()].copy_from_slice(row);
        }
    }

    let image = context.mut_device().create_image::<ColorFormat, _>(
        &mut data,
        gfx::image::TRANSFER_DST | gfx::image::SAMPLED,
        kind,
        1,
    ).unwrap();

    let image_srv = context.mut_device()
        .view_image_as_shader_resource(&image)
        .unwrap();

    let sampler = context.mut_device().create_sampler(
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
            write: pso::DescriptorWrite::SampledImage(vec![(image_srv.resource(), i::ImageLayout::Undefined)]),
        },
        pso::DescriptorSetWrite {
            set: &set1[0],
            binding: 0,
            array_offset: 0,
            write: pso::DescriptorWrite::Sampler(vec![sampler.resource()]),
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

    let mut fence = device.create_fence(false);
    let mut graphics_pool = context.mut_queue()
        .create_graphics_pool(16, pool::CommandPoolCreateFlags::empty());

    println!("copy buffer to texture");
    {
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            let image_barrier = m::Barrier::Image {
                states: (i::Access::empty(), i::ImageLayout::Undefined) ..
                        (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                target: image.resource(),
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(pso::TOP_OF_PIPE .. pso::TRANSFER, &[image_barrier]);

            cmd_buffer.copy_buffer_to_image(
                image_upload_buffer.resource(),
                image.resource(),
                i::ImageLayout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_pitch: row_pitch,
                    buffer_slice_pitch: row_pitch * (height as u32),
                    image_aspect: i::ASPECT_COLOR,
                    image_subresource: (0, 0..1),
                    image_offset: command::Offset { x: 0, y: 0, z: 0 },
                    image_extent: d::Extent { width, height, depth: 1 },
                }]);

            let image_barrier = m::Barrier::Image {
                states: (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal) ..
                        (i::SHADER_READ, i::ImageLayout::ShaderReadOnlyOptimal),
                target: image.resource(),
                range: (0..1, 0..1),
            };
            cmd_buffer.pipeline_barrier(pso::TRANSFER .. pso::BOTTOM_OF_PIPE, &[image_barrier]);

            cmd_buffer.finish()
        };

        let submission = Submission::new()
            .submit(&[submit]);
        context.mut_queue().submit(submission, Some(&mut fence));

        device.wait_for_fences(&[&fence], d::WaitFor::All, !0);
    }

    device.destroy_fence(fence);

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

        let frame = context.acquire_frame();
        let mut encoder_pool = context.acquire_encoder_pool();

        // Rendering
        let submit = {
            let mut encoder = encoder_pool.acquire_encoder();
            {
            let cmd_buffer = encoder.mut_buffer();

            cmd_buffer.set_viewports(&[viewport]);
            cmd_buffer.set_scissors(&[scissor]);
            cmd_buffer.bind_graphics_pipeline(&pipelines[0].as_ref().unwrap());
            cmd_buffer.bind_vertex_buffers(pso::VertexBufferSet(vec![(vertex_buffer.resource(), 0)]));
            cmd_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, &[&set0[0], &set1[0]]); //TODO

            {
                let mut encoder = cmd_buffer.begin_renderpass_inline(
                    &render_pass,
                    &framebuffers[frame.id()],
                    Rect { x: 0, y: 0, w: pixel_width, h: pixel_height },
                    &[command::ClearValue::Color(command::ClearColor::Float([0.8, 0.8, 0.8, 1.0]))],
                );
                encoder.draw(0..6, 0..1);
            }

            }
            encoder.finish()
        };

        context.present(vec![submit]);
    }

    println!("cleanup!");
    device.destroy_descriptor_pool(srv_pool);
    device.destroy_descriptor_pool(sampler_pool);
    device.destroy_descriptor_set_layout(set0_layout);
    device.destroy_descriptor_set_layout(set1_layout);
    device.destroy_shader_module(vs_module);
    device.destroy_shader_module(fs_module);
    device.destroy_pipeline_layout(pipeline_layout);
    device.destroy_renderpass(render_pass);
    for pipeline in pipelines {
        if let Ok(pipeline) = pipeline {
            device.destroy_graphics_pipeline(pipeline);
        }
    }
    for framebuffer in framebuffers {
        device.destroy_framebuffer(framebuffer);
    }
}
