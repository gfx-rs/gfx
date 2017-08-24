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
use {Backend, VertexCount, VertexOffset, Viewport};
use buffer::IndexBufferView;
use image::ImageLayout;
use memory::Barrier;
use super::{BufferCopy, BufferImageCopy, ClearColor, ClearValue, ImageCopy, ImageResolve,
            InstanceParams, SubpassContents};

///
pub trait RawCommandBuffer<B: Backend> {
    ///
    fn finish(&mut self) -> B::SubmitInfo;

    ///
    fn pipeline_barrier(&mut self, &[Barrier<B>]);

    ///
    fn clear_color(&mut self, &B::RenderTargetView, ImageLayout, ClearColor);

    /// Clear depth-stencil target
    fn clear_depth_stencil(
        &mut self,
        &B::DepthStencilView,
        ImageLayout,
        Option<target::Depth>,
        Option<target::Stencil>,
    );

    ///
    fn resolve_image(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[ImageResolve],
    );

    /// Bind index buffer view.
    fn bind_index_buffer(&mut self, IndexBufferView<B>);

    /// Bind vertex buffers.
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<B>);

    /// Set the viewport parameters for the rasterizer.
    ///
    /// Every other viewport, which is not specified in this call,
    /// will be disabled.
    ///
    /// Ensure that the number of set viewports at draw time is equal
    /// (or higher) to the number specified in the bound pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Number of viewports must be between 1 and `max_viewports`.
    /// - Only queues with graphics capability support this function.
    fn set_viewports(&mut self, &[Viewport]);

    /// Set the scissor rectangles for the rasterizer.
    ///
    /// Every other scissor, which is not specified in this call,
    /// will be disabled.
    ///
    /// Each scissor corresponds to the viewport with the same index.
    ///
    /// Ensure that the number of set scissors at draw time is equal (or higher)
    /// to the number of viewports specified in the bound pipeline.
    ///
    /// # Errors
    ///
    /// This function does not return an error. Invalid usage of this function
    /// will result in an error on `finish`.
    ///
    /// - Number of scissors must be between 1 and `max_viewports`.
    /// - Only queues with graphics capability support this function.
    fn set_scissors(&mut self, &[target::Rect]);

    ///
    fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil);

    ///
    fn set_blend_constants(&mut self, target::ColorValue);

    ///
    fn begin_renderpass(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    );
    ///
    fn next_subpass(&mut self, contents: SubpassContents);
    ///
    fn end_renderpass(&mut self);

    /// Bind a graphics pipeline.
    ///
    /// There is only *one* pipeline slot for compute and graphics.
    /// Calling the corresponding `bind_pipeline` functions will override the slot.
    fn bind_graphics_pipeline(&mut self, &B::GraphicsPipeline);
    ///
    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    );
    ///
    fn bind_compute_pipeline(&mut self, &B::ComputePipeline);
    ///
    fn dispatch(&mut self, u32, u32, u32);
    ///
    fn dispatch_indirect(&mut self, buffer: &B::Buffer, offset: u64);
    ///
    fn copy_buffer(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: &[BufferCopy]);
    ///
    fn copy_image(
        &mut self,
        src: &B::Image,
        src_layout: ImageLayout,
        dst: &B::Image,
        dst_layout: ImageLayout,
        regions: &[ImageCopy],
    );
    ///
    fn copy_buffer_to_image(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        layout: ImageLayout,
        regions: &[BufferImageCopy],
    );
    ///
    fn copy_image_to_buffer(
        &mut self,
        src: &B::Image,
        dst: &B::Buffer,
        layout: ImageLayout,
        regions: &[BufferImageCopy],
    );
    ///
    fn draw(&mut self, start: VertexCount, count: VertexCount, instance: Option<InstanceParams>);
    ///
    fn draw_indexed(
        &mut self,
        start: VertexCount,
        count: VertexCount,
        base: VertexOffset,
        instance: Option<InstanceParams>,
    );
    ///
    fn draw_indirect(&mut self, buffer: &B::Buffer, offset: u64, draw_count: u32, stride: u32);
    ///
    fn draw_indexed_indirect(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    );
}
