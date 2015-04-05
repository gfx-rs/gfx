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
#![plugin(gfx_macros)]

//! Demonstrates how to initialize gfx-rs using the gl-init-rs library.

extern crate gfx;
extern crate gfx_device_gl;
extern crate glutin;

use gfx::traits::*;

pub fn main() {
    let window = glutin::Window::new().unwrap();
    window.set_title("glutin initialization example");
    unsafe { window.make_current() };
    let (w, h) = window.get_inner_size().unwrap();

    let mut graphics = gfx_device_gl::create(|s| window.get_proc_address(s))
                                     .into_graphics();

    'main: loop {
        // quit when Esc is pressed.
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        graphics.clear(
            gfx::ClearData {
                color: [0.3, 0.3, 0.3, 1.0],
                depth: 1.0,
                stencil: 0,
            },
            gfx::COLOR,
            &gfx::Frame::new(w as u16, h as u16)
        );

        graphics.end_frame();
        window.swap_buffers();
        graphics.cleanup();
    }
}
