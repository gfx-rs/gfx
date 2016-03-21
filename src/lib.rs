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
extern crate getopts;
extern crate time;
extern crate glutin;
extern crate gfx;
extern crate gfx_device_gl;
#[cfg(target_os = "windows")]
extern crate gfx_device_dx11;
extern crate gfx_window_glutin;
//extern crate gfx_window_glfw;
#[cfg(target_os = "windows")]
extern crate gfx_window_dxgi;

pub mod shade;


pub type ColorFormat = gfx::format::Srgba8;
pub type DepthFormat = gfx::format::DepthStencil;

pub struct Init<R: gfx::Resources> {
    pub backend: shade::Backend,
    pub color: gfx::handle::RenderTargetView<R, ColorFormat>,
    pub depth: gfx::handle::DepthStencilView<R, DepthFormat>,
    pub aspect_ratio: f32,
}

pub enum Backend {
    OpenGL2,
    Direct3D11 {
        pix_mode: bool,
    },
}

pub struct Config {
    //pub backend: Backend,
    pub size: (u16, u16),
}

pub const DEFAULT_CONFIG: Config = Config {
    //backend: Backend::OpenGL2,
    size: (800, 520),
};

struct Harness {
    start: f64,
    num_frames: f64,
}

impl Harness {
    fn new() -> Harness {
        Harness {
            start: time::precise_time_s(),
            num_frames: 0.0,
        }
    }
    fn bump(&mut self) {
        self.num_frames += 1.0;
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let time_end = time::precise_time_s();
        println!("Avg frame time: {} ms",
            (time_end - self.start) * 1000.0 / self.num_frames
        );
    }
}

pub trait ApplicationBase<R: gfx::Resources, C: gfx::CommandBuffer<R>> {
    fn new<F>(F, gfx::Encoder<R, C>, Init<R>) -> Self where
        F: gfx::Factory<R>;
    fn render<D>(&mut self, &mut D) where
        D: gfx::Device<Resources=R, CommandBuffer=C>;
}

pub trait Application<R: gfx::Resources>: Sized {
    fn new<F: gfx::Factory<R>>(F, Init<R>) -> Self;
    fn render<C: gfx::CommandBuffer<R>>(&mut self, &mut gfx::Encoder<R, C>);
    #[cfg(target_os = "windows")]
    fn launch_default(name: &str) where WrapD3D11<Self>: ApplicationD3D11 {
        WrapD3D11::<Self>::launch(name, DEFAULT_CONFIG);
    }
    #[cfg(not(target_os = "windows"))]
    fn launch_default(name: &str) where WrapGL2<Self>: ApplicationGL2 {
        WrapGL2::<Self>::launch(name, DEFAULT_CONFIG);
    }
}

pub struct Wrap<R: gfx::Resources, C: gfx::CommandBuffer<R>, A>{
    encoder: gfx::Encoder<R, C>,
    app: A,
}

#[cfg(target_os = "windows")]
pub type D3D11CommandBuffer = gfx_device_dx11::CommandBuffer<gfx_device_dx11::DeferredContext>;
#[cfg(target_os = "windows")]
pub type D3D11CommandBufferFake = gfx_device_dx11::CommandBuffer<gfx_device_dx11::CommandList>;
#[cfg(target_os = "windows")]
pub type WrapD3D11<A> = Wrap<gfx_device_dx11::Resources, D3D11CommandBuffer, A>;
pub type WrapGL2<A> = Wrap<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, A>;

impl<
    R: gfx::Resources,
    C: gfx::CommandBuffer<R>,
    A: Application<R>,
> ApplicationBase<R, C> for Wrap<R, C, A> {
    fn new<F>(factory: F, encoder: gfx::Encoder<R, C>, init: Init<R>) -> Self where
        F: gfx::Factory<R>
    {
        Wrap {
            encoder: encoder,
            app: A::new(factory, init),
        }
    }

    fn render<D>(&mut self, device: &mut D) where
        D: gfx::Device<Resources=R, CommandBuffer=C>
    {
        self.app.render(&mut self.encoder);
        self.encoder.flush(device);
    }
}


pub trait ApplicationGL2 {
    fn launch(&str, Config);
}

#[cfg(target_os = "windows")]
pub trait ApplicationD3D11 {
    fn launch(&str, Config);
}

impl<
    A: ApplicationBase<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer>
> ApplicationGL2 for A {
    fn launch(title: &str, config: Config) {
        use gfx::traits::Device;

        env_logger::init().unwrap();
        let builder = glutin::WindowBuilder::new()
            .with_title(title.to_string())
            .with_dimensions(config.size.0 as u32, config.size.1 as u32)
            .with_vsync();
        let (window, mut device, mut factory, main_color, main_depth) =
            gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
        let (width, height) = window.get_inner_size().unwrap();
        let combuf = factory.create_command_buffer();

        let mut app = Self::new(factory, combuf.into(), Init {
            backend: shade::Backend::Glsl(device.get_info().shading_language),
            color: main_color,
            depth: main_depth,
            aspect_ratio: width as f32 / height as f32,
        });

        let mut harness = Harness::new();
        'main: loop {
            // quit when Esc is pressed.
            for event in window.poll_events() {
                match event {
                    glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                    glutin::Event::Closed => break 'main,
                    _ => {},
                }
            }
            // draw a frame
            app.render(&mut device);
            window.swap_buffers().unwrap();
            device.cleanup();
            harness.bump()
        }
    }
}

#[cfg(target_os = "windows")]
impl<
    A: ApplicationBase<gfx_device_dx11::Resources, D3D11CommandBuffer>
> ApplicationD3D11 for A {
    fn launch(title: &str, config: Config) {
        use gfx::traits::{Device, Factory};

        env_logger::init().unwrap();
        let (window, device, mut factory, main_color) =
            gfx_window_dxgi::init::<ColorFormat>(title, config.size.0, config.size.1)
            .unwrap();
        let main_depth = factory.create_depth_stencil_view_only(
            window.size.0, window.size.1).unwrap();

        //let combuf = factory.create_command_buffer();
        let combuf = factory.create_command_buffer_native();

        let mut app = Self::new(factory, combuf.into(), Init {
            backend: shade::Backend::Hlsl(device.get_shader_model()),
            color: main_color,
            depth: main_depth,
            aspect_ratio: window.size.0 as f32 / window.size.1 as f32,
        });

        let mut device: gfx_device_dx11::Deferred = device.into();

        let mut harness = Harness::new();
        while window.dispatch() {
            app.render(&mut device);
            window.swap_buffers(1);
            device.cleanup();
            harness.bump();
        }
    }
}
