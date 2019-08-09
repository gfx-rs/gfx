#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx11",
        feature = "dx12",
        feature = "metal",
        feature = "gl",
        feature = "wgl"
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(any(feature = "gl", feature = "wgl"))]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main();
}

use hal::format::{AsFormat, ChannelType, Rgba8Srgb as ColorFormat, Swizzle};
use hal::pass::Subpass;
use hal::pso::{PipelineStage, ShaderStageFlags, VertexInputRate};
use hal::queue::Submission;
use hal::{
    buffer,
    command,
    format as f,
    image as i,
    memory as m,
    pass,
    pool,
    pso,
    window::Extent2D,
};
use hal::{DescriptorPool, Primitive, SwapchainConfig};
use hal::{Device, Instance, PhysicalDevice, Surface, Swapchain};

use std::io::Cursor;
use std::mem::ManuallyDrop;

#[cfg_attr(rustfmt, rustfmt_skip)]
const DIMS: Extent2D = Extent2D { width: 1024, height: 768 };

const ENTRY_NAME: &str = "main";

#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
struct Vertex {
    a_Pos: [f32; 2],
    a_Uv: [f32; 2],
}

#[cfg_attr(rustfmt, rustfmt_skip)]
const QUAD: [Vertex; 6] = [
    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5, 0.33 ], a_Uv: [1.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },

    Vertex { a_Pos: [ -0.5, 0.33 ], a_Uv: [0.0, 1.0] },
    Vertex { a_Pos: [  0.5,-0.33 ], a_Uv: [1.0, 0.0] },
    Vertex { a_Pos: [ -0.5,-0.33 ], a_Uv: [0.0, 0.0] },
];

const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: f::Aspects::COLOR,
    levels: 0 .. 1,
    layers: 0 .. 1,
};

#[cfg(any(
    feature = "vulkan",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
    feature = "gl",
    feature = "wgl"
))]
fn main() {
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Debug).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    #[cfg(not(target_arch = "wasm32"))]
    let event_loop = winit::event_loop::EventLoop::new();

    #[cfg(not(target_arch = "wasm32"))]
    let wb = winit::window::WindowBuilder::new()
        .with_min_inner_size(winit::dpi::LogicalSize::new(1.0, 1.0))
        .with_inner_size(winit::dpi::LogicalSize::new(
            DIMS.width as _,
            DIMS.height as _,
        ))
        .with_title("quad".to_string());
    // instantiate backend
    #[cfg(not(feature = "gl"))]
    let (_window, _instance, mut adapters, surface) = {
        let window = wb.build(&event_loop).unwrap();
        let instance = back::Instance::create("gfx-rs quad", 1);
        let surface = instance.create_surface(&window);
        let adapters = instance.enumerate_adapters();
        (window, instance, adapters, surface)
    };
    #[cfg(feature = "gl")]
    let (window, mut adapters, surface) = {
        #[cfg(not(target_arch = "wasm32"))]
        let (window, surface) = {
            let builder =
                back::config_context(back::glutin::ContextBuilder::new(), ColorFormat::SELF, None)
                    .with_vsync(true);
            let windowed_context = builder.build_windowed(wb, &event_loop).unwrap();
            let (context, window) = unsafe { windowed_context.make_current().expect("Unable to make context current").split() };
            let surface = back::Surface::from_context(context);
            (window, surface)
        };
        #[cfg(target_arch = "wasm32")]
        let (window, surface) = {
            let window = back::Window;
            let surface = back::Surface::from_window(&window);
            (window, surface)
        };

        let adapters = surface.enumerate_adapters();
        (window, adapters, surface)
    };

    for adapter in &adapters {
        println!("{:?}", adapter.info);
    }

    let adapter = adapters.remove(0);

    let mut renderer = Renderer::new(surface, adapter);

    #[cfg(target_arch = "wasm32")]
    renderer.render();

    #[cfg(not(target_arch = "wasm32"))]
    // It is important that the closure move captures the Renderer,
    // otherwise it will not be dropped when the event loop exits.
    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent { event, .. } => {
                match event {
                    winit::event::WindowEvent::CloseRequested => *control_flow = winit::event_loop::ControlFlow::Exit,
                    winit::event::WindowEvent::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = winit::event_loop::ControlFlow::Exit,
                    winit::event::WindowEvent::Resized(dims) => {
                        println!("resized to {:?}", dims);
                        #[cfg(feature = "gl")]
                        {
                            let context = renderer.surface.get_context();
                            context.resize(dims.to_physical(window.hidpi_factor()));
                        }
                        let dimensions = Extent2D {
                            width: dims.width as u32,
                            height: dims.height as u32,
                        };
                        renderer.dimensions = dimensions;
                        renderer.recreate_swapchain();
                    }
                    _ => { }
                }
            }
            winit::event::Event::EventsCleared => {
                renderer.render();
            }
            _ => { }
        }
    });
}

