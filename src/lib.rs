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
extern crate glutin;

use gfx::tex::Size;

/// A wrapper around the window that implements `Output`.
pub struct Wrap<R: gfx::Resources> {
    /// Glutin window in the open.
    pub window: glutin::Window,
    frame: gfx::FrameBufferHandle<R>,
    mask: gfx::Mask,
    gamma: gfx::Gamma,
}

impl<R: gfx::Resources> gfx::Output<R> for Wrap<R> {
    fn get_handle(&self) -> Option<&gfx::FrameBufferHandle<R>> {
        Some(&self.frame)
    }

    fn get_size(&self) -> (Size, Size) {
        let factor = self.window.hidpi_factor();
        let (w, h) = self.window.get_inner_size().unwrap_or((0, 0));
        ((w as f32 * factor) as Size, (h as f32 * factor) as Size)
    }

    fn get_mask(&self) -> gfx::Mask {
        self.mask
    }

    fn get_gamma(&self) -> gfx::Gamma {
        self.gamma
    }
}

impl<R: gfx::Resources> gfx::Window<R> for Wrap<R> {
    fn swap_buffers(&mut self) {
        self.window.swap_buffers();
    }
}


/// Result of successful context initialization.
pub type Success = (
    Wrap<gfx_device_gl::Resources>,
    gfx_device_gl::Device,
    gfx_device_gl::Factory,
);


/// Initialize with a window.
pub fn init(window: glutin::Window) -> Success {
    // actual queries are WIP: https://github.com/tomaka/glutin/pull/372
    unsafe { window.make_current() };
    let (device, factory) = gfx_device_gl::create(|s| window.get_proc_address(s));
    let wrap = Wrap {
        window: window,
        frame: factory.get_main_frame_buffer(),
        mask: gfx::COLOR | gfx::DEPTH | gfx::STENCIL, //TODO
        gamma: gfx::Gamma::Original, //TODO
    };
    (wrap, device, factory)
}
