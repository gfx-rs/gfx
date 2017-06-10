// Copyright 2016 The Gfx-rs Developers.
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

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate winit;
extern crate glutin;
extern crate gfx;
extern crate gfx_core;

#[cfg(feature = "gl")]
extern crate gfx_device_gl;
#[cfg(feature = "gl")]
extern crate gfx_window_glutin;

#[cfg(feature = "dx11")]
extern crate gfx_device_dx11;
#[cfg(feature = "dx11")]
extern crate gfx_window_dxgi;

#[cfg(feature = "metal")]
extern crate gfx_device_metal;
#[cfg(feature = "metal")]
extern crate gfx_window_metal;

#[cfg(feature = "vulkan")]
extern crate gfx_device_vulkan;
#[cfg(feature = "vulkan")]
extern crate gfx_window_vulkan;

use gfx_core::memory::Typed;
use gfx_core::{Adapter, Backend, CommandQueue, FrameSync, Surface, SwapChain, QueueFamily, WindowExt};
use gfx_core::pool::GraphicsCommandPool;

pub mod shade;

#[cfg(not(feature = "metal"))]
pub type ColorFormat = gfx::format::Rgba8;
#[cfg(feature = "metal")]
pub type ColorFormat = (gfx::format::B8_G8_R8_A8, gfx::format::Srgb);

#[cfg(feature = "metal")]
pub type DepthFormat = gfx::format::Depth32F;
#[cfg(not(feature = "metal"))]
pub type DepthFormat = gfx::format::DepthStencil;

pub type BackbufferView<R: gfx::Resources> = (gfx::handle::RenderTargetView<R, ColorFormat>,
                                              gfx::handle::DepthStencilView<R, DepthFormat>);

pub struct WindowTargets<R: gfx::Resources> {
    pub views: Vec<BackbufferView<R>>,
    pub aspect_ratio: f32,
}

struct Harness {
    start: std::time::Instant,
    num_frames: f64,
}

impl Harness {
    fn new() -> Harness {
        Harness {
            start: std::time::Instant::now(),
            num_frames: 0.0,
        }
    }
    fn bump(&mut self) {
        self.num_frames += 1.0;
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let time_end = self.start.elapsed();
        println!("Avg frame time: {} ms",
                 ((time_end.as_secs() * 1000) as f64 +
                  (time_end.subsec_nanos() / 1000_000) as f64) / self.num_frames);
    }
}

pub trait ApplicationBase<B: Backend, C: gfx::CommandBuffer<B::Resources>> {
    fn new(&mut B::Factory, shade::Backend, WindowTargets<B::Resources>) -> Self;
    fn render<D>(&mut self, &mut D);
    fn get_exit_key() -> Option<winit::VirtualKeyCode>;
    fn on(&mut self, winit::WindowEvent);
    fn on_resize(&mut self, &mut B::Factory, WindowTargets<B::Resources>);
}

#[cfg(feature = "dx11")]
pub type D3D11CommandBuffer = gfx_device_dx11::CommandBuffer<gfx_device_dx11::DeferredContext>;
#[cfg(feature = "dx11")]
pub type D3D11CommandBufferFake = gfx_device_dx11::CommandBuffer<gfx_device_dx11::CommandList>;

#[cfg(feature = "dx11")]
pub fn launch_d3d11<A>(wb: winit::WindowBuilder) where
A: Sized + Application<gfx_device_dx11::Backend>
{
    use gfx::traits::Factory;

    env_logger::init().unwrap();
    let events_loop = winit::EventsLoop::new();
    let (mut window, mut factory, main_color) =
        gfx_window_dxgi::init::<ColorFormat>(wb, &events_loop).unwrap();
    let main_depth = factory.create_depth_stencil_view_only(window.size.0, window.size.1)
                            .unwrap();

    let backend = shade::Backend::Hlsl(factory.get_shader_model());
    let mut app = A::new(&mut factory, backend, WindowTargets {
        colors: vec![main_color],
        depth: main_depth,
        aspect_ratio: window.size.0 as f32 / window.size.1 as f32,
    });
    // let mut device = gfx_device_dx11::Deferred::from(device);

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        let mut new_size = None;
        events_loop.poll_events(|winit::Event::WindowEvent{window_id: _, event}| {
            match event {
                winit::WindowEvent::Closed => running = false,
                winit::WindowEvent::KeyboardInput(winit::ElementState::Pressed, _, key, _) if key == A::get_exit_key() => return,
                winit::WindowEvent::Resized(width, height) => {
                    let size = (width as gfx::texture::Size, height as gfx::texture::Size);
                    if size != window.size {
                        // working around the borrow checker: window is already borrowed here
                        new_size = Some(size);
                    }
                },
                _ => app.on(event),
            }
        });
        if let Some((width, height)) = new_size {
            use gfx_window_dxgi::update_views;
            match update_views(&mut window, &mut factory, width, height) {
                Ok(new_color) => {
                    let new_depth = factory.create_depth_stencil_view_only(width, height).unwrap();
                    app.on_resize_ext(&mut factory, WindowTargets {
                        colors: vec![new_color],
                        depth: new_depth,
                        aspect_ratio: width as f32 / height as f32,
                    });
                },
                Err(e) => error!("Resize failed: {}", e),
            }
            continue;
        }
        // TODO: app.render_ext();
        window.swap_buffers(1);
        // device.cleanup();
        harness.bump();
    }
}

