#![cfg_attr(
    not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")))]
extern crate gfx_backend_empty as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

#[cfg(feature = "gl")]
use back::glutin::GlContext;

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate gfx_hal as hal;
extern crate glsl_to_spirv;
extern crate image;
extern crate winit;

struct Dimensions<T> {
    width: T,
    height: T,
}

use std::cell::RefCell;
use std::io::Cursor;
use std::mem::size_of;
use std::rc::Rc;

use hal::{
    buffer, command, format as f, image as i, memory as m, pass, pool, pso, Adapter, Backbuffer,
    Backend, DescriptorPool, Device, FrameSync, Instance, Limits, MemoryType, PhysicalDevice,
    Primitive, QueueGroup, Surface, Swapchain, SwapchainConfig, window::Extent2D,
};

use hal::format::{AsFormat, ChannelType, Rgba8Srgb as ColorFormat, Swizzle};
use hal::pass::Subpass;
use hal::pso::{PipelineStage, ShaderStageFlags, Specialization};
use hal::queue::Submission;

use std::fs;
use std::io::Read;

const ENTRY_NAME: &str = "main";
const DIMS: Extent2D = Extent2D { width: 1024, height: 768 };

#[derive(Debug, Clone, Copy)]
struct Vertex {
    a_pos: [f32; 2],
    a_uv: [f32; 2],
}

#[cfg(not(feature = "gl"))]
type WindowType = winit::Window;

trait SurfaceTrait {
    #[cfg(feature = "gl")]
    fn get_window_t(&self) -> &back::glutin::GlWindow;
}

impl SurfaceTrait for <back::Backend as hal::Backend>::Surface {
    #[cfg(feature = "gl")]
    fn get_window_t(&self) -> &back::glutin::GlWindow {
        self.get_window()
    }
}

const QUAD: [Vertex; 6] = [
    Vertex { a_pos: [ -0.5, 0.33 ], a_uv: [0.0, 1.0] },
    Vertex { a_pos: [  0.5, 0.33 ], a_uv: [1.0, 1.0] },
    Vertex { a_pos: [  0.5,-0.33 ], a_uv: [1.0, 0.0] },

    Vertex { a_pos: [ -0.5, 0.33 ], a_uv: [0.0, 1.0] },
    Vertex { a_pos: [  0.5,-0.33 ], a_uv: [1.0, 0.0] },
    Vertex { a_pos: [ -0.5,-0.33 ], a_uv: [0.0, 0.0] },
];

const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: f::Aspects::COLOR,
    levels: 0..1,
    layers: 0..1,
};

struct RendererState<B: Backend>
where
    B::Surface: SurfaceTrait {
    uniform_desc_pool: Option<B::DescriptorPool>,
    img_desc_pool: Option<B::DescriptorPool>,
    swapchain: Option<SwapchainState<B>>,
    device: Rc<RefCell<DeviceState<B>>>,
    backend: BackendState<B>,
    window: WindowState,
    vertex_buffer: BufferState<B>,
    render_pass: RenderPassState<B>,
    uniform: Uniform<B>,
    pipeline: PipelineState<B>,
    framebuffer: FramebufferState<B>,
    viewport: pso::Viewport,
    image: ImageState<B>,
}

#[derive(Debug)]
enum Color {
    Red,
    Green,
    Blue,
    Alpha,
}

