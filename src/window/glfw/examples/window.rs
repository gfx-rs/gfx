// Copyright 2017 The Gfx-rs Developers.
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

extern crate gfx;
extern crate gfx_window_glfw;
extern crate glfw;

use glfw::{Action, Context, Key};

pub fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
        .ok()
        .expect("Failed to initialize GLFW");

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));

    let (mut window, events) = glfw.create_window(1024, 768, "Window example", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    window.set_close_polling(true);
    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    let (_, _, _, _) = gfx_window_glfw::init(&mut window);

    //Note: actual drawing code is no different from the triangle example, or any other.

    let mut running = true;
    while running {
        glfw.poll_events();

        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => running = false,
                glfw::WindowEvent::Close => running = false,
                _ => {},
            }
        }

        window.swap_buffers();
    }
}
