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

#[cfg(not(feature = "vulkan"))]
pub type ColorFormat = gfx::format::Rgba8;
#[cfg(feature = "vulkan")]
pub type ColorFormat = gfx::format::Bgra8;

#[cfg(feature = "metal")]
pub type DepthFormat = gfx::format::Depth32F;
#[cfg(not(feature = "metal"))]
pub type DepthFormat = gfx::format::DepthStencil;

pub struct Init<R: gfx::Resources> {
    pub backend: shade::Backend,
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
    fn new<F>(F, Init<R>) -> Self where F: Factory<R, CommandBuffer = C>;
    fn render<D>(&mut self, &mut D) where D: gfx::Device<Resources = R, CommandBuffer = C>;
    fn on(&mut self, winit::Event) -> bool;
}

#[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
pub type DefaultResources = gfx_device_gl::Resources;

pub trait Application<R: gfx::Resources>: Sized {
    fn new<F: gfx::Factory<R>>(F, Init<R>) -> Self;
    fn render<C: gfx::CommandBuffer<R>>(&mut self, &mut gfx::Encoder<R, C>);
    fn on(&mut self, event: winit::Event) -> bool {
        match event {
            winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
            winit::Event::Closed => false,
            _ => true
        }
    }

    fn launch_simple(name: &str) where Self: Application<DefaultResources> {
        let wb = winit::WindowBuilder::new().with_title(name);
        <Self as Application<DefaultResources>>::launch_default(wb)
    }
    #[cfg(all(target_os = "windows", not(feature = "vulkan")))]
    fn launch_default(wb: winit::WindowBuilder) where WrapD3D11<Self>: ApplicationD3D11 {
        WrapD3D11::<Self>::launch_d3d11(wb);
    }
    #[cfg(all(not(target_os = "windows"), not(feature = "vulkan"), not(feature = "metal")))]
    fn launch_default(wb: winit::WindowBuilder) where Self: Application<gfx_device_gl::Resources> {
        Wrap::<_, _, Self>::launch_gl3(wb);
    }
    #[cfg(feature = "metal")]
    fn launch_default(wb: winit::WindowBuilder) where WrapMetal<Self>: ApplicationMetal {
        WrapMetal::<Self>::launch_metal(wb)
    }
    #[cfg(feature = "vulkan")]
    fn launch_default(wb: winit::WindowBuilder) where WrapVulkan<Self>: ApplicationVulkan {
        WrapVulkan::<Self>::launch_vulkan(wb);
    }
}

pub struct Wrap<R: gfx::Resources, C, A> {
    encoder: gfx::Encoder<R, C>,
    app: A,
}

#[cfg(feature = "metal")]
pub type WrapMetal<A> = Wrap<gfx_device_metal::Resources, gfx_device_metal::CommandBuffer, A>;
#[cfg(target_os = "windows")]
pub type D3D11CommandBuffer = gfx_device_dx11::CommandBuffer<gfx_device_dx11::DeferredContext>;
#[cfg(target_os = "windows")]
pub type D3D11CommandBufferFake = gfx_device_dx11::CommandBuffer<gfx_device_dx11::CommandList>;
#[cfg(target_os = "windows")]
pub type WrapD3D11<A> = Wrap<gfx_device_dx11::Resources, D3D11CommandBuffer, A>;

impl<A: Application<gfx_device_gl::Resources>>
Wrap<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, A> {
    pub fn launch_gl3(wb: winit::WindowBuilder) {
        use gfx::traits::Device;

        env_logger::init().unwrap();
        let gl_version = glutin::GlRequest::GlThenGles {
            opengl_version: (3, 2), // TODO: try more versions
            opengles_version: (2, 0),
        };
        let builder = glutin::WindowBuilder::from_winit_builder(wb)
                                            .with_gl(gl_version)
                                            .with_vsync();
        let (window, mut device, factory, main_color, main_depth) =
            gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
        let (width, height) = window.get_inner_size().unwrap();
        let shade_lang = device.get_info().shading_language;

        let init = Init {
            backend: if shade_lang.is_embedded {
                shade::Backend::GlslEs(shade_lang)
            } else {
                shade::Backend::Glsl(shade_lang)
            },
            color: main_color,
            depth: main_depth,
            aspect_ratio: width as f32 / height as f32,
        };
        let mut app = Self::new(factory, init);

        let mut harness = Harness::new();
        'main: loop {
            for event in window.poll_events() {
                if !app.on(event) {
                    break 'main
                }
            }
            // draw a frame
            app.render(&mut device);
            window.swap_buffers().unwrap();
            device.cleanup();
            harness.bump();
        }
    }
}

#[cfg(feature = "vulkan")]
pub type WrapVulkan<A> = Wrap<gfx_device_vulkan::Resources, gfx_device_vulkan::CommandBuffer, A>;

impl<R, C, A> ApplicationBase<R, C> for Wrap<R, C, A>
    where R: gfx::Resources,
          C: gfx::CommandBuffer<R>,
          A: Application<R>
{
    fn new<F>(mut factory: F, init: Init<R>) -> Self
        where F: Factory<R, CommandBuffer = C>
    {
        Wrap {
            encoder: factory.create_encoder(),
            app: A::new(factory, init),
        }
    }

    fn render<D>(&mut self, device: &mut D)
        where D: gfx::Device<Resources = R, CommandBuffer = C>
    {
        self.app.render(&mut self.encoder);
        self.encoder.flush(device);
    }

    fn on(&mut self, event: winit::Event) -> bool {
        self.app.on(event)
    }
}

