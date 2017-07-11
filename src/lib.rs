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
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
// extern crate gfx_window_glfw;

#[cfg(target_os = "windows")]
extern crate gfx_device_dx11;
#[cfg(target_os = "windows")]
extern crate gfx_window_dxgi;

#[cfg(feature = "metal")]
extern crate gfx_device_metal;
#[cfg(feature = "metal")]
extern crate gfx_window_metal;

#[cfg(feature = "vulkan")]
extern crate gfx_device_vulkan;
#[cfg(feature = "vulkan")]
extern crate gfx_window_vulkan;

pub mod shade;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
pub type ColorFormat = gfx::format::Rgba8;
#[cfg(feature = "vulkan")]
pub type ColorFormat = gfx::format::Bgra8;
#[cfg(feature = "metal")]
pub type ColorFormat = (gfx::format::B8_G8_R8_A8, gfx::format::Srgb);

#[cfg(feature = "metal")]
pub type DepthFormat = gfx::format::Depth32F;
#[cfg(not(feature = "metal"))]
pub type DepthFormat = gfx::format::DepthStencil;

pub struct WindowTargets<R: gfx::Resources> {
    pub color: gfx::handle::RenderTargetView<R, ColorFormat>,
    pub depth: gfx::handle::DepthStencilView<R, DepthFormat>,
    pub aspect_ratio: f32,
}

pub enum Backend {
    OpenGL2,
    Direct3D11 { pix_mode: bool },
    Metal,
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

pub trait Factory<R: gfx::Resources>: gfx::Factory<R> {
    type CommandBuffer: gfx::CommandBuffer<R>;
    fn create_encoder(&mut self) -> gfx::Encoder<R, Self::CommandBuffer>;
}

pub trait ApplicationBase<R: gfx::Resources, C: gfx::CommandBuffer<R>> {
    fn new<F>(&mut F, shade::Backend, WindowTargets<R>) -> Self where F: Factory<R, CommandBuffer = C>;
    fn render<D>(&mut self, &mut D) where D: gfx::Device<Resources = R, CommandBuffer = C>;
    fn get_exit_key() -> Option<winit::VirtualKeyCode>;
    fn on(&mut self, winit::WindowEvent);
    fn on_resize<F>(&mut self, &mut F, WindowTargets<R>) where F: Factory<R, CommandBuffer = C>;
}


impl Factory<gfx_device_gl::Resources> for gfx_device_gl::Factory {
    type CommandBuffer = gfx_device_gl::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_gl::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

pub fn launch_gl3<A>(window: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer>
{
    use gfx::traits::Device;
    use glutin::GlContext;

    env_logger::init().unwrap();
    let gl_version = glutin::GlRequest::GlThenGles {
        opengl_version: (3, 2), // TODO: try more versions
        opengles_version: (2, 0),
    };
    let context = glutin::ContextBuilder::new()
        .with_gl(gl_version)
        .with_vsync(true);
    let mut events_loop = glutin::EventsLoop::new();
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(window, context, &events_loop);
    let (mut cur_width, mut cur_height) = window.get_inner_size_points().unwrap();
    let shade_lang = device.get_info().shading_language;

    let backend = if shade_lang.is_embedded {
        shade::Backend::GlslEs(shade_lang)
    } else {
        shade::Backend::Glsl(shade_lang)
    };
    let mut app = A::new(&mut factory, backend, WindowTargets {
        color: main_color,
        depth: main_depth,
        aspect_ratio: cur_width as f32 / cur_height as f32,
    });

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::Closed => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => return,
                    winit::WindowEvent::Resized(width, height) => if width != cur_width || height != cur_height {
                        window.resize(width, height);
                        cur_width = width;
                        cur_height = height;
                        let (new_color, new_depth) = gfx_window_glutin::new_views(&window);
                        app.on_resize(&mut factory, WindowTargets {
                            color: new_color,
                            depth: new_depth,
                            aspect_ratio: width as f32 / height as f32,
                        });
                    },
                    _ => app.on(event),
                }
            }
        });
        // draw a frame
        app.render(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
        harness.bump();
    }
}


