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
use render::{batch, Renderer};
use render::ext::factory::RenderFactory;
use render::shade::ShaderParam;
use render::target::Frame;


/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<W, D: device::Device, F> {
    /// Owner window.
    pub window: W,
    /// Main frame buffer. Private to avoid modification.
    main_frame: Frame<D::Resources>,
    /// Graphics device.
    pub device: D,
    /// Resource factory.
    pub factory: F,
    /// Renderer front-end.
    pub renderer: Renderer<D::Resources, D::CommandBuffer>,
    /// Hidden batch context.
    pub context: batch::Context<D::Resources>,
}

impl<W, D: device::Device, F> ops::Deref for Graphics<W, D, F> {
    type Target = batch::Context<D::Resources>;

    fn deref(&self) -> &batch::Context<D::Resources> {
        &self.context
    }
}

impl<W, D: device::Device, F> ops::DerefMut for Graphics<W, D, F> {
    fn deref_mut(&mut self) -> &mut batch::Context<D::Resources> {
        &mut self.context
    }
}


impl<W: ::Window, D: device::Device, F: device::Factory<D::Resources>> Graphics<W, D, F> {
    /// Clear the main frame with a given `ClearData`.
    pub fn clear(&mut self, data: ::ClearData, mask: ::Mask) {
        self.renderer.clear(data, mask, &self.main_frame)
    }

    /// Draw a `RefBatch` batch.
    pub fn draw<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                batch: &'a batch::RefBatch<T>)
                -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&(batch, &self.context), &self.main_frame)
    }

    /// Draw a `CoreBatch` batch.
    pub fn draw_core<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                     core: &'a batch::CoreBatch<T>, slice: &'a ::Slice<D::Resources>,
                     params: &'a T)
                     -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&self.context.bind(core, slice, params), &self.main_frame)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        // execute the commands
        self.device.submit(self.renderer.as_buffer());
        // cleanup commands and resources
        self.renderer.reset();
        self.device.after_frame();
        self.factory.cleanup();
        // update the frame dimension
        let (w, h) = self.window.get_dimensions();
        self.main_frame.width = w;
        self.main_frame.height = h;
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceExt<W, D: device::Device, F> {
    /// Convert to single-threaded wrapper
    fn into_graphics(mut self, window: W) -> Graphics<W, D, F>;
}

impl<
    W: ::Window,
    D: device::Device,
    F: device::Factory<D::Resources>,
> DeviceExt<W, D, F> for (D, F) {
    fn into_graphics(mut self, window: W) -> Graphics<W, D, F> {
        let (w, h) = window.get_dimensions();
        let rend = self.1.create_renderer();
        Graphics {
            window: window,
            main_frame: Frame::new(w, h),
            device: self.0,
            factory: self.1,
            renderer: rend,
            context: batch::Context::new(),
        }
    }
}
