#![cfg_attr(
    not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

extern crate env_logger;
extern crate gfx_core as core;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "gl")]
extern crate glutin;

extern crate winit;
extern crate image;

use core::{buffer, command, device as d, image as i, memory as m, pass, pso, pool, state};
use core::{Adapter, Device, Instance};
use core::{
    DescriptorPool, Gpu, FrameSync, Primitive, QueueType,
    Backbuffer, Surface, Swapchain, SwapchainConfig,
};
use core::format::{Formatted, Srgba8 as ColorFormat, Vec2};
use core::pass::Subpass;
use core::queue::Submission;
use core::target::Rect;

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

const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: i::ASPECT_COLOR,
    levels: 0 .. 1,
    layers: 0 .. 1,
};

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl"))]
fn main() {
    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
    let wb = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("quad".to_string());
    #[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
    let window = wb
        .build(&events_loop)
        .unwrap();
    #[cfg(feature = "gl")]
    let window = {
        use core::format::ChannelType;

        let color_format = ColorFormat::get_format();
        let color_total_bits = color_format.0.get_total_bits();
        let alpha_bits = color_format.0.get_alpha_stencil_bits();
        let builder = glutin::ContextBuilder::new()
            .with_vsync(true)
            .with_pixel_format(color_total_bits - alpha_bits, alpha_bits)
            .with_srgb(color_format.1 == ChannelType::Srgb);
        glutin::GlWindow::new(wb, builder, &events_loop).unwrap()
    };

    let window_size = window.get_inner_size_pixels().unwrap();
    let pixel_width = window_size.0 as u16;
    let pixel_height = window_size.1 as u16;

    // instantiate backend
    #[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
    let (_instance, adapters, mut surface) = {
        let instance = back::Instance::create("gfx-rs quad", 1);
        let surface = instance.create_surface(&window);
        let adapters = instance.enumerate_adapters();
        (instance, adapters, surface)
    };
    #[cfg(feature = "gl")]
    let (adapters, mut surface) = {
        let surface = back::Surface::from_window(window);
        let adapters = surface.enumerate_adapters();
        (adapters, surface)
    };

    for adapter in &adapters {
        println!("{:?}", adapter.get_info());
    }
    let adapter = &adapters[0];

    // Build a new device and associated command queues
    let Gpu { mut device, mut graphics_queues, memory_types, .. } =
        adapter.open_with(|ref family, qtype| {
            if qtype.supports_graphics() && surface.supports_queue(family) {
                (1, QueueType::Graphics)
            } else {
                (0, QueueType::Transfer)
            }
        });
    let mut queue = graphics_queues.remove(0);
    let swap_config = SwapchainConfig::new()
        .with_color::<ColorFormat>();
    let (mut swap_chain, backbuffer) = surface.build_swapchain(swap_config, &queue);

    // Setup renderpass and pipeline
    #[cfg(any(feature = "vulkan", feature = "dx12"))]
    let vs_module = device
        .create_shader_module(include_bytes!("data/vert.spv"))
        .unwrap();
    #[cfg(any(feature = "vulkan", feature = "dx12"))]
    let fs_module = device
        .create_shader_module(include_bytes!("data/frag.spv"))
        .unwrap();

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

    #[cfg(feature = "gl")]
    let vs_module = device
        .create_shader_module_from_source(
            include_bytes!("shader/quad_450.glslv"),
            pso::Stage::Vertex,
        ).unwrap();
    #[cfg(feature = "gl")]
    let fs_module = device
        .create_shader_module_from_source(
            include_bytes!("shader/quad_450.glslf"),
            pso::Stage::Fragment,
        ).unwrap();

    let set_layout = device.create_descriptor_set_layout(&[
            pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::SampledImage,
                count: 1,
                stage_flags: pso::STAGE_FRAGMENT,
            },
            pso::DescriptorSetLayoutBinding {
                binding: 1,
                ty: pso::DescriptorType::Sampler,
                count: 1,
                stage_flags: pso::STAGE_FRAGMENT,
            },
        ],
    );

    let pipeline_layout = device.create_pipeline_layout(&[&set_layout]);

    let render_pass = {
        let attachment = pass::Attachment {
            format: ColorFormat::get_format(),
            ops: pass::AttachmentOps::new(pass::AttachmentLoadOp::Clear, pass::AttachmentStoreOp::Store),
            stencil_ops: pass::AttachmentOps::DONT_CARE,
            layouts: i::ImageLayout::Undefined .. i::ImageLayout::Present,
        };

        let subpass = pass::SubpassDesc {
            color_attachments: &[(0, i::ImageLayout::ColorAttachmentOptimal)],
            input_attachments: &[],
            preserve_attachments: &[],
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
        #[cfg(any(feature = "vulkan", feature = "dx12", feature = "gl"))]
        let (vs_entry, fs_entry) = (
            pso::EntryPoint { entry: "main", module: &vs_module },
            pso::EntryPoint { entry: "main", module: &fs_module },
        );

        #[cfg(feature = "metal")]
        let (vs_entry, fs_entry) = (
            pso::EntryPoint { entry: "vs_main", module: &shader_lib },
            pso::EntryPoint { entry: "ps_main", module: &shader_lib },
        );

        let shader_entries = pso::GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };
        let subpass = Subpass { index: 0, main_pass: &render_pass };
        device.create_graphics_pipelines(&[
            (shader_entries, &pipeline_layout, subpass, &pipeline_desc)
        ])
    };

    println!("pipelines: {:?}", pipelines);

    // Descriptors
    let mut desc_pool = device.create_descriptor_pool(
        1, // sets
        &[
            pso::DescriptorRangeDesc {
                ty: pso::DescriptorType::SampledImage,
                count: 1,
            },
            pso::DescriptorRangeDesc {
                ty: pso::DescriptorType::Sampler,
                count: 1,
            },
        ],
    );
    let desc_sets = desc_pool.allocate_sets(&[&set_layout]);

    // Framebuffer and render target creation
    let (frame_images, framebuffers) = match backbuffer {
        Backbuffer::Images(images) => {
            let extent = d::Extent { width: pixel_width as _, height: pixel_height as _, depth: 1 };
            let pairs = images
                .into_iter()
                .map(|image| {
                    let rtv = device.create_image_view(&image, ColorFormat::get_format(), COLOR_RANGE.clone()).unwrap();
                    (image, rtv)
                })
                .collect::<Vec<_>>();
            let fbos = pairs
                .iter()
                .map(|&(_, ref rtv)| {
                    device.create_framebuffer(&render_pass, &[rtv], extent).unwrap()
                })
                .collect();
            (pairs, fbos)
        }
        Backbuffer::Framebuffer(fbo) => {
            (Vec::new(), vec![fbo])
        }
    };

    // Buffer allocations
    println!("Memory types: {:?}", memory_types);

    let buffer_stride = std::mem::size_of::<Vertex>() as u64;
    let buffer_len = QUAD.len() as u64 * buffer_stride;

    let buffer_unbound = device.create_buffer(buffer_len, buffer_stride, buffer::VERTEX).unwrap();
    println!("{:?}", buffer_unbound);
    let buffer_req = device.get_buffer_requirements(&buffer_unbound);

    let upload_type =
        memory_types.iter().find(|mem_type| {
            buffer_req.type_mask & (1 << mem_type.id) != 0 &&
            mem_type.properties.contains(m::CPU_VISIBLE)
            //&& !mem_type.properties.contains(m::CPU_CACHED)
        })
        .unwrap();

    let buffer_memory = device.allocate_memory(upload_type, 1024).unwrap();
    let vertex_buffer = device.bind_buffer_memory(&buffer_memory, 0, buffer_unbound).unwrap();

    // TODO: check transitions: read/write mapping and vertex buffer read
    {
        let mut vertices = device
            .acquire_mapping_writer::<Vertex>(&vertex_buffer, 0..buffer_len)
            .unwrap();
        vertices.copy_from_slice(&QUAD);
        device.release_mapping_writer(vertices);
    }

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

    let image_upload_memory = device.allocate_memory(upload_type, upload_size).unwrap();
    let image_upload_buffer = {
        let buffer = device.create_buffer(upload_size, image_stride as u64, buffer::TRANSFER_SRC).unwrap();
        device.bind_buffer_memory(&image_upload_memory, 0, buffer).unwrap()
    };

    // copy image data into staging buffer
    {
        let mut data = device
            .acquire_mapping_writer::<u8>(&image_upload_buffer, 0..upload_size)
            .unwrap();
        for y in 0 .. height as usize {
            let row = &(*img)[y*(width as usize)*image_stride .. (y+1)*(width as usize)*image_stride];
            let dest_base = y * row_pitch as usize;
            data[dest_base .. dest_base + row.len()].copy_from_slice(row);
        }
        device.release_mapping_writer(data);
    }

    let image_unbound = device.create_image(kind, 1, ColorFormat::get_format(), i::TRANSFER_DST | i::SAMPLED).unwrap(); // TODO: usage
    println!("{:?}", image_unbound);
    let image_req = device.get_image_requirements(&image_unbound);

    let device_type = memory_types
        .iter()
        .find(|memory_type| {
            image_req.type_mask & (1 << memory_type.id) != 0 &&
            memory_type.properties.contains(m::DEVICE_LOCAL)
        })
        .unwrap();
    let image_memory = device.allocate_memory(device_type, image_req.size).unwrap();

    let image_logo = device.bind_image_memory(&image_memory, 0, image_unbound).unwrap();
    let image_srv = device.create_image_view(&image_logo, ColorFormat::get_format(), COLOR_RANGE.clone()).unwrap();

    let sampler = device.create_sampler(
        i::SamplerInfo::new(
            i::FilterMethod::Bilinear,
            i::WrapMode::Clamp,
        )
    );

    device.update_descriptor_sets(&[
        pso::DescriptorSetWrite {
            set: &desc_sets[0],
            binding: 0,
            array_offset: 0,
            write: pso::DescriptorWrite::SampledImage(vec![(&image_srv, i::ImageLayout::Undefined)]),
        },
        pso::DescriptorSetWrite {
            set: &desc_sets[0],
            binding: 1,
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
    let mut graphics_pool = queue.create_graphics_pool(16, pool::CommandPoolCreateFlags::empty());

    // copy buffer to texture
    {
        let submit = {
            let mut cmd_buffer = graphics_pool.acquire_command_buffer();

            let image_barrier = m::Barrier::Image {
                states: (i::Access::empty(), i::ImageLayout::Undefined) ..
                        (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                target: &image_logo,
                range: COLOR_RANGE.clone(),
            };
            cmd_buffer.pipeline_barrier(pso::TOP_OF_PIPE .. pso::TRANSFER, &[image_barrier]);

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                i::ImageLayout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_pitch: row_pitch,
                    buffer_slice_pitch: row_pitch * (height as u32),
                    image_range: COLOR_RANGE.clone(),
                    image_offset: command::Offset { x: 0, y: 0, z: 0 },
                    image_extent: d::Extent { width, height, depth: 1 },
                }]);

            let image_barrier = m::Barrier::Image {
                states: (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal) ..
                        (i::SHADER_READ, i::ImageLayout::ShaderReadOnlyOptimal),
                target: &image_logo,
                range: COLOR_RANGE.clone(),
            };
            cmd_buffer.pipeline_barrier(pso::TRANSFER .. pso::BOTTOM_OF_PIPE, &[image_barrier]);

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

            cmd_buffer.set_viewports(&[viewport]);
            cmd_buffer.set_scissors(&[scissor]);
            cmd_buffer.bind_graphics_pipeline(&pipelines[0].as_ref().unwrap());
            cmd_buffer.bind_vertex_buffers(pso::VertexBufferSet(vec![(&vertex_buffer, 0)]));
            cmd_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, &[&desc_sets[0]]); //TODO

            {
                let mut encoder = cmd_buffer.begin_renderpass_inline(
                    &render_pass,
                    &framebuffers[frame.id()],
                    Rect { x: 0, y: 0, w: pixel_width, h: pixel_height },
                    &[command::ClearValue::Color(command::ClearColor::Float([0.8, 0.8, 0.8, 1.0]))],
                );
                encoder.draw(0..6, 0..1);
            }

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
    device.destroy_descriptor_pool(desc_pool);
    device.destroy_descriptor_set_layout(set_layout);

    #[cfg(any(feature = "vulkan", feature = "dx12", feature = "gl"))]
    {
        device.destroy_shader_module(vs_module);
        device.destroy_shader_module(fs_module);
    }
    #[cfg(feature = "metal")]
    device.destroy_shader_module(shader_lib);

    device.destroy_buffer(vertex_buffer);
    device.destroy_buffer(image_upload_buffer);
    device.destroy_image(image_logo);
    device.destroy_image_view(image_srv);
    device.destroy_sampler(sampler);
    device.destroy_fence(frame_fence);
    device.destroy_semaphore(frame_semaphore);
    device.destroy_pipeline_layout(pipeline_layout);
    device.destroy_renderpass(render_pass);
    device.free_memory(buffer_memory);
    device.free_memory(image_memory);
    device.free_memory(image_upload_memory);
    for pipeline in pipelines {
        if let Ok(pipeline) = pipeline {
            device.destroy_graphics_pipeline(pipeline);
        }
    }
    for framebuffer in framebuffers {
        device.destroy_framebuffer(framebuffer);
    }
    for (image, rtv) in frame_images {
        device.destroy_image_view(rtv);
        device.destroy_image(image);
    }
}

#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")))]
fn main() {
    println!("You need to enable the native API feature (vulkan/metal) in order to test the LL");
}