impl<B: Backend> RendererState<B>
where
    B::Surface: SurfaceTrait {
    fn new(mut backend: BackendState<B>, window: WindowState) -> Self {
        let device = Rc::new(RefCell::new(DeviceState::new(
            backend.adapter.adapter.take().unwrap(),
            &backend.surface,
        )));

        let image_desc = DescSetLayout::new(
            Rc::clone(&device),
            vec![
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
        );

        let uniform_desc = DescSetLayout::new(
            Rc::clone(&device),
            vec![pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::UniformBuffer,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            }],
        );

        let mut img_desc_pool = Some(device.borrow().device.create_descriptor_pool(
            1, // # of sets
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
        ));

        let mut uniform_desc_pool = Some(device.borrow().device.create_descriptor_pool(
            1, // # of sets
            &[pso::DescriptorRangeDesc {
                ty: pso::DescriptorType::UniformBuffer,
                count: 1,
            }],
        ));

        let image_desc = image_desc.create_desc_set(img_desc_pool.as_mut().unwrap());
        let uniform_desc = uniform_desc.create_desc_set(uniform_desc_pool.as_mut().unwrap());

        println!("Memory types: {:?}", backend.adapter.memory_types);

        const IMAGE_LOGO: &'static [u8] = include_bytes!("data/logo.png");
        let img = image::load(Cursor::new(&IMAGE_LOGO[..]), image::PNG)
            .unwrap()
            .to_rgba();

        let mut staging_pool = device.borrow().device.create_command_pool_typed(
            &device.borrow().queues,
            pool::CommandPoolCreateFlags::empty(),
            16,
        );

        let image = ImageState::new::<hal::Graphics>(
            image_desc,
            &img,
            &backend.adapter,
            buffer::Usage::TRANSFER_SRC,
            &mut device.borrow_mut(),
            &mut staging_pool,
        );

        let vertex_buffer = BufferState::new::<Vertex>(
            Rc::clone(&device),
            &QUAD,
            buffer::Usage::VERTEX,
            &backend.adapter.memory_types,
        );

        let uniform = Uniform::new(
            Rc::clone(&device),
            &backend.adapter.memory_types,
            &[1f32, 1.0f32, 1.0f32, 1.0f32],
            uniform_desc,
            0,
        );

        image.wait_for_transfer_completion();

        device
            .borrow()
            .device
            .destroy_command_pool(staging_pool.into_raw());

        let mut swapchain = Some(SwapchainState::new(
            &mut backend,
            Rc::clone(&device),
        ));

        let render_pass = RenderPassState::new(
            swapchain.as_ref().unwrap(),
            Rc::clone(&device),
        );

        let framebuffer = FramebufferState::new(
            Rc::clone(&device),
            &render_pass,
            swapchain.as_mut().unwrap(),
        );

        let pipeline = PipelineState::new(
            vec![image.get_layout(), uniform.get_layout()],
            render_pass.render_pass.as_ref().unwrap(),
            Rc::clone(&device),
        );

        let viewport = RendererState::create_viewport(swapchain.as_ref().unwrap());

        RendererState {
            window,
            backend,
            device,
            image,
            img_desc_pool,
            uniform_desc_pool,
            vertex_buffer,
            uniform,
            render_pass,
            pipeline,
            swapchain,
            framebuffer,
            viewport,
        }
    }

    fn recreate_swapchain(&mut self) {
        self.device.borrow().device.wait_idle().unwrap();

        self.swapchain.take().unwrap();

        self.swapchain = Some(SwapchainState::new(
            &mut self.backend,
            Rc::clone(&self.device),
        ));

        self.render_pass = RenderPassState::new(
            self.swapchain.as_ref().unwrap(),
            Rc::clone(&self.device),
        );

        self.framebuffer = FramebufferState::new(
            Rc::clone(&self.device),
            &self.render_pass,
            self.swapchain.as_mut().unwrap(),
        );

        self.pipeline = PipelineState::new(
            vec![self.image.get_layout(), self.uniform.get_layout()],
            self.render_pass.render_pass.as_ref().unwrap(),
            Rc::clone(&self.device),
        );

        self.viewport = RendererState::create_viewport(self.swapchain.as_ref().unwrap());
    }

    fn create_viewport(swapchain: &SwapchainState<B>) -> pso::Viewport {
        pso::Viewport {
            rect: pso::Rect {
                x: 0,
                y: 0,
                w: swapchain.extent.width as i16,
                h: swapchain.extent.height as i16,
            },
            depth: 0.0..1.0,
        }
    }

    fn mainloop(&mut self) {
        let mut running = true;
        let mut recreate_swapchain = false;

        let mut r = 1.0f32;
        let mut g = 1.0f32;
        let mut b = 1.0f32;
        let mut a = 1.0f32;

        let mut cr = 0.8;
        let mut cg = 0.8;
        let mut cb = 0.8;

        let mut cur_color = Color::Red;
        let mut cur_value: u32 = 0;

        println!("\nInstructions:");
        println!("\tChoose whether to change the (R)ed, (G)reen or (B)lue color by pressing the appropriate key.");
        println!("\tType in the value you want to change it to, where 0 is nothing, 255 is normal and 510 is double, ect.");
        println!("\tThen press C to change the (C)lear colour or (Enter) for the image color.");
        println!(
            "\tSet {:?} color to: {} (press enter/C to confirm)",
            cur_color, cur_value
        );

        while running {
            {
                let uniform = &mut self.uniform;
                #[cfg(feature = "gl")]
                let backend = &self.backend;

                self.window.events_loop.poll_events(|event| {
                    if let winit::Event::WindowEvent { event, .. } = event {
                        #[allow(unused_variables)]
                        match event {
                            winit::WindowEvent::KeyboardInput {
                                input:
                                    winit::KeyboardInput {
                                        virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                                        ..
                                    },
                                ..
                            }
                            | winit::WindowEvent::CloseRequested => running = false,
                            winit::WindowEvent::Resized(dims) => {
                                #[cfg(feature = "gl")]
                                backend
                                    .surface
                                    .get_window_t()
                                    .resize(
                                        dims.to_physical(
                                            backend.surface.get_window_t().get_hidpi_factor()
                                        )
                                    );
                                recreate_swapchain = true;
                            }
                            winit::WindowEvent::KeyboardInput {
                                input:
                                    winit::KeyboardInput {
                                        virtual_keycode,
                                        state: winit::ElementState::Pressed,
                                        ..
                                    },
                                ..
                            } => {
                                if let Some(kc) = virtual_keycode {
                                    match kc {
                                        winit::VirtualKeyCode::Key0 => {
                                            cur_value = cur_value * 10 + 0
                                        }
                                        winit::VirtualKeyCode::Key1 => {
                                            cur_value = cur_value * 10 + 1
                                        }
                                        winit::VirtualKeyCode::Key2 => {
                                            cur_value = cur_value * 10 + 2
                                        }
                                        winit::VirtualKeyCode::Key3 => {
                                            cur_value = cur_value * 10 + 3
                                        }
                                        winit::VirtualKeyCode::Key4 => {
                                            cur_value = cur_value * 10 + 4
                                        }
                                        winit::VirtualKeyCode::Key5 => {
                                            cur_value = cur_value * 10 + 5
                                        }
                                        winit::VirtualKeyCode::Key6 => {
                                            cur_value = cur_value * 10 + 6
                                        }
                                        winit::VirtualKeyCode::Key7 => {
                                            cur_value = cur_value * 10 + 7
                                        }
                                        winit::VirtualKeyCode::Key8 => {
                                            cur_value = cur_value * 10 + 8
                                        }
                                        winit::VirtualKeyCode::Key9 => {
                                            cur_value = cur_value * 10 + 9
                                        }
                                        winit::VirtualKeyCode::R => {
                                            cur_value = 0;
                                            cur_color = Color::Red
                                        }
                                        winit::VirtualKeyCode::G => {
                                            cur_value = 0;
                                            cur_color = Color::Green
                                        }
                                        winit::VirtualKeyCode::B => {
                                            cur_value = 0;
                                            cur_color = Color::Blue
                                        }
                                        winit::VirtualKeyCode::A => {
                                            cur_value = 0;
                                            cur_color = Color::Alpha
                                        }
                                        winit::VirtualKeyCode::Return => {
                                            match cur_color {
                                                Color::Red => r = cur_value as f32 / 255.0,
                                                Color::Green => g = cur_value as f32 / 255.0,
                                                Color::Blue => b = cur_value as f32 / 255.0,
                                                Color::Alpha => a = cur_value as f32 / 255.0,
                                            }
                                            uniform.buffer.as_mut().unwrap().update_data(0, &[r, g, b, a]);
                                            cur_value = 0;

                                            println!("Colour updated!");
                                        }
                                        winit::VirtualKeyCode::C => {
                                            match cur_color {
                                                Color::Red => cr = cur_value as f32 / 255.0,
                                                Color::Green => cg = cur_value as f32 / 255.0,
                                                Color::Blue => cb = cur_value as f32 / 255.0,
                                                Color::Alpha => {
                                                    error!(
                                                        "Alpha is not valid for the background."
                                                    );
                                                    return;
                                                }
                                            }
                                            cur_value = 0;

                                            println!("Background color updated!");
                                        }
                                        _ => return,
                                    }
                                    println!(
                                        "Set {:?} color to: {} (press enter/C to confirm)",
                                        cur_color, cur_value
                                    )
                                }
                            }
                            _ => (),
                        }
                    }
                });
            }

            if recreate_swapchain {
                self.recreate_swapchain();
                recreate_swapchain = false;
            }

            let sem_index = self.framebuffer.next_acq_pre_pair_index();

            let frame: hal::SwapImageIndex = {
                let (acquire_semaphore, _) = self
                    .framebuffer
                    .get_frame_data(None, Some(sem_index))
                    .1
                    .unwrap();
                match self
                    .swapchain
                    .as_mut()
                    .unwrap()
                    .swapchain
                    .as_mut()
                    .unwrap()
                    .acquire_image(FrameSync::Semaphore(acquire_semaphore))
                {
                    Ok(i) => i,
                    Err(_) => {
                        recreate_swapchain = true;
                        continue;
                    }
                }
            };

            let (fid, sid) = self
                .framebuffer
                .get_frame_data(Some(frame as usize), Some(sem_index));

            let (framebuffer_fence, framebuffer, command_pool) = fid.unwrap();
            let (image_acquired, image_present) = sid.unwrap();

            self.device
                .borrow()
                .device
                .wait_for_fence(framebuffer_fence, !0);
            self.device.borrow().device.reset_fence(framebuffer_fence);
            command_pool.reset();

            // Rendering
            let submit = {
                let mut cmd_buffer = command_pool.acquire_command_buffer(false);

                cmd_buffer.set_viewports(0, &[self.viewport.clone()]);
                cmd_buffer.set_scissors(0, &[self.viewport.rect]);
                cmd_buffer.bind_graphics_pipeline(self.pipeline.pipeline.as_ref().unwrap());
                cmd_buffer.bind_vertex_buffers(
                    0,
                    Some((self.vertex_buffer.get_buffer(), 0)),
                );
                cmd_buffer.bind_graphics_descriptor_sets(
                    self.pipeline.pipeline_layout.as_ref().unwrap(),
                    0,
                    vec![self.image.desc.set.as_ref().unwrap(), self.uniform.desc.as_ref().unwrap().set.as_ref().unwrap()],
                    &[],
                ); //TODO

                {
                    let mut encoder = cmd_buffer.begin_render_pass_inline(
                        self.render_pass.render_pass.as_ref().unwrap(),
                        framebuffer,
                        self.viewport.rect,
                        &[command::ClearValue::Color(command::ClearColor::Float([
                            cr, cg, cb, 1.0,
                        ]))],
                    );
                    encoder.draw(0..6, 0..1);
                }

                cmd_buffer.finish()
            };

            let submission = Submission::new()
                .wait_on(&[(&*image_acquired, PipelineStage::BOTTOM_OF_PIPE)])
                .signal(&[&*image_present])
                .submit(Some(submit));
            self.device.borrow_mut().queues.queues[0].submit(submission, Some(framebuffer_fence));

            // present frame
            if let Err(_) = self.swapchain.as_ref().unwrap().swapchain.as_ref().unwrap().present(
                &mut self.device.borrow_mut().queues.queues[0],
                frame,
                Some(&*image_present),
            ) {
                recreate_swapchain = true;
                continue;
            }
        }
    }
}

