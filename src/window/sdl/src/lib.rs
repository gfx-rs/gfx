//! Builds an SDL2 window from a WindowBuilder struct.
//!
//! # Example
//!
//! ```no_run
//! extern crate gfx_core;
//! extern crate gfx_window_sdl;
//! extern crate sdl2;
//!
//! use gfx_core::WindowExt;
//! use gfx_core::format::{Formatted, DepthStencil, Rgba8};
//!
//! fn main() {
//!     let sdl = sdl2::init().unwrap();
//!
//!     let builder = sdl.video().unwrap().window("Example", 800, 600);
//!     let (window, glcontext) = gfx_window_sdl::build(
//!             builder, Rgba8::get_format(), DepthStencil::get_format()).unwrap();
//!     let mut window = gfx_window_sdl::Window::new(window);
//!     let (surface, adapters) = window.get_surface_and_adapters();
//!
//!     // some code...
//! }
//! ```

#[macro_use]
extern crate log;
extern crate sdl2;
extern crate gfx_core as core;
extern crate gfx_device_gl as device_gl;

use core::handle;
use core::format::{ChannelType, DepthFormat, Format, RenderFormat};
pub use device_gl::{Backend, Resources};
use sdl2::video::{DisplayMode, GLContext, WindowBuilder, WindowBuildError};
use sdl2::pixels::PixelFormatEnum;
use core::{format, memory, texture};
use core::texture::Size;
use core::memory::Typed;
use device_gl::Resources as R;
use std::rc::Rc;

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
        D24_S8 | D32 | D32_S8 => None,
    }
}

fn get_window_dimensions(window: &sdl2::video::Window) -> texture::Dimensions {
    let (width, height) = window.size();
    let aa = window.subsystem().gl_attr().multisample_samples() as texture::NumSamples;
    (width as texture::Size, height as texture::Size, 1, aa.into())
}

pub struct Swapchain {
    // Underlying window, required for presentation
    window: Rc<sdl2::video::Window>,
    // Single element backbuffer
    backbuffer: [core::Backbuffer<Backend>; 1],
}

impl core::Swapchain<Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<Backend>] {
        &self.backbuffer
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<R>) -> Result<core::Frame, ()> {
        // TODO: fence sync
        Ok(core::Frame::new(0))
    }

    fn present<Q>(&mut self, _: &mut Q, _: &[&handle::Semaphore<device_gl::Resources>])
        where Q: AsMut<device_gl::CommandQueue>
    {
        self.window.gl_swap_window();
    }
}

pub struct Surface {
    window: Rc<sdl2::video::Window>,
    manager: handle::Manager<R>,
}

impl core::Surface<Backend> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, _: &device_gl::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, _: &Q) -> Swapchain
        where Q: AsRef<device_gl::CommandQueue>
    {
        use core::handle::Producer;
        let dim = get_window_dimensions(&self.window);
        let color = self.manager.make_texture(
            device_gl::NewTexture::Surface(0),
            texture::Info {
                levels: 1,
                kind: texture::Kind::D2(dim.0, dim.1, dim.3),
                format: config.color_format.0,
                bind: memory::RENDER_TARGET | memory::TRANSFER_SRC,
                usage: memory::Usage::Data,
            },
        );

        let ds = config.depth_stencil_format.map(|ds_format| {
            self.manager.make_texture(
                device_gl::NewTexture::Surface(0),
                texture::Info {
                    levels: 1,
                    kind: texture::Kind::D2(dim.0, dim.1, dim.3),
                    format: ds_format.0,
                    bind: memory::DEPTH_STENCIL | memory::TRANSFER_SRC,
                    usage: memory::Usage::Data,
                },
            )
        });

        Swapchain {
            window: self.window.clone(),
            backbuffer: [(color, ds); 1],
        }
    }
}

pub struct Window(Rc<sdl2::video::Window>);
impl Window {
    /// Create a new window.
    pub fn new(window: sdl2::video::Window) -> Self {
        Window(Rc::new(window))
    }

    /// Get the internal SDL2 window.
    pub fn raw(&self) -> &sdl2::video::Window {
        &self.0
    }
}

impl core::WindowExt<Backend> for Window {
    type Surface = Surface;
    type Adapter = device_gl::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_gl::Adapter>) {
        let adapter = device_gl::Adapter::new(|s|
            self.0.subsystem().gl_get_proc_address(s) as *const std::os::raw::c_void);
        let surface = Surface {
            window: self.0.clone(),
            manager: handle::Manager::new(),
        };

        (surface, vec![adapter])
    }
}

/// Helper function for setting up an GL window and context
pub fn build(mut builder: WindowBuilder, cf: Format, df: Format) -> Result<(sdl2::video::Window, GLContext), InitError> {
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

    Ok((window, context))
}
