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
use {memory, texture};
use queue::capability::{Capability, Transfer};
use super::{BufferCopy, BufferImageCopy, CommandBufferShim, RawCommandBuffer, Submit};

/// Command buffer with transfer functionality.
pub struct TransferCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for TransferCommandBuffer<'a, B> {
    type Capability = Transfer;
}

impl<'a, B: Backend> CommandBufferShim<'a, B> for TransferCommandBuffer<'a, B> {
    fn raw(&'a mut self) -> &'a mut B::RawCommandBuffer {
        &mut self.0
    }
}

impl<'a, B: Backend> TransferCommandBuffer<'a, B> {
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, Transfer> {
        Submit::new(self.0.finish())
    }

    ///
    pub fn pipeline_barrier(
        &mut self,
        memory_barriers: &[memory::MemoryBarrier],
        buffer_barriers: &[memory::BufferBarrier],
        image_barriers: &[memory::ImageBarrier])
    {
        self.0.pipeline_barrier(memory_barriers, buffer_barriers, image_barriers)
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
