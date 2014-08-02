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

use std::rc::Rc;

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

impl Window {
    #[inline]
    pub fn set_title(&self, title: &str) {
        let &Window(ref win) = self;
        win.set_title(title)
    }

    #[inline]
    pub fn get_position(&self) -> Option<(int, int)> {
        let &Window(ref win) = self;
        win.get_position()
    }

    #[inline]
    pub fn set_position(&self, x: uint, y: uint) {
        let &Window(ref win) = self;
        win.set_position(x, y)
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        let &Window(ref win) = self;
        win.get_inner_size()
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        let &Window(ref win) = self;
        win.get_outer_size()
    }

    #[inline]
    pub fn set_inner_size(&self, x: uint, y: uint) {
        let &Window(ref win) = self;
        win.set_inner_size(x, y)
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        let &Window(ref win) = self;
        win.is_closed()
    }

    #[inline]
    pub fn poll_events(&self) -> glinit::PollEventsIterator {
        let &Window(ref win) = self;
        win.poll_events()
    }

    #[inline]
    pub fn wait_events(&self) -> glinit::WaitEventsIterator {
        let &Window(ref win) = self;
        win.wait_events()
    }

    #[inline]
    pub unsafe fn make_current(&self) {
        let &Window(ref win) = self;
        win.make_current();
    }
}