impl<B: Backend> Drop for RendererState<B>
where
    B::Surface: SurfaceTrait {
    fn drop(&mut self) {
        self.device.borrow().device.wait_idle().unwrap();
        self.device
            .borrow()
            .device
            .destroy_descriptor_pool(self.img_desc_pool.take().unwrap());
        self.device
            .borrow()
            .device
            .destroy_descriptor_pool(self.uniform_desc_pool.take().unwrap());
        self.swapchain.take();
    }
}

struct WindowState {
    events_loop: winit::EventsLoop,
    wb: Option<winit::WindowBuilder>,
}

impl WindowState {
    fn new() -> WindowState {
        let events_loop = winit::EventsLoop::new();

        let wb = winit::WindowBuilder::new()
            .with_dimensions(winit::dpi::LogicalSize::from_physical(winit::dpi::PhysicalSize {
                width: DIMS.width as _,
                height: DIMS.height as _,
            }, 1.0))
            .with_title("quad".to_string());

        WindowState {
            events_loop,
            wb: Some(wb),
        }
    }
}

struct BackendState<B: Backend> {
    surface: B::Surface,
    adapter: AdapterState<B>,
    #[cfg(not(feature = "gl"))]
    window: WindowType,
}

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
fn create_backend(window_state: &mut WindowState) -> (BackendState<back::Backend>, back::Instance) {
    let window = window_state.wb.take().unwrap()
        .build(&window_state.events_loop)
        .unwrap();
    let instance = back::Instance::create("gfx-rs quad", 1);
    let surface = instance.create_surface(&window);
    let mut adapters = instance.enumerate_adapters();
    (
        BackendState {
            adapter: AdapterState::new(&mut adapters),
            surface,
            window,
        },
        instance,
    )
}

