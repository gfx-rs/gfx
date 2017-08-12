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

use {Backend, Viewport};
use {memory, pso, state, target, texture};
use buffer::IndexBufferView;
use queue::capability::{Capability, General};
use super::{BufferCopy, BufferImageCopy, CommandBufferShim, ImageCopy, RawCommandBuffer, Submit};

/// Command buffer with compute, graphics and transfer functionality.
pub struct GeneralCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where
    B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for GeneralCommandBuffer<'a, B> {
    type Capability = General;
}

impl<'a, B: Backend> CommandBufferShim<'a, B> for GeneralCommandBuffer<'a, B> {
    fn raw(&'a mut self) -> &'a mut B::RawCommandBuffer {
        &mut self.0
    }
}

impl<'a, B: Backend> GeneralCommandBuffer<'a, B> {
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(mut self) -> Submit<B, General> {
        Submit::new(self.0.finish())
    }

    ///
    pub fn pipeline_barrier(&mut self, barriers: &[memory::Barrier]) {
        self.0.pipeline_barrier(barriers)
    }

    ///
    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.0.dispatch(x, y, z)
    }

    ///
    fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: u64) {
        self.0.dispatch_indirect(buffer, offset)
    }

    /// Bind index buffer view.
    pub fn bind_index_buffer(&mut self, ibv: IndexBufferView<B>) {
        self.0.bind_index_buffer(ibv)
    }

    /// Bind vertex buffers.
    pub fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<B>) {
        self.0.bind_vertex_buffers(vbs)
    }

    /// Bind a graphics pipeline.
    ///
    /// There is only *one* pipeline slot for compute and graphics.
    /// Calling the corresponding `bind_pipeline` functions will override the slot.
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.0.bind_graphics_pipeline(pipeline)
    }

    ///
    pub fn set_viewports(&mut self, viewports: &[Viewport]) {
        self.0.set_viewports(viewports)
    }

    ///
    pub fn set_scissors(&mut self, scissors: &[target::Rect]) {
        self.0.set_scissors(scissors)
    }

    ///
    pub fn set_ref_values(&mut self, rv: state::RefValues) {
        self.0.set_ref_values(rv)
    }

    ///
    pub fn copy_buffer(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: &[BufferCopy]) {
        self.0.copy_buffer(src, dst, regions)
    }

    ///
    pub fn copy_image(
        &mut self,
        src: &B::Image,
        src_layout: texture::ImageLayout,
        dst: &B::Image,
        dst_layout: texture::ImageLayout,
        regions: &[ImageCopy],
    ) {
        self.0.copy_image(src, src_layout, dst, dst_layout, regions)
    }

    ///
    pub fn copy_buffer_to_image(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.0.copy_buffer_to_image(src, dst, layout, regions)
    }

    ///
    pub fn copy_image_to_buffer(
        &mut self,
        src: &B::Image,
        dst: &B::Buffer,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.0.copy_image_to_buffer(src, dst, layout, regions)
    }
}
