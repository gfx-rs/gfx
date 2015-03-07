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
use render::{batch, Renderer, RenderState, ParamStorage};
use render::shade::ShaderParam;

/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<D: device::Device> {
    /// Graphics device.
    pub device: D,
    /// Renderer front-end.
    pub renderer: Renderer<D::Resources, D::CommandBuffer>,
    /// Hidden batch context.
    context: batch::Context<D::Resources>,
}

impl<D: device::Device> ops::Deref for Graphics<D> {
    type Target = batch::Context<D::Resources>;

    fn deref(&self) -> &batch::Context<D::Resources> {
        &self.context
    }
}

impl<D: device::Device> ops::DerefMut for Graphics<D> {
    fn deref_mut(&mut self) -> &mut batch::Context<D::Resources> {
        &mut self.context
    }
}


impl<D: device::Device> Graphics<D> {
    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ::ClearData, mask: ::Mask, frame: &::Frame<D::Resources>) {
        self.renderer.clear(data, mask, frame)
    }

    /// Draw a `RefBatch` batch.
    pub fn draw<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                batch: &'a batch::RefBatch<T>, frame: &::Frame<D::Resources>)
                -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&(batch, &self.context), frame)
    }

    /// Draw a `CoreBatch` batch.
    pub fn draw_core<'a, T: ShaderParam<Resources = D::Resources>>(&'a mut self,
                     core: &'a batch::CoreBatch<T>, slice: &'a ::Slice<D::Resources>,
                     params: &'a T, frame: &::Frame<D::Resources>)
                     -> Result<(), ::DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&self.context.bind(core, slice, params), frame)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceExt<R: device::Resources, C: device::draw::CommandBuffer<R>>:
    device::Factory<R> + device::Device<Resources = R, CommandBuffer = C>
{
    /// Create a new renderer
    fn create_renderer(&mut self) -> ::Renderer<R, C>;
    /// Convert to single-threaded wrapper
    fn into_graphics(mut self) -> Graphics<Self>;
}

impl<
    R: device::Resources,
    C: device::draw::CommandBuffer<R>,
    D: device::Factory<R> + device::Device<Resources = R, CommandBuffer = C>,
> DeviceExt<R, C> for D {
    fn create_renderer(&mut self) -> ::Renderer<R, C> {
        ::Renderer {
            command_buffer: device::draw::CommandBuffer::new(),
            data_buffer: device::draw::DataBuffer::new(),
            ref_storage: device::handle::RefStorage::new(),
            common_array_buffer: self.create_array_buffer(),
            draw_frame_buffer: self.create_frame_buffer(),
            read_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: self.get_main_frame_buffer(),
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }

    fn into_graphics(mut self) -> Graphics<D> {
        let rend = self.create_renderer();
        Graphics {
            device: self,
            renderer: rend,
            context: batch::Context::new(),
        }
    }
}