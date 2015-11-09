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
extern crate libc;

use gfx::tex::Size;

/// A wrapper around the window that implements `Output`.
pub struct Output<R: gfx::Resources> {
    /// Glutin window in the open.
    pub window: glutin::Window,
    frame: gfx::handle::FrameBuffer<R>,
    mask: gfx::Mask,
    supports_gamma_convertion: bool,
    gamma: gfx::Gamma,
}

impl<R: gfx::Resources> Output<R> {
    /// Try to set the gamma conversion.
    pub fn set_gamma(&mut self, gamma: gfx::Gamma) -> Result<(), ()> {
        if self.supports_gamma_convertion || gamma == gfx::Gamma::Original {
            self.gamma = gamma;
            Ok(())
        }else {
            Err(())
        }
    }
}

impl<R: gfx::Resources> gfx::Output<R> for Output<R> {
    fn get_handle(&self) -> Option<&gfx::handle::FrameBuffer<R>> {
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

impl<R: gfx::Resources> gfx::Window<R> for Output<R> {
    fn swap_buffers(&mut self) {
        self.window.swap_buffers().unwrap();
    }
}


/// Result of successful context initialization.
pub type Success = (
    gfx::OwnedStream<
        gfx_device_gl::Device,
        Output<gfx_device_gl::Resources>,
    >,
    gfx_device_gl::Device,
    gfx_device_gl::Factory,
);

/// Initialize with a window.
pub fn init(window: glutin::Window) -> Success {
    use gfx::traits::StreamFactory;
    unsafe { window.make_current().unwrap() };
    let format = window.get_pixel_format();
    let (device, mut factory) = gfx_device_gl::create(|s| window.get_proc_address(s) as *const libc::c_void);
    let out = Output {
        window: window,
        frame: factory.get_main_frame_buffer(),
        mask: if format.color_bits != 0 { gfx::COLOR } else { gfx::Mask::empty() } |
            if format.depth_bits != 0 { gfx::DEPTH } else  { gfx::Mask::empty() } |
            if format.stencil_bits != 0 { gfx::STENCIL } else { gfx::Mask::empty() },
        supports_gamma_convertion: format.srgb,
        gamma: gfx::Gamma::Original,
    };
    let stream = factory.create_stream(out);
    (stream, device, factory)
}