#[cfg(target_os = "windows")]
pub type D3D11CommandBuffer = gfx_device_dx11::CommandBuffer<gfx_device_dx11::DeferredContext>;
#[cfg(target_os = "windows")]
pub type D3D11CommandBufferFake = gfx_device_dx11::CommandBuffer<gfx_device_dx11::CommandList>;

#[cfg(target_os = "windows")]
impl Factory<gfx_device_dx11::Resources> for gfx_device_dx11::Factory {
    type CommandBuffer = D3D11CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_dx11::Resources, Self::CommandBuffer> {
        self.create_command_buffer_native().into()
    }
}

#[cfg(target_os = "windows")]
pub fn launch_d3d11<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_dx11::Resources, D3D11CommandBuffer>
{
    use gfx::traits::{Device, Factory};

    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
    let (mut window, device, mut factory, main_color) =
        gfx_window_dxgi::init::<ColorFormat>(wb, &events_loop).unwrap();
    let main_depth = factory.create_depth_stencil_view_only(window.size.0, window.size.1)
                            .unwrap();

    let backend = shade::Backend::Hlsl(device.get_shader_model());
    let mut app = A::new(&mut factory, backend, WindowTargets {
        color: main_color,
        depth: main_depth,
        aspect_ratio: window.size.0 as f32 / window.size.1 as f32,
    });
    let mut device = gfx_device_dx11::Deferred::from(device);

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        let mut new_size = None;
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::Closed => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => return,
                    winit::WindowEvent::Resized(width, height) => {
                        let size = (width as gfx::texture::Size, height as gfx::texture::Size);
                        if size != window.size {
                            // working around the borrow checker: window is already borrowed here
                            new_size = Some(size);
                        }
                    },
                    _ => app.on(event),
                }
            }
        });
        if let Some((width, height)) = new_size {
            use gfx_window_dxgi::update_views;
            match update_views(&mut window, &mut factory, &mut device, width, height) {
                Ok(new_color) => {
                    let new_depth = factory.create_depth_stencil_view_only(width, height).unwrap();
                    app.on_resize(&mut factory, WindowTargets {
                        color: new_color,
                        depth: new_depth,
                        aspect_ratio: width as f32 / height as f32,
                    });
                },
                Err(e) => error!("Resize failed: {}", e),
            }
            continue;
        }
        app.render(&mut device);
        window.swap_buffers(1);
        device.cleanup();
        harness.bump();
    }
}


#[cfg(feature = "metal")]
impl Factory<gfx_device_metal::Resources> for gfx_device_metal::Factory {
    type CommandBuffer = gfx_device_metal::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_metal::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

#[cfg(feature = "metal")]
pub fn launch_metal<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_metal::Resources, gfx_device_metal::CommandBuffer>
{
    use gfx::traits::{Device, Factory};
    use gfx::texture::Size;

    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
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
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::Closed => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => return,
                    winit::WindowEvent::Resized(_width, _height) => {
                        warn!("TODO: resize on Metal");
                    },
                    _ => app.on(event),
                }
            }
        });
        app.render(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
        harness.bump();
    }
}


