extern crate env_logger;
#[macro_use]
extern crate gfx_render as gfx;
extern crate gfx_core as core;
extern crate gfx_backend_vulkan as back;

extern crate winit;
extern crate image;

use std::io::Cursor;

use core::{command, device as d, image as i, pso, state};
use core::{Adapter, Device, Instance, Primitive};
use gfx::format::{Srgba8 as ColorFormat};
use core::target::Rect;
use gfx::allocators::StackAllocator as Allocator;

gfx_buffer_struct! {
    Vertex {
        a_Pos: [f32; 2],
        a_Uv: [f32; 2],
    }
}

const QUAD: [Vertex; 6] = [
    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5, 0.33 ], a_Uv: [1.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },

    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },
    Vertex { a_Pos: [ -0.5,-0.33 ], a_Uv: [0.0, 0.0] },
];

gfx_descriptors! {
    desc {
        sampled_image: gfx::pso::SampledImage,
        sampler: gfx::pso::Sampler,
    }
}

gfx_graphics_pipeline! {
    pipe {
        desc: desc::Component,
        color: gfx::pso::RenderTarget<ColorFormat>,
        vertices: gfx::pso::VertexBuffer<Vertex>,
    }
}

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
    let mut device = (*context.ref_device()).clone();

    // Setup renderpass and pipeline
    let vs_module = device.mut_raw().create_shader_module(include_bytes!("data/vs_main.spv")).unwrap();
    let fs_module = device.mut_raw().create_shader_module(include_bytes!("data/ps_main.spv")).unwrap();

    let (desc, mut desc_data) = device.create_descriptors(1).pop().unwrap();
    let pipe_init = pipe::Init {
        desc: &desc,
        color: pso::ColorInfo {
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
        },
        vertices: (),
    };
    let pipeline = device.create_graphics_pipeline(
        pso::GraphicsShaderSet {
            vertex: pso::EntryPoint { entry: "main", module: &vs_module },
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(pso::EntryPoint { entry: "main", module: &fs_module },),
        },
        Primitive::TriangleList,
        pso::Rasterizer::new_fill(),
        pipe_init,
    ).unwrap();

    // Framebuffer creation
    let frame_rtvs = backbuffers.iter().map(|backbuffer| {
        device.view_image_as_render_target(&backbuffer.color, (0, 0..1))
            .unwrap()
    }).collect::<Vec<_>>();
    let framebuffers = frame_rtvs.iter().map(|rtv| {
        let extent = d::Extent { width: pixel_width as _, height: pixel_height as _, depth: 1 };
        device.create_framebuffer(&pipeline, &[&rtv], &[], extent)
            .unwrap()
    }).collect::<Vec<_>>();

    let mut upload = Allocator::new(
        gfx::memory::Usage::Upload,
        &context.ref_device());
    let mut data = Allocator::new(
        gfx::memory::Usage::Data,
        &context.ref_device());
    println!("Memory types: {:?}", context.ref_device().memory_types());
    println!("Memory heaps: {:?}", context.ref_device().memory_heaps());

    let mut init_tokens = Vec::new();
    let vertex_count = QUAD.len() as u64;
    let (vertex_buffer, token) = device.create_buffer::<Vertex, _>(
        &mut upload,
        gfx::buffer::VERTEX,
        vertex_count
    ).unwrap();
    init_tokens.push(token);

    device.write_mapping(&vertex_buffer, 0..vertex_count)
        .unwrap()
        .copy_from_slice(&QUAD);

    let img_data = include_bytes!("../../core/quad/data/logo.png");
    let img = image::load(Cursor::new(&img_data[..]), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = i::Kind::D2(width as i::Size, height as i::Size, i::AaMode::Single);
    let row_alignment_mask = device.ref_raw().get_limits().min_buffer_copy_pitch_alignment as u32 - 1;
    let image_stride = 4usize;
    let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
    let upload_size = (height * row_pitch) as u64;
    println!("upload row pitch {}, total size {}", row_pitch, upload_size);

    let (image_upload_buffer, token) = device.create_buffer_raw(
        &mut upload,
        gfx::buffer::TRANSFER_SRC,
        upload_size,
        image_stride as u64
    ).unwrap();
    init_tokens.push(token);

    println!("copy image data into staging buffer");

    if let Ok(mut image_data) = device.write_mapping(&image_upload_buffer, 0..upload_size)
    {
        for y in 0 .. height as usize {
            let row = &(*img)[y*(width as usize)*image_stride .. (y+1)*(width as usize)*image_stride];
            let dest_base = y * row_pitch as usize;
            image_data[dest_base .. dest_base + row.len()].copy_from_slice(row);
        }
    }

    let (image, token) = device.create_image::<ColorFormat, _>(
        &mut data,
        gfx::image::TRANSFER_DST | gfx::image::SAMPLED,
        kind,
        1,
    ).unwrap();
    init_tokens.push(token);

    let image_srv = device.view_image_as_shader_resource(&image)
        .unwrap();

    let sampler = device.create_sampler(
        i::SamplerInfo::new(
            i::FilterMethod::Bilinear,
            i::WrapMode::Clamp,
        )
    );

    device.update_descriptor_sets()
        .write(desc_data.sampled_image(&desc), 0, &[&image_srv as _])
        .write(desc_data.sampler(&desc), 0, &[&sampler as _])
        .finish();

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

    let mut encoder_pool = context.acquire_encoder_pool();
    let mut init_encoder = encoder_pool.acquire_encoder();
    init_encoder.init_resources(init_tokens);
    init_encoder.copy_buffer_to_image(
        &image_upload_buffer,
        &image,
        &[command::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_pitch: row_pitch,
            buffer_slice_pitch: row_pitch * (height as u32),
            image_aspect: i::ASPECT_COLOR,
            image_subresource: (0, 0..1),
            image_offset: command::Offset { x: 0, y: 0, z: 0 },
            image_extent: d::Extent { width, height, depth: 1 },
        }]);

    let init_submit = init_encoder.finish();
    let mut submits = vec![init_submit];

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
        let mut encoder = encoder_pool.acquire_encoder();

        {
            let data = pipe::Data {
                desc: (&desc, &desc_data),
                color: &frame_rtvs[frame.id()],
                vertices: &vertex_buffer,
                viewports: &[viewport],
                scissors: &[scissor],
                framebuffer: &framebuffers[frame.id()],
            };
            encoder.draw(0..6, &pipeline, data);
        }
        
        submits.push(encoder.finish());
        context.present(submits.drain(..).collect::<Vec<_>>());
    }

    println!("cleanup!");
    device.mut_raw().destroy_shader_module(vs_module);
    device.mut_raw().destroy_shader_module(fs_module);
}
