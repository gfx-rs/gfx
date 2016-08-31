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

extern crate gfx_core as core;
extern crate gfx_device_gl as device_gl;
extern crate glfw;

use core::format::{Rgba8, DepthStencil, SurfaceType};
use core::handle;
use core::memory::Typed;
use core::texture::{AaMode, Size};
use glfw::Context;

/// Initialize with a window.
///
/// # Example
///
/// ```no_run
/// extern crate gfx_window_glfw;
/// extern crate glfw;
/// 
/// fn main() {
///     use glfw::Context;
///
///     let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
///         .ok().expect("Failed to initialize GLFW");
/// 
///     glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
///     glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
///     glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
/// 
///     let (mut window, events) = glfw
///         .create_window(800, 600, "Example", glfw::WindowMode::Windowed)
///         .expect("Failed to create GLFW window.");
/// 
///     window.make_current();
///     glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
///     let (device, mut factory, color_view, depth_view) =
///         gfx_window_glfw::init(&mut window);
///
///     // some code...
/// }
/// ```
pub fn init(window: &mut glfw::Window) ->
    (device_gl::Device,
     device_gl::Factory,
     handle::RenderTargetView<device_gl::Resources, Rgba8>,
     handle::DepthStencilView<device_gl::Resources, DepthStencil>)
{
    window.make_current();
    let (device, factory) = device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);
    // create the main color/depth targets
    let (width, height) = window.get_framebuffer_size();
    let dim = (width as Size, height as Size, 1, AaMode::Single);
    let (color_view, ds_view) = device_gl::create_main_targets_raw(
        dim, SurfaceType::R8_G8_B8_A8, SurfaceType::D24);
    // done
    (device, factory, Typed::new(color_view), Typed::new(ds_view))
}
