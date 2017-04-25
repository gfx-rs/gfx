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

use ash::version::DeviceV1_0;
use ash::vk;
use std::ptr;
use std::sync::Arc;
use std::marker::PhantomData;
use std::ops::DerefMut;

use core::{self, command, memory, pso, state, target, VertexCount, VertexOffset};
use core::buffer::IndexBufferView;
use core::memory::{ImageStateSrc, ImageStateDst};
use data;
use native::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use {DeviceInner, Resources as R};

pub struct SubmitInfo {
    pub command_buffer: vk::CommandBuffer,
}

pub struct CommandBuffer {
    pub inner: vk::CommandBuffer,
    pub device: Arc<DeviceInner>,
}

impl CommandBuffer {
    fn end(&mut self) -> SubmitInfo {
        unsafe {
            self.device.0.end_command_buffer(self.inner); // TODO: error handling
        }

        SubmitInfo {
            command_buffer: self.inner,
        }
    }

    fn pipeline_barrier<'a>(&mut self, memory_barriers: &[memory::MemoryBarrier],
        buffer_barriers: &[memory::BufferBarrier<'a, R>], image_barriers: &[memory::ImageBarrier<'a, R>])
    {
        let image_barriers = image_barriers.iter().map(|barrier| {
            // TODO
            let base_range = vk::ImageSubresourceRange {
                aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
                base_mip_level: 0, 
                level_count: 1,
                base_array_layer: 0,
                layer_count: vk::VK_REMAINING_ARRAY_LAYERS,
            };

            let (src_access, old_layout) = match barrier.state_src {
                ImageStateSrc::Present(access) => {
                    (data::map_image_access(access), vk::ImageLayout::PresentSrcKhr)
                }
                ImageStateSrc::State(access, layout) => {
                    (data::map_image_access(access), data::map_image_layout(layout))
                }
            };

            let (dst_access, new_layout) = match barrier.state_dst {
                ImageStateDst::Present => {
                    (vk::AccessFlags::empty(), vk::ImageLayout::PresentSrcKhr) // TODO
                }
                ImageStateDst::State(access, layout) => {
                    (data::map_image_access(access), data::map_image_layout(layout))
                }
            };

            vk::ImageMemoryBarrier {
                s_type: vk::StructureType::ImageMemoryBarrier,
                p_next: ptr::null(),
                src_access_mask: src_access,
                dst_access_mask: dst_access,
                old_layout: old_layout,
                new_layout: new_layout,
                src_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                dst_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                image: barrier.image.0,
                subresource_range: base_range,
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_pipeline_barrier(
                self.inner, // commandBuffer
                vk::PIPELINE_STAGE_ALL_GRAPHICS_BIT, // srcStageMask // TODO
                vk::PIPELINE_STAGE_ALL_GRAPHICS_BIT, // dstStageMask // TODO
                vk::DependencyFlags::empty(), // dependencyFlags // TODO
                &[], // pMemoryBarriers // TODO
                &[], // pBufferMemoryBarriers // TODO
                &image_barriers// pImageMemoryBarriers
            );
        }
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }

    fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &native::Buffer, dst: &native::Buffer, regions:&[command::BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(&mut self, src: &native::Image, dst: &native::Image) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self, src: &native::Buffer, dst: &native::Image, layout: memory::ImageLayout, regions: &[command::BufferImageCopy]) {
        let regions = regions.iter().map(|region| {
            let subresource_layers = vk::ImageSubresourceLayers {
                aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT, // TODO
                mip_level: region.image_mip_level as u32,
                base_array_layer: region.image_base_layer as u32,
                layer_count: region.image_layers as u32,
            };

            vk::BufferImageCopy {
                buffer_offset: region.buffer_offset,
                buffer_row_length: 0,
                buffer_image_height: 0,
                image_subresource: subresource_layers,
                image_offset: vk::Offset3D {
                    x: region.image_offset.x,
                    y: region.image_offset.y,
                    z: region.image_offset.z,
                },
                image_extent: vk::Extent3D {
                    width:  region.image_extent.width,
                    height: region.image_extent.height,
                    depth:  region.image_extent.depth,
                },
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_copy_buffer_to_image(
                self.inner, // commandBuffer
                src.inner, // srcBuffer
                dst.0, // dstImage
                data::map_image_layout(layout), // dstImageLayout
                &regions, // pRegions
            );
        }
    }

    fn copy_image_to_buffer(&mut self) {
        unimplemented!()
    }

    fn clear_color(&mut self, rtv: &native::RenderTargetView, value: command::ClearColor) {
        let clear_value = data::map_clear_color(value);
        // TODO: use actual subresource range from rtv
        let base_range = vk::ImageSubresourceRange {
            aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
            base_mip_level: 0, 
            level_count: 1,
            base_array_layer: 0,
            layer_count: vk::VK_REMAINING_ARRAY_LAYERS,
        };

        unsafe {
            self.device.0.cmd_clear_color_image(
                self.inner,
                rtv.image,
                vk::ImageLayout::TransferDstOptimal,
                &clear_value,
                &[base_range],
            )
        };
    }

    fn clear_buffer(&mut self) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pso: &native::GraphicsPipeline) {
        unsafe {
            self.device.0.cmd_bind_pipeline(
                self.inner,
                vk::PipelineBindPoint::Graphics,
                pso.pipeline,
            )
        }
    }

    fn bind_compute_pipeline(&mut self, pso: &native::ComputePipeline) {
        unsafe {
            self.device.0.cmd_bind_pipeline(
                self.inner,
                vk::PipelineBindPoint::Compute,
                pso.pipeline,
            )
        }
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }

    fn clear_attachment(&mut self) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<command::InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.device.0.cmd_draw(
                self.inner,     // commandBuffer
                count,          // vertexCount
                num_instances,  // instanceCount
                start,          // firstVertex
                start_instance, // firstInstance
            )
        }
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instances: Option<command::InstanceParams>) {
         let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.device.0.cmd_draw_indexed(
                self.inner,     // commandBuffer
                count,          // indexCount
                num_instances,  // instanceCount
                start,          // firstIndex
                base,           // vertexOffset
                start_instance, // firstInstance
            )
        }
    }

    fn draw_indirect(&mut self) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.device.0.cmd_dispatch(
                self.inner, // commandBuffer
                x,          // groupCountX
                y,          // groupCountY
                z,          // groupCountZ
            )
        }
    }

    fn dispatch_indirect(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<R>) {
        unsafe {
            self.device.0.cmd_bind_index_buffer(
                self.inner,       // commandBuffer
                ibv.buffer.inner, // buffer
                ibv.offset,       // offset
                data::map_index_type(ibv.index_type), // indexType
            );
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
        let buffers = vbs.0.iter().map(|&(ref buffer, _)| buffer.inner).collect::<Vec<_>>();
        let offsets = vbs.0.iter().map(|&(_, offset)| offset as u64).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_bind_vertex_buffers(
                self.inner,
                0,
                &buffers,
                &offsets,
            );
        }
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        let viewports = viewports.iter().map(|viewport| {
            vk::Viewport {
                x: viewport.x as f32,
                y: viewport.y as f32,
                width: viewport.w as f32,
                height: viewport.h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_set_viewport(self.inner, &viewports);
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let scissors = scissors.iter().map(|scissor| {
            vk::Rect2D {
                offset: vk::Offset2D {
                    x: scissor.x as i32,
                    y: scissor.y as i32,
                },
                extent: vk::Extent2D {
                    width: scissor.w as u32,
                    height: scissor.h as u32,
                }
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_set_scissor(self.inner, &scissors);
        }
    }

    fn set_ref_values(&mut self, _: state::RefValues) {
        unimplemented!()
    }

    fn clear_depth_stencil(&mut self, dsv: &native::DepthStencilView, depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unimplemented!()
    }

    fn resolve_image(&mut self) {
        unimplemented!()
    }

    fn bind_descriptor_heaps(&mut self, srv_cbv_uav: Option<&native::DescriptorHeap>, samplers: Option<&native::DescriptorHeap>) {
        // TODO: unset all active descriptor sets?
    }

    fn bind_descriptor_sets(
        &mut self,
        bind_point: vk::PipelineBindPoint,
        layout: &native::PipelineLayout,
        first_set: usize,
        sets: &[&native::DescriptorSet])
    {
        // TODO: verify sets from currently bound descriptor heap
        let sets = sets.iter().map(|set| {
            set.inner
        }).collect::<Vec<_>>();

        unsafe {
            self.device.0.cmd_bind_descriptor_sets(
                self.inner, // commandBuffer
                bind_point, // pipelineBindPoint
                layout.layout, // layout
                first_set as u32, // firstSet
                &sets, // pDescriptorSets
                &[]// pDynamicOffsets // TODO
            );
        }
    }
}

// CommandBuffer trait implementation
macro_rules! impl_cmd_buffer {
    ($buffer:ident) => (
        impl command::CommandBuffer for $buffer {
            type SubmitInfo = SubmitInfo;
            unsafe fn end(&mut self) -> SubmitInfo {
                self.0.end()
            }
        }
    )
}

impl_cmd_buffer!(GeneralCommandBuffer);
impl_cmd_buffer!(GraphicsCommandBuffer);
impl_cmd_buffer!(ComputeCommandBuffer);
impl_cmd_buffer!(TransferCommandBuffer);
impl_cmd_buffer!(SubpassCommandBuffer);

// PrimaryCommandBuffer trait implementation
macro_rules! impl_primary_cmd_buffer {
    ($buffer:ident) => (
        impl core::PrimaryCommandBuffer<R> for $buffer {
            fn pipeline_barrier<'a>(&mut self, memory_barriers: &[memory::MemoryBarrier],
                buffer_barriers: &[memory::BufferBarrier<'a, R>], image_barriers: &[memory::ImageBarrier<'a, R>])
            {
                self.0.pipeline_barrier(memory_barriers, buffer_barriers, image_barriers)
            }

            fn execute_commands(&mut self) {
                self.0.execute_commands()
            }
        }
    )
}

impl_primary_cmd_buffer!(GeneralCommandBuffer);
impl_primary_cmd_buffer!(GraphicsCommandBuffer);
impl_primary_cmd_buffer!(ComputeCommandBuffer);
impl_primary_cmd_buffer!(TransferCommandBuffer);

// ProcessingCommandBuffer trait implementation
macro_rules! impl_processing_cmd_buffer {
    ($buffer:ident) => (
        impl core::ProcessingCommandBuffer<R> for $buffer {
            fn clear_color(&mut self, rtv: &native::RenderTargetView, value: command::ClearColor) {
                self.0.clear_color(rtv, value)
            }

            fn clear_buffer(&mut self) {
                self.0.clear_buffer()
            }

            fn bind_descriptor_heaps(&mut self, srv_cbv_uav: Option<&native::DescriptorHeap>, samplers: Option<&native::DescriptorHeap>) {
                self.0.bind_descriptor_heaps(srv_cbv_uav, samplers)
            }

            fn push_constants(&mut self) {
                self.0.push_constants()
            }
        }
    )
}

impl_processing_cmd_buffer!(GeneralCommandBuffer);
impl_processing_cmd_buffer!(GraphicsCommandBuffer);
impl_processing_cmd_buffer!(ComputeCommandBuffer);

// TransferCommandBuffer trait implementation
macro_rules! impl_transfer_cmd_buffer {
    ($buffer:ident) => (
        impl core::TransferCommandBuffer<R> for $buffer {
            fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
                self.0.update_buffer(buffer, data, offset)
            }

            fn copy_buffer(&mut self, src: &native::Buffer, dst: &native::Buffer, regions: &[command::BufferCopy]) {
                self.0.copy_buffer(src, dst, regions)
            }

            fn copy_image(&mut self, src: &native::Image, dst: &native::Image) {
                self.0.copy_image(src, dst)
            }

            fn copy_buffer_to_image(&mut self, src: &native::Buffer, dst: &native::Image, layout: memory::ImageLayout, regions: &[command::BufferImageCopy]) {
                self.0.copy_buffer_to_image(src, dst, layout, regions)
            }

            fn copy_image_to_buffer(&mut self) {
                self.0.copy_image_to_buffer()
            } 
        }
    )
}

impl_transfer_cmd_buffer!(GeneralCommandBuffer);
impl_transfer_cmd_buffer!(GraphicsCommandBuffer);
impl_transfer_cmd_buffer!(ComputeCommandBuffer);
impl_transfer_cmd_buffer!(TransferCommandBuffer);

// GraphicsCommandBuffer trait implementation
macro_rules! impl_graphics_cmd_buffer {
    ($buffer:ident) => (
        impl core::GraphicsCommandBuffer<R> for $buffer {
            fn clear_depth_stencil(&mut self, dsv: &native::DepthStencilView, depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
                self.0.clear_depth_stencil(dsv, depth, stencil)
            }

            fn resolve_image(&mut self) {
                self.0.resolve_image()
            }

            fn bind_index_buffer(&mut self, ibv: IndexBufferView<R>) {
                self.0.bind_index_buffer(ibv)
            }

            fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
                self.0.bind_vertex_buffers(vbs)
            }

            fn set_viewports(&mut self, viewports: &[target::Rect]) {
                self.0.set_viewports(viewports)
            }

            fn set_scissors(&mut self, scissors: &[target::Rect]) {
                self.0.set_scissors(scissors)
            }

            fn set_ref_values(&mut self, rv: state::RefValues) {
                self.0.set_ref_values(rv)
            }

            fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
                self.0.bind_graphics_pipeline(pipeline)
            }

            fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
                self.0.bind_descriptor_sets(vk::PipelineBindPoint::Graphics, layout, first_set, sets)
            }
        }
    )
}

