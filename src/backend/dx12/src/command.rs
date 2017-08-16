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
use core::{IndexType, VertexCount, VertexOffset, Viewport};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearColor, ClearValue, ImageCopy, ImageResolve,
                    InstanceParams, SubpassContents};
use winapi::{self, UINT8, FLOAT, UINT, UINT64};
use {native as n, Backend};
use smallvec::SmallVec;
use std::{cmp, mem, ptr};

#[derive(Clone)]
pub struct SubmitInfo(pub(crate) ComPtr<winapi::ID3D12GraphicsCommandList>);
unsafe impl Send for SubmitInfo {}

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
        unsafe {
            self.raw.Close();
        }
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
        self.pass_cache = Some(RenderPassCache {
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

    fn pipeline_barrier(&mut self, _barries: &[memory::Barrier]) {
        unimplemented!()
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        _: texture::ImageLayout,
        color: ClearColor,
    ) {
        match color {
            ClearColor::Float(ref c) => unsafe {
                self.raw
                    .ClearRenderTargetView(rtv.handle, c, 0, ptr::null());
            },
            _ => {
                // TODO: Can we support uint/int?
                error!("Unable to clear int/uint target");
            }
        }
    }

    fn clear_depth_stencil(
        &mut self,
        dsv: &n::DepthStencilView,
        layout: texture::ImageLayout,
        depth: Option<target::Depth>,
        stencil: Option<target::Stencil>,
    ) {
        let mut flags = winapi::D3D12_CLEAR_FLAGS(0);
        if depth.is_some() {
            flags = flags | winapi::D3D12_CLEAR_FLAG_DEPTH;
        }
        if stencil.is_some() {
            flags = flags | winapi::D3D12_CLEAR_FLAG_STENCIL;
        }

        unsafe {
            self.raw.ClearDepthStencilView(
                dsv.handle,
                flags,
                depth.unwrap_or_default() as FLOAT,
                stencil.unwrap_or_default() as UINT8,
                0,
                ptr::null(),
            );
        }
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        src_layout: texture::ImageLayout,
        dst: &n::Image,
        dst_layout: texture::ImageLayout,
        regions: &[ImageResolve],
    ) {
        for region in regions {
            for l in 0..region.num_layers as UINT {
                unsafe {
                    self.raw.ResolveSubresource(
                        src.resource,
                        src.calc_subresource(region.src_subresource.0 as UINT, l + region.src_subresource.1 as UINT),
                        dst.resource,
                        dst.calc_subresource(region.dst_subresource.0 as UINT, l + region.dst_subresource.1 as UINT),
                        src.dxgi_format,
                    );
                }
            }
        }
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        let format = match ibv.index_type {
            IndexType::U16 => winapi::DXGI_FORMAT_R16_UINT,
            IndexType::U32 => winapi::DXGI_FORMAT_R32_UINT,
        };
        let location = unsafe { (*ibv.buffer.resource).GetGPUVirtualAddress() };

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
        let buffers: SmallVec<[winapi::D3D12_VERTEX_BUFFER_VIEW; 16]> = vbs.0
            .iter()
            .map(|&(ref buffer, offset)| {
                let base = unsafe { (*buffer.resource).GetGPUVirtualAddress() };
                winapi::D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: base + offset as u64,
                    SizeInBytes: buffer.size_in_bytes,
                    StrideInBytes: buffer.stride,
                }
            })
            .collect();

        unsafe {
            self.raw
                .IASetVertexBuffers(0, vbs.0.len() as UINT, buffers.as_ptr());
        }
    }

    fn set_viewports(&mut self, viewports: &[Viewport]) {
        let viewports: SmallVec<[winapi::D3D12_VIEWPORT; 16]> = viewports
            .iter()
            .map(|viewport| {
                winapi::D3D12_VIEWPORT {
                    TopLeftX: viewport.x as FLOAT,
                    TopLeftY: viewport.y as FLOAT,
                    Width: viewport.w as FLOAT,
                    Height: viewport.h as FLOAT,
                    MinDepth: viewport.near,
                    MaxDepth: viewport.far,
                }
            })
            .collect();

        unsafe {
            self.raw.RSSetViewports(
                viewports.len() as UINT,
                viewports.as_ptr(),
            );
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> = scissors.iter().map(get_rect).collect();
        unsafe {
            self.raw
                .RSSetScissorRects(rects.len() as UINT, rects.as_ptr())
        };
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        if rv.stencil.0 != rv.stencil.1 {
            error!(
                "Unable to set different stencil ref values for front ({}) and back ({})",
                rv.stencil.0,
                rv.stencil.1
            );
        }

        unsafe {
            self.raw.OMSetStencilRef(rv.stencil.0 as UINT);
            self.raw.OMSetBlendFactor(&rv.blend);
        }
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        unsafe {
            self.raw.SetPipelineState(pipeline.raw);
            self.raw.IASetPrimitiveTopology(pipeline.topology);
        };
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&()],
    ) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unsafe {
            self.raw.SetPipelineState(pipeline.raw);
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.raw.Dispatch(x, y, z);
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        // copy each region
        for region in regions {
            unsafe {
                self.raw.CopyBufferRegion(
                    dst.resource,
                    region.dst as UINT64,
                    src.resource,
                    region.src as UINT64,
                    region.size as UINT64,
                );
            }
        }

        // TODO: Optimization: Copy whole resource if possible
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        _: texture::ImageLayout,
        dst: &n::Image,
        _: texture::ImageLayout,
        regions: &[ImageCopy],
    ) {
        let mut src_image = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: src.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };

        let mut dst_image = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: dst.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };

        for region in regions {
            for layer in 0..region.num_layers {
                *unsafe { src_image.SubresourceIndex_mut() } =
                    src.calc_subresource(region.src_subresource.0 as UINT, (region.src_subresource.1 + layer) as UINT);
                *unsafe { dst_image.SubresourceIndex_mut() } =
                    dst.calc_subresource(region.dst_subresource.0 as UINT, (region.dst_subresource.1 + layer) as UINT);

                let src_box = winapi::D3D12_BOX {
                    left: region.src_offset.x as UINT,
                    top: region.src_offset.y as UINT,
                    right: (region.src_offset.x + region.extent.width as i32) as UINT,
                    bottom: (region.src_offset.y + region.extent.height as i32) as UINT,
                    front: region.src_offset.z as UINT,
                    back: (region.src_offset.z + region.extent.depth as i32) as UINT,
                };
                unsafe {
                    self.raw.CopyTextureRegion(
                        &dst_image,
                        region.dst_offset.x as UINT,
                        region.dst_offset.y as UINT,
                        region.dst_offset.z as UINT,
                        &src_image,
                        &src_box,
                    );
                }
            }
        }
    }

    fn copy_buffer_to_image(
        &mut self,
        buffer: &n::Buffer,
        image: &n::Image,
        _: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        let mut src = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: buffer.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            u: unsafe { mem::zeroed() },
        };
        let mut dst = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: image.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };
        let (width, height, depth, _) = image.kind.get_dimensions();
        for region in regions {
            // Copy each layer in the region
            let layers = region.image_subresource.1.clone();
            for layer in layers {
                assert_eq!(region.buffer_offset % winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64, 0);
                assert_eq!(region.buffer_row_pitch % winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u32, 0);
                assert!(region.buffer_row_pitch >= width as u32 * image.bits_per_texel as u32 / 8);

                let height = cmp::max(1, height as UINT);
                let depth = cmp::max(1, depth as UINT);

                // Advance buffer offset with each layer
                *unsafe { src.PlacedFootprint_mut() } = winapi::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: region.buffer_offset as UINT64 + (layer as u32 * region.buffer_row_pitch * height * depth) as UINT64,
                    Footprint: winapi::D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: image.dxgi_format,
                        Width: width as UINT,
                        Height: height,
                        Depth: depth,
                        RowPitch: region.buffer_row_pitch,
                    },
                };
                *unsafe { dst.SubresourceIndex_mut() } =
                    image.calc_subresource(region.image_subresource.0 as UINT, layer as UINT);
                let src_box = winapi::D3D12_BOX {
                    left: 0,
                    top: 0,
                    right: region.image_extent.width as UINT,
                    bottom: region.image_extent.height as UINT,
                    front: 0,
                    back: region.image_extent.depth as UINT,
                };
                unsafe {
                    self.raw.CopyTextureRegion(
                        &dst,
                        region.image_offset.x as UINT,
                        region.image_offset.y as UINT,
                        region.image_offset.z as UINT,
                        &src,
                        &src_box,
                    );
                }
            }
        }
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &n::Image,
        dst: &n::Buffer,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.raw
                .DrawInstanced(count, num_instances, start, start_instance);
        }
    }

    fn draw_indexed(
        &mut self,
        start: VertexCount,
        count: VertexCount,
        base: VertexOffset,
        instances: Option<InstanceParams>,
    ) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.raw
                .DrawIndexedInstanced(count, num_instances, start, base, start_instance);
        }
    }

    fn draw_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &n::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unimplemented!()
    }
}

pub struct SubpassCommandBuffer {}
