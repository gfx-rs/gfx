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

use wio::com::ComPtr;
use core::{command, memory, pso, shade, state, target, texture};
use core::{IndexType, VertexCount, VertexOffset};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearColor, ClearValue, InstanceParams, SubpassContents};
use winapi::{self, FLOAT, UINT};
use {native as n, Backend};
use smallvec::SmallVec;

#[derive(Clone)]
pub struct SubmitInfo(pub(crate) ComPtr<winapi::ID3D12GraphicsCommandList>);
unsafe impl Send for SubmitInfo { }

fn get_rect(rect: &target::Rect) -> winapi::D3D12_RECT {
    winapi::D3D12_RECT {
        left: rect.x as i32,
        top: rect.y as i32,
        right: (rect.x + rect.w) as i32,
        bottom: (rect.y + rect.h) as i32,
    }
}

pub struct RenderPassCache {
    render_pass: n::RenderPass,
    frame_buffer: n::FrameBuffer,
    next_subpass: usize,
    render_area: winapi::D3D12_RECT,
    clear_values: Vec<ClearValue>,
}

pub struct CommandBuffer {
    pub(crate) raw: ComPtr<winapi::ID3D12GraphicsCommandList>,

    // Cache renderpasses for graphics operations
    // TODO: Use pointers to the actual resources to minimize memory overhead.
    pub(crate) pass_cache: Option<RenderPassCache>,
}

impl CommandBuffer {
    fn begin_subpass(&mut self) {
        let mut pass = self.pass_cache.as_mut().unwrap();
        assert!(pass.next_subpass < pass.render_pass.subpasses.len());

        // TODO
        pass.next_subpass += 1;
    }
}

impl command::RawCommandBuffer<Backend> for CommandBuffer {
    fn finish(&mut self) -> SubmitInfo {
        unsafe { self.raw.Close(); }
        SubmitInfo(self.raw.clone())
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        frame_buffer: &n::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    ) {
        let area = get_rect(&render_area);
        self.pass_cache = Some(
            RenderPassCache {
                render_pass: render_pass.clone(),
                frame_buffer: frame_buffer.clone(),
                render_area: area,
                clear_values: clear_values.into(),
                next_subpass: 0,
            });

        self.begin_subpass();
    }

    fn next_subpass(&mut self, _: SubpassContents) {
        self.begin_subpass();
    }

    fn end_renderpass(&mut self) {
        unimplemented!()
    }

    fn pipeline_barrier(
        &mut self,
        _memory_barries: &[memory::MemoryBarrier],
        _buffer_barriers: &[memory::BufferBarrier],
        _image_barriers: &[memory::ImageBarrier],
    ) {
        unimplemented!()
    }

    fn clear_color(&mut self, rtv: &(), layout: texture::ImageLayout, color: ClearColor) {
        unimplemented!()
    }

    fn clear_depth_stencil(&mut self, dsv: &(), depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unimplemented!()
    }

    fn resolve_image(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        let format = match ibv.index_type {
            IndexType::U16 => winapi::DXGI_FORMAT_R16_UINT,
            IndexType::U32 => winapi::DXGI_FORMAT_R32_UINT,
        };
        let location = unsafe {
            (*ibv.buffer.resource).GetGPUVirtualAddress()
        };

        let mut ibv_raw = winapi::D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: location,
            SizeInBytes: ibv.buffer.size_in_bytes,
            Format: format,
        };

        unsafe {
            self.raw.IASetIndexBuffer(&mut ibv_raw);
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Backend>) {
        let buffers: SmallVec<[winapi::D3D12_VERTEX_BUFFER_VIEW; 16]> =
            vbs.0.iter()
                 .map(|&(ref buffer, offset)| {
                    let base = unsafe {
                        (*buffer.resource).GetGPUVirtualAddress()
                    };
                    winapi::D3D12_VERTEX_BUFFER_VIEW {
                        BufferLocation: base + offset as u64,
                        SizeInBytes: buffer.size_in_bytes,
                        StrideInBytes: buffer.stride,
                    }
                 })
                 .collect();

        unsafe {
            self.raw.IASetVertexBuffers(
                0,
                vbs.0.len() as UINT,
                buffers.as_ptr(),
            );
        }
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        let viewports: SmallVec<[winapi::D3D12_VIEWPORT; 16]> =
            viewports.iter()
                     .map(|viewport| {
                        winapi::D3D12_VIEWPORT {
                            TopLeftX: viewport.x as FLOAT,
                            TopLeftY: viewport.y as FLOAT,
                            Width: viewport.w as FLOAT,
                            Height: viewport.h as FLOAT,
                            MinDepth: 0.0,
                            MaxDepth: 1.0,
                        }
                     })
                     .collect();

        unsafe {
            self.raw.RSSetViewports(
                viewports.len() as UINT, // NumViewports
                viewports.as_ptr(),      // pViewports
            );
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> =
            scissors.iter().map(get_rect).collect();
        unsafe {
            self.raw.RSSetScissorRects(rects.len() as UINT, rects.as_ptr())
        };
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        unsafe {
            self.raw.SetPipelineState(pipeline.raw);
            self.raw.IASetPrimitiveTopology(pipeline.topology);
        };
    }

    fn bind_graphics_descriptor_sets(&mut self, layout: &n::PipelineLayout, first_set: usize, sets: &[&()]) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unsafe {
            self.raw.SetPipelineState(pipeline.raw);
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.raw.Dispatch(
                x, // ThreadGroupCountX
                y, // ThreadGroupCountY
                z, // ThreadGroupCountZ
            );
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        unimplemented!()
    }

    /*
    fn update_buffer(&mut self, buffer: &n::Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }
    */

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(&mut self, src: &n::Image, dst: &n::Image) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self, src: &n::Buffer, dst: &n::Image, layout: texture::ImageLayout, regions: &[BufferImageCopy]) {
        unimplemented!()
    }

    fn copy_image_to_buffer(&mut self, src: &n::Image, dst: &n::Buffer, layout: texture::ImageLayout, regions: &[BufferImageCopy]) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.raw.DrawInstanced(
                count,          // VertexCountPerInstance
                num_instances,  // InstanceCount
                start,          // StartVertexLocation
                start_instance, // StartInstanceLocation
            );
        }
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, instances: Option<InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.raw.DrawIndexedInstanced(
                count,          // IndexCountPerInstance
                num_instances,  // InstanceCount
                start,          // StartIndexLocation
                base,           // BaseVertexLocation
                start_instance, // StartInstanceLocation
            );
        }
    }

    fn draw_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unimplemented!()
    }
}

pub struct SubpassCommandBuffer {
}