impl_graphics_cmd_buffer!(GeneralCommandBuffer);
impl_graphics_cmd_buffer!(GraphicsCommandBuffer);

// ComputeCommandBuffer trait implementation
macro_rules! impl_compute_cmd_buffer {
    ($buffer:ident) => (
        impl core::ComputeCommandBuffer<R> for $buffer {
            fn dispatch(&mut self, x: u32, y: u32, z: u32) {
                self.0.dispatch(x, y, z)
            }

            fn dispatch_indirect(&mut self) {
                self.0.dispatch_indirect()
            }

            fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
                self.0.bind_compute_pipeline(pipeline)
            }
        }
    )
}

impl_compute_cmd_buffer!(GeneralCommandBuffer);
impl_compute_cmd_buffer!(ComputeCommandBuffer);

// TODO: subpass command buffer

// TODO: not only GraphicsCommandBuffer
pub struct RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc: 'cb, C, R>
    where C: core::GraphicsCommandBuffer<R> + 'enc + DerefMut<Target=native::CommandBuffer>,
          R: core::Resources
{
    command_buffer: &'cb mut command::Encoder<'enc, C>,
    render_pass: &'rp native::RenderPass,
    framebuffer: &'fb native::FrameBuffer,
    _marker: PhantomData<*const R>,
}

