// Copyright 2014 The Gfx-rs Developers.
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

#![feature(plugin)]

//! Demonstrates how to initialize gfx-rs using the gl-init-rs library.

extern crate gfx;
#[macro_use]
#[plugin]
extern crate gfx_macros;
extern crate glutin;

use gfx::{Device, DeviceExt};

fn main() {
    let window = glutin::Window::new().unwrap();
    window.set_title("glutin initialization example");
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
        gfx::COLOR,
        &gfx::Frame::new(w as u16, h as u16)
    );

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }
        device.submit(renderer.as_buffer());
        window.swap_buffers();
    }
}