#[cfg(feature = "gl")]
fn create_backend(window_state: &mut WindowState) -> (BackendState<back::Backend>, ()) {
    let window = {
        let builder =
            back::config_context(
                back::glutin::ContextBuilder::new(),
                ColorFormat::SELF,
                None,
            )
            .with_vsync(true);
        back::glutin::GlWindow::new(
            window_state.wb.take().unwrap(),
            builder,
            &window_state.events_loop,
        ).unwrap()
    };

    let surface = back::Surface::from_window(window);
    let mut adapters = surface.enumerate_adapters();
    (
        BackendState {
            adapter: AdapterState::new(&mut adapters),
            surface,
        },
        (),
    )
}

struct AdapterState<B: Backend> {
    adapter: Option<Adapter<B>>,
    memory_types: Vec<MemoryType>,
    limits: Limits,
}

impl<B: Backend> AdapterState<B> {
    fn new(adapters: &mut Vec<Adapter<B>>) -> Self {
        print!("Chosen: ");

        for adapter in adapters.iter() {
            println!("{:?}", adapter.info);
        }

        AdapterState::<B>::new_adapter(adapters.remove(0))
    }

    fn new_adapter(adapter: Adapter<B>) -> Self {
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();
        println!("{:?}", limits);

        AdapterState {
            adapter: Some(adapter),
            memory_types,
            limits,
        }
    }
}

struct DeviceState<B: Backend> {
    device: B::Device,
    physical_device: B::PhysicalDevice,
    queues: QueueGroup<B, ::hal::Graphics>,
}

impl<B: Backend> DeviceState<B> {
    fn new(mut adapter: Adapter<B>, surface: &B::Surface) -> Self {
        let (device, queues) = adapter
            .open_with::<_, ::hal::Graphics>(1, |family| surface.supports_queue_family(family))
            .unwrap();

        DeviceState { device, queues, physical_device: adapter.physical_device }
    }
}

struct RenderPassState<B: Backend>
where
    B::Surface: SurfaceTrait {
    render_pass: Option<B::RenderPass>,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> RenderPassState<B>
where
    B::Surface: SurfaceTrait {
    fn new(swapchain: &SwapchainState<B>, device: Rc<RefCell<DeviceState<B>>>) -> Self {
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(swapchain.format.clone()),
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

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: i::Access::empty()
                    ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            };

            device
                .borrow()
                .device
                .create_render_pass(&[attachment], &[subpass], &[dependency])
        };

        RenderPassState {
            render_pass: Some(render_pass),
            device,
        }
    }
}

impl<B: Backend> Drop for RenderPassState<B>
where
    B::Surface: SurfaceTrait {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        device.destroy_render_pass(self.render_pass.take().unwrap());
    }
}

struct BufferState<B: Backend> {
    memory: Option<B::Memory>,
    buffer: Option<B::Buffer>,
    device: Rc<RefCell<DeviceState<B>>>,
    size: u64,
}

impl<B: Backend> BufferState<B> {
    fn get_buffer(&self) -> &B::Buffer {
        self.buffer.as_ref().unwrap()
    }

    fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        data_source: &[T],
        usage: buffer::Usage,
        memory_types: &[MemoryType],
    ) -> Self
    where
        T: Copy,
    {
        let memory: B::Memory;
        let buffer: B::Buffer;

        let stride = size_of::<T>() as u64;
        let upload_size = data_source.len() as u64 * stride;

        {
            let device = &device_ptr.borrow().device;

            let unbound = device.create_buffer(upload_size, usage).unwrap();
            let mem_req = device.get_buffer_requirements(&unbound);

            let upload_type = memory_types
                .iter()
                .enumerate()
                .position(|(id, mem_type)| {
                    mem_req.type_mask & (1 << id) != 0
                        && mem_type.properties.contains(m::Properties::CPU_VISIBLE)
                })
                .unwrap()
                .into();

            memory = device.allocate_memory(upload_type, mem_req.size).unwrap();
            buffer = device.bind_buffer_memory(&memory, 0, unbound).unwrap();

            // TODO: check transitions: read/write mapping and vertex buffer read
            {
                let mut data_target = device
                    .acquire_mapping_writer::<T>(&memory, 0..upload_size)
                    .unwrap();
                data_target.copy_from_slice(data_source);
                device.release_mapping_writer(data_target);
            }
        }

        BufferState {
            memory: Some(memory),
            buffer: Some(buffer),
            device: device_ptr,
            size: upload_size,
        }
    }

    fn update_data<T>(&mut self, offset: u64, data_source: &[T])
    where
        T: Copy,
    {
        let device = &self.device.borrow().device;

        let stride = size_of::<T>() as u64;
        let upload_size = data_source.len() as u64 * stride;

        assert!(offset + upload_size <= self.size);

        let mut data_target = device
            .acquire_mapping_writer::<T>(self.memory.as_ref().unwrap(), offset..offset + upload_size)
            .unwrap();
        data_target.copy_from_slice(data_source);
        device.release_mapping_writer(data_target);
    }

    fn new_texture(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        device: &mut B::Device,
        img: &::image::ImageBuffer<::image::Rgba<u8>, Vec<u8>>,
        adapter: &AdapterState<B>,
        usage: buffer::Usage,
    ) -> (Self, Dimensions<u32>, u32, usize) {
        let (width, height) = img.dimensions();

        let row_alignment_mask = adapter.limits.min_buffer_copy_pitch_alignment as u32 - 1;
        let stride = 4usize;

        let row_pitch = (width * stride as u32 + row_alignment_mask) & !row_alignment_mask;
        let upload_size = (height * row_pitch) as u64;

        let memory: B::Memory;
        let buffer: B::Buffer;

        {
            let unbound = device.create_buffer(upload_size, usage).unwrap();
            let mem_reqs = device.get_buffer_requirements(&unbound);

            let upload_type = adapter
                .memory_types
                .iter()
                .enumerate()
                .position(|(id, mem_type)| {
                    mem_reqs.type_mask & (1 << id) != 0
                        && mem_type.properties.contains(m::Properties::CPU_VISIBLE)
                })
                .unwrap()
                .into();

            memory = device.allocate_memory(upload_type, mem_reqs.size).unwrap();
            buffer = device.bind_buffer_memory(&memory, 0, unbound).unwrap();

            // copy image data into staging buffer
            {
                let mut data_target = device
                    .acquire_mapping_writer::<u8>(&memory, 0..upload_size)
                    .unwrap();

                for y in 0..height as usize {
                    let data_source_slice = &(**img)
                        [y * (width as usize) * stride..(y + 1) * (width as usize) * stride];
                    let dest_base = y * row_pitch as usize;

                    data_target[dest_base..dest_base + data_source_slice.len()]
                        .copy_from_slice(data_source_slice);
                }

                device.release_mapping_writer(data_target);
            }
        }

        (
            BufferState {
                memory: Some(memory),
                buffer: Some(buffer),
                device: device_ptr,
                size: upload_size,
            },
            Dimensions { width, height },
            row_pitch,
            stride,
        )
    }
}

impl<B: Backend> Drop for BufferState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        device.destroy_buffer(self.buffer.take().unwrap());
        device.free_memory(self.memory.take().unwrap());
    }
}

struct Uniform<B: Backend> {
    buffer: Option<BufferState<B>>,
    desc: Option<DescSet<B>>,
}

impl<B: Backend> Uniform<B> {
    fn new<T>(
        device: Rc<RefCell<DeviceState<B>>>,
        memory_types: &[MemoryType],
        data: &[T],
        mut desc: DescSet<B>,
        binding: u32,
    ) -> Self
    where
        T: Copy,
    {
        let buffer = BufferState::new(
            Rc::clone(&device),
            &data,
            buffer::Usage::UNIFORM,
            memory_types,
        );
        let buffer = Some(buffer);

        desc.write_to_state(
            vec![DescSetWrite {
                binding,
                array_offset: 0,
                descriptors: Some(pso::Descriptor::Buffer(buffer.as_ref().unwrap().get_buffer(), None..None)),
            }],
            &mut device.borrow_mut().device,
        );

        Uniform {
            buffer,
            desc: Some(desc),
        }
    }

    fn get_layout(&self) -> &B::DescriptorSetLayout {
        self.desc.as_ref().unwrap().get_layout()
    }
}

struct DescSetLayout<B: Backend> {
    layout: Option<B::DescriptorSetLayout>,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> DescSetLayout<B> {
    fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        bindings: Vec<pso::DescriptorSetLayoutBinding>,
    ) -> Self {
        let desc_set_layout = device
            .borrow()
            .device
            .create_descriptor_set_layout(bindings, &[]);

        DescSetLayout {
            layout: Some(desc_set_layout),
            device,
        }
    }

    fn create_desc_set(self, desc_pool: &mut B::DescriptorPool) -> DescSet<B> {
        let desc_set = desc_pool.allocate_set(self.layout.as_ref().unwrap()).unwrap();
        DescSet {
            layout: self,
            set: Some(desc_set),
        }
    }
}

impl<B: Backend> Drop for DescSetLayout<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        device.destroy_descriptor_set_layout(self.layout.take().unwrap());
    }
}

struct DescSet<B: Backend> {
    set: Option<B::DescriptorSet>,
    layout: DescSetLayout<B>,
}

struct DescSetWrite<W> {
    binding: pso::DescriptorBinding,
    array_offset: pso::DescriptorArrayIndex,
    descriptors: W,
}

impl<B: Backend> DescSet<B> {
    fn write_to_state<'a, 'b: 'a, W>(
        &'b mut self,
        write: Vec<DescSetWrite<W>>,
        device: &mut B::Device,
    ) where
        W: IntoIterator,
        W::Item: std::borrow::Borrow<pso::Descriptor<'a, B>>,
    {
        let set = self.set.as_ref().unwrap();
        let write: Vec<_> = write
            .into_iter()
            .map(|d| pso::DescriptorSetWrite {
                binding: d.binding,
                array_offset: d.array_offset,
                descriptors: d.descriptors,
                set,
            })
            .collect();
        device.write_descriptor_sets(write);
    }

    fn get_layout(&self) -> &B::DescriptorSetLayout {
        self.layout.layout.as_ref().unwrap()
    }
}

