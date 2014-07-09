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

#![crate_name = "glfw_platform"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]

extern crate glfw;
#[phase(plugin, link)]
extern crate log;
extern crate libc;

extern crate device;

use glfw::Context;

struct Wrap<'a>(&'a glfw::Glfw);

impl<'a> device::GlProvider for Wrap<'a> {
    fn get_proc_address(&self, name: &str) -> *const libc::c_void {
        let Wrap(provider) = *self;
        provider.get_proc_address(name)
    }
    fn is_extension_supported(&self, name: &str) -> bool {
        let Wrap(provider) = *self;
        provider.extension_supported(name)
    }
}

pub struct Platform<C> {
    pub context: C,
}

impl<C: Context> Platform<C> {
    #[allow(visible_private_types)]
    pub fn new<'a>(context: C, provider: &'a glfw::Glfw) -> (Platform<C>, Wrap<'a>)  {
        context.make_current();
        (Platform { context: context }, Wrap(provider))
    }
}

impl<C: Context> device::GraphicsContext<device::GlBackEnd> for Platform<C> {
    fn make_current(&self) {
        self.context.make_current();
    }

    fn swap_buffers(&self) {
        self.context.swap_buffers();
    }
}

pub fn create_window<'glfw, 'title, 'monitor, 'hints>(
    glfw: &'glfw glfw::Glfw,
    width: u32,
    height: u32,
    title: &'title str,
    mode: glfw::WindowMode<'monitor>,
    preferred_setup: &'hints [glfw::WindowHint],
) -> CreateWindow<'glfw, 'title, 'monitor, 'hints> {
    CreateWindow {
        glfw: glfw,
        args: (width, height, title, mode),
        hints: vec![preferred_setup],
    }
}

pub fn create_default_window(
    glfw: &glfw::Glfw,
    width: u32,
    height: u32,
    title: &str,
    mode: glfw::WindowMode
) -> Option<(glfw::Window, Receiver<(f64, glfw::WindowEvent)>)> {
    create_window(glfw, width, height, title, mode, [
        glfw::ContextVersion(3, 2),
        glfw::OpenglForwardCompat(true),
        glfw::OpenglProfile(glfw::OpenGlCoreProfile),
    ])
    .fallback([
        glfw::ContextVersion(2, 1),
    ])
    .init().map(|(window, events)| {
        info!("[glfw_platform] Initialized with context version {}", window.get_context_version());
        (window, events)
    })
}

pub struct CreateWindow<'glfw, 'title, 'monitor, 'hints> {
    glfw: &'glfw glfw::Glfw,
    args: (u32, u32, &'title str, glfw::WindowMode<'monitor>),
    hints: Vec<&'hints [glfw::WindowHint]>,
}

impl<'glfw, 'title, 'monitor, 'hints> CreateWindow<'glfw, 'title, 'monitor, 'hints> {
    pub fn fallback(mut self, f: &'hints [glfw::WindowHint])
    -> CreateWindow<'glfw, 'title, 'monitor, 'hints> {
        self.hints.push(f);
        self
    }
    pub fn init(self) -> Option<(glfw::Window, Receiver<(f64, glfw::WindowEvent)>)> {
        let CreateWindow {
            glfw,
            mut hints,
            args: (width, height, title, mode),
        } = self;

        glfw.set_error_callback::<()>(None);
        for setup in hints.mut_iter() {
            glfw.default_window_hints();
            for hint in setup.iter() {
                glfw.window_hint(*hint);
            }
            let r = glfw.create_window(width, height, title, mode);
            if r.is_some() {
                return r;
            }
        }
        None
    }
}
