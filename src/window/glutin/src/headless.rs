// Copyright 2015 The Gfx-rs Developers.
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

use device_gl::{Device, Factory, Resources as Res, create as gl_create, create_main_targets_raw};
use glutin::{GlContext, HeadlessContext};

use core::format::{Format, DepthFormat, RenderFormat};
use core::handle::{DepthStencilView, RawDepthStencilView, RawRenderTargetView, RenderTargetView};
use core::memory::Typed;
use core::texture::Dimensions;

/// Initializes device and factory from a headless context.
/// This is useful for testing as it does not require a
/// X server, thus runs on CI.
///
/// Only compiled with `headless` feature.
///
/// # Example
///
/// ```rust
/// extern crate gfx_core;
/// extern crate gfx_window_glutin;
/// extern crate glutin;
///
/// use gfx_core::format::{DepthStencil, Rgba8};
/// use gfx_core::texture::AaMode;
/// use gfx_window_glutin::init_headless;
/// use glutin::HeadlessRendererBuilder;
///
/// # fn main() {
/// let dim = (256, 256, 8, AaMode::Multi(4));
///
/// let context = HeadlessRendererBuilder::new(dim.0 as u32, dim.1 as u32)
///     .build()
///     .expect("Failed to build headless context");
///
/// let (mut device, _, _, _) = init_headless::<Rgba8, DepthStencil>(&context, dim);
/// # }
/// ```
pub fn init_headless<Cf, Df>(context: &HeadlessContext, dim: Dimensions)
                             -> (Device, Factory,
                                 RenderTargetView<Res, Cf>, DepthStencilView<Res, Df>)
    where
        Cf: RenderFormat,
        Df: DepthFormat,
{
    let (device, factory, color_view, ds_view) = init_headless_raw(context, dim,
                                                                   Cf::get_format(),
                                                                   Df::get_format());
    (device, factory, Typed::new(color_view), Typed::new(ds_view))
}

/// Raw version of [`init_headless`].
///
/// [`init_headless`]: fn.init_headless.html
pub fn init_headless_raw(context: &HeadlessContext, dim: Dimensions, color: Format, depth: Format)
                         -> (Device, Factory,
                             RawRenderTargetView<Res>, RawDepthStencilView<Res>)
{
    unsafe { context.make_current().unwrap() };

    let (device, factory) = gl_create(|s|
        context.get_proc_address(s) as *const c_void);

    // create the main color/depth targets
    let (color_view, ds_view) = create_main_targets_raw(dim, color.0, depth.0);

    // done
    (device, factory, color_view, ds_view)
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::format::{DepthStencil, Rgba8};
    use core::texture::AaMode;
    use core::Device;

    #[test]
    fn test_headless() {
        use glutin::{HeadlessRendererBuilder};

        let dim = (256, 256, 8, AaMode::Multi(4));

        let context: HeadlessContext = HeadlessRendererBuilder::new(dim.0 as u32, dim.1 as u32)
            .build()
            .expect("Failed to build headless context");

        let (mut device, _, _, _) = init_headless::<Rgba8, DepthStencil>(&context, dim);

        device.cleanup();
    }
}
