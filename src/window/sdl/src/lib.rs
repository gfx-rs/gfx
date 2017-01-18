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
extern crate gfx_device_gl as device_gl;

use core::handle;
use core::format::{Format, SurfaceType, DepthStencil, RenderFormat, Srgb};
use core::memory::Typed;
use device_gl::Resources;
use sdl2::video::{DisplayMode, GLContext, Window, WindowBuilder, WindowBuildError};
use sdl2::pixels::PixelFormatEnum;

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

fn sdl2_pixel_format_from_gfx(format: Format) -> Option<PixelFormatEnum>
{
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

pub type InitOk<Cf, Df> =
    (Window, GLContext, device_gl::Device, device_gl::Factory,
     handle::RenderTargetView<Resources, Cf>,
     handle::DepthStencilView<Resources, Df>);

/// Builds an SDL2 window from a WindowBuilder struct.
///
/// # Example
///
/// ```no_run
/// extern crate gfx_window_sdl;
/// extern crate sdl2;
///
/// fn main() {
///     let sdl = sdl2::init().unwrap();
///
///     let mut builder = sdl.video().unwrap().window("Example", 800, 600);
///     let (window, glcontext, device, factory, color_view, depth_view) =
///         gfx_window_sdl::init(&mut builder).expect("gfx_window_sdl::init failed!");
///
///     // some code...
/// }
/// ```
pub fn init<Cf>(builder: &mut WindowBuilder) -> Result<InitOk<Cf, DepthStencil>, InitError>
where
    Cf: RenderFormat<Channel = Srgb>,
{
    // TODO: Support different color channel types and/or other depth formats if possible
    use core::texture::{AaMode, Size};

    let mut window = builder.opengl().build()?;

    let display_mode = DisplayMode {
        format: sdl2_pixel_format_from_gfx(Cf::get_format())
                    .ok_or(InitError::PixelFormatUnsupportedError)?,
        ..window.display_mode()?
    };
    window.set_display_mode((Some(display_mode)))?;
    window.subsystem().gl_attr().set_framebuffer_srgb_compatible(true);

    let context = window.gl_create_context()?;

    let (device, factory) = device_gl::create(|s| {
        window.subsystem().gl_get_proc_address(s) as *const std::os::raw::c_void
    });

    let (width, height) = window.drawable_size();
    let dim = (width as Size, height as Size, 1, AaMode::Single);
    let (color_view, ds_view) = device_gl::create_main_targets_raw(
            dim, SurfaceType::R8_G8_B8_A8, SurfaceType::D24);

    Ok((window, context, device, factory, Typed::new(color_view), Typed::new(ds_view)))
}