#[cfg(feature = "vulkan")]
impl Factory<gfx_device_vulkan::Resources> for gfx_device_vulkan::Factory {
    type CommandBuffer = gfx_device_vulkan::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_vulkan::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

#[cfg(feature = "vulkan")]
pub fn launch_vulkan<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_vulkan::Resources, gfx_device_vulkan::CommandBuffer>
{
    use gfx::traits::{Device, Factory};
    use gfx::texture::Size;

    env_logger::init().unwrap();
    let mut events_loop = winit::EventsLoop::new();
    let (mut win, mut factory) = gfx_window_vulkan::init::<ColorFormat>(wb, &events_loop);
    let (width, height) = win.get_size();
    let main_depth = factory.create_depth_stencil::<DepthFormat>(width as Size, height as Size).unwrap();

    let backend = shade::Backend::Vulkan;
    let mut app = A::new(&mut factory, backend, WindowTargets {
        color: win.get_any_target(),
        depth: main_depth.2,
        aspect_ratio: width as f32 / height as f32, //TODO
    });

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::Closed => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => return,
                    winit::WindowEvent::Resized(_width, _height) => {
                        warn!("TODO: resize on Vulkan");
                    },
                    _ => app.on(event),
                }
            }
        });
        let mut frame = win.start_frame();
        app.render(frame.get_queue());
        frame.get_queue().cleanup();
        harness.bump();
    }
}


#[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
pub type DefaultResources = gfx_device_gl::Resources;
#[cfg(all(target_os = "windows", not(feature = "vulkan")))]
pub type DefaultResources = gfx_device_dx11::Resources;
#[cfg(feature = "metal")]
pub type DefaultResources = gfx_device_metal::Resources;
#[cfg(feature = "vulkan")]
pub type DefaultResources = gfx_device_vulkan::Resources;

pub trait Application<R: gfx::Resources>: Sized {
    fn new<F: gfx::Factory<R>>(&mut F, shade::Backend, WindowTargets<R>) -> Self;
    fn render<C: gfx::CommandBuffer<R>>(&mut self, &mut gfx::Encoder<R, C>);

    fn get_exit_key() -> Option<winit::VirtualKeyCode> {
        Some(winit::VirtualKeyCode::Escape)
    }
    fn on_resize(&mut self, WindowTargets<R>) {}
    fn on_resize_ext<F: gfx::Factory<R>>(&mut self, _factory: &mut F, targets: WindowTargets<R>) {
        self.on_resize(targets);
    }
    fn on(&mut self, _event: winit::WindowEvent) {}

    fn launch_simple(name: &str) where Self: Application<DefaultResources> {
        let wb = winit::WindowBuilder::new().with_title(name);
        <Self as Application<DefaultResources>>::launch_default(wb)
    }
    #[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<DefaultResources> {
        launch_gl3::<Wrap<_, _, Self>>(wb);
    }
    #[cfg(all(target_os = "windows", not(feature = "vulkan")))]
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<DefaultResources> {
        launch_d3d11::<Wrap<_, _, Self>>(wb);
    }
    #[cfg(feature = "metal")]
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<DefaultResources> {
        launch_metal::<Wrap<_, _, Self>>(wb);
    }
    #[cfg(feature = "vulkan")]
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<DefaultResources> {
        launch_vulkan::<Wrap<_, _, Self>>(wb);
    }
}

pub struct Wrap<R: gfx::Resources, C, A> {
    encoder: gfx::Encoder<R, C>,
    app: A,
}

impl<R, C, A> ApplicationBase<R, C> for Wrap<R, C, A>
    where R: gfx::Resources,
          C: gfx::CommandBuffer<R>,
          A: Application<R>
{
    fn new<F>(factory: &mut F, backend: shade::Backend, window_targets: WindowTargets<R>) -> Self
        where F: Factory<R, CommandBuffer = C>
    {
        Wrap {
            encoder: factory.create_encoder(),
            app: A::new(factory, backend, window_targets),
        }
    }

    fn render<D>(&mut self, device: &mut D)
        where D: gfx::Device<Resources = R, CommandBuffer = C>
    {
        self.app.render(&mut self.encoder);
        self.encoder.flush(device);
    }

    fn get_exit_key() -> Option<winit::VirtualKeyCode> {
        A::get_exit_key()
    }

    fn on(&mut self, event: winit::WindowEvent) {
        self.app.on(event)
    }

    fn on_resize<F>(&mut self, factory: &mut F, window_targets: WindowTargets<R>)
        where F: Factory<R, CommandBuffer = C>
    {
        self.app.on_resize_ext(factory, window_targets);
    }
}
