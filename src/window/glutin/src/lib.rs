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

extern crate gfx_core;
extern crate gfx_device_gl;
extern crate glutin;

use gfx_core::tex::Size;

/// Initialize with a window builder.
pub fn init<Cf, Df>(builder: glutin::WindowBuilder) ->
    (glutin::Window, gfx_device_gl::Device, gfx_device_gl::Factory,
    gfx_core::handle::RenderTargetView<gfx_device_gl::Resources, Cf>,
    gfx_core::handle::DepthStencilView<gfx_device_gl::Resources, Df>)
where
    Cf: gfx_core::format::RenderFormat,
    Df: gfx_core::format::DepthFormat,
{
    use gfx_core::factory::Phantom;

    let window = {
        let format = Cf::get_format();
        let color_total_bits = format.0.get_total_bits();
        let alpha_bits = format.0.get_alpha_stencil_bits();
        let depth_total_bits = Df::get_format().0.get_total_bits();
        let stencil_bits = Df::get_format().0.get_alpha_stencil_bits();
        builder
            .with_depth_buffer(depth_total_bits - stencil_bits)
            .with_stencil_buffer(stencil_bits)
            .with_pixel_format(color_total_bits - alpha_bits, alpha_bits)
            .with_srgb(Some(format.1 == gfx_core::format::ChannelType::Srgb))
            .build()
    }.unwrap();

    unsafe { window.make_current().unwrap() };
    let (device, factory) = gfx_device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);

    // create the main color/depth targets
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as gfx_core::tex::NumSamples;
    let dim = (width as Size, height as Size, 1, aa.into());
    let (color_view, ds_view) = gfx_device_gl::create_main_targets(
        dim, Cf::get_format().0, Df::get_format().0);

    // done
    (window, device, factory, Phantom::new(color_view), Phantom::new(ds_view))
}
