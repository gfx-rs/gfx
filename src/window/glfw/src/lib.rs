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
extern crate glfw;

use gfx_core::format::{Rgba8, DepthStencil, SurfaceType};
use gfx_core::handle;
use gfx_core::tex::{AaMode, Size};
use glfw::Context;

/// Initialize with a window.
pub fn init(window: &mut glfw::Window) -> (gfx_device_gl::Device, gfx_device_gl::Factory,
            handle::RenderTargetView<gfx_device_gl::Resources, Rgba8>,
            handle::DepthStencilView<gfx_device_gl::Resources, DepthStencil>)
{
    use gfx_core::factory::Phantom;
    window.make_current();
    let (device, factory) = gfx_device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);
    // create the main color/depth targets
    let (width, height) = window.get_framebuffer_size();
    let dim = (width as tex::Size, height as tex::Size, 1, tex::AaMode::Single);
    let (color_view, ds_view) = gfx_device_gl::create_main_targets_raw(
        dim, SurfaceType::R8_G8_B8_A8, SurfaceType::D24);
    // done
    (device, factory, Phantom::new(color_view), Phantom::new(ds_view))
}
