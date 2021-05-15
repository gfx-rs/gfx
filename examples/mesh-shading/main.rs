#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx11",
        feature = "dx12",
        feature = "metal",
        feature = "gl",
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main();
}

use hal::{
    buffer, command, format as f, format::ChannelType, image as i, memory as m, pass,
    pass::Subpass, pool, prelude::*, pso, pso::ShaderStageFlags, queue::QueueGroup, window,
};

use std::{
    borrow::Borrow,
    io::Cursor,
    iter,
    mem::{self, ManuallyDrop},
    ptr,
};

#[cfg_attr(rustfmt, rustfmt_skip)]
const DIMS: window::Extent2D = window::Extent2D { width: 1024, height: 768 };

const ENTRY_NAME: &str = "main";

#[cfg(any(
    feature = "vulkan",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
    feature = "gl",
))]
fn main() {
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Debug).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let wb = winit::window::WindowBuilder::new()
        .with_min_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
            64.0, 64.0,
        )))
        .with_inner_size(winit::dpi::Size::Physical(winit::dpi::PhysicalSize::new(
            DIMS.width,
            DIMS.height,
        )))
        .with_title("mesh shading".to_string());

    // instantiate backend
    #[cfg(not(target_arch = "wasm32"))]
    let (_window, instance, mut adapters, surface) = {
        let window = wb.build(&event_loop).unwrap();
        let instance = back::Instance::create("gfx-rs mesh shading", 1)
            .expect("Failed to create an instance!");
        let adapters = instance.enumerate_adapters();
        let surface = unsafe {
            instance
                .create_surface(&window)
                .expect("Failed to create a surface!")
        };
        // Return `window` so it is not dropped: dropping it invalidates `surface`.
        (window, Some(instance), adapters, surface)
    };

    #[cfg(target_arch = "wasm32")]
    let (_window, instance, mut adapters, surface) = {
        let (window, surface) = {
            let window = wb.build(&event_loop).unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .body()
                .unwrap()
                .append_child(&winit::platform::web::WindowExtWebSys::canvas(&window))
                .unwrap();
            let surface = back::Surface::from_raw_handle(&window);
            (window, surface)
        };

        let adapters = surface.enumerate_adapters();
        (window, None, adapters, surface)
    };

    for adapter in &adapters {
        println!("{:?}", adapter.info);
    }

    let adapter = adapters.drain(..).next().unwrap();

    let mut renderer = Renderer::new(instance, surface, adapter);

    renderer.render();

    // It is important that the closure move captures the Renderer,
    // otherwise it will not be dropped when the event loop exits.
    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit
                }
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
                    renderer.dimensions = window::Extent2D {
                        width: dims.width,
                        height: dims.height,
                    };
                    renderer.recreate_swapchain();
                }
                _ => {}
            },
            winit::event::Event::RedrawEventsCleared => {
                renderer.render();
            }
            _ => {}
        }
    });
}

struct Renderer<B: hal::Backend> {
    instance: Option<B::Instance>,
    device: B::Device,
    queue_group: QueueGroup<B>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    surface: ManuallyDrop<B::Surface>,
    adapter: hal::adapter::Adapter<B>,
    format: hal::format::Format,
    dimensions: window::Extent2D,
    viewport: pso::Viewport,
    render_pass: ManuallyDrop<B::RenderPass>,
    framebuffer: ManuallyDrop<B::Framebuffer>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    desc_set: B::DescriptorSet,
    set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    submission_complete_semaphores: Vec<B::Semaphore>,
    submission_complete_fences: Vec<B::Fence>,

    cmd_pools: Vec<B::CommandPool>,
    cmd_buffers: Vec<B::CommandBuffer>,

    positions_buffer: ManuallyDrop<B::Buffer>,
    buffer_memory: ManuallyDrop<B::Memory>,

    frames_in_flight: usize,
    frame: u64,
}

