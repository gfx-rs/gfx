extern crate env_logger;
extern crate gfx_hal as hal;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[macro_use]
extern crate gfx_render as gfx;

extern crate winit;
extern crate image;

use std::io::Cursor;

use hal::{command, device as d, format as f, image as i, pso};
use hal::{Device, Instance, PhysicalDevice, Primitive};
use gfx::format::{Rgba8Srgb as ColorFormat};
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
    env_logger::init();

    #[cfg(feature = "metal")]
    let mut autorelease_pool = unsafe { back::AutoreleasePool::new() };

    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title("quad".to_string())
        .build(&events_loop)
        .unwrap();
    let window_size = window.get_inner_size().unwrap();
    let pixel_width = window_size.0 as u16;
    let pixel_height = window_size.1 as u16;

    // instantiate backend
    let instance = back::Instance::create("gfx-rs quad", 1);
    let surface = instance.create_surface(&window);
    let mut adapters = instance.enumerate_adapters();
    for adapter in &adapters {
        println!("{:?}", adapter.info);
    }
    let adapter = adapters.remove(0);
    let limits = adapter.physical_device.limits();

    let (mut context, backbuffers) =
        gfx::Context::<back::Backend, hal::Graphics>
        ::init::<ColorFormat>(surface, adapter).unwrap();
    let mut device = (*context.ref_device()).clone();

    // Setup renderpass and pipeline
    let vs_module = device.raw.create_shader_module(include_bytes!("../../hal/quad/data/vert.spv")).unwrap();
    let fs_module = device.raw.create_shader_module(include_bytes!("../../hal/quad/data/frag.spv")).unwrap();

    let (desc, mut desc_data) = device.create_descriptors(1).pop().unwrap();
    let pipe_init = pipe::Init {
        desc: &desc,
        color: pso::ColorBlendDesc(pso::ColorMask::ALL, pso::BlendState::ALPHA),
        vertices: (),
    };
    let pipeline = device.create_graphics_pipeline(
        pso::GraphicsShaderSet {
            vertex: pso::EntryPoint { entry: "main", module: &vs_module, specialization: &[] },
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(pso::EntryPoint { entry: "main", module: &fs_module, specialization: &[] },),
        },
        Primitive::TriangleList,
        pso::Rasterizer::FILL,
        pipe_init,
    ).unwrap();

    let image_range = gfx::image::SubresourceRange {
        aspects: f::Aspects::COLOR,
        levels: 0 .. 1,
        layers: 0 .. 1,
    };

    // Framebuffer creation
    let frame_rtvs = backbuffers.iter().map(|backbuffer| {
        device.create_image_view(&backbuffer.color, image_range.clone())
            .unwrap()
    }).collect::<Vec<_>>();
    let framebuffers = frame_rtvs.iter().map(|rtv| {
        let extent = d::Extent { width: pixel_width as _, height: pixel_height as _, depth: 1 };
        device.create_framebuffer(&pipeline, &[rtv.as_ref()], extent)
            .unwrap()
    }).collect::<Vec<_>>();

    let mut upload = Allocator::new(
        gfx::memory::Usage::Upload,
        &context.ref_device(),
        limits,
    );
    let mut data = Allocator::new(
        gfx::memory::Usage::Data,
        &context.ref_device(),
        limits,
    );
    println!("Memory types: {:?}", context.ref_device().memory_types());
    println!("Memory heaps: {:?}", context.ref_device().memory_heaps());

    let vertex_count = QUAD.len() as u64;
    let (vertex_buffer, vertex_token) = device.create_buffer::<Vertex, _>(
        &mut upload,
        gfx::buffer::Usage::VERTEX,
        vertex_count
    ).unwrap();

    println!("vertex data uploading not implemented!");
    // TODO
    /*
    device.write_mapping(&vertex_buffer, 0..vertex_count)
        .unwrap()
        .copy_from_slice(&QUAD);
    */

    let img_data = include_bytes!("../../hal/quad/data/logo.png");
    let img = image::load(Cursor::new(&img_data[..]), image::PNG).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = i::Kind::D2(width as i::Size, height as i::Size, i::AaMode::Single);
    let row_alignment_mask = limits.min_buffer_copy_pitch_alignment as u32 - 1;
    let image_stride = 4usize;
    let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
    let upload_size = (height * row_pitch) as u64;
    println!("upload row pitch {}, total size {}", row_pitch, upload_size);

    let (image_upload_buffer, image_upload_token) = device.create_buffer_raw(
        &mut upload,
        gfx::buffer::Usage::TRANSFER_SRC,
        upload_size,
        image_stride as u64,
    ).unwrap();

    println!("copy image data into staging buffer");

    // TODO:
    println!("image uploading not implemented!");
    /*
    if let Ok(mut image_data) = device.write_mapping(&image_upload_buffer, 0..upload_size) {
        for y in 0 .. height as usize {
            let row = &(*img)[y*(width as usize)*image_stride .. (y+1)*(width as usize)*image_stride];
            let dest_base = y * row_pitch as usize;
            image_data[dest_base .. dest_base + row.len()].copy_from_slice(row);
        }
    }
    */

    let (image, image_token) = device.create_image::<ColorFormat, _>(
        &mut data,
        gfx::image::Usage::TRANSFER_DST | gfx::image::Usage::SAMPLED,
        kind,
        1,
    ).unwrap();

    let image_srv = device.create_image_view(&image, image_range)
        .unwrap();

    let sampler = device.create_sampler(
        i::SamplerInfo::new(
            i::FilterMethod::Bilinear,
            i::WrapMode::Clamp,
        )
    );

    device.update_descriptor_sets()
        .write(desc_data.sampled_image(&desc), 0, &[image_srv.as_ref()])
        .write(desc_data.sampler(&desc), 0, &[sampler.as_ref()])
        .finish();

    // Rendering setup
    let scissor = command::Rect {
        x: 0, y: 0,
        w: pixel_width, h: pixel_height,
    };
    let viewport = command::Viewport {
        rect: scissor,
        depth: 0.0 .. 1.0,
    };

    let mut encoder_pool = context.acquire_encoder_pool();
    let mut init_encoder = encoder_pool.acquire_encoder();
    init_encoder.init_resources(vec![
        vertex_token,
        image_upload_token,
        image_token,
    ]);
    init_encoder.copy_buffer_to_image(
        &image_upload_buffer,
        &image,
        &[command::BufferImageCopy {
            buffer_offset: 0,
            buffer_width: row_pitch / image_stride as u32,
            buffer_height: height as u32,
            image_layers: gfx::image::SubresourceLayers {
                aspects: f::Aspects::COLOR,
                level: 0,
                layers: 0 .. 1,
            },
            image_offset: i::Offset { x: 0, y: 0, z: 0 },
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
                viewports: &[viewport.clone()],
                scissors: &[scissor],
                framebuffer: &framebuffers[frame.id()],
            };
            encoder.draw(0..6, &pipeline, data);
        }

        submits.push(encoder.finish());
        context.present(submits.drain(..).collect::<Vec<_>>());

        #[cfg(feature = "metal")]
        unsafe {
            autorelease_pool.reset();
        }
    }

    println!("cleanup!");
    device.raw.destroy_shader_module(vs_module);
    device.raw.destroy_shader_module(fs_module);
}
