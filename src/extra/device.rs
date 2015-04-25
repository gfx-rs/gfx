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

//! Device extension. Allows creating a renderer or converting into
//! a single-threaded wrapper.

use std::ops;
use device;
use render::{batch, Renderer, RenderFactory};
use render::shade::ShaderParam;

/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<D: device::Device, F> {
    /// Graphics device.
    pub device: D,
    /// Resource factory.
    pub factory: F,
    /// Renderer front-end.
    pub renderer: Renderer<D::Resources, D::CommandBuffer>,
    /// Hidden batch context.
    context: batch::Context<D::Resources>,
}

impl<D: device::Device, F> ops::Deref for Graphics<D, F> {
    type Target = batch::Context<D::Resources>;

    fn deref(&self) -> &batch::Context<D::Resources> {
        &self.context
    }
}

impl<D: device::Device, F> ops::DerefMut for Graphics<D, F> {
    fn deref_mut(&mut self) -> &mut batch::Context<D::Resources> {
        &mut self.context
    }
}


impl<D: device::Device, F: device::Factory<D::Resources>> Graphics<D, F> {
    /// Clear the output with given `ClearData`.
    pub fn clear<O: ::Output<D::Resources>>(&mut self,
                 data: ::ClearData, mask: ::Mask, out: &O) {
        self.renderer.clear(data, mask, out)
    }

    /// Draw a `RefBatch` batch.
    pub fn draw<'a,
        T: ShaderParam<Resources = D::Resources>,
        O: ::Output<D::Resources>,
    >(
        &'a mut self, batch: &'a batch::RefBatch<T>, out: &O)
        -> Result<(), ::DrawError<batch::OutOfBounds>>
    {
        self.renderer.draw(&(batch, &self.context), out)
    }

    /// Draw a `CoreBatch` batch.
    pub fn draw_core<'a,
        T: ShaderParam<Resources = D::Resources>,
        O: ::Output<D::Resources>,
    >(
        &'a mut self, core: &'a batch::CoreBatch<T>, slice: &'a ::Slice<D::Resources>,
        params: &'a T, out: &O) -> Result<(), ::DrawError<batch::OutOfBounds>>
    {
        self.renderer.draw(&self.context.bind(core, slice, params), out)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }

    /// Cleanup resources after the frame.
    pub fn cleanup(&mut self) {
        self.device.after_frame();
        self.factory.cleanup();
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceExt<D: device::Device, F> {
    /// Convert to single-threaded wrapper
    fn into_graphics(mut self) -> Graphics<D, F>;
}

impl<
    D: device::Device,
    F: device::Factory<D::Resources>,
> DeviceExt<D, F> for (D, F) {
    fn into_graphics(mut self) -> Graphics<D, F> {
        let rend = self.1.create_renderer();
        Graphics {
            device: self.0,
            factory: self.1,
            renderer: rend,
            context: batch::Context::new(),
        }
    }
}
