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

use {Backend, Resources};
use {memory, target, texture};
use queue::capability::{Capability, Graphics};
use super::{BufferCopy, BufferImageCopy, Submit};

/// Command buffer with graphics and transfer functionality.
pub struct GraphicsCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for GraphicsCommandBuffer<'a, B> {
    type Capability = Graphics;
}

impl<'a, B, R> GraphicsCommandBuffer<'a, B>
where
    B: Backend,
    R: Resources,
{
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, Graphics> {
        Submit::new(self.0.finish())
    }

    ///
    fn pipeline_barrier(
        &mut self,
        memory_barriers: &[memory::MemoryBarrier],
        buffer_barriers: &[memory::BufferBarrier],
        image_barriers: &[memory::ImageBarrier])
    {
        self.0.pipeline_barrier(memory_barriers, buffer_barriers, image_barriers)
    }

    ///
    fn clear_depth_stencil(
        &mut self,
        dsv: &R::DepthStencilView,
        depth_value: Option<target::Depth>,
        stencil_value: Option<target::Stencil>)
    {
        self.0.clear_depth_stencil(dsv, depth_value, stencil_value)
    }

    ///
    fn update_buffer(&mut self, buffer: &R::Buffer, data: &[u8], offset: usize) {
        self.0.update_buffer(buffer, data, offset)
    }

    fn copy_buffer(&mut self, src: &R::Buffer, dst: &R::Buffer, regions: &[BufferCopy]) {
        self.0.copy_buffer(src, dst, regions)
    }

    fn copy_image(&mut self, src: &R::Image, dst: &R::Image) {
        self.0.copy_image(src, dst)
    }

    fn copy_buffer_to_image(&mut self, src: &R::Buffer, dst: &R::Image, layout: texture::ImageLayout, regions: &[BufferImageCopy]) {
        self.0.copy_buffer_to_image(src, dst, layout, regions)
    }

    fn copy_image_to_buffer(&mut self) {
        self.0.copy_image_to_buffer()
    }
}
