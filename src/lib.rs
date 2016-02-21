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

extern crate glutin;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_device_dx11;
extern crate gfx_window_glutin;
//extern crate gfx_window_glfw;
extern crate gfx_window_dxgi;


pub type ColorFormat = gfx::format::Srgb8;
pub type DepthFormat = gfx::format::DepthStencil;

pub struct Init<R: gfx::Resources> {
    pub color: gfx::handle::RenderTargetView<R, ColorFormat>,
    pub depth: gfx::handle::DepthStencilView<R, DepthFormat>,
}

pub enum Backend {
    OpenGL2,
    Direct3D11,
}

pub struct Config {
    backend: Backend,
    size: (u16, u16),
}

impl Config {
    pub fn new(back: Backend) -> Config {
        Config {
            backend: back,
            size: (800, 520),
        }
    }
}


pub trait Application<R: gfx::Resources> {
    fn new<F: gfx::Factory<R>>(F, Init<R>) -> Self;
    fn render<C: gfx::CommandBuffer<R>>(&mut self, &mut gfx::Encoder<R, C>);

    fn launch(title: &str, config: Config) where
        Self: Sized + Application<gfx_device_gl::Resources> + Application<gfx_device_dx11::Resources>
    {
        use gfx::traits::{Device, Factory, FactoryExt};
        match config.backend {
            Backend::OpenGL2 => {
                let builder = glutin::WindowBuilder::new()
                    .with_title(title.to_string())
                    .with_dimensions(config.size.0 as u32, config.size.1 as u32)
                    .with_vsync();
                let (window, mut device, mut factory, main_color, main_depth) =
                    gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
                let mut encoder = factory.create_encoder();
                let mut app = Self::new(factory, Init {
                    color: main_color,
                    depth: main_depth,
                });
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
                    encoder.reset();
                    app.render(&mut encoder);
                    device.submit(encoder.as_buffer());
                    window.swap_buffers().unwrap();
                    device.cleanup();
                }
            },
            Backend::Direct3D11 => {
                use gfx::tex::Size;
                let (window, mut device, mut factory, main_color) =
                    gfx_window_dxgi::init::<ColorFormat>(title, config.size.0, config.size.1)
                    .unwrap();
                let mut encoder = factory.create_encoder();
                let (_, _, main_depth) = factory.create_depth_stencil(
                    config.size.0 as Size, config.size.1 as Size
                    ).unwrap();
                let mut app = Self::new(factory, Init {
                    color: main_color,
                    depth: main_depth,
                });
                while window.dispatch() {
                    encoder.reset();
                    app.render(&mut encoder);
                    device.submit(encoder.as_buffer());
                    window.swap_buffers(1);
                    device.cleanup();
                }
            },
        }
    }
}