struct Renderer<B: hal::Backend> {
    device: B::Device,
    queue_group: hal::QueueGroup<B, hal::Graphics>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    surface: B::Surface,
    adapter: hal::adapter::Adapter<B>,
    format: hal::format::Format,
    swap_chain: Option<B::Swapchain>,
    dimensions: Extent2D,
    framebuffers: Vec<B::Framebuffer>,
    frame_images: Vec<(B::Image, B::ImageView)>,
    viewport: hal::pso::Viewport,
    render_pass: ManuallyDrop<B::RenderPass>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    desc_set: B::DescriptorSet,
    set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    submission_complete_semaphores: Vec<B::Semaphore>,
    image_acquire_semaphores: Vec<B::Semaphore>,
    free_acquire_semaphore: Option<B::Semaphore>,
    submission_complete_fences: Vec<B::Fence>,
    cmd_pools: Vec<hal::CommandPool<B, hal::Graphics>>,
    cmd_buffers: Vec<hal::command::CommandBuffer<B, hal::Graphics, hal::command::MultiShot>>,
    vertex_buffer: ManuallyDrop<B::Buffer>,
    image_upload_buffer: ManuallyDrop<B::Buffer>,
    image_logo: ManuallyDrop<B::Image>,
    image_srv: ManuallyDrop<B::ImageView>,
    buffer_memory: ManuallyDrop<B::Memory>,
    image_memory: ManuallyDrop<B::Memory>,
    image_upload_memory: ManuallyDrop<B::Memory>,
    sampler: ManuallyDrop<B::Sampler>,
    frames_in_flight: usize,
    frame: u64,
}

impl<B> Renderer<B> where B: hal::Backend {
    fn new(mut surface: B::Surface, mut adapter: hal::adapter::Adapter<B>) -> Renderer<B> {
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();

        // Build a new device and associated command queues
        let (device, mut queue_group) = adapter
            .open_with::<_, hal::Graphics>(1, |family| surface.supports_queue_family(family))
            .unwrap();

        let mut command_pool = unsafe {
            device.create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty())
        }
        .expect("Can't create command pool");