struct ImageState<B: Backend> {
    desc: DescSet<B>,
    buffer: Option<BufferState<B>>,
    sampler: Option<B::Sampler>,
    image_view: Option<B::ImageView>,
    image: Option<B::Image>,
    memory: Option<B::Memory>,
    transfered_image_fence: Option<B::Fence>,
}

impl<B: Backend> ImageState<B> {
    fn new<T: ::hal::Supports<::hal::Transfer>>(
        mut desc: DescSet<B>,
        img: &::image::ImageBuffer<::image::Rgba<u8>, Vec<u8>>,
        adapter: &AdapterState<B>,
        usage: buffer::Usage,
        device_state: &mut DeviceState<B>,
        staging_pool: &mut ::hal::CommandPool<B, ::hal::Graphics>,
    ) -> Self {
        let (buffer, dims, row_pitch, stride) = BufferState::new_texture(
            Rc::clone(&desc.layout.device),
            &mut device_state.device,
            img,
            adapter,
            usage,
        );

        let buffer = Some(buffer);
        let device = &mut device_state.device;

        let kind = i::Kind::D2(dims.width as i::Size, dims.height as i::Size, 1, 1);
        let unbound = device
            .create_image(
                kind,
                1,
                ColorFormat::SELF,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_DST | i::Usage::SAMPLED,
                i::StorageFlags::empty(),
            )
            .unwrap(); // TODO: usage
        let req = device.get_image_requirements(&unbound);

        let device_type = adapter
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                req.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(m::Properties::DEVICE_LOCAL)
            })
            .unwrap()
            .into();

        let memory = device.allocate_memory(device_type, req.size).unwrap();

        let image = device.bind_image_memory(&memory, 0, unbound).unwrap();
        let image_view = device
            .create_image_view(
                &image,
                i::ViewKind::D2,
                ColorFormat::SELF,
                Swizzle::NO,
                COLOR_RANGE.clone(),
            )
            .unwrap();

        let sampler =
            device.create_sampler(i::SamplerInfo::new(i::Filter::Linear, i::WrapMode::Clamp));

        desc.write_to_state(
            vec![
                DescSetWrite {
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(&image_view, i::Layout::Undefined)),
                },
                DescSetWrite {
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Sampler(&sampler)),
                },
            ],
            device,
        );

        let mut transfered_image_fence = device.create_fence(false);

        // copy buffer to texture
        {
            let submit = {
                let mut cmd_buffer = staging_pool.acquire_command_buffer(false);

                let image_barrier = m::Barrier::Image {
                    states: (i::Access::empty(), i::Layout::Undefined)
                        ..(i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                    target: &image,
                    range: COLOR_RANGE.clone(),
                };

                cmd_buffer.pipeline_barrier(
                    PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                    m::Dependencies::empty(),
                    &[image_barrier],
                );

                cmd_buffer.copy_buffer_to_image(
                    buffer.as_ref().unwrap().get_buffer(),
                    &image,
                    i::Layout::TransferDstOptimal,
                    &[command::BufferImageCopy {
                        buffer_offset: 0,
                        buffer_width: row_pitch / (stride as u32),
                        buffer_height: dims.height as u32,
                        image_layers: i::SubresourceLayers {
                            aspects: f::Aspects::COLOR,
                            level: 0,
                            layers: 0..1,
                        },
                        image_offset: i::Offset { x: 0, y: 0, z: 0 },
                        image_extent: i::Extent {
                            width: dims.width,
                            height: dims.height,
                            depth: 1,
                        },
                    }],
                );

                let image_barrier = m::Barrier::Image {
                    states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                        ..(i::Access::SHADER_READ, i::Layout::ShaderReadOnlyOptimal),
                    target: &image,
                    range: COLOR_RANGE.clone(),
                };
                cmd_buffer.pipeline_barrier(
                    PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                    m::Dependencies::empty(),
                    &[image_barrier],
                );

                cmd_buffer.finish()
            };

            let submission = Submission::new().submit(Some(submit));
            device_state.queues.queues[0].submit(submission, Some(&mut transfered_image_fence));
        }

        ImageState {
            desc: desc,
            buffer: buffer,
            sampler: Some(sampler),
            image_view: Some(image_view),
            image: Some(image),
            memory: Some(memory),
            transfered_image_fence: Some(transfered_image_fence),
        }
    }

    fn wait_for_transfer_completion(&self) {
        let device = &self.desc.layout.device.borrow().device;
        device.wait_for_fence(self.transfered_image_fence.as_ref().unwrap(), !0);
    }

    fn get_layout(&self) -> &B::DescriptorSetLayout {
        self.desc.get_layout()
    }
}

