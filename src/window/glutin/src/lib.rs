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

/// Initialize with a window builder.
pub fn init_new<
    Cf: gfx::format::RenderFormat = gfx::format::Rgba8,
    Df: gfx::format::DepthFormat = gfx::format::DepthStencil,
>(builder: glutin::WindowBuilder) -> (glutin::Window,
        gfx_device_gl::Device, gfx_device_gl::Factory,
        gfx::handle::RenderTargetView<gfx_device_gl::Resources, Cf>,
        gfx::handle::DepthStencilView<gfx_device_gl::Resources, Df>)
{
    // TODO: set the color/depth/stencil bits according to Cf and Df
    let window = builder.build().unwrap();
    unsafe { window.make_current().unwrap() };
    let (device, factory) = gfx_device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);
    // create the main color/depth targets
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as gfx::tex::NumSamples;
    let dim = (width as Size, height as Size, 1, aa.into());
    let (color_view, ds_view) = gfx_device_gl::create_main_targets(dim);
    // done
    (window, device, factory, color_view, ds_view)
}
