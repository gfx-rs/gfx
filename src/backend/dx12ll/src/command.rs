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

use comptr::ComPtr;
use std::{cmp, mem, ptr};
use winapi::{self, UINT, UINT64, FLOAT};

use core::{self, command, memory, pso, state, target, IndexType, VertexCount, VertexOffset};
use core::buffer::IndexBufferView;
use core::command::{RenderPassInlineEncoder, RenderPassSecondaryEncoder, Encoder};
use core::pass::{Attachment, AttachmentLoadOp};
use data;
use native::{self, CommandBuffer, GeneralCommandBuffer, GraphicsCommandBuffer,
    ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer, RenderPass, FrameBuffer};
use {Resources as R};


fn get_rect(rect: &target::Rect) -> winapi::D3D12_RECT {
    winapi::D3D12_RECT {
        left: rect.x as i32,
        top: rect.y as i32,
        right: (rect.x + rect.w) as i32,
        bottom: (rect.y + rect.h) as i32,
    }
}

pub struct SubmitInfo(pub ComPtr<winapi::ID3D12GraphicsCommandList>);

impl CommandBuffer {
    fn end(&mut self) -> SubmitInfo {
        unsafe { self.inner.Close(); }
        SubmitInfo(self.inner.clone())
    }

    fn pipeline_barrier<'a>(&mut self, memory_barriers: &[memory::MemoryBarrier],
        buffer_barriers: &[memory::BufferBarrier<'a, R>], image_barriers: &[memory::ImageBarrier<'a, R>])
    {
        let mut transition_barriers = Vec::new();

        for barrier in image_barriers {
            let state_src = match barrier.state_src {
                memory::ImageStateSrc::Present(_access) => winapi::D3D12_RESOURCE_STATE_PRESENT,
                memory::ImageStateSrc::State(access, layout) => data::map_image_resource_state(access, layout)
            };
            let state_dst = match barrier.state_dst {
                memory::ImageStateDst::Present => winapi::D3D12_RESOURCE_STATE_PRESENT,
                memory::ImageStateDst::State(access, layout) => data::map_image_resource_state(access, layout)
            };

            if state_src == state_dst {
                warn!("Image pipeline barrier requested with no effect: {:?}", barrier);
            }

            transition_barriers.push(
                winapi::D3D12_RESOURCE_BARRIER {
                    Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                    Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                    u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: barrier.image.resource.as_mut_ptr(),
                        Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: state_src,
                        StateAfter: state_dst,
                    },
                }
            );
        }

        unsafe {
            self.inner.ResourceBarrier(
                transition_barriers.len() as UINT,
                transition_barriers.as_ptr(),
            );
        }
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }

    fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &native::Buffer, dst: &native::Buffer, regions: &[command::BufferCopy]) {
        if true { //TODO
            // copy each region
            for region in regions {
                unsafe {
                    self.inner.CopyBufferRegion(
                        dst.resource.as_mut_ptr(), // pDstResource
                        region.dst as UINT64,      // DstOffset
                        src.resource.as_mut_ptr(), // pSrcResource
                        region.src as UINT64,      // SrcOffset
                        region.size as UINT64,     // NumBytes
                    );
                }
            }
        } else {
            // copy the whole resource
            unsafe {
                self.inner.CopyResource(
                    dst.resource.as_mut_ptr(), // pDstResource
                    src.resource.as_mut_ptr(), // pSrcResource
                );
            }
        }
    }

    fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self, buffer: &native::Buffer, image: &native::Image,
                            _layout: memory::ImageLayout, regions: &[command::BufferImageCopy]) {
        //TODO: handle layout?
        let mut src = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: buffer.resource.as_mut_ptr(),
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            u: unsafe { mem::zeroed() },
        };
        let mut dest = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: image.resource.as_mut_ptr(),
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };
        let (width, height, depth, _) = image.kind.get_dimensions();
        for region in regions {
            assert_eq!(region.buffer_offset % winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64, 0);
            assert_eq!(region.buffer_row_pitch % winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u32, 0);
            assert!(region.buffer_row_pitch >= width as u32 * image.bits_per_texel as u32 / 8);
            *unsafe { src.PlacedFootprint_mut() } = winapi::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                Offset: region.buffer_offset as UINT64,
                Footprint: winapi::D3D12_SUBRESOURCE_FOOTPRINT {
                    Format: image.dxgi_format,
                    Width: width as UINT,
                    Height: cmp::max(1, height as UINT),
                    Depth: cmp::max(1, depth as UINT),
                    RowPitch: region.buffer_row_pitch,
                },
            };
            // TODO: `D3D12CalcSubresource`
            *unsafe { dest.SubresourceIndex_mut() } = region.image_mip_level as UINT +
                region.image_base_layer as UINT * image.kind.get_num_slices().unwrap_or(1) as UINT;
            // fast path
            unsafe {
                self.inner.CopyTextureRegion(
                    &dest,
                    region.image_offset.x as UINT,
                    region.image_offset.y as UINT,
                    region.image_offset.z as UINT,
                    &src, ptr::null(),
                );
            }
        }
    }

    fn copy_image_to_buffer(&mut self) {
        unimplemented!()
    }

    fn clear_color(&mut self, rtv: &native::RenderTargetView, value: command::ClearColor) {
        let clear_color = match value {
            command::ClearColor::Float(c) => c,
            command::ClearColor::Int(c) => [c[0] as FLOAT, c[1] as FLOAT, c[2] as FLOAT, c[3] as FLOAT], // TODO: error?
            command::ClearColor::Uint(c) => [c[0] as FLOAT, c[1] as FLOAT, c[2] as FLOAT, c[3] as FLOAT], // TODO: error?
        };

        unsafe {
            self.inner.ClearRenderTargetView(
                rtv.handle,      // RenderTargetView
                &clear_color,    // ColorRGBA
                0,               // NumRects
                ptr::null_mut(), // pRects
            );
        }
    }

    fn clear_buffer(&mut self) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pso: &native::GraphicsPipeline) {
        unsafe {
            self.inner.SetPipelineState(pso.inner.as_mut_ptr());
            self.inner.IASetPrimitiveTopology(pso.topology);
        };
    }

    fn bind_compute_pipeline(&mut self, pso: &native::ComputePipeline) {
        unimplemented!()
    }

    fn bind_descriptor_heaps(&mut self, srv_cbv_uav: Option<&native::DescriptorHeap>, samplers: Option<&native::DescriptorHeap>) {
        let mut heaps = [ptr::null_mut(); 2];
        let mut count = 0;
        if let Some(heap) = srv_cbv_uav {
            heaps[count] = heap.inner.as_mut_ptr();
            count += 1;
        }
        if let Some(heap) = samplers {
            heaps[count] = heap.inner.as_mut_ptr();
            count += 1;
        }
        unsafe {
            self.inner.SetDescriptorHeaps(count as UINT, heaps.as_mut_ptr())
        }
    }

    fn bind_descriptor_sets(&mut self, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
        //TODO: Compute?
        unsafe {
            self.inner.SetGraphicsRootSignature(layout.inner.as_mut_ptr());
        }
        for i in 0 .. sets.len() {
            let handle = sets[i].ranges[0].handle.gpu;
            unsafe {
                self.inner.SetGraphicsRootDescriptorTable((first_set + i) as UINT, handle)
            }
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
            self.inner.DrawInstanced(
                count,          // VertexCountPerInstance
                num_instances,  // InstanceCount
                start,          // StartVertexLocation
                start_instance, // StartInstanceLocation
            );
        }
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instances: Option<command::InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.inner.DrawIndexedInstanced(
                count,          // IndexCountPerInstance
                num_instances,  // InstanceCount
                start,          // StartIndexLocation
                base,           // BaseVertexLocation
                start_instance, // StartInstanceLocation
            );
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
            self.inner.Dispatch(
                x, // ThreadGroupCountX
                y, // ThreadGroupCountY
                z, // ThreadGroupCountZ
            );
        }
    }

    fn dispatch_indirect(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<R>) {
        let format = match ibv.index_type {
            IndexType::U16 => winapi::DXGI_FORMAT_R16_UINT,
            IndexType::U32 => winapi::DXGI_FORMAT_R32_UINT,
        };
        let location = unsafe {
            (*ibv.buffer.resource.as_mut_ptr()).GetGPUVirtualAddress()
        };

        let mut ibv_raw = winapi::D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: location,
            SizeInBytes: ibv.buffer.size_in_bytes,
            Format: format,
        };

        unsafe {
            self.inner.IASetIndexBuffer(&mut ibv_raw);
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
        let buffers = vbs.0.iter().map(|&(ref buffer, offset)| {
            let base = unsafe {
                buffer.resource.clone().GetGPUVirtualAddress()
            };
            winapi::D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: base + offset as u64,
                SizeInBytes: buffer.size_in_bytes,
                StrideInBytes: buffer.stride,
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.inner.IASetVertexBuffers(
                0,
                vbs.0.len() as UINT,
                buffers.as_ptr(),
            );
        }
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        let viewports = viewports.iter().map(|viewport| {
            winapi::D3D12_VIEWPORT {
                TopLeftX: viewport.x as FLOAT,
                TopLeftY: viewport.y as FLOAT,
                Width: viewport.w as FLOAT,
                Height: viewport.h as FLOAT,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.inner.RSSetViewports(
                viewports.len() as UINT, // NumViewports
                viewports.as_ptr(),      // pViewports
            );
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let rects = scissors.iter().map(get_rect).collect::<Vec<_>>();
        unsafe {
            self.inner.RSSetScissorRects(rects.len() as UINT, rects.as_ptr())
        };
    }

    fn set_ref_values(&mut self, _: state::RefValues) {
        unimplemented!()
    }

    fn clear_depth_stencil(&mut self, _: &native::DepthStencilView, depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unimplemented!()
    }

    fn resolve_image(&mut self) {
        unimplemented!()
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

            fn copy_buffer(&mut self, src: &native::Buffer, dest: &native::Buffer, regions: &[command::BufferCopy]) {
                self.0.copy_buffer(src, dest, regions)
            }

            fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
                self.0.copy_image(src, dest)
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
            type InlineBuffer = RenderPassInlineBuffer;
            type SecondaryBuffer = RenderPassSecondaryBuffer;

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
                self.0.bind_descriptor_sets(layout, first_set, sets)
            }
        }
    )
}

impl_graphics_cmd_buffer!(GeneralCommandBuffer);
impl_graphics_cmd_buffer!(GraphicsCommandBuffer);

// ComputeCommandBuffer trait implementation
macro_rules! impl_graphics_cmd_buffer {
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

impl_graphics_cmd_buffer!(GeneralCommandBuffer);
impl_graphics_cmd_buffer!(ComputeCommandBuffer);

// TODO: subpass command buffer

pub struct RenderPassInlineBuffer {
}

macro_rules! impl_render_pass_inline_buffer {
    ($graphics_buffer:ident) => {
        impl command::RenderPassInlineBuffer<$graphics_buffer, R> for RenderPassInlineBuffer {
            fn begin(command_buffer: &mut Encoder<$graphics_buffer>,
                     render_pass: &RenderPass,
                     framebuffer: &FrameBuffer,
                     render_area: target::Rect,
                     clear_values: &[command::ClearValue]
            ) -> Self {
				let color_views = framebuffer.color.iter().map(|view| view.handle).collect::<Vec<_>>();
       			assert!(framebuffer.depth_stencil.len() <= 1);
		        assert_eq!(framebuffer.color.len() + framebuffer.depth_stencil.len(), render_pass.attachments.len());

 		        let ds_view = match framebuffer.depth_stencil.first() {
		            Some(ref view) => &view.handle as *const _,
		            None => ptr::null(),
		        };
		        unsafe {
		            command_buffer.0.inner.OMSetRenderTargets(
		                color_views.len() as UINT,
		                color_views.as_ptr(),
		                winapi::FALSE,
		                ds_view,
		            );
		        }

		        let area = get_rect(&render_area);

		        let mut clear_iter = clear_values.iter();
		        for (color, attachment) in framebuffer.color.iter().zip(render_pass.attachments.iter()) {
        		    if attachment.load_op == AttachmentLoadOp::Clear {
		                match clear_iter.next() {
        		            Some(&command::ClearValue::Color(value)) => {
		                        let data = match value {
        		                    command::ClearColor::Float(v) => v,
		                            _ => {
        		                        error!("Integer clear is not implemented yet");
		                                [0.0; 4]
        		                    }
		                        };
        		                unsafe {
                		            command_buffer.0.inner.ClearRenderTargetView(color.handle, &data, 1, &area);
                        		}
		                    },
        		            other => error!("Invalid clear value for view {:?}: {:?}", color, other),
                		}
		            }
        		}
		        if let (Some(depth), Some(&Attachment{ load_op: AttachmentLoadOp::Clear, .. })) = (framebuffer.depth_stencil.first(), render_pass.attachments.last()) {
        		    match clear_iter.next() {
                		Some(&command::ClearValue::DepthStencil(value)) => {
		                    unsafe {
        		                command_buffer.0.inner.ClearDepthStencilView(depth.handle,
                		            winapi::D3D12_CLEAR_FLAG_DEPTH | winapi::D3D12_CLEAR_FLAG_STENCIL,
                        		    value.depth, value.stencil as u8, 1, &area);
		                    }
        		        },
                		other => error!("Invalid clear value for view {:?}: {:?}",
		                    framebuffer.depth_stencil[0], other),
        		    }
		        }

                RenderPassInlineBuffer {
                }
            }

            fn finish(&mut self,
                      command_buffer: &mut $graphics_buffer,
                      render_pass: &native::RenderPass,
                      framebuffer: &native::FrameBuffer) {
            }

            fn next_subpass(&mut self) -> RenderPassSecondaryBuffer {
                unimplemented!()
            }

            fn next_subpass_inline(&mut self) -> Self {
                unimplemented!()
            }

            fn clear_attachment(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>) {
                unimplemented!()
            }

            fn draw(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, start: VertexCount, count: VertexCount, instance: Option<command::InstanceParams>) {
                encoder.command_buffer.0.draw(start, count, instance)
            }

            fn draw_indexed(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, start: VertexCount, count: VertexCount, base: VertexOffset, instance: Option<command::InstanceParams>) {
                unimplemented!()
            }

            fn draw_indirect(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>) {
                unimplemented!()
            }

            fn draw_indexed_indirect(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>) {
                unimplemented!()
            }

            fn bind_index_buffer(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, ibv: IndexBufferView<R>) {
                unimplemented!()
            }

            fn bind_vertex_buffers(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, vbs: pso::VertexBufferSet<R>) {
                unimplemented!()
            }

            fn set_viewports(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, viewports: &[target::Rect]) {
                encoder.command_buffer.0.set_viewports(viewports)
            }

            fn set_scissors(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, scissors: &[target::Rect]) {
                encoder.command_buffer.0.set_scissors(scissors)
            }

            fn set_ref_values(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, rv: state::RefValues) {
                encoder.command_buffer.0.set_ref_values(rv)
            }

            fn bind_graphics_pipeline(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, pipeline: &native::GraphicsPipeline) {
                encoder.command_buffer.0.bind_graphics_pipeline(pipeline)
            }

            fn bind_graphics_descriptor_sets(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>, layout: &native::PipelineLayout, first_set: usize, sets: &[&native::DescriptorSet]) {
                //self.0.bind_descriptor_sets(vk::PipelineBindPoint::Graphics, layout, first_set, sets)
                unimplemented!()
            }

            fn push_constants(encoder: &mut RenderPassInlineEncoder<$graphics_buffer, R>) {

            }
        }
    }
}

impl_render_pass_inline_buffer!(GeneralCommandBuffer);
impl_render_pass_inline_buffer!(GraphicsCommandBuffer);

pub struct RenderPassSecondaryBuffer {
}

macro_rules! impl_render_pass_secondary_buffer {
    ($graphics_buffer:ident) => {
        impl command::RenderPassSecondaryBuffer<$graphics_buffer, R> for RenderPassSecondaryBuffer {
            fn begin(command_buffer: &mut Encoder<$graphics_buffer>,
                     render_pass: &RenderPass,
                     framebuffer: &FrameBuffer,
                     render_area: target::Rect,
                     clear_values: &[command::ClearValue]
            ) -> Self {
                RenderPassSecondaryBuffer {
                }
            }

            fn finish(&mut self,
                      command_buffer: &mut $graphics_buffer,
                      render_pass: &native::RenderPass,
                      framebuffer: &native::FrameBuffer) {
            }

            fn next_subpass(&mut self) -> Self {
                unimplemented!()
            }

            fn next_subpass_inline(&mut self) -> RenderPassInlineBuffer {
                unimplemented!()
            }
        }
    }
}

impl_render_pass_secondary_buffer!(GeneralCommandBuffer);
impl_render_pass_secondary_buffer!(GraphicsCommandBuffer);
