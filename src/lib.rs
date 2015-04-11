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
    srgb: bool,
}

impl<R: gfx::Resources> gfx::Output<R> for Wrap<R> {
    fn get_handle(&self) -> Option<&gfx::FrameBufferHandle<R>> {
        Some(&self.frame)
    }

    fn get_size(&self) -> (Size, Size) {
        let (w, h) = self.window.get_inner_size().unwrap_or((0, 0));
        (w as Size, h as Size)
    }

    fn get_mask(&self) -> gfx::Mask {
        self.mask
    }

    fn does_convert_gamma(&self) -> bool {
        self.srgb
    }
}


/// Result of successful context initialization.
pub type Success = (
    Wrap<gfx_device_gl::Resources>,
    gfx_device_gl::Device,
    gfx_device_gl::Factory,
);


/// Initialize with a window builder.
pub fn init<'a>(builder: glutin::WindowBuilder<'a>)
            -> Result<Success, glutin::CreationError>
{
    let (mask, srgb) = {
        let attribs = builder.get_attributes();
        let mut mask = gfx::Mask::empty();
        match attribs.color_bits {
            Some(b) if b>0 => mask.insert(gfx::COLOR),
            _ => (),
        }
        match attribs.depth_bits {
            Some(b) if b>0 => mask.insert(gfx::DEPTH),
            _ => (),
        }
        match attribs.stencil_bits {
            Some(b) if b>0 => mask.insert(gfx::STENCIL),
            _ => (),
        }
        (mask, attribs.srgb == Some(true))
    };
    // create window
    builder.build().map(|window| {
        unsafe { window.make_current() };
        let (device, factory) = gfx_device_gl::create(|s| window.get_proc_address(s));
        let wrap = Wrap {
            window: window,
            frame: factory.get_main_frame_buffer(),
            mask: mask,
            srgb: srgb,
        };
        (wrap, device, factory)
    })
}

/// Initialize with just a title string.
pub fn init_titled(title: &str) -> Result<Success, glutin::CreationError>
{
    let builder = glutin::WindowBuilder::new()
        .with_title(title.to_string());
    init(builder)
}
