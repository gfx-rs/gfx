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

use std::os::raw::c_void;

use core;
use device_gl;
use glutin::{GlContext, HeadlessContext};

/// Initializes device and factory from a headless context.
/// This is useful for testing as it does not require a
/// X server, thus runs on CI.
///
/// # Example
///
/// ```rust
/// extern crate gfx_core;
/// extern crate gfx_window_glutin;
/// extern crate glutin;
///
/// use gfx_core::texture::AaMode;
/// use gfx_core::Headless;
/// use glutin::HeadlessRendererBuilder;
///
/// # fn main() {
/// let dim = (256, 256, 8, AaMode::Multi(4));
///
/// let context = HeadlessRendererBuilder::new(dim.0 as u32, dim.1 as u32)
///     .build()
///     .expect("Failed to build headless context");
///
/// let mut headless = gfx_window_glutin::Headless(context);
/// let adapters = headless.get_adapters();
/// # }
/// ```

pub struct Headless(pub HeadlessContext);

impl core::Headless<device_gl::Backend> for Headless {
    type Adapter = device_gl::Adapter;

    fn get_adapters(&mut self) -> Vec<device_gl::Adapter> {
        unsafe { self.0.make_current().unwrap() };
        let adapter = device_gl::Adapter::new(|s| self.0.get_proc_address(s) as *const c_void);
        vec![adapter]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless() {
        use core::Headless;
        use glutin::{HeadlessRendererBuilder};
        let context: HeadlessContext = HeadlessRendererBuilder::new(256, 256)
            .build()
            .expect("Failed to build headless context");

        let mut headless = Headless(context);
        let adapters = headless.get_adapters();
    }
}
