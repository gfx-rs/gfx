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

extern crate glfw;

use self::glfw::Context;

use platform::{GlApi, Platform};

pub struct GlfwPlatform<C> {
    pub context: C,
}

impl<C: Context> GlfwPlatform<C> {
    pub fn new(context: C) -> GlfwPlatform<C> {
    	context.make_current();
        GlfwPlatform { context: context }
    }
}

impl<C: Context> Platform<GlApi> for GlfwPlatform<C> {
    fn swap_buffers(&self) {
        self.context.swap_buffers();
    }
}

impl super::GlProvider for glfw::Glfw {
	fn get_proc_address(&self, name: &str) -> *::libc::c_void {
		self.get_proc_address(name)
	}
	fn is_extension_supported(&self, name: &str) -> bool {
		self.extension_supported(name)
	}
}
