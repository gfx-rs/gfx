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

//! Cross platform GFX example application framework.
//! 
//! Supports OpenGL 3.2, OpenGL ES 2.0, WebGL 2, and DirectX 11. Handles windowing via `winit`.
//! 
//! Note: The documentation will be available only for the backends corresponding to the platform you're compiling to.
//! 
//! `gfx_app` exposes the following helpers:
//!  - `gfx_app::Application` - trait that creates a window and backend for the given compile target and feature set (DirectX on Windows, OpenGL elsewhere. The feature `vulkan` enables Vulkan and `metal` enables Metal on macOS).
//!  - `gfx_app::ColorFormat`/`gfx_app::DepthFormat` - the pixel formats for the window's color and depth buffers.
//!  - `gfx_app::DefaultResources` - type that picks the correct `gfx::Resources` for the current backend.
//!  - `gfx_app::shade::Source` - container for shaders for multiple backends.
//! 
//! ## Sample usage
//! 
//! ```
//! struct App<R: gfx::Resources> {
//!     window_targets: gfx_app::WindowTargets<R>,
//! }
//! 
//! impl<R: gfx::Resources> gfx_app::Application<R> for App<R> {
//!     fn new<F: gfx::Factory<R>>(
//!         _factory: &mut F,
//!         _backend: gfx_app::shade::Backend,
//!         window_targets: gfx_app::WindowTargets<R>,
//!     ) -> Self {
//!         App {
//!             window_targets
//!         }
//!     }
//! 
//!     fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
//!         encoder.clear(&self.window_targets.color, [1.0, 0.0, 1.0, 1.0]);
//!     }
//! 
//!     fn on_resize(&mut self, window_targets: gfx_app::WindowTargets<R>) {
//!         self.window_targets = window_targets;
//!     }
//! 
//!     fn on(&mut self, event: winit::WindowEvent) {
//!         match event {
//!             _ => (),
//!         }
//!     }
//! }
//! 
//! // Then just call `App::launch_simple("Window title");` from `main`.
//! ```
//! 
//! Note: only `new` and `render` are required to be implemented, but implementing `on` (and `on_resize` or `on_resize_ext`) allows an app to handle events.

#[allow(unused_imports)]
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

/// The canonical color format for the current backend.
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
pub type ColorFormat = gfx::format::Rgba8;
/// The canonical color format for the current backend.
#[cfg(feature = "vulkan")]
pub type ColorFormat = gfx::format::Bgra8;
/// The canonical color format for the current backend.
#[cfg(feature = "metal")]
pub type ColorFormat = (gfx::format::B8_G8_R8_A8, gfx::format::Srgb);

/// The canonical depth format for the current backend.
#[cfg(feature = "metal")]
pub type DepthFormat = gfx::format::Depth32F;
/// The canonical depth format for the current backend.
#[cfg(not(feature = "metal"))]
pub type DepthFormat = gfx::format::DepthStencil;

/// Aggregator of the render and depth targets (plus additional information about them) for a window.
pub struct WindowTargets<R: gfx::Resources> {
    /// Color target for the window
    pub color: gfx::handle::RenderTargetView<R, ColorFormat>,

    /// Depth target for the window
    pub depth: gfx::handle::DepthStencilView<R, DepthFormat>,

    /// For use in projection matrices. Calculated as `width/height`.
    pub aspect_ratio: f32,
}

/// Helper to calculate frame statistics.
/// 
/// Prints results on `drop`.
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

    /// Increment internal counters.
    /// 
    /// Call once per frame.
    fn bump(&mut self) {
        self.num_frames += 1.0;
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let time_end = self.start.elapsed();
        println!("Avg frame time: {} ms",
                 ((time_end.as_secs() * 1000) as f64 +
                  (time_end.subsec_nanos() / 1_000_000) as f64) / self.num_frames);
    }
}

/// Trait to allow `Application` to create a backend-specific resources.
pub trait Factory<R: gfx::Resources>: gfx::Factory<R> {
    type CommandBuffer: gfx::CommandBuffer<R>;
    fn create_encoder(&mut self) -> gfx::Encoder<R, Self::CommandBuffer>;
}

/// Represents an application container. Consider using `Application`, as it is simpler to use.
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

