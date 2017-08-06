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

use Backend;
use {memory, pso, state, target, texture};
use queue::capability::{Capability, Compute};
use super::{BufferCopy, BufferImageCopy, CommandBufferShim, RawCommandBuffer, Submit};

/// Command buffer with compute and transfer functionality.
pub struct ComputeCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for ComputeCommandBuffer<'a, B> {
    type Capability = Compute;
}

impl<'a, B: Backend> CommandBufferShim<'a, B> for ComputeCommandBuffer<'a, B> {
    fn raw(&'a mut self) -> &'a mut B::RawCommandBuffer {
        &mut self.0
    }
}

impl<'a, B: Backend> ComputeCommandBuffer<'a, B> {
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, Compute> {
        Submit::new(self.0.finish())
    }

    ///
    fn bind_compute_pipeline(&mut self, pipeline: &B::ComputePipeline) {
        self.0.bind_compute_pipeline(pipeline)
    }

    ///
    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.0.dispatch(x, y, z)
    }

    ///
    fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: u64) {
        self.0.dispatch_indirect(buffer, offset)
    }

    ///
    pub fn update_buffer(&mut self, buffer: &B::Buffer, data: &[u8], offset: usize) {
        self.0.update_buffer(buffer, data, offset)
    }

    ///
    pub fn copy_buffer(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: &[BufferCopy]) {
        self.0.copy_buffer(src, dst, regions)
    }

    ///
    pub fn copy_image(&mut self, src: &B::Image, dst: &B::Image) {
        self.0.copy_image(src, dst)
    }

    ///
    pub fn copy_buffer_to_image(&mut self, src: &B::Buffer, dst: &B::Image, layout: texture::ImageLayout, regions: &[BufferImageCopy]) {
        self.0.copy_buffer_to_image(src, dst, layout, regions)
    }

    ///
    pub fn copy_image_to_buffer(&mut self, src: &B::Image, dst: &B::Buffer, layout: texture::ImageLayout, regions: &[BufferImageCopy]) {
        self.0.copy_image_to_buffer(src, dst, layout, regions)
    }
}
