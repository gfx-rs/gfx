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
extern crate glutin;

use core::{format, handle, texture};
use core::memory::{self, Typed};
use device_gl::Resources as R;
use std::borrow::Borrow;

/// Initialize with a window builder.
/// Generically parametrized version over the main framebuffer format.
///
/// # Example
///
/// ```no_run
/// extern crate gfx_core;
/// extern crate gfx_device_gl;
/// extern crate gfx_window_glutin;
/// extern crate glutin;
///
/// use gfx_core::format::{DepthStencil, Rgba8};
///
/// fn main() {
///     let events_loop = glutin::EventsLoop::new();
///     let builder = glutin::WindowBuilder::new().with_title("Example".to_string());
///     let (window, device, factory, rtv, stv) =
///         gfx_window_glutin::init::<Rgba8, DepthStencil>(builder, &events_loop);
///
///     // your code
/// }
/// ```
pub fn init<Cf, Df>(builder: glutin::WindowBuilder, events_loop: &glutin::EventsLoop) ->
            (glutin::Window, device_gl::Device, device_gl::Factory,
            handle::RenderTargetView<R, Cf>, handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    let (window, device, factory, color_view, ds_view) = init_raw(builder, events_loop, Cf::get_format(), Df::get_format());
    (window, device, factory, Typed::new(color_view), Typed::new(ds_view))
}

/// Initialize with an existing Glutin window.
/// Generically parametrized version over the main framebuffer format.
///
/// # Example (using Piston to create the window)
///
/// ```rust,ignore
/// extern crate piston;
/// extern crate glutin_window;
/// extern crate gfx_window_glutin;
///
/// // Create window with Piston
/// let settings = piston::window::WindowSettings::new("Example", [800, 600]);
/// let mut glutin_window = glutin_window::GlutinWindow::new(&settings).unwrap();
///
/// // Initialise gfx
/// let (mut device, mut factory, main_color, main_depth) =
///     gfx_window_glutin::init_existing::<ColorFormat, DepthFormat>(&glutin_window.window);
///
/// let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();
/// ```
pub fn init_existing<Cf, Df>(window: &glutin::Window) ->
            (device_gl::Device, device_gl::Factory,
            handle::RenderTargetView<R, Cf>, handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    let (device, factory, color_view, ds_view) = init_existing_raw(window, Cf::get_format(), Df::get_format());
    (device, factory, Typed::new(color_view), Typed::new(ds_view))
}

fn get_window_dimensions(window: &glutin::Window) -> texture::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    let aa = window.get_pixel_format().multisampling
                   .unwrap_or(0) as texture::NumSamples;
    ((width as f32 * window.hidpi_factor()) as texture::Size, (height as f32 * window.hidpi_factor()) as texture::Size, 1, aa.into())
}

/// Initialize with a window builder. Raw version.
pub fn init_raw(builder: glutin::WindowBuilder, events_loop: &glutin::EventsLoop,
                color_format: format::Format, ds_format: format::Format) ->
                (glutin::Window, device_gl::Device, device_gl::Factory,
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
            .build(events_loop)
    }.unwrap();

    let (device, factory, color_view, ds_view) = init_existing_raw(&window, color_format, ds_format);

    (window, device, factory, color_view, ds_view)
}

/// Initialize with an existing Glutin window. Raw version.
pub fn init_existing_raw(window: &glutin::Window,
                color_format: format::Format, ds_format: format::Format) ->
                (device_gl::Device, device_gl::Factory,
                handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)
{
    unsafe { window.make_current().unwrap() };
    let (device, factory) = device_gl::create(|s|
        window.get_proc_address(s) as *const std::os::raw::c_void);

    // create the main color/depth targets
    let dim = get_window_dimensions(window);
    let (color_view, ds_view) = device_gl::create_main_targets_raw(dim, color_format.0, ds_format.0);

    // done
    (device, factory, color_view, ds_view)
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf, Df>(window: &glutin::Window, color_view: &mut handle::RenderTargetView<R, Cf>,
                    ds_view: &mut handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    let dim = color_view.get_dimensions();
    assert_eq!(dim, ds_view.get_dimensions());
    if let Some((cv, dv)) = update_views_raw(window, dim, Cf::get_format(), Df::get_format()) {
        *color_view = Typed::new(cv);
        *ds_view = Typed::new(dv);
    }
}

/// Return new main target views if the window resolution has changed from the old dimensions.
pub fn update_views_raw(window: &glutin::Window, old_dimensions: texture::Dimensions,
                        color_format: format::Format, ds_format: format::Format)
                        -> Option<(handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)>
{
    let dim = get_window_dimensions(window);
    if dim != old_dimensions {
        Some(device_gl::create_main_targets_raw(dim, color_format.0, ds_format.0))
    }else {
        None
    }
}

/// Create new main target views based on the current size of the window.
/// Best called just after a WindowResize event.
pub fn new_views<Cf, Df>(window: &glutin::Window)
        -> (handle::RenderTargetView<R, Cf>, handle::DepthStencilView<R, Df>) where
    Cf: format::RenderFormat,
    Df: format::DepthFormat,
{
    let dim = get_window_dimensions(window);
    let (color_view_raw, depth_view_raw) =
        device_gl::create_main_targets_raw(dim, Cf::get_format().0, Df::get_format().0);
    (Typed::new(color_view_raw), Typed::new(depth_view_raw))
}

pub struct SwapChain<'a> {
    // Underlying window, required for presentation
    window: &'a glutin::Window,
    // Single element backbuffer
    backbuffer: Vec<handle::RawTexture<device_gl::Resources>>,
}

impl<'a> core::SwapChain<device_gl::Backend> for SwapChain<'a> {
    fn get_images(&mut self) -> &[handle::RawTexture<device_gl::Resources>] {
        &self.backbuffer
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_gl::Resources>) -> core::Frame {
        // TODO: sync
        core::Frame::new(0)
    }

    fn present(&mut self) {
        self.window.swap_buffers();
    }
}

pub struct Surface<'a> {
    window: &'a glutin::Window,
}

impl<'a> core::Surface<device_gl::Backend> for Surface<'a> {
    type SwapChain = SwapChain<'a>;

    fn supports_queue(&self, queue_family: &device_gl::QueueFamily) -> bool { true }
    fn build_swapchain<T, Q>(&self, present_queue: Q) -> SwapChain<'a>
        where T: core::format::RenderFormat,
              Q: Borrow<device_gl::CommandQueue>
    {
        use core::handle::Producer;
        let dim = get_window_dimensions(self.window);
        let mut temp = handle::Manager::new();
        let backbuffer = temp.make_texture(
            device_gl::NewTexture::Surface(0),
            texture::Info {
                levels: 1,
                kind: texture::Kind::D2(dim.0, dim.1, dim.3),
                format: T::get_format().0,
                bind: memory::RENDER_TARGET | memory::TRANSFER_SRC,
                usage: memory::Usage::Data,
            },
        );

        SwapChain {
            window: self.window,
            backbuffer: vec![backbuffer],
        }
    }
}

pub struct Window<'a>(&'a glutin::Window);

impl<'a> core::WindowExt<device_gl::Backend> for Window<'a> {
    type Surface = Surface<'a>;
    type Adapter = device_gl::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface<'a>, Vec<device_gl::Adapter>) {
        unsafe { self.0.make_current().unwrap() };
        let adapter = device_gl::Adapter::new(|s| self.0.get_proc_address(s) as *const std::os::raw::c_void);
        let surface = Surface { window: self.0 };

        (surface, vec![adapter])
    }
}