/// Creates a window and starts the main loop for OpenGL
/// 
/// Can target WebGL if the `target_os` is `emscripten`. Otherwise, tries OpenGL 3.2, then OpenGL ES 2.0.
pub fn launch_gl3<A>(window: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer>
{
    use gfx::traits::Device;

    env_logger::init();
    #[cfg(target_os = "emscripten")]
    let gl_version = glutin::GlRequest::Specific(
        glutin::Api::WebGl, (2, 0),
    );
    #[cfg(not(target_os = "emscripten"))]
    let gl_version = glutin::GlRequest::GlThenGles {
        opengl_version: (3, 2), // TODO: try more versions
        opengles_version: (2, 0),
    };
    let context = glutin::ContextBuilder::new()
        .with_gl(gl_version)
        .with_vsync(true);
    let mut events_loop = glutin::EventsLoop::new();
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(window, context, &events_loop)
	    .expect("Failed to create window");
    let mut current_size = window.window()
        .get_inner_size()
        .unwrap()
        .to_physical(window.window().get_hidpi_factor());
    let shade_lang = device.get_info().shading_language;

    let backend = if shade_lang.is_embedded {
        shade::Backend::GlslEs(shade_lang)
    } else {
        shade::Backend::Glsl(shade_lang)
    };
    let mut app = A::new(&mut factory, backend, WindowTargets {
        color: main_color,
        depth: main_depth,
        aspect_ratio: current_size.width as f32 / current_size.height as f32,
    });

    let mut harness = Harness::new();
    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::CloseRequested => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => running = false,
                    winit::WindowEvent::Resized(size) => {
                        let physical = size.to_physical(window.window().get_hidpi_factor());
                        if physical != current_size {
                            window.resize(physical);
                            current_size = physical;
                            let (new_color, new_depth) = gfx_window_glutin::new_views(&window);
                            app.on_resize(&mut factory, WindowTargets {
                                color: new_color,
                                depth: new_depth,
                                aspect_ratio: size.width as f32 / size.height as f32,
                            });
                        }
                    }
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

/// Creates a window and starts the main loop for DirectX 11
#[cfg(target_os = "windows")]
pub fn launch_d3d11<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_dx11::Resources, D3D11CommandBuffer>
{
    use gfx::traits::{Device, Factory};

    env_logger::init();
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
                    winit::WindowEvent::CloseRequested => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => running = false,
                    winit::WindowEvent::Resized(size) => {
                        let physical = size.to_physical(window.inner.get_hidpi_factor());
                        let (width, height): (u32, u32) = physical.into();
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

/// Creates a window and starts the main loop for Metal
#[cfg(feature = "metal")]
pub fn launch_metal<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_metal::Resources, gfx_device_metal::CommandBuffer>
{
    use gfx::traits::{Device, Factory};
    use gfx::texture::Size;

    env_logger::init();
    let mut events_loop = winit::EventsLoop::new();
    let (window, mut device, mut factory, main_color) = gfx_window_metal::init::<ColorFormat>(wb, &events_loop)
                                                                                .unwrap();
    let (width, height): (u32, u32) = window.get_inner_size().unwrap().into();
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
                    winit::WindowEvent::CloseRequested => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => running = false,
                    winit::WindowEvent::Resized(_size) => {
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

/// Creates a window and starts the main loop for Vulkan
#[cfg(feature = "vulkan")]
pub fn launch_vulkan<A>(wb: winit::WindowBuilder) where
A: Sized + ApplicationBase<gfx_device_vulkan::Resources, gfx_device_vulkan::CommandBuffer>
{
    use gfx::traits::{Device, Factory};
    use gfx::texture::Size;

    env_logger::init();
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
                    winit::WindowEvent::CloseRequested => running = false,
                    winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            state: winit::ElementState::Pressed,
                            virtual_keycode: key,
                            ..
                        },
                        ..
                    } if key == A::get_exit_key() => running = false,
                    winit::WindowEvent::Resized(_size) => {
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

/// The implementation of `gfx::Resources` for the current backend.
#[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
pub type DefaultResources = gfx_device_gl::Resources;
/// The implementation of `gfx::Resources` for the current backend.
#[cfg(all(target_os = "windows", not(feature = "vulkan")))]
pub type DefaultResources = gfx_device_dx11::Resources;
/// The implementation of `gfx::Resources` for the current backend.
#[cfg(feature = "metal")]
pub type DefaultResources = gfx_device_metal::Resources;
/// The implementation of `gfx::Resources` for the current backend.
#[cfg(feature = "vulkan")]
pub type DefaultResources = gfx_device_vulkan::Resources;

/// Represents a cross-platform application container.
pub trait Application<R: gfx::Resources>: Sized {
    /// Called once to initialize the app.
    fn new<F: gfx::Factory<R>>(&mut F, shade::Backend, WindowTargets<R>) -> Self;

    /// Render the application. Called once per frame.
    fn render<C: gfx::CommandBuffer<R>>(&mut self, &mut gfx::Encoder<R, C>);

    /// The application will exit when this key is pressed.
    /// 
    /// Note: Implement this and return `None` to disable hotkey exit.
    fn get_exit_key() -> Option<winit::VirtualKeyCode> {
        Some(winit::VirtualKeyCode::Escape)
    }

    /// See `on_resize_ext`.
    fn on_resize(&mut self, WindowTargets<R>) {}
    
    /// User-specified handler for `winit::WindowEvent::Resized`.
    /// 
    /// Note: implement this method if you have resources that need to be recreated when the window size changes (e.g. G-buffers), otherwise just implement `on_resize`.
    fn on_resize_ext<F: gfx::Factory<R>>(&mut self, _factory: &mut F, targets: WindowTargets<R>) {
        self.on_resize(targets);
    }

    /// User-specified handler for `winit::WindowEvent`s.
    /// 
    /// Note: will not be called for the following events:
    ///  - `winit::WindowEvent::CloseRequested`
    ///  - `winit::WindowEvent::Resized`
    ///  - `winit::WindowEvent::KeyboardInput` when the `winit::VirtualKeyCode` is equal to `get_exit_key()`
    fn on(&mut self, _event: winit::WindowEvent) {}

    /// Launch the app with the default `winit::WindowBuilder` parameters and run the main loop.
    fn launch_simple(name: &str) where Self: Application<DefaultResources> {
        let wb = winit::WindowBuilder::new().with_title(name);
        <Self as Application<DefaultResources>>::launch_default(wb)
    }

    /// Launch the app with a specified `winit::WindowBuilder` and run the main loop.
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<DefaultResources> {
        #[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
        launch_gl3::<Wrap<_, _, Self>>(wb);

        #[cfg(all(target_os = "windows", not(feature = "vulkan")))]
        launch_d3d11::<Wrap<_, _, Self>>(wb);

        #[cfg(feature = "metal")]
        launch_metal::<Wrap<_, _, Self>>(wb);

        #[cfg(feature = "vulkan")]
        launch_vulkan::<Wrap<_, _, Self>>(wb);
    }
}

/// Wraps a `gfx::Encoder` and `Application` to simplify usage of `ApplicationBase`.
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