impl<B: Backend> Drop for ImageState<B> {
    fn drop(&mut self) {
        {
            let device = &self.desc.layout.device.borrow().device;

            let fence = self.transfered_image_fence.take().unwrap();
            device.wait_for_fence(&fence, !0);
            device.destroy_fence(fence);

            device.destroy_sampler(self.sampler.take().unwrap());
            device.destroy_image_view(self.image_view.take().unwrap());
            device.destroy_image(self.image.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }

        self.buffer.take().unwrap();
    }
}

struct PipelineState<B: Backend> {
    pipeline: Option<B::GraphicsPipeline>,
    pipeline_layout: Option<B::PipelineLayout>,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> PipelineState<B> {
    fn new<IS>(
        desc_layouts: IS,
        render_pass: &B::RenderPass,
        device_ptr: Rc<RefCell<DeviceState<B>>>,
    ) -> Self
    where
        IS: IntoIterator,
        IS::Item: std::borrow::Borrow<B::DescriptorSetLayout>,
    {
        let device = &device_ptr.borrow().device;
        let pipeline_layout =
            device.create_pipeline_layout(desc_layouts, &[(pso::ShaderStageFlags::VERTEX, 0..8)]);

        let pipeline = {
            let vs_module = {
                let glsl = fs::read_to_string("colour-uniform/data/quad.vert").unwrap();
                let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Vertex)
                    .unwrap()
                    .bytes()
                    .map(|b| b.unwrap())
                    .collect();
                device.create_shader_module(&spirv).unwrap()
            };
            let fs_module = {
                let glsl = fs::read_to_string("colour-uniform/data/quad.frag").unwrap();
                let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Fragment)
                    .unwrap()
                    .bytes()
                    .map(|b| b.unwrap())
                    .collect();
                device.create_shader_module(&spirv).unwrap()
            };

            let pipeline = {
                let (vs_entry, fs_entry) = (
                    pso::EntryPoint::<B> {
                        entry: ENTRY_NAME,
                        module: &vs_module,
                        specialization: &[Specialization {
                            id: 0,
                            value: pso::Constant::F32(0.8),
                        }],
                    },
                    pso::EntryPoint::<B> {
                        entry: ENTRY_NAME,
                        module: &fs_module,
                        specialization: &[],
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
                    main_pass: render_pass,
                };

                let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
                    shader_entries,
                    Primitive::TriangleList,
                    pso::Rasterizer::FILL,
                    &pipeline_layout,
                    subpass,
                );
                pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
                    pso::ColorMask::ALL,
                    pso::BlendState::ALPHA,
                ));
                pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
                    binding: 0,
                    stride: size_of::<Vertex>() as u32,
                    rate: 0,
                });

                pipeline_desc.attributes.push(pso::AttributeDesc {
                    location: 0,
                    binding: 0,
                    element: pso::Element {
                        format: f::Format::Rg32Float,
                        offset: 0,
                    },
                });
                pipeline_desc.attributes.push(pso::AttributeDesc {
                    location: 1,
                    binding: 0,
                    element: pso::Element {
                        format: f::Format::Rg32Float,
                        offset: 8,
                    },
                });

                device.create_graphics_pipeline(&pipeline_desc)
            };

            device.destroy_shader_module(vs_module);
            device.destroy_shader_module(fs_module);

            pipeline.unwrap()
        };

        PipelineState {
            pipeline: Some(pipeline),
            pipeline_layout: Some(pipeline_layout),
            device: Rc::clone(&device_ptr),
        }
    }
}

impl<B: Backend> Drop for PipelineState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        device.destroy_graphics_pipeline(self.pipeline.take().unwrap());
        device.destroy_pipeline_layout(self.pipeline_layout.take().unwrap());
    }
}

struct SwapchainState<B: Backend>
where
    B::Surface: SurfaceTrait {
    swapchain: Option<B::Swapchain>,
    backbuffer: Option<Backbuffer<B>>,
    device: Rc<RefCell<DeviceState<B>>>,
    extent: hal::window::Extent2D,
    format: f::Format,
}

impl<B: Backend> SwapchainState<B>
where
    B::Surface: SurfaceTrait {
    fn new(
        backend: &mut BackendState<B>,
        device: Rc<RefCell<DeviceState<B>>>,
    ) -> Self {
        let (caps, formats, _present_modes) = backend.surface.compatibility(&device.borrow().physical_device);
        println!("formats: {:?}", formats);
        let format = formats
            .map_or(f::Format::Rgba8Srgb, |formats| {
                formats
                    .iter()
                    .find(|format| format.base_format().1 == ChannelType::Srgb)
                    .map(|format| *format)
                    .unwrap_or(formats[0])
            });

        let extent = match caps.current_extent {
            Some(e) => e,
            None => {
                #[cfg(not(feature = "gl"))]
                let window = &backend.window;
                #[cfg(feature = "gl")]
                let window = backend.surface.get_window_t();

                let window_size = window.get_inner_size().unwrap().to_physical(window.get_hidpi_factor());
                let mut extent = hal::window::Extent2D { width: window_size.width as _, height: window_size.height as _};

                extent.width = extent.width.max(caps.extents.start.width).min(caps.extents.end.width);
                extent.height = extent.height.max(caps.extents.start.height).min(caps.extents.end.height);

                extent
            }
        };

        println!("Surface format: {:?}", format);

        let swap_config = SwapchainConfig::new()
            .with_color(format)
            .with_image_count(caps.image_count.start)
            .with_image_usage(i::Usage::COLOR_ATTACHMENT);
        let (swapchain, backbuffer) = device.borrow().device.create_swapchain(
            &mut backend.surface,
            swap_config,
            None,
            &extent,
        );

        let swapchain = SwapchainState {
            swapchain: Some(swapchain),
            backbuffer: Some(backbuffer),
            device,
            extent,
            format,
        };
        swapchain
    }
}

impl<B: Backend> Drop for SwapchainState<B>
where
    B::Surface: SurfaceTrait {
    fn drop(&mut self) {
        self.device
            .borrow()
            .device
            .destroy_swapchain(self.swapchain.take().unwrap());
    }
}

