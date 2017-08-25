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

use {pso, target};
use {Backend, IndexCount, VertexCount, VertexOffset, Viewport};
use buffer::IndexBufferView;
use image::ImageLayout;
use memory::Barrier;
use queue::capability::{Capability, Graphics};
use super::{BufferCopy, BufferImageCopy, ImageCopy,
    ClearColor, ClearValue, CommandBufferShim, RawCommandBuffer,
    InstanceParams, Submit, SubpassContents,
};

/// Command buffer with graphics and transfer functionality.
pub struct GraphicsCommandBuffer<'a, B: Backend>(pub(crate) &'a mut B::RawCommandBuffer)
where
    B::RawCommandBuffer: 'a;

impl<'a, B: Backend> Capability for GraphicsCommandBuffer<'a, B> {
    type Capability = Graphics;
}

impl<'a, B: Backend> CommandBufferShim<'a, B> for GraphicsCommandBuffer<'a, B> {
    fn raw(&'a mut self) -> &'a mut B::RawCommandBuffer {
        &mut self.0
    }
}

impl<'a, B> GraphicsCommandBuffer<'a, B>
where
    B: Backend,
{
    /// Finish recording commands to the command buffers.
    ///
    /// The command buffer will be consumed and can't be modified further.
    /// The command pool must be reset to able to re-record commands.
    pub fn finish(self) -> Submit<B, Graphics> {
        Submit::new(self.0.finish())
    }

    ///
    pub fn begin_renderpass_inline(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
    ) {
        self.0.begin_renderpass(render_pass, frame_buffer, render_area, clear_values, SubpassContents::Inline)
    }
    ///
    pub fn next_subpass_inline(&mut self) {
        self.0.next_subpass(SubpassContents::Inline)
    }
    ///
    pub fn end_renderpass(&mut self) {
        self.0.end_renderpass()
    }

    ///
    pub fn pipeline_barrier(&mut self, barriers: &[Barrier<B>]) {
        self.0.pipeline_barrier(barriers)
    }

    ///
    pub fn clear_color(&mut self, rtv: &B::RenderTargetView, layout: ImageLayout, clear_value: ClearColor) {
        self.0.clear_color(rtv, layout, clear_value)
    }

    ///
    pub fn clear_depth_stencil(
        &mut self,
        dsv: &B::DepthStencilView,
        layout: ImageLayout,
        depth_value: Option<target::Depth>,
        stencil_value: Option<target::Stencil>,
    ) {
        self.0
            .clear_depth_stencil(dsv, layout, depth_value, stencil_value)
    }

    /*
    ///
    pub fn update_buffer(&mut self, buffer: &B::Buffer, data: &[u8], offset: usize) {
        self.0.update_buffer(buffer, data, offset)
    }
    */

    ///
    pub fn copy_buffer(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: &[BufferCopy]) {
        self.0.copy_buffer(src, dst, regions)
    }

    ///
    pub fn copy_image(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[ImageCopy],
    ) {
        self.0.copy_image(src, src_layout, dst, dst_layout, regions)
    }

    ///
    pub fn copy_buffer_to_image(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.0.copy_buffer_to_image(src, dst, layout, regions)
    }

    ///
    pub fn copy_image_to_buffer(
        &mut self,
        src: &B::Image,
        dst: &B::Buffer,
        layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.0.copy_image_to_buffer(src, dst, layout, regions)
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
    pub fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    ) {
        self.0.bind_graphics_descriptor_sets(layout, first_set, sets)
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
    pub fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil) {
        self.0.set_stencil_reference(front, back)
    }

    ///
    pub fn set_blend_constants(&mut self, cv: target::ColorValue) {
        self.0.set_blend_constants(cv)
    }

    ///
    pub fn draw(&mut self,
        start: VertexCount,
        count: VertexCount,
        instance: Option<InstanceParams>,
    ) {
        self.0.draw(start, count, instance)
    }

    ///
    pub fn draw_indexed(
        &mut self,
        start: IndexCount,
        count: IndexCount,
        base: VertexOffset,
        instance: Option<InstanceParams>,
    ) {
        self.0.draw_indexed(start, count, base, instance)
    }
}