impl<'cb, 'rp, 'fb, 'enc, C, R> Drop for RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: core::GraphicsCommandBuffer<R> + DerefMut<Target=native::CommandBuffer>,
          R: core::Resources
{
    fn drop(&mut self) {
        unsafe { self.command_buffer.device.0.cmd_end_render_pass(self.command_buffer.inner); }
    }
}

impl<'cb, 'rp, 'fb, 'enc, C> command::RenderPassEncoder<'cb, 'rp, 'fb, 'enc, C, R> for RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: core::GraphicsCommandBuffer<R> + DerefMut<Target=native::CommandBuffer>
{
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>;

    fn begin(command_buffer: &'cb mut command::Encoder<'enc, C>,
             render_pass: &'rp native::RenderPass,
             framebuffer: &'fb native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[command::ClearValue]) -> Self
    {
        let render_area = vk::Rect2D {
            offset: vk::Offset2D {
                x: render_area.x as i32,
                y: render_area.y as i32,
            },
            extent: vk::Extent2D {
                width: render_area.w as u32,
                height: render_area.h as u32,
            },
        };

        let clear_values = clear_values.iter().map(|cv| {
            use core::command::ClearValue;
            match *cv {
                ClearValue::Color(color) => vk::ClearValue::new_color(data::map_clear_color(color)),
                ClearValue::DepthStencil(_) => unimplemented!(),
            }
        }).collect::<Vec<_>>();

        let info = vk::RenderPassBeginInfo {
            s_type: vk::StructureType::RenderPassBeginInfo,
            p_next: ptr::null(),
            render_pass: render_pass.inner,
            framebuffer: framebuffer.inner,
            render_area: render_area,
            clear_value_count: clear_values.len() as u32,
            p_clear_values: clear_values.as_ptr(),

        };

        unsafe {
            command_buffer.device.0.cmd_begin_render_pass(
                command_buffer.inner,
                &info,
                vk::SubpassContents::Inline);
        }

        RenderPassInlineEncoder {
            command_buffer: command_buffer,
            render_pass: render_pass,
            framebuffer: framebuffer,
            _marker: PhantomData,
        }
    }

    fn next_subpass(self) -> Self::SecondaryEncoder {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> Self::InlineEncoder {
        unimplemented!()
    }
}


impl<'cb, 'rp, 'fb, 'enc, C> command::RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R> for RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: core::GraphicsCommandBuffer<R> + DerefMut<Target=native::CommandBuffer>
{
    fn clear_attachment(&mut self) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instance: Option<command::InstanceParams>) {
        self.command_buffer.draw(start, count, instance)
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instance: Option<command::InstanceParams>) {
        self.command_buffer.draw_indexed(start, count, base, instance)
    }

    fn draw_indirect(&mut self) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<R>) {
        self.command_buffer.bind_index_buffer(ibv)
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
        self.command_buffer.bind_vertex_buffers(vbs)
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        self.command_buffer.set_viewports(viewports)
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        self.command_buffer.set_scissors(scissors)
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        self.command_buffer.set_ref_values(rv)
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        self.command_buffer.bind_graphics_pipeline(pipeline)
    }

