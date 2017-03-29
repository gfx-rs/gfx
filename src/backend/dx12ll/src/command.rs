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
use std::ptr;
use winapi;
use winapi::*;

use core::{self, command, memory, pso, state, target, IndexType, VertexCount, VertexOffset};
use data;
use native::{self, CommandBuffer, GeneralCommandBuffer, GraphicsCommandBuffer,
    ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer, RenderPass, FrameBuffer};
use {Resources as R};

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

        // TODO: very experimental state!
        for barrier in image_barriers {
            let state_src = data::map_resource_state(barrier.state_src);
            let state_dst = data::map_resource_state(barrier.state_dst);

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

    fn copy_buffer(&mut self, src: &native::Buffer, dst: &native::Buffer, regions: Option<&[command::BufferCopy]>) {
        match regions {
            Some(regions) => {
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
            }
            None => {
                // copy the whole resource
                unsafe {
                    self.inner.CopyResource(
                        dst.resource.as_mut_ptr(), // pDstResource
                        src.resource.as_mut_ptr(), // pSrcResource
                    );
                }
            }
        }
    }

    fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self) {
        unimplemented!()
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
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pso: &native::ComputePipeline) {
        unimplemented!()
    }

    fn bind_descriptor_sets(&mut self) {
        unimplemented!()
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

    fn bind_index_buffer(&mut self, ib: &native::Buffer, index_type: IndexType) {
        let format = match index_type {
            IndexType::U16 => winapi::DXGI_FORMAT_R16_UINT,
            IndexType::U32 => winapi::DXGI_FORMAT_R32_UINT,
        };
        let location = unsafe { (*ib.resource.as_mut_ptr()).GetGPUVirtualAddress() };

        let mut ibv = winapi::D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: location,
            SizeInBytes: ib.size,
            Format: format,
        };

        unsafe {
            self.inner.IASetIndexBuffer(&mut ibv);
        }
    }

    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<R>) {
        unimplemented!()
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
        unimplemented!()
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

            fn bind_descriptor_sets(&mut self) {
                self.0.bind_descriptor_sets()
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

            fn copy_buffer(&mut self, src: &native::Buffer, dest: &native::Buffer, regions: Option<&[command::BufferCopy]>) {
                self.0.copy_buffer(src, dest, regions)
            }

            fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
                self.0.copy_image(src, dest)
            }

            fn copy_buffer_to_image(&mut self) {
                self.0.copy_buffer_to_image()
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

            fn bind_index_buffer(&mut self, buffer: &native::Buffer, index_type: IndexType) {
                self.0.bind_index_buffer(buffer, index_type)
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

pub struct RenderPassInlineEncoder<'cb, 'rp, 'fb> {
    command_list: &'cb mut GraphicsCommandBuffer,
    render_pass: &'rp RenderPass,
    framebuffer: &'fb FrameBuffer,
}

impl<'cb, 'rp, 'fb> command::RenderPassEncoder<'cb, 'rp, 'fb, GraphicsCommandBuffer, R> for RenderPassInlineEncoder<'cb, 'rp, 'fb> {
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb>;

    fn begin(command_buffer: &'cb mut GraphicsCommandBuffer,
             render_pass: &'rp RenderPass,
             framebuffer: &'fb FrameBuffer,
             render_area: target::Rect,
             clear_values: &[command::ClearValue]
    ) -> Self {
        RenderPassInlineEncoder {
            command_list: command_buffer,
            render_pass: render_pass,
            framebuffer: framebuffer,
        }
    }

    fn next_subpass(self) -> RenderPassSecondaryEncoder<'cb, 'rp, 'fb> {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> RenderPassInlineEncoder<'cb, 'rp, 'fb>{
        unimplemented!()
    }
}

impl<'cb, 'rp, 'fb> command::RenderPassInlineEncoder<'cb, 'rp, 'fb, GraphicsCommandBuffer, R> for RenderPassInlineEncoder<'cb, 'rp, 'fb> {
    fn clear_attachment(&mut self) {

    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instance: Option<command::InstanceParams>) {
        self.command_list.0.draw(start, count, instance)
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instance: Option<command::InstanceParams>) {

    }

    fn draw_indirect(&mut self) {

    }

    fn draw_indexed_indirect(&mut self) {

    }

    fn bind_index_buffer(&mut self, ib: &native::Buffer, index_type: IndexType) {

    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {

    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        self.command_list.0.set_viewports(viewports)
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        self.command_list.0.set_scissors(scissors)
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        self.command_list.0.set_ref_values(rv)
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        self.command_list.0.bind_graphics_pipeline(pipeline)
    }

    fn bind_descriptor_sets(&mut self) {

    }

    fn push_constants(&mut self) {

    }
}

pub struct RenderPassSecondaryEncoder<'cb, 'rp, 'fb> {
    command_list: &'cb mut GraphicsCommandBuffer,
    render_pass: &'rp RenderPass,
    framebuffer: &'fb FrameBuffer,
}

impl<'cb, 'rp, 'fb> command::RenderPassEncoder<'cb, 'rp, 'fb, GraphicsCommandBuffer, R> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb> {
    type SecondaryEncoder = RenderPassSecondaryEncoder<'cb, 'rp, 'fb>;
    type InlineEncoder = RenderPassInlineEncoder<'cb, 'rp, 'fb>;

    fn begin(command_buffer: &'cb mut GraphicsCommandBuffer,
             render_pass: &'rp RenderPass,
             framebuffer: &'fb FrameBuffer,
             render_area: target::Rect,
             clear_values: &[command::ClearValue]
    ) -> Self {
        RenderPassSecondaryEncoder {
            command_list: command_buffer,
            render_pass: render_pass,
            framebuffer: framebuffer,
        }
    }

    fn next_subpass(self) -> RenderPassSecondaryEncoder<'cb, 'rp, 'fb> {
        unimplemented!()
    }

    fn next_subpass_inline(self) -> RenderPassInlineEncoder<'cb, 'rp, 'fb>{
        unimplemented!()
    }
}

impl<'cb, 'rp, 'fb> command::RenderPassSecondaryEncoder<'cb, 'rp, 'fb, GraphicsCommandBuffer, R> for RenderPassSecondaryEncoder<'cb, 'rp, 'fb> { }
