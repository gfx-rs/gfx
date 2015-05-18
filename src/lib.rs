// Copyright 2015 The Gfx-rs Developers.
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

#[deny(missing_docs)]

extern crate gfx;
extern crate gfx_device_gl;
extern crate glfw;

use gfx::tex::Size;

use glfw::Context;

/// A wrapper around the window that implements `Output`.
pub struct Output<R: gfx::Resources> {
    /// Glutin window in the open.
    pub window: glfw::Window,
    frame: gfx::handle::FrameBuffer<R>,
    mask: gfx::Mask,
    gamma: gfx::Gamma,
}

impl<R: gfx::Resources> gfx::Output<R> for Output<R> {
    fn get_handle(&self) -> Option<&gfx::handle::FrameBuffer<R>> {
        Some(&self.frame)
    }

    fn get_size(&self) -> (Size, Size) {
        let (w, h) = self.window.get_framebuffer_size();
        (w as Size, h as Size)
    }

    fn get_mask(&self) -> gfx::Mask {
        self.mask
    }

    fn get_gamma(&self) -> gfx::Gamma {
        self.gamma
    }
}

impl<R: gfx::Resources> gfx::Window<R> for Output<R> {
    fn swap_buffers(&mut self) {
        self.window.swap_buffers();
    }
}


/// Result of successful context initialization.
pub type Success = (
    Output<gfx_device_gl::Resources>,
    gfx_device_gl::Device,
    gfx_device_gl::Factory,
);

/// A streamed version of the success struct.
pub type SuccessStream = (
    gfx::OwnedStream<
        gfx_device_gl::Resources,
        gfx_device_gl::CommandBuffer,
        Output<gfx_device_gl::Resources>,
    >,
    gfx_device_gl::Device,
    gfx_device_gl::Factory,
);


/// Initialize with a window.
pub fn init(mut window: glfw::Window) -> Success {
    window.make_current();
    let device = gfx_device_gl::Device::new(|s| window.get_proc_address(s));
    let factory = device.spawn_factory();
    let out = Output {
        window: window,
        frame: factory.get_main_frame_buffer(),
        mask: gfx::COLOR | gfx::DEPTH | gfx::STENCIL, //TODO
        gamma: gfx::Gamma::Original, //TODO
    };
    (out, device, factory)
}

/// Initialize with a window, return a `Stream`.
pub fn init_stream(window: glfw::Window) -> SuccessStream {
    use gfx::traits::StreamFactory;
    let (out, device, mut factory) = init(window);
    let stream = factory.create_stream(out);
    (stream, device, factory)
}