    fn bind_graphics_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        self.command_buffer.bind_descriptor_sets(vk::PipelineBindPoint::Graphics, layout, first_set, sets)
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }
}

pub struct RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc: 'cb, C, R>
    where C: core::GraphicsCommandBuffer<R> + 'enc,
          R: core::Resources,
{
    command_buffer: &'cb mut command::Encoder<'enc, C>,
    render_pass: &'rp native::RenderPass,
    framebuffer: &'fb native::FrameBuffer,
    _marker: PhantomData<*const R>,
}

impl<'cb, 'rp, 'fb, 'enc, C> command::RenderPassEncoder<'cb, 'rp, 'fb, 'enc, C, R> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: core::GraphicsCommandBuffer<R> + DerefMut<Target=native::CommandBuffer>
{
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>;

    fn begin(command_buffer: &'cb mut command::Encoder<'enc, C>,
             render_pass: &'rp native::RenderPass,
             framebuffer: &'fb native::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[command::ClearValue]
    ) -> Self {
        RenderPassSecondaryEncoder {
            command_buffer: command_buffer,
            render_pass: render_pass,
            framebuffer: framebuffer,
            _marker: PhantomData,
        }
    }

    fn next_subpass(self) -> Self::SecondaryEncoder {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> Self::InlineEncoder {
        unimplemented!()
    }
}

impl<'cb, 'rp, 'fb, 'enc, C> command::RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: core::GraphicsCommandBuffer<R> + DerefMut<Target=native::CommandBuffer>
{

}