impl<B> Renderer<B>
where
    B: hal::Backend,
{
    fn new(
        instance: Option<B::Instance>,
        mut surface: B::Surface,
        adapter: hal::adapter::Adapter<B>,
    ) -> Renderer<B> {
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.properties().limits;

        // Build a new device and associated command queues
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                surface.supports_queue_family(family) && family.queue_type().supports_graphics()
            })
            .unwrap();
        let mut gpu = unsafe {
            adapter
                .physical_device
                .open(&[(family, &[1.0])], hal::Features::MESH_SHADER)
                .unwrap()
        };
        let queue_group = gpu.queue_groups.pop().unwrap();
        let device = gpu.device;

        let command_pool = unsafe {
            device.create_command_pool(queue_group.family, pool::CommandPoolCreateFlags::empty())
        }
        .expect("Can't create command pool");

        // Setup renderpass and pipeline
        let set_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    iter::once(pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Storage { read_only: true },
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                        },
                        count: 1,
                        stage_flags: ShaderStageFlags::MESH,
                        immutable_samplers: false,
                    }),
                    iter::empty(),
                )
            }
            .expect("Can't create descriptor set layout"),
        );

        // Descriptors
        let mut desc_pool = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_pool(
                    1, // sets
                    iter::once(pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Storage { read_only: true },
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                        },
                        count: 1,
                    }),
                    pso::DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Can't create descriptor pool"),
        );
        let mut desc_set = unsafe { desc_pool.allocate_one(&set_layout) }.unwrap();

        // Buffer allocations
        println!("Memory types: {:?}", memory_types);
        let non_coherent_alignment = limits.non_coherent_atom_size as u64;

        let mut positions = Vec::new();
        for x in -4..4 {
            for y in -4..4 {
                positions.push(x as f32 / 8.0);
                positions.push(y as f32 / 8.0);
            }
        }
        let buffer_len = (positions.len() * mem::size_of::<f32>()) as u64;
        assert_ne!(buffer_len, 0);
        let padded_buffer_len = ((buffer_len + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;

        let mut positions_buffer = ManuallyDrop::new(
            unsafe {
                device.create_buffer(
                    padded_buffer_len,
                    buffer::Usage::STORAGE,
                    hal::memory::SparseFlags::empty(),
                )
            }
            .unwrap(),
        );

        let buffer_req = unsafe { device.get_buffer_requirements(&positions_buffer) };

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

        // TODO: check transitions: read/write mapping and vertex buffer read
        let buffer_memory = unsafe {
            let mut memory = device
                .allocate_memory(upload_type, buffer_req.size)
                .unwrap();
            device
                .bind_buffer_memory(&memory, 0, &mut positions_buffer)
                .unwrap();
            let mapping = device.map_memory(&mut memory, m::Segment::ALL).unwrap();
            ptr::copy_nonoverlapping(
                positions.as_ptr() as *const u8,
                mapping,
                buffer_len as usize,
            );
            device
                .flush_mapped_memory_ranges(iter::once((&memory, m::Segment::ALL)))
                .unwrap();
            device.unmap_memory(&mut memory);
            ManuallyDrop::new(memory)
        };

        unsafe {
            device.write_descriptor_set(pso::DescriptorSetWrite {
                set: &mut desc_set,
                binding: 0,
                array_offset: 0,
                descriptors: iter::once(pso::Descriptor::Buffer(
                    &*positions_buffer,
                    buffer::SubRange::WHOLE,
                )),
            });
        }

        let caps = surface.capabilities(&adapter.physical_device);
        let formats = surface.supported_formats(&adapter.physical_device);
        println!("formats: {:?}", formats);
        let format = formats.map_or(f::Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });

        let swap_config = window::SwapchainConfig::from_caps(&caps, format, DIMS);
        println!("{:?}", swap_config);
        let extent = swap_config.extent;
        let fat = swap_config.framebuffer_attachment();
        unsafe {
            surface
                .configure_swapchain(&device, swap_config)
                .expect("Can't configure swapchain");
        };

        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            ManuallyDrop::new(
                unsafe {
                    device.create_render_pass(
                        iter::once(attachment),
                        iter::once(subpass),
                        iter::empty(),
                    )
                }
                .expect("Can't create render pass"),
            )
        };

        let framebuffer = ManuallyDrop::new(unsafe {
            device
                .create_framebuffer(
                    &render_pass,
                    iter::once(fat),
                    i::Extent {
                        width: DIMS.width,
                        height: DIMS.height,
                        depth: 1,
                    },
                )
                .unwrap()
        });

        // Define maximum number of frames we want to be able to be "in flight" (being computed
        // simultaneously) at once
        let frames_in_flight = 3;

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
        for _ in 1..frames_in_flight {
            unsafe {
                cmd_pools.push(
                    device
                        .create_command_pool(
                            queue_group.family,
                            pool::CommandPoolCreateFlags::empty(),
                        )
                        .expect("Can't create command pool"),
                );
            }
        }

        for i in 0..frames_in_flight {
            submission_complete_semaphores.push(
                device
                    .create_semaphore()
                    .expect("Could not create semaphore"),
            );
            submission_complete_fences
                .push(device.create_fence(true).expect("Could not create fence"));
            cmd_buffers.push(unsafe { cmd_pools[i].allocate_one(command::Level::Primary) });
        }

        let pipeline_layout = ManuallyDrop::new(
            unsafe { device.create_pipeline_layout(iter::once(&*set_layout), iter::empty()) }
                .expect("Can't create pipeline layout"),
        );
        let pipeline = {
            let ms_module = {
                let spirv =
                    auxil::read_spirv(Cursor::new(&include_bytes!("data/triangles.mesh.spv")[..]))
                        .unwrap();
                unsafe { device.create_shader_module(&spirv) }.unwrap()
            };
            let fs_module = {
                let spirv =
                    auxil::read_spirv(Cursor::new(&include_bytes!("data/triangles.frag.spv")[..]))
                        .unwrap();
                unsafe { device.create_shader_module(&spirv) }.unwrap()
            };

            let pipeline = {
                let (ms_entry, fs_entry) = (
                    pso::EntryPoint {
                        entry: ENTRY_NAME,
                        module: &ms_module,
                        specialization: pso::Specialization::default(),
                    },
                    pso::EntryPoint {
                        entry: ENTRY_NAME,
                        module: &fs_module,
                        specialization: pso::Specialization::default(),
                    },
                );

                let subpass = Subpass {
                    index: 0,
                    main_pass: &*render_pass,
                };

                let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
                    pso::PrimitiveAssemblerDesc::Mesh {
                        task: None,
                        mesh: ms_entry,
                    },
                    pso::Rasterizer::FILL,
                    Some(fs_entry),
                    &*pipeline_layout,
                    subpass,
                );

                pipeline_desc.blender.targets.push(pso::ColorBlendDesc {
                    mask: pso::ColorMask::ALL,
                    blend: Some(pso::BlendState::ALPHA),
                });

                unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            };

            unsafe {
                device.destroy_shader_module(ms_module);
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
            depth: 0.0..1.0,
        };

        Renderer {
            instance,
            device,
            queue_group,
            desc_pool,
            surface: ManuallyDrop::new(surface),
            adapter,
            format,
            dimensions: DIMS,
            viewport,
            render_pass,
            framebuffer,
            pipeline,
            pipeline_layout,
            desc_set,
            set_layout,
            submission_complete_semaphores,
            submission_complete_fences,
            cmd_pools,
            cmd_buffers,
            positions_buffer,
            buffer_memory,
            frames_in_flight,
            frame: 0,
        }
    }

    fn recreate_swapchain(&mut self) {
        let caps = self.surface.capabilities(&self.adapter.physical_device);
        let swap_config = window::SwapchainConfig::from_caps(&caps, self.format, self.dimensions);
        println!("{:?}", swap_config);

        let extent = swap_config.extent.to_extent();
        self.viewport.rect.w = extent.width as _;
        self.viewport.rect.h = extent.height as _;

        unsafe {
            self.device
                .destroy_framebuffer(ManuallyDrop::into_inner(ptr::read(&self.framebuffer)));
            self.framebuffer = ManuallyDrop::new(
                self.device
                    .create_framebuffer(
                        &self.render_pass,
                        iter::once(swap_config.framebuffer_attachment()),
                        extent,
                    )
                    .unwrap(),
            )
        };

        unsafe {
            self.surface
                .configure_swapchain(&self.device, swap_config)
                .expect("Can't create swapchain");
        }
    }

    fn render(&mut self) {
        let surface_image = unsafe {
            match self.surface.acquire_image(!0) {
                Ok((image, _)) => image,
                Err(_) => {
                    self.recreate_swapchain();
                    return;
                }
            }
        };

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
            let fence = &mut self.submission_complete_fences[frame_idx];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.cmd_pools[frame_idx].reset(false);
        }

        // Rendering
        let cmd_buffer = &mut self.cmd_buffers[frame_idx];
        unsafe {
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);

            cmd_buffer.set_viewports(0, iter::once(self.viewport.clone()));
            cmd_buffer.set_scissors(0, iter::once(self.viewport.rect));
            cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            cmd_buffer.bind_graphics_descriptor_sets(
                &self.pipeline_layout,
                0,
                iter::once(&self.desc_set),
                iter::empty(),
            );

            cmd_buffer.begin_render_pass(
                &self.render_pass,
                &self.framebuffer,
                self.viewport.rect,
                iter::once(command::RenderAttachmentInfo {
                    image_view: surface_image.borrow(),
                    clear_value: command::ClearValue {
                        color: command::ClearColor {
                            float32: [0.8, 0.8, 0.8, 1.0],
                        },
                    },
                }),
                command::SubpassContents::Inline,
            );
            cmd_buffer.draw_mesh_tasks(1, 0);
            cmd_buffer.end_render_pass();
            cmd_buffer.finish();

            self.queue_group.queues[0].submit(
                iter::once(&*cmd_buffer),
                iter::empty(),
                iter::once(&self.submission_complete_semaphores[frame_idx]),
                Some(&mut self.submission_complete_fences[frame_idx]),
            );

            // present frame
            let result = self.queue_group.queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&mut self.submission_complete_semaphores[frame_idx]),
            );

            if result.is_err() {
                self.recreate_swapchain();
            }
        }

        // Increment our frame
        self.frame += 1;
    }
}

impl<B> Drop for Renderer<B>
where
    B: hal::Backend,
{
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            // TODO: When ManuallyDrop::take (soon to be renamed to ManuallyDrop::read) is stabilized we should use that instead.
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.set_layout,
                )));

            self.device
                .destroy_buffer(ManuallyDrop::into_inner(ptr::read(&self.positions_buffer)));
            for p in self.cmd_pools.drain(..) {
                self.device.destroy_command_pool(p);
            }
            for s in self.submission_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for f in self.submission_complete_fences.drain(..) {
                self.device.destroy_fence(f);
            }
            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(ptr::read(&self.render_pass)));
            self.device
                .destroy_framebuffer(ManuallyDrop::into_inner(ptr::read(&self.framebuffer)));
            self.surface.unconfigure_swapchain(&self.device);
            self.device
                .free_memory(ManuallyDrop::into_inner(ptr::read(&self.buffer_memory)));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(ptr::read(&self.pipeline)));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.pipeline_layout,
                )));
            if let Some(instance) = &self.instance {
                let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
                instance.destroy_surface(surface);
            }
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
)))]
fn main() {
    println!("You need to enable the native API feature (vulkan/metal/dx11/dx12/gl) in order to run the example");
}