        // Setup renderpass and pipeline
        let set_layout = ManuallyDrop::new(unsafe {
            device.create_descriptor_set_layout(
                &[
                    pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::SampledImage,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    pso::DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: pso::DescriptorType::Sampler,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
                &[],
            )
        }
        .expect("Can't create descriptor set layout"));

        // Descriptors
        let mut desc_pool = ManuallyDrop::new(unsafe {
            device.create_descriptor_pool(
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
                pso::DescriptorPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create descriptor pool"));
        let desc_set = unsafe { desc_pool.allocate_set(&set_layout) }.unwrap();

        // Buffer allocations
        println!("Memory types: {:?}", memory_types);

        let buffer_stride = std::mem::size_of::<Vertex>() as u64;
        let buffer_len = QUAD.len() as u64 * buffer_stride;

        assert_ne!(buffer_len, 0);
        let mut vertex_buffer =
            ManuallyDrop::new(unsafe { device.create_buffer(buffer_len, buffer::Usage::VERTEX) }.unwrap());

        let buffer_req = unsafe { device.get_buffer_requirements(&vertex_buffer) };

        let upload_type = memory_types
            .iter()
            .enumerate()
            .position(|(id, mem_type)| {
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                buffer_req.type_mask & (1 << id) != 0
                    && mem_type.properties.contains(m::Properties::CPU_VISIBLE)
            })
            .unwrap()
            .into();

        let buffer_memory = ManuallyDrop::new(unsafe { device.allocate_memory(upload_type, buffer_req.size) }.unwrap());

        unsafe { device.bind_buffer_memory(&buffer_memory, 0, &mut vertex_buffer) }.unwrap();

        // TODO: check transitions: read/write mapping and vertex buffer read
        unsafe {
            let mut vertices = device
                .acquire_mapping_writer::<Vertex>(&buffer_memory, 0 .. buffer_req.size)
                .unwrap();
            vertices[0 .. QUAD.len()].copy_from_slice(&QUAD);
            device.release_mapping_writer(vertices).unwrap();
        }

        // Image
        let img_data = include_bytes!("data/logo.png");

        let img = image::load(Cursor::new(&img_data[..]), image::PNG)
            .unwrap()
            .to_rgba();
        let (width, height) = img.dimensions();
        let kind = i::Kind::D2(width as i::Size, height as i::Size, 1, 1);
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;
        let image_stride = 4usize;
        let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
        let upload_size = (height * row_pitch) as u64;

        let mut image_upload_buffer =
            ManuallyDrop::new(unsafe { device.create_buffer(upload_size, buffer::Usage::TRANSFER_SRC) }.unwrap());
        let image_mem_reqs = unsafe { device.get_buffer_requirements(&image_upload_buffer) };
        let image_upload_memory =
            ManuallyDrop::new(unsafe { device.allocate_memory(upload_type, image_mem_reqs.size) }.unwrap());

        unsafe { device.bind_buffer_memory(&image_upload_memory, 0, &mut image_upload_buffer) }
            .unwrap();

        // copy image data into staging buffer
        unsafe {
            let mut data = device
                .acquire_mapping_writer::<u8>(&image_upload_memory, 0 .. image_mem_reqs.size)
                .unwrap();
            for y in 0 .. height as usize {
                let row = &(*img)
                    [y * (width as usize) * image_stride .. (y + 1) * (width as usize) * image_stride];
                let dest_base = y * row_pitch as usize;
                data[dest_base .. dest_base + row.len()].copy_from_slice(row);
            }
            device.release_mapping_writer(data).unwrap();
        }

        let mut image_logo = ManuallyDrop::new(unsafe {
            device.create_image(
                kind,
                1,
                ColorFormat::SELF,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_DST | i::Usage::SAMPLED,
                i::ViewCapabilities::empty(),
            )
        }
        .unwrap());
        let image_req = unsafe { device.get_image_requirements(&image_logo) };

        let device_type = memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                image_req.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(m::Properties::DEVICE_LOCAL)
            })
            .unwrap()
            .into();
        let image_memory = ManuallyDrop::new(unsafe { device.allocate_memory(device_type, image_req.size) }.unwrap());

        unsafe { device.bind_image_memory(&image_memory, 0, &mut image_logo) }.unwrap();
        let image_srv = ManuallyDrop::new(unsafe {
            device.create_image_view(
                &image_logo,
                i::ViewKind::D2,
                ColorFormat::SELF,
                Swizzle::NO,
                COLOR_RANGE.clone(),
            )
        }
        .unwrap());

        let sampler = ManuallyDrop::new(unsafe {
            device.create_sampler(i::SamplerInfo::new(i::Filter::Linear, i::WrapMode::Clamp))
        }
        .expect("Can't create sampler"));;

        unsafe {
            device.write_descriptor_sets(vec![
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(&*image_srv, i::Layout::Undefined)),
                },
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Sampler(&*sampler)),
                },
            ]);
        }

        // copy buffer to texture
        let mut copy_fence = device.create_fence(false).expect("Could not create fence");
        unsafe {
            let mut cmd_buffer = command_pool.acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let image_barrier = m::Barrier::Image {
                states: (i::Access::empty(), i::Layout::Undefined)
                    .. (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                target: &*image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE .. PipelineStage::TRANSFER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                i::Layout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: row_pitch / (image_stride as u32),
                    buffer_height: height as u32,
                    image_layers: i::SubresourceLayers {
                        aspects: f::Aspects::COLOR,
                        level: 0,
                        layers: 0 .. 1,
                    },
                    image_offset: i::Offset { x: 0, y: 0, z: 0 },
                    image_extent: i::Extent {
                        width,
                        height,
                        depth: 1,
                    },
                }],
            );

            let image_barrier = m::Barrier::Image {
                states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                    .. (i::Access::SHADER_READ, i::Layout::ShaderReadOnlyOptimal),
                target: &*image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };
            cmd_buffer.pipeline_barrier(
                PipelineStage::TRANSFER .. PipelineStage::FRAGMENT_SHADER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.finish();

            queue_group.queues[0].submit_without_semaphores(Some(&cmd_buffer), Some(&mut copy_fence));

            device
                .wait_for_fence(&copy_fence, !0)
                .expect("Can't wait for fence");
        }

        unsafe {
            device.destroy_fence(copy_fence);
        }

        let (caps, formats, _present_modes) = surface.compatibility(&mut adapter.physical_device);
        println!("formats: {:?}", formats);
        let format = formats.map_or(f::Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });

        let swap_config = SwapchainConfig::from_caps(&caps, format, DIMS);
        println!("{:?}", swap_config);
        let extent = swap_config.extent.to_extent();

        let (swap_chain, backbuffer) =
            unsafe { device.create_swapchain(&mut surface, swap_config, None) }
                .expect("Can't create swapchain");
        let swap_chain = Some(swap_chain);

        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined .. i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External .. pass::SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    .. PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: i::Access::empty()
                    .. (i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            };

            ManuallyDrop::new(unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
                .expect("Can't create render pass"))
        };

        let (frame_images, framebuffers) = {
            let pairs = backbuffer
                .into_iter()
                .map(|image| unsafe {
                    let rtv = device
                        .create_image_view(
                            &image,
                            i::ViewKind::D2,
                            format,
                            Swizzle::NO,
                            COLOR_RANGE.clone(),
                        )
                        .unwrap();
                    (image, rtv)
                })
                .collect::<Vec<_>>();
            let fbos = pairs
                .iter()
                .map(|&(_, ref rtv)| unsafe {
                    device
                        .create_framebuffer(&render_pass, Some(rtv), extent)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            (pairs, fbos)
        };

        // Define maximum number of frames we want to be able to be "in flight" (being computed
        // simultaneously) at once
        let frames_in_flight = 3;

        // Number of image acquisition semaphores is based on the number of swapchain images, not frames in flight,
        // plus one extra which we can guarantee is unused at any given time by swapping it out with the ones
        // in the rest of the queue.
        let mut image_acquire_semaphores = Vec::with_capacity(frame_images.len());
        let free_acquire_semaphore = Option::Some(device
            .create_semaphore()
            .expect("Could not create semaphore"));

        // The number of the rest of the resources is based on the frames in flight.
        let mut submission_complete_semaphores = Vec::with_capacity(frames_in_flight);
        let mut submission_complete_fences = Vec::with_capacity(frames_in_flight);
        // Note: We don't really need a different command pool per frame in such a simple demo like this,
        // but in a more 'real' application, it's generally seen as optimal to have one command pool per
        // thread per frame. There is a flag that lets a command pool reset individual command buffers
        // which are created from it, but by default the whole pool (and therefore all buffers in it)
        // must be reset at once. Furthermore, it is often the case that resetting a whole pool is actually
        // faster and more efficient for the hardware than resetting individual command buffers, so it's
        // usually best to just make a command pool for each set of buffers which need to be reset at the
        // same time (each frame). In our case, each pool will only have one command buffer created from it,
        // though.
        let mut cmd_pools = Vec::with_capacity(frames_in_flight);
        let mut cmd_buffers = Vec::with_capacity(frames_in_flight);

        cmd_pools.push(command_pool);
        for _ in 1 .. frames_in_flight {
            unsafe {
                cmd_pools.push(
                    device
                        .create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty())
                        .expect("Can't create command pool"),
                );
            }
        }

        for _ in 0 .. frame_images.len() {
            image_acquire_semaphores.push(
                device
                    .create_semaphore()
                    .expect("Could not create semaphore"),
            );
        }

        for i in 0 .. frames_in_flight {
            submission_complete_semaphores.push(
                device
                    .create_semaphore()
                    .expect("Could not create semaphore"),
            );
            submission_complete_fences.push(
                device
                    .create_fence(true)
                    .expect("Could not create semaphore"),
            );
            cmd_buffers.push(cmd_pools[i].acquire_command_buffer::<command::MultiShot>());
        }

        let pipeline_layout = ManuallyDrop::new(unsafe {
            device.create_pipeline_layout(
                std::iter::once(&*set_layout),
                &[(pso::ShaderStageFlags::VERTEX, 0 .. 8)],
            )
        }
        .expect("Can't create pipeline layout"));
        let pipeline = {
            let vs_module = {
                let spirv =
                    hal::read_spirv(Cursor::new(&include_bytes!("data/quad.vert.spv")[..])).unwrap();
                unsafe { device.create_shader_module(&spirv) }.unwrap()
            };
            let fs_module = {
                let spirv =
                    hal::read_spirv(Cursor::new(&include_bytes!("./data/quad.frag.spv")[..])).unwrap();
                unsafe { device.create_shader_module(&spirv) }.unwrap()
            };

            let pipeline = {
                let (vs_entry, fs_entry) = (
                    pso::EntryPoint {
                        entry: ENTRY_NAME,
                        module: &vs_module,
                        specialization: hal::spec_const_list![0.8f32],
                    },
                    pso::EntryPoint {
                        entry: ENTRY_NAME,
                        module: &fs_module,
                        specialization: pso::Specialization::default(),
                    },
                );

                let shader_entries = pso::GraphicsShaderSet {
                    vertex: vs_entry,
                    hull: None,
                    domain: None,
                    geometry: None,
                    fragment: Some(fs_entry),
                };

                let subpass = Subpass {
                    index: 0,
                    main_pass: &*render_pass,
                };

                let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
                    shader_entries,
                    Primitive::TriangleList,
                    pso::Rasterizer::FILL,
                    &*pipeline_layout,
                    subpass,
                );
                pipeline_desc.blender.targets.push(pso::ColorBlendDesc {
                    mask: pso::ColorMask::ALL,
                    blend: Some(pso::BlendState::ALPHA),
                });
                pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
                    binding: 0,
                    stride: std::mem::size_of::<Vertex>() as u32,
                    rate: VertexInputRate::Vertex,
                });

                pipeline_desc.attributes.push(pso::AttributeDesc {
                    location: 0,
                    binding: 0,
                    element: pso::Element {
                        format: f::Format::Rg32Sfloat,
                        offset: 0,
                    },
                });
                pipeline_desc.attributes.push(pso::AttributeDesc {
                    location: 1,
                    binding: 0,
                    element: pso::Element {
                        format: f::Format::Rg32Sfloat,
                        offset: 8,
                    },
                });

                unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            };

            unsafe {
                device.destroy_shader_module(vs_module);
            }
            unsafe {
                device.destroy_shader_module(fs_module);
            }

            ManuallyDrop::new(pipeline.unwrap())
        };

        // Rendering setup
        let viewport = pso::Viewport {
            rect: pso::Rect {
                x: 0,
                y: 0,
                w: extent.width as _,
                h: extent.height as _,
            },
            depth: 0.0 .. 1.0,
        };

        let dimensions = Extent2D {
            width: 0,
            height: 0,
        };

        Renderer {
            device,
            queue_group,
            desc_pool,
            surface,
            adapter,
            format,
            dimensions,
            swap_chain,
            framebuffers,
            frame_images,
            viewport,
            render_pass,
            pipeline,
            pipeline_layout,
            desc_set,
            set_layout,
            submission_complete_semaphores,
            image_acquire_semaphores,
            free_acquire_semaphore,
            submission_complete_fences,
            cmd_pools,
            cmd_buffers,
            vertex_buffer,
            image_upload_buffer,
            image_logo,
            image_srv,
            buffer_memory,
            image_memory,
            image_upload_memory,
            sampler,
            frames_in_flight,
            frame: 0,
        }
    }

    fn recreate_swapchain(&mut self) {
        self.device.wait_idle().unwrap();

        let (caps, formats, _present_modes) =
            self.surface.compatibility(&mut self.adapter.physical_device);
        // Verify that previous format still exists so we may reuse it.
        assert!(formats.iter().any(|fs| fs.contains(&self.format)));

        let swap_config = SwapchainConfig::from_caps(&caps, self.format, self.dimensions);
        println!("{:?}", swap_config);
        let extent = swap_config.extent.to_extent();

        let (new_swap_chain, new_backbuffer) =
            unsafe { self.device.create_swapchain(&mut self.surface, swap_config, self.swap_chain.take()) }
                .expect("Can't create swapchain");

        unsafe {
            // Clean up the old framebuffers and images
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer);
            }
            for (_, rtv) in self.frame_images.drain(..) {
                self.device.destroy_image_view(rtv);
            }
        }

        self.swap_chain = Some(new_swap_chain);

        let (new_frame_images, new_framebuffers) = {
            let pairs = new_backbuffer
                .into_iter()
                .map(|image| unsafe {
                    let rtv = self.device
                        .create_image_view(
                            &image,
                            i::ViewKind::D2,
                            self.format,
                            Swizzle::NO,
                            COLOR_RANGE.clone(),
                        )
                        .unwrap();
                    (image, rtv)
                })
                .collect::<Vec<_>>();
            let fbos = pairs
                .iter()
                .map(|&(_, ref rtv)| unsafe {
                    self.device
                        .create_framebuffer(&self.render_pass, Some(rtv), extent)
                        .unwrap()
                })
                .collect();
            (pairs, fbos)
        };

        self.framebuffers = new_framebuffers;
        self.frame_images = new_frame_images;
        self.viewport.rect.w = extent.width as _;
        self.viewport.rect.h = extent.height as _;
    }

    fn render(&mut self) {
        // Use guaranteed unused acquire semaphore to get the index of the next frame we will render to
        // by using acquire_image
        let swap_image = unsafe {
            match self.swap_chain.as_mut().unwrap().acquire_image(!0, self.free_acquire_semaphore.as_ref(), None) {
                Ok((i, _)) => i as usize,
                Err(_) => {
                    self.recreate_swapchain();
                    return;
                }
            }
        };

        // Swap the acquire semaphore with the one previously associated with the image we are acquiring
        core::mem::swap(
            self.free_acquire_semaphore.as_mut().unwrap(),
            &mut self.image_acquire_semaphores[swap_image],
        );

        // Compute index into our resource ring buffers based on the frame number
        // and number of frames in flight. Pay close attention to where this index is needed
        // versus when the swapchain image index we got from acquire_image is needed.
        let frame_idx = self.frame as usize % self.frames_in_flight;

        // Wait for the fence of the previous submission of this frame and reset it; ensures we are
        // submitting only up to maximum number of frames_in_flight if we are submitting faster than
        // the gpu can keep up with. This would also guarantee that any resources which need to be
        // updated with a CPU->GPU data copy are not in use by the GPU, so we can perform those updates.
        // In this case there are none to be done, however.
        unsafe {
            self.device
                .wait_for_fence(&self.submission_complete_fences[frame_idx], !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(&self.submission_complete_fences[frame_idx])
                .expect("Failed to reset fence");
            self.cmd_pools[frame_idx].reset(false);
        }

        // Rendering
        let cmd_buffer = &mut self.cmd_buffers[frame_idx];
        unsafe {
            cmd_buffer.begin(false);

            cmd_buffer.set_viewports(0, &[self.viewport.clone()]);
            cmd_buffer.set_scissors(0, &[self.viewport.rect]);
            cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            cmd_buffer.bind_vertex_buffers(0, Some((&*self.vertex_buffer, 0)));
            cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout, 0, Some(&self.desc_set), &[]);

            {
                let mut encoder = cmd_buffer.begin_render_pass_inline(
                    &self.render_pass,
                    &self.framebuffers[swap_image],
                    self.viewport.rect,
                    &[command::ClearValue::Color(command::ClearColor::Sfloat([
                        0.8, 0.8, 0.8, 1.0,
                    ]))],
                );
                encoder.draw(0 .. 6, 0 .. 1);
            }

            cmd_buffer.finish();

            let submission = Submission {
                command_buffers: Some(&*cmd_buffer),
                wait_semaphores: Some((
                    &self.image_acquire_semaphores[swap_image],
                    PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                )),
                signal_semaphores: Some(&self.submission_complete_semaphores[frame_idx]),
            };
            self.queue_group.queues[0].submit(submission, Some(&self.submission_complete_fences[frame_idx]));

            // present frame
            if let Err(_) = self.swap_chain.as_ref().unwrap().present(
                &mut self.queue_group.queues[0],
                swap_image as hal::SwapImageIndex,
                Some(&self.submission_complete_semaphores[frame_idx]),
            ) {
                self.recreate_swapchain();
                return
            }
        }

        // Increment our frame
        self.frame += 1;
    }
}

