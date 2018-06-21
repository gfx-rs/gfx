//! Initialize with a window.
//!
//! # Example
//!
//! ```no_run
//! extern crate gfx_window_glfw;
//! extern crate glfw;
//! extern crate gfx_core;
//!
//! use gfx_core::WindowExt;
//!
//! fn main() {
//!     use glfw::Context;
//!
//!     let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
//!         .ok().expect("Failed to initialize GLFW");
//!
//!     glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
//!     glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
//!     glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
//!
//!     let (window, events) = glfw
//!         .create_window(800, 600, "Example", glfw::WindowMode::Windowed)
//!         .expect("Failed to create GLFW window.");
//!
//!     let mut window = gfx_window_glfw::Window::new(window);
//!     glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
//!     let (surface, adapters) = window.get_surface_and_adapters();
//!
//!     // some code...
//! }
//! ```

extern crate gfx_core as core;
extern crate gfx_device_gl as device_gl;
extern crate glfw;

use std::rc::Rc;
use std::cell::RefCell;
use core::format::{Rgba8, DepthStencil, SurfaceType};
use core::{handle, memory};
use core::texture::{self, AaMode, Size};
use glfw::Context;

pub struct Swapchain {
    // Underlying window, required for presentation
    window: Rc<RefCell<glfw::Window>>,
    // Single element backbuffer
    backbuffer: [core::Backbuffer<device_gl::Backend>; 1],
}

impl<'a> core::Swapchain<device_gl::Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_gl::Backend>] {
        &self.backbuffer
    }

    fn acquire_image(&mut self, sync: core::FrameSync<device_gl::Resources>) -> Result<core::Frame, ()> {
        // TODO: fence sync
        Ok(core::Frame::new(0))
    }

    fn present<'i, Q, I>(&mut self, _: &mut Q, _: I)
    where
        Q: AsMut<device_gl::CommandQueue>,
        I: IntoIterator,
        I::Item: Borrow<handle::Semaphore<device_gl::Resources>>,
    {
        self.window.borrow_mut().swap_buffers();
    }
}

pub struct Surface {
    window: Rc<RefCell<glfw::Window>>,
    manager: handle::Manager<device_gl::Resources>,
}

impl<'a> core::Surface<device_gl::Backend> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, _: &device_gl::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, _: &Q) -> Swapchain
        where Q: AsRef<device_gl::CommandQueue>
    {
        use core::handle::Producer;
        let (width, height) = self.window.borrow_mut().get_framebuffer_size();
        let dim = (width as Size, height as Size, 1, AaMode::Single);
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

pub struct Window(pub Rc<RefCell<glfw::Window>>);
impl Window {
    pub fn new(window: glfw::Window) -> Self {
        Window(Rc::new(RefCell::new(window)))
    }
}

impl<'a> core::WindowExt<device_gl::Backend> for Window {
    type Surface = Surface;
    type Adapter = device_gl::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_gl::Adapter>) {
        self.0.borrow_mut().make_current();
        let adapter = device_gl::Adapter::new(|s| self.0.borrow_mut().get_proc_address(s) as *const std::os::raw::c_void);
        let surface = Surface {
            window: self.0.clone(),
            manager: handle::Manager::new(),
        };

        (surface, vec![adapter])
    }
}
