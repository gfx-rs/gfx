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

#[macro_use]
extern crate log;
extern crate sdl2;
extern crate gfx_core as core;
extern crate gfx_device_gl;

use core::handle;
use core::format::{ChannelType, DepthFormat, Format, RenderFormat};
pub use gfx_device_gl::{Device, Factory, Resources};
use sdl2::video::{DisplayMode, GLContext, Window, WindowBuilder, WindowBuildError};
use sdl2::pixels::PixelFormatEnum;
use core::{format, texture};
use core::memory::Typed;
use gfx_device_gl::Resources as R;

#[derive(Debug)]
pub enum InitError {
    PixelFormatUnsupportedError,
    WindowBuildError(WindowBuildError),
    SdlError(String),
}

impl From<String> for InitError {
    fn from(e: String) -> Self {
        InitError::SdlError(e)
    }
}

impl From<WindowBuildError> for InitError {
    fn from(e: WindowBuildError) -> Self {
        InitError::WindowBuildError(e)
    }
}

fn sdl2_pixel_format_from_gfx(format: Format) -> Option<PixelFormatEnum> {
    use core::format::SurfaceType::*;
    use sdl2::pixels::PixelFormatEnum as SdlFmt;

    let Format(surface, _) = format;

    match surface {
        R4_G4_B4_A4 => Some(SdlFmt::RGBA4444),
        R5_G5_B5_A1 => Some(SdlFmt::RGBA5551),
        R5_G6_B5 => Some(SdlFmt::RGB565),
        R8_G8_B8_A8 => Some(SdlFmt::RGBA8888),
        B8_G8_R8_A8 => Some(SdlFmt::BGRA8888),
        R10_G10_B10_A2 => {
            warn!("The transfer operations with this format may produce different results on SDL \
                   compared to Glutin/GLFW, beware!");
            Some(SdlFmt::ARGB2101010)
        }
        R4_G4 | R8 | R8_G8 | R11_G11_B10 | R16 | R16_G16 | R16_G16_B16 |
        R16_G16_B16_A16 | R32 | R32_G32 | R32_G32_B32 | R32_G32_B32_A32 | D16 | D24 |
        D24_S8 | D32 => None,
    }
}

pub type InitRawOk = (Window, GLContext, Device, Factory,
    handle::RawRenderTargetView<Resources>, handle::RawDepthStencilView<Resources>);

pub type InitOk<Cf, Df> =
    (Window, GLContext, Device, Factory,
     handle::RenderTargetView<Resources, Cf>,
     handle::DepthStencilView<Resources, Df>);

/// Builds an SDL2 window from a WindowBuilder struct.
///
/// # Example
///
/// ```no_run
/// extern crate gfx_core;
/// extern crate gfx_window_sdl;
/// extern crate sdl2;
///
/// use gfx_core::format::{DepthStencil, Rgba8};
///
/// fn main() {
///     let sdl = sdl2::init().unwrap();
///
///     let builder = sdl.video().unwrap().window("Example", 800, 600);
///     let (window, glcontext, device, factory, color_view, depth_view) =
///         gfx_window_sdl::init::<Rgba8, DepthStencil>(builder).expect("gfx_window_sdl::init failed!");
///
///     // some code...
/// }
/// ```
pub fn init<Cf, Df>(builder: WindowBuilder) -> Result<InitOk<Cf, Df>, InitError>
where
    Cf: RenderFormat,
    Df: DepthFormat,
{
    use core::memory::Typed;
    init_raw(builder, Cf::get_format(), Df::get_format())
        .map(|(w, gl, d, f, color_view, ds_view)|
            (w, gl, d, f, Typed::new(color_view), Typed::new(ds_view)))
}

pub fn init_raw(mut builder: WindowBuilder, cf: Format, df: Format)
                -> Result<InitRawOk, InitError> {
    use core::texture::{AaMode, Size};

    let mut window = builder.opengl().build()?;

    let display_mode = DisplayMode {
        format: sdl2_pixel_format_from_gfx(cf)
                    .ok_or(InitError::PixelFormatUnsupportedError)?,
        ..window.display_mode()?
    };
    window.set_display_mode((Some(display_mode)))?;
    {
        let depth_total_bits = df.0.get_total_bits();
        let stencil_bits = df.0.get_alpha_stencil_bits();
        let attr = window.subsystem().gl_attr();
        attr.set_framebuffer_srgb_compatible(cf.1 == ChannelType::Srgb);
        attr.set_alpha_size(cf.0.get_alpha_stencil_bits());
        attr.set_depth_size(depth_total_bits - stencil_bits);
        attr.set_stencil_size(stencil_bits);
        attr.set_context_flags().set();
    }

    let context = window.gl_create_context()?;

    let (device, factory) = gfx_device_gl::create(|s| {
        window.subsystem().gl_get_proc_address(s) as *const std::os::raw::c_void
    });

    let (width, height) = window.drawable_size();
    let dim = (width as Size, height as Size, 1, AaMode::Single);
    let (color_view, ds_view) = gfx_device_gl::create_main_targets_raw(dim, cf.0, df.0);

    Ok((window, context, device, factory, color_view, ds_view))
}

fn get_window_dimensions(window: &sdl2::video::Window) -> texture::Dimensions {
    let (width, height) = window.size();
    let aa = window.subsystem().gl_attr().multisample_samples() as texture::NumSamples;
    (width as texture::Size, height as texture::Size, 1, aa.into())
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf, Df>(window: &sdl2::video::Window, color_view: &mut handle::RenderTargetView<R, Cf>,
                            ds_view: &mut handle::DepthStencilView<R, Df>)
where
    Cf: format::RenderFormat,
    Df: format::DepthFormat
{
    let dim = color_view.get_dimensions();
    assert_eq!(dim, ds_view.get_dimensions());
    if let Some((cv, dv)) = update_views_raw(window, dim, Cf::get_format(), Df::get_format()) {
        *color_view = Typed::new(cv);
        *ds_view = Typed::new(dv);
    }
}

/// Return new main target views if the window resolution has changed from the old dimensions.
pub fn update_views_raw(window: &sdl2::video::Window, old_dimensions: texture::Dimensions,
                        color_format: format::Format, ds_format: format::Format)
                        -> Option<(handle::RawRenderTargetView<R>, handle::RawDepthStencilView<R>)>
{
    let dim = get_window_dimensions(window);
    if dim != old_dimensions {
        Some(gfx_device_gl::create_main_targets_raw(dim, color_format.0, ds_format.0))
    } else {
        None
    }
}