impl<B> Drop for Renderer<B> where B: hal::Backend {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            // TODO: When ManuallyDrop::take (soon to be renamed to ManuallyDrop::read) is stabilized we should use that instead.
            self.device.destroy_descriptor_pool(ManuallyDrop::into_inner(std::ptr::read(&self.desc_pool)));
            self.device.destroy_descriptor_set_layout(ManuallyDrop::into_inner(std::ptr::read(&self.set_layout)));

            self.device.destroy_buffer(ManuallyDrop::into_inner(std::ptr::read(&self.vertex_buffer)));
            self.device.destroy_buffer(ManuallyDrop::into_inner(std::ptr::read(&self.image_upload_buffer)));
            self.device.destroy_image(ManuallyDrop::into_inner(std::ptr::read(&self.image_logo)));
            self.device.destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(&self.image_srv)));
            self.device.destroy_sampler(ManuallyDrop::into_inner(std::ptr::read(&self.sampler)));
            self.device.destroy_semaphore(self.free_acquire_semaphore.take().unwrap());
            for p in self.cmd_pools.drain(..) {
                self.device.destroy_command_pool(p.into_raw());
            }
            for s in self.image_acquire_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for s in self.submission_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for f in self.submission_complete_fences.drain(..) {
                self.device.destroy_fence(f);
            }
            self.device.destroy_render_pass(ManuallyDrop::into_inner(std::ptr::read(&self.render_pass)));
            self.device.free_memory(ManuallyDrop::into_inner(std::ptr::read(&self.buffer_memory)));
            self.device.free_memory(ManuallyDrop::into_inner(std::ptr::read(&self.image_memory)));
            self.device.free_memory(ManuallyDrop::into_inner(std::ptr::read(&self.image_upload_memory)));
            self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(std::ptr::read(&self.pipeline)));
            self.device.destroy_pipeline_layout(ManuallyDrop::into_inner(std::ptr::read(&self.pipeline_layout)));
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer);
            }
            for (_, rtv) in self.frame_images.drain(..) {
                self.device.destroy_image_view(rtv);
            }

            self.device.destroy_swapchain(self.swap_chain.take().unwrap());
        }
        println!("DROPPED!");
    }
}

#[cfg(not(any(
    feature = "vulkan",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
    feature = "gl",
    feature = "wgl"
)))]
fn main() {
    println!("You need to enable the native API feature (vulkan/metal/dx11/dx12/gl/wgl) in order to run the example");
}