struct FramebufferState<B: Backend>
where
    B::Surface: SurfaceTrait {
    framebuffers: Option<Vec<B::Framebuffer>>,
    framebuffer_fences: Option<Vec<B::Fence>>,
    command_pools: Option<Vec<hal::CommandPool<B, hal::Graphics>>>,
    frame_images: Option<Vec<(B::Image, B::ImageView)>>,
    acquire_semaphores: Option<Vec<B::Semaphore>>,
    present_semaphores: Option<Vec<B::Semaphore>>,
    last_ref: usize,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> FramebufferState<B>
where
    B::Surface: SurfaceTrait {
    fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        render_pass: &RenderPassState<B>,
        swapchain: &mut SwapchainState<B>,
    ) -> Self {
        let (frame_images, framebuffers) = match swapchain.backbuffer.take().unwrap() {
            Backbuffer::Images(images) => {
                let extent = i::Extent {
                    width: swapchain.extent.width as _,
                    height: swapchain.extent.height as _,
                    depth: 1,
                };
                let pairs = images
                    .into_iter()
                    .map(|image| {
                        let rtv = device
                            .borrow()
                            .device
                            .create_image_view(
                                &image,
                                i::ViewKind::D2,
                                swapchain.format,
                                Swizzle::NO,
                                COLOR_RANGE.clone(),
                            )
                            .unwrap();
                        (image, rtv)
                    })
                    .collect::<Vec<_>>();
                let fbos = pairs
                    .iter()
                    .map(|&(_, ref rtv)| {
                        device
                            .borrow()
                            .device
                            .create_framebuffer(render_pass.render_pass.as_ref().unwrap(), Some(rtv), extent)
                            .unwrap()
                    })
                    .collect();
                (pairs, fbos)
            }
            Backbuffer::Framebuffer(fbo) => (Vec::new(), vec![fbo]),
        };

        let iter_count = if frame_images.len() != 0 {
            frame_images.len()
        } else {
            1 // GL can have zero
        };

        let mut fences: Vec<B::Fence> = vec![];
        let mut command_pools: Vec<hal::CommandPool<B, hal::Graphics>> = vec![];
        let mut acquire_semaphores: Vec<B::Semaphore> = vec![];
        let mut present_semaphores: Vec<B::Semaphore> = vec![];

        for _ in 0..iter_count {
            fences.push(device.borrow().device.create_fence(true));
            command_pools.push(device.borrow().device.create_command_pool_typed(
                &device.borrow().queues,
                pool::CommandPoolCreateFlags::empty(),
                16,
            ));

            acquire_semaphores.push(device.borrow().device.create_semaphore());
            present_semaphores.push(device.borrow().device.create_semaphore());
        }

        FramebufferState {
            frame_images: Some(frame_images),
            framebuffers: Some(framebuffers),
            framebuffer_fences: Some(fences),
            command_pools: Some(command_pools),
            present_semaphores: Some(present_semaphores),
            acquire_semaphores: Some(acquire_semaphores),
            device,
            last_ref: 0,
        }
    }

    fn next_acq_pre_pair_index(&mut self) -> usize {
        if self.last_ref >= self.acquire_semaphores.as_ref().unwrap().len() {
            self.last_ref = 0
        }

        let ret = self.last_ref;
        self.last_ref += 1;
        ret
    }

    fn get_frame_data(
        &mut self,
        frame_id: Option<usize>,
        sem_index: Option<usize>,
    ) -> (
        Option<(
            &mut B::Fence,
            &mut B::Framebuffer,
            &mut hal::CommandPool<B, ::hal::Graphics>,
        )>,
        Option<(&mut B::Semaphore, &mut B::Semaphore)>,
    ) {
        (
            if let Some(fid) = frame_id {
                Some((
                    &mut self.framebuffer_fences.as_mut().unwrap()[fid],
                    &mut self.framebuffers.as_mut().unwrap()[fid],
                    &mut self.command_pools.as_mut().unwrap()[fid],
                ))
            } else {
                None
            },
            if let Some(sid) = sem_index {
                Some((
                    &mut self.acquire_semaphores.as_mut().unwrap()[sid],
                    &mut self.present_semaphores.as_mut().unwrap()[sid],
                ))
            } else {
                None
            },
        )
    }
}

impl<B: Backend> Drop for FramebufferState<B>
where
    B::Surface: SurfaceTrait {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;

        for fence in self.framebuffer_fences.take().unwrap() {
            device.wait_for_fence(&fence, !0);
            device.destroy_fence(fence);
        }

        for command_pool in self.command_pools.take().unwrap() {
            device.destroy_command_pool(command_pool.into_raw());
        }

        for acquire_semaphore in self.acquire_semaphores.take().unwrap() {
            device.destroy_semaphore(acquire_semaphore);
        }

        for present_semaphore in self.present_semaphores.take().unwrap() {
            device.destroy_semaphore(present_semaphore);
        }

        for framebuffer in self.framebuffers.take().unwrap() {
            device.destroy_framebuffer(framebuffer);
        }

        for (_, rtv) in self.frame_images.take().unwrap() {
            device.destroy_image_view(rtv);
        }
    }
}

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl"))]
fn main() {
    env_logger::init();

    let mut window = WindowState::new();
    let (backend, _instance) = create_backend(&mut window);

    let mut renderer_state =
        RendererState::new(backend, window);
    renderer_state.mainloop();
}

#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal", feature = "gl")))]
fn main() {
    println!("You need to enable the native API feature (vulkan/metal) in order to test the LL");
}
