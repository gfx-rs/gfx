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

//! Swapchain extension.
//!
//! This module serves as an extension to the `Swapchain` trait from the core. This module
//! exposes extension functions and shortcuts to aid with handling the swapchain.

use {format, handle, texture, Backend, Device, Swapchain};
use memory::Typed;

/// Extension trait for Swapchains
///
/// Every `Swapchain` automatically implements `SwapchainExt`.
pub trait SwapchainExt<B: Backend>: Swapchain<B> {
    /// Create color RTVs for all backbuffer images.
    // TODO: error handling
    fn create_color_views<T: format::RenderFormat>(&mut self, device: &mut B::Device) -> Vec<handle::RenderTargetView<B::Resources, T>> {
        self.get_backbuffers()
            .iter()
            .map(|&(ref color, _)| {
                let color_desc = texture::RenderDesc {
                    channel: T::get_format().1,
                    level: 0,
                    layer: None,
                };
                let rtv = device.view_texture_as_render_target_raw(color, color_desc)
                                .unwrap();
                Typed::new(rtv)
            })
            .collect()
    }
}

impl <T, B: Backend> SwapchainExt<B> for T where T: Swapchain<B> { }