#[cfg(feature = "metal")]
pub fn launch_metal<A>(wb: winit::WindowBuilder) where
A: Sized + Application<gfx_device_metal::Backend>
{
    use gfx::traits::Factory;
    use gfx::texture::Size;

    env_logger::init().unwrap();
    let events_loop = winit::EventsLoop::new();
    let (window, mut device, mut factory, main_color) = gfx_window_metal::init::<ColorFormat>(wb, &events_loop)
                                                                                .unwrap();
    let (width, height) = window.get_inner_size_points().unwrap();
    let main_depth = factory.create_depth_stencil_view_only(width as Size, height as Size).unwrap();

    let backend = shade::Backend::Msl(device.get_shader_model());
    let mut app = A::new(&mut factory, backend, WindowTargets {
        color: main_color,
        depth: main_depth,
        aspect_ratio: width as f32 / height as f32
    });

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        events_loop.poll_events(|winit::Event::WindowEvent{window_id: _, event}| {
            match event {
                winit::WindowEvent::Closed => running = false,
                winit::WindowEvent::KeyboardInput(winit::ElementState::Pressed, _, key, _) if key == A::get_exit_key() => return,
                winit::WindowEvent::Resized(_width, _height) => {
                    warn!("TODO: resize on Metal");
                },
                _ => app.on(event),
            }
        });
        // TODO: app.render_ext();
        window.swap_buffers().unwrap();
        device.cleanup();
        harness.bump();
    }
}

fn run<A, B, S, EL>((width, height): (u32, u32),
                    events_loop: EL,
                    surface: S,
                    adapters: Vec<B::Adapter>)
    where A: Sized + Application<B>,
          B: Backend,
          S: gfx_core::Surface<B>,
          EL: EventsLoop,
          B::Factory: shade::ShadeExt,
{
    use shade::ShadeExt;
    use gfx::format::Formatted;
    use gfx::traits::Factory;
    use gfx::texture::{self, Size};

    // Init device
    let queue_descs = adapters[0].get_queue_families().iter()
                                 .filter(|family| surface.supports_queue(&family) )
                                 .map(|family| { (family, family.num_queues()) })
                                 .collect::<Vec<_>>();
    let gfx_core::Device_ { mut factory, mut general_queues, mut graphics_queues, .. } = adapters[0].open(&queue_descs);

    let mut queue = if let Some(queue) = general_queues.first_mut() {
        queue.as_mut().into()
    } else if let Some(queue) = graphics_queues.first_mut() {
        queue.as_mut()
    } else {
        error!("Unable to find a matching general or graphics queue.");
        return
    };

    let config = gfx_core::SwapchainConfig::new()
                    .with_color::<ColorFormat>()
                    .with_depth_stencil::<DepthFormat>();
    let mut swap_chain = surface.build_swapchain(config, &queue);

    let views =
        swap_chain
            .get_backbuffers()
            .iter()
            .map(|&(ref color, ref ds)| {
                let color_desc = texture::RenderDesc {
                    channel: ColorFormat::get_format().1,
                    level: 0,
                    layer: None,
                };
                let rtv = factory.view_texture_as_render_target_raw(color, color_desc)
                                 .unwrap();

                let ds_desc = texture::DepthStencilDesc {
                    level: 0,
                    layer: None,
                    flags: texture::RO_DEPTH_STENCIL,
                };
                let dsv = factory.view_texture_as_depth_stencil_raw(
                                    ds.as_ref().unwrap(),
                                    ds_desc)
                                 .unwrap();

                (Typed::new(rtv), Typed::new(dsv))
            })
            .collect();

    let shader_backend = factory.shader_backend();
    let mut app = A::new(&mut factory, shader_backend, WindowTargets {
        views: views,
        aspect_ratio: width as f32 / height as f32, //TODO
    });

    let mut harness = Harness::new();
    let mut running = true;
    let mut frame_semaphore = factory.create_semaphore();

    let mut graphics_pool = B::GraphicsCommandPool::from_queue(queue.as_ref(), 1);

    while running {
        events_loop.poll_events(|winit::Event::WindowEvent{window_id: _, event}| {
            match event {
                winit::WindowEvent::Closed => running = false,
                winit::WindowEvent::KeyboardInput(winit::ElementState::Pressed, _, key, _) if key == A::get_exit_key() => return,
                winit::WindowEvent::Resized(_width, _height) => {
                    warn!("TODO: resize not implemented");
                },
                _ => app.on(event),
            }
        });

        let frame = swap_chain.acquire_frame(FrameSync::Semaphore(&mut frame_semaphore));

        app.render((frame, &frame_semaphore), &mut graphics_pool, &mut queue);

        // Wait til rendering has finished
        queue.wait_idle();

        swap_chain.present(&mut queue);
        harness.bump();
    }
}