impl Factory<gfx_device_gl::Resources> for gfx_device_gl::Factory {
    type CommandBuffer = gfx_device_gl::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_gl::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

#[cfg(target_os = "windows")]
pub trait ApplicationD3D11 {
    fn launch_d3d11(winit::WindowBuilder);
}

#[cfg(target_os = "windows")]
impl Factory<gfx_device_dx11::Resources> for gfx_device_dx11::Factory {
    type CommandBuffer = D3D11CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_dx11::Resources, Self::CommandBuffer> {
        self.create_command_buffer_native().into()
    }
}

#[cfg(target_os = "windows")]
impl<A: ApplicationBase<gfx_device_dx11::Resources, D3D11CommandBuffer>> ApplicationD3D11 for A {
    fn launch_d3d11(wb: winit::WindowBuilder) {
        use gfx::traits::{Device, Factory};

        env_logger::init().unwrap();
        let (window, device, mut factory, main_color) =
            gfx_window_dxgi::init::<ColorFormat>(wb).unwrap();
        let main_depth = factory.create_depth_stencil_view_only(window.size.0, window.size.1)
            .unwrap();

        let mut app = Self::new(factory,
                                Init {
                                    backend: shade::Backend::Hlsl(device.get_shader_model()),
                                    color: main_color,
                                    depth: main_depth,
                                    aspect_ratio: window.size.0 as f32 / window.size.1 as f32,
                                });
        let mut device: gfx_device_dx11::Deferred = device.into();

        let mut harness = Harness::new();
        'main: loop {
            // quit when Esc is pressed.
            for event in window.poll_events() {
                match event {
                    winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                    winit::Event::Closed => break 'main,
                    _ => {}
                }
            }
            // draw a frame
            app.render(&mut device);
            window.swap_buffers(1);
            device.cleanup();
            harness.bump();
        }
    }
}


#[cfg(feature = "metal")]
pub trait ApplicationMetal {
    fn launch_metal(winit::WindowBuilder);
}

#[cfg(feature = "metal")]
impl Factory<gfx_device_metal::Resources> for gfx_device_metal::Factory {
    type CommandBuffer = gfx_device_metal::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_metal::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

#[cfg(feature = "metal")]
impl<
    A: ApplicationBase<gfx_device_metal::Resources, gfx_device_metal::CommandBuffer>
> ApplicationMetal for A {
    fn launch_metal(wb: winit::WindowBuilder) {
        use gfx::traits::{Device, Factory};

        env_logger::init().unwrap();
        let (window, mut device, mut factory, main_color) = gfx_window_metal::init::<ColorFormat>(wb)
                                                                                   .unwrap();

        let (width, height) = window.get_inner_size().unwrap();

        let main_depth = factory.create_depth_stencil_view_only(width as u16, height as u16).unwrap();

        let mut app = Self::new(factory, Init {
            backend: shade::Backend::Msl(device.get_shader_model()),
            color: main_color,
            depth: main_depth,
            aspect_ratio: width as f32 / height as f32
        });

        let mut harness = Harness::new();
        'main: loop {
            for event in window.poll_events() {
                match event {
                    winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                    winit::Event::Closed => break 'main,
                    _ => {},
                }
            }

            app.render(&mut device);
            window.swap_buffers().unwrap();
            device.cleanup();
            harness.bump();
        }
    }
}


#[cfg(feature = "vulkan")]
pub trait ApplicationVulkan {
    fn launch_vulkan(winit::WindowBuilder);
}

#[cfg(feature = "vulkan")]
impl Factory<gfx_device_vulkan::Resources> for gfx_device_vulkan::Factory {
    type CommandBuffer = gfx_device_vulkan::CommandBuffer;
    fn create_encoder(&mut self) -> gfx::Encoder<gfx_device_vulkan::Resources, Self::CommandBuffer> {
        self.create_command_buffer().into()
    }
}

#[cfg(feature = "vulkan")]
impl<
    A: ApplicationBase<gfx_device_vulkan::Resources, gfx_device_vulkan::CommandBuffer>
> ApplicationVulkan for A {
    fn launch(wb: winit::WindowBuilder) {
        use gfx::traits::{Device, Factory};

        env_logger::init().unwrap();
        let (mut win, mut factory) = gfx_window_vulkan::init::<ColorFormat>(wb);
        let (width, height) = win.get_size();
        let main_depth = factory.create_depth_stencil::<DepthFormat>(width, height).unwrap();

        let mut app = Self::new(factory, Init {
            backend: shade::Backend::Vulkan,
            color: win.get_any_target(),
            depth: main_depth.2,
            aspect_ratio: width as f32 / height as f32, //TODO
        });

        let mut harness = Harness::new();
        'main: loop {
            for event in win.get_window().poll_events() {
                match event {
                    winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) |
                    winit::Event::Closed => break 'main,
                    _ => {},
                }
            }

            let mut frame = win.start_frame();
            app.render(frame.get_queue());
            frame.get_queue().cleanup();
            harness.bump();
        }
    }
}
