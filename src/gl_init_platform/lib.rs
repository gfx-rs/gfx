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

#![crate_name = "gl_init_platform"]
#![comment = "An adaptor for gl-init-rs `Context`s that allows interoperability \
              with gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]

extern crate glinit = "gl-init-rs";
#[phase(plugin, link)]
extern crate log;
extern crate libc;

extern crate device;

pub struct Window(glinit::Window);

impl Window {
    #[inline]
    pub fn new() -> Option<Window> {
        let win = match glinit::Window::new() {
            Err(_) => return None,
            Ok(w) => w
        };

        Some(Window(win))
    }

    #[inline]
    pub fn from_builder(builder: glinit::WindowBuilder) -> Option<Window> {
        let win = match builder.build() {
            Err(_) => return None,
            Ok(w) => w
        };

        Some(Window(win))
    }

    #[inline]
    pub fn from_existing(win: glinit::Window) -> Window {
        Window(win)
    }
}

impl<'a> device::GlProvider for &'a Window {
    fn get_proc_address(&self, name: &str) -> *const libc::c_void {
        let &&Window(ref win) = self;
        win.get_proc_address(name) as *const libc::c_void
    }
}

impl<'a> device::GraphicsContext<device::GlBackEnd> for &'a Window {
    fn make_current(&self) {
        let &&Window(ref win) = self;
        unsafe { win.make_current() };
    }

    fn swap_buffers(&self) {
        let &&Window(ref win) = self;
        win.swap_buffers();
    }
}

impl Deref<glinit::Window> for Window {
    fn deref(&self) -> &glinit::Window {
        let &Window(ref win) = self;
        win
    }
}
