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

use gfx_core::{format, handle, tex};
use gfx_device_gl::Resources as R;

/// Initialize with a window builder.
/// Generically parametrized version over the main framebuffer format.
pub fn init<Cf, Df>(builder: glutin::WindowBuilder) ->
            (glutin::Window, gfx_device_gl::Device, gfx_device_gl::Factory,
            handle::RenderTargetView<R, Cf>, handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    use gfx_core::factory::Phantom;
    let (window, device, factory, color_view, ds_view) = init_raw(builder, Cf::get_format(), Df::get_format());
    (window, device, factory, Phantom::new(color_view), Phantom::new(ds_view))
}

fn get_window_dimensions(window: &glutin::Window) -> tex::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as tex::NumSamples;
    (width as tex::Size, height as tex::Size, 1, aa.into())
}

/// Initialize with a window builder. Raw version.
pub fn init_raw(builder: glutin::WindowBuilder,
                color_format: format::Format, ds_format: format::Format) ->
                (glutin::Window, gfx_device_gl::Device, gfx_device_gl::Factory,
                handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)
{
    let window = {
        let color_total_bits = color_format.0.get_total_bits();
        let alpha_bits = color_format.0.get_alpha_stencil_bits();
        let depth_total_bits = ds_format.0.get_total_bits();
        let stencil_bits = ds_format.0.get_alpha_stencil_bits();
        builder
            .with_depth_buffer(depth_total_bits - stencil_bits)
            .with_stencil_buffer(stencil_bits)
            .with_pixel_format(color_total_bits - alpha_bits, alpha_bits)
            .with_srgb(Some(color_format.1 == format::ChannelType::Srgb))
            .build()
    }.unwrap();

    unsafe { window.make_current().unwrap() };
    let (device, factory) = gfx_device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);

    // create the main color/depth targets
    let dim = get_window_dimensions(&window);
    let (color_view, ds_view) = gfx_device_gl::create_main_targets(dim, color_format.0, ds_format.0);

    // done
    (window, device, factory, color_view, ds_view)
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf, Df>(window: &glutin::Window, color_view: &mut handle::RenderTargetView<R, Cf>,
                    ds_view: &mut handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    use gfx_core::factory::Phantom;
    let dim = color_view.get_dimensions();
    assert_eq!(dim, ds_view.get_dimensions());
    if let Some((cv, dv)) = update_views_raw(window, dim, Cf::get_format(), Df::get_format()) {
        *color_view = Phantom::new(cv);
        *ds_view = Phantom::new(dv);
    }
}

/// Return new main target views if the window resolution has changed from the old dimensions.
pub fn update_views_raw(window: &glutin::Window, old_dimensions: tex::Dimensions,
                        color_format: format::Format, ds_format: format::Format)
                        -> Option<(handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)>
{
    let dim = get_window_dimensions(window);
    if dim != old_dimensions {
        Some(gfx_device_gl::create_main_targets(dim, color_format.0, ds_format.0))
    }else {
        None
    }
}
