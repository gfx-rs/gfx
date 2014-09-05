#![feature(phase)]
#![crate_name = "gl-init"]

//! Demonstrates how to initialize gfx-rs using the gl-init-rs library.

extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate gl_init;
extern crate native;

use gfx::{Device, DeviceHelper};

// We need to run on the main thread for GLFW, so ensure we are using the `native` runtime. This is
// technically not needed, since this is the default, but it's not guaranteed.
#[start]
fn start(argc: int, argv: *const *const u8) -> int {
     native::start(argc, argv, main)
}

fn main() {
    let window = gl_init::Window::new().unwrap();
    window.set_title("gl-init-rs initialization example");
    unsafe { window.make_current() };
    let (w, h) = window.get_inner_size().unwrap();

    let mut device = gfx::GlDevice::new(|s| window.get_proc_address(s));
    let mut renderer = device.create_renderer();

    renderer.clear(
        gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        },
        gfx::Color,
        &gfx::Frame::new(w as u16, h as u16)
    );

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                gl_init::KeyboardInput(_, _, Some(gl_init::Escape), _) => break 'main,
                gl_init::Closed => break 'main,
                _ => {},
            }
        }
        device.submit(renderer.as_buffer());
        window.swap_buffers();
    }
}
