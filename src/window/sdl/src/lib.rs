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

extern crate sdl2;
extern crate gfx_core;
extern crate gfx_device_gl;

use gfx_core::handle;
use gfx_core::format::{SurfaceType, DepthStencil, Srgba8};
use gfx_device_gl::Resources;

/// Builds an SDL2 window from a WindowBuilder struct.
///
/// # Example
///
/// ```
/// extern crate gfx_window_sdl;
/// extern crate sdl2;
/// 
/// fn main() {
///     let sdl = sdl2::init().unwrap();
/// 
///     let mut builder = sdl.video().unwrap().window("Example", 800, 600);
///     let (window, glcontext, device, factory, color_view, depth_view) =
///         gfx_window_sdl::init(&mut builder);
///
///     // some code...
/// }
/// ```
pub fn init(builder: &mut sdl2::video::WindowBuilder) ->
    (sdl2::video::Window, sdl2::video::GLContext,
     gfx_device_gl::Device, gfx_device_gl::Factory,
     handle::RenderTargetView<Resources, Srgba8>,
     handle::DepthStencilView<Resources, DepthStencil>)
{
    use gfx_core::factory::Typed;
    use gfx_core::tex::{AaMode, Size};

    let window = builder.opengl().build().unwrap(); //TODO: use actual settings
    let context = window.gl_create_context().unwrap();

    let (device, factory) = gfx_device_gl::create(|s| {
        window.subsystem().gl_get_proc_address(s) as *const std::os::raw::c_void
    });

    let (width, height) = window.drawable_size();
    let dim = (width as Size, height as Size, 1, AaMode::Single);
    let (color_view, ds_view) = gfx_device_gl::create_main_targets_raw(
            dim, SurfaceType::R8_G8_B8_A8, SurfaceType::D24);
    (window, context, device, factory, Typed::new(color_view), Typed::new(ds_view))
}