trait EventsLoop {
    fn poll_events<F>(&self, callback: F) where F: FnMut(winit::Event);
}

impl EventsLoop for winit::EventsLoop {
    fn poll_events<F>(&self, callback: F) where F: FnMut(winit::Event) {
        self.poll_events(callback)
    }
}

impl EventsLoop for glutin::EventsLoop {
    fn poll_events<F>(&self, callback: F) where F: FnMut(winit::Event) {
        self.poll_events(callback)
    }
}

#[cfg(all(feature = "gl", not(any(feature = "dx11", feature = "metal", feature = "vulkan"))))]
pub type DefaultBackend = gfx_device_gl::Backend;
#[cfg(feature = "dx11")]
pub type DefaultBackend = gfx_device_dx11::Backend;
#[cfg(feature = "metal")]
pub type DefaultBackend = gfx_device_metal::Backend;
#[cfg(feature = "vulkan")]
pub type DefaultBackend = gfx_device_vulkan::Backend;

pub trait Application<B: Backend>: Sized {
    fn new(&mut B::Factory, shade::Backend, WindowTargets<B::Resources>) -> Self;
    fn render<Gp>(&mut self, frame: (gfx_core::Frame, &<B::Resources as gfx::Resources>::Semaphore),
                     pool: &mut Gp, queue: &mut gfx_core::queue::GraphicsQueueMut<B>)
        where Gp: GraphicsCommandPool<B>;

    fn get_exit_key() -> Option<winit::VirtualKeyCode> {
        Some(winit::VirtualKeyCode::Escape)
    }
    fn on_resize(&mut self, WindowTargets<B::Resources>) {}
    fn on_resize_ext(&mut self, _factory: &mut B::Factory, targets: WindowTargets<B::Resources>) {
        self.on_resize(targets);
    }
    fn on(&mut self, _event: winit::WindowEvent) {}

    fn launch_simple(name: &str) where Self: Application<DefaultBackend> {
        env_logger::init().unwrap();
        let wb = winit::WindowBuilder::new().with_title(name);
        <Self as Application<DefaultBackend>>::launch_default(wb)
    }
    #[cfg(all(feature = "gl", not(any(feature = "dx11", feature = "metal", feature = "vulkan"))))]
    fn launch_default(wb: winit::WindowBuilder)
        where Self: Application<DefaultBackend>
    {
        use gfx_core::format::Formatted;

        let events_loop = glutin::EventsLoop::new();
        let gl_version = glutin::GlRequest::GlThenGles {
            opengl_version: (3, 2), // TODO: try more versions
            opengles_version: (2, 0),
        };
        let builder = glutin::WindowBuilder::from_winit_builder(wb)
                                            .with_gl(gl_version)
                                            .with_vsync();

        let window = gfx_window_glutin::build(builder, &events_loop, ColorFormat::get_format(), DepthFormat::get_format());
        let (surface, adapters) = gfx_window_glutin::Window(&window).get_surface_and_adapters();
        let dim = window.get_inner_size_points().unwrap();
        run::<Self, _, _, _>(dim, events_loop, surface, adapters)
    }
    #[cfg(feature = "dx11")]
    fn launch_default(wb: winit::WindowBuilder)
        where Self: Application<DefaultBackend> {
        launch_d3d11::<Self>(wb);
    }
    #[cfg(feature = "metal")]
    fn launch_default(wb: winit::WindowBuilder)
        where Self: Application<DefaultBackend> {
        launch_metal::<Self>(wb);
    }
    #[cfg(feature = "vulkan")]
    fn launch_default(wb: winit::WindowBuilder)
        where Self: Application<DefaultBackend>
    {
        let events_loop = winit::EventsLoop::new();
        let win = wb.build(&events_loop).unwrap();
        let dim = win.get_inner_size_points().unwrap();
        let mut window = gfx_window_vulkan::Window(&win);

        let (surface, adapters) = window.get_surface_and_adapters();
        run::<Self, _, _, _>(dim, events_loop, surface, adapters)
    }
}
