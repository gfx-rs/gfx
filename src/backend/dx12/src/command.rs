use wio::com::ComPtr;
use core::{command, image, memory, pso, target};
use core::{IndexCount, IndexType, InstanceCount, VertexCount, VertexOffset, Viewport};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearColor, ClearValue, ImageCopy, ImageResolve,
                    SubpassContents};
use core::pass::{Attachment, AttachmentLoadOp, AttachmentOps};
use winapi::{self, UINT64, UINT};
use {conv, native as n, Backend};
use smallvec::SmallVec;
use std::{cmp, mem, ptr};
use std::ops::Range;

fn get_rect(rect: &target::Rect) -> winapi::D3D12_RECT {
    winapi::D3D12_RECT {
        left: rect.x as i32,
        top: rect.y as i32,
        right: (rect.x + rect.w) as i32,
        bottom: (rect.y + rect.h) as i32,
    }
}

#[derive(Debug, Clone)]
pub struct RenderPassCache {
    render_pass: n::RenderPass,
    frame_buffer: n::FrameBuffer,
    next_subpass: usize,
    render_area: winapi::D3D12_RECT,
    clear_values: Vec<ClearValue>,
}

#[derive(Clone)]
pub struct CommandBuffer {
    pub(crate) raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
    pub(crate) allocator: ComPtr<winapi::ID3D12CommandAllocator>,

    // Cache renderpasses for graphics operations
    pub(crate) pass_cache: Option<RenderPassCache>,
}

unsafe impl Send for CommandBuffer { }

impl CommandBuffer {
    fn begin_subpass(&mut self) {
        let mut pass = self.pass_cache.as_mut().unwrap();
        assert!(pass.next_subpass < pass.render_pass.subpasses.len());

        // TODO
        pass.next_subpass += 1;
    }
}

impl command::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self) {
        unsafe { self.raw.Reset(self.allocator.as_mut(), ptr::null_mut()); }
    }

    fn finish(&mut self) {
        unsafe { self.raw.Close(); }
    }

    fn reset(&mut self, _release_resources: bool) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        framebuffer: &n::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        _first_subpass: SubpassContents,
    ) {
        let color_views = framebuffer.color.iter().map(|view| view.handle).collect::<Vec<_>>();
        assert!(framebuffer.depth_stencil.len() <= 1);
        assert_eq!(framebuffer.color.len() + framebuffer.depth_stencil.len(), render_pass.attachments.len());

        let ds_view = match framebuffer.depth_stencil.first() {
            Some(ref view) => &view.handle as *const _,
            None => ptr::null(),
        };
        unsafe {
            self.raw.OMSetRenderTargets(
                color_views.len() as UINT,
                color_views.as_ptr(),
                winapi::FALSE,
                ds_view,
            );
        }

        let area = get_rect(&render_area);

        let mut clear_iter = clear_values.iter();
        for (color, attachment) in framebuffer.color.iter().zip(render_pass.attachments.iter()) {
            if attachment.ops.load == AttachmentLoadOp::Clear {
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
                            self.raw.ClearRenderTargetView(color.handle, &data, 1, &area);
                        }
                    },
                    other => error!("Invalid clear value for view {:?}: {:?}", color, other),
                }
            }
        }
        if let (Some(depth), Some(&Attachment{ ops: AttachmentOps { load: AttachmentLoadOp::Clear, .. }, ..})) = (framebuffer.depth_stencil.first(), render_pass.attachments.last()) {
            match clear_iter.next() {
                Some(&command::ClearValue::DepthStencil(value)) => {
                    unsafe {
                        self.raw.ClearDepthStencilView(depth.handle,
                            winapi::D3D12_CLEAR_FLAG_DEPTH | winapi::D3D12_CLEAR_FLAG_STENCIL,
                            value.depth, value.stencil as u8, 1, &area);
                    }
                },
                other => error!("Invalid clear value for view {:?}: {:?}",
                    framebuffer.depth_stencil[0], other),
            }
        }

        self.pass_cache = Some(RenderPassCache {
            render_pass: render_pass.clone(),
            frame_buffer: framebuffer.clone(),
            render_area: area,
            clear_values: clear_values.into(),
            next_subpass: 0,
        });
        self.begin_subpass();
    }

    fn next_subpass(&mut self, _contents: SubpassContents) {
        self.begin_subpass();
    }

    fn end_renderpass(&mut self) {
        warn!("end renderpass unimplemented")
    }

    fn pipeline_barrier(
        &mut self,
        _stages: Range<pso::PipelineStage>,
        barriers: &[memory::Barrier<Backend>],
    ) {
        // TODO: very much WIP
        warn!("pipeline barriers unimplemented");

        let mut transition_barriers = Vec::new();

        for barrier in barriers {
            match *barrier {
                memory::Barrier::Image { ref states, target, ref range } => {
                    let state_src = conv::map_image_resource_state(states.start.0, states.start.1);
                    let state_dst = conv::map_image_resource_state(states.end.0, states.end.1);

                    if state_src == state_dst {
                        warn!("Image pipeline barrier requested with no effect: {:?}", barrier);
                        continue;
                    }

                    transition_barriers.push(
                        winapi::D3D12_RESOURCE_BARRIER {
                            Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                            Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                            u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                                pResource: target.resource,
                                Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                                StateBefore: state_src,
                                StateAfter: state_dst,
                            },
                        }
                    );
                }
                _ => {}
            }
        }

        unsafe {
            self.raw.ResourceBarrier(
                transition_barriers.len() as _,
                transition_barriers.as_ptr(),
            );
        }
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        _: image::ImageLayout,
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
        _layout: image::ImageLayout,
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
                depth.unwrap_or_default() as _,
                stencil.unwrap_or_default() as _,
                0,
                ptr::null(),
            );
        }
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        _: image::ImageLayout,
        dst: &n::Image,
        _: image::ImageLayout,
        regions: &[ImageResolve],
    ) {
        for region in regions {
            for l in 0..region.num_layers as _ {
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
                .IASetVertexBuffers(0, vbs.0.len() as _, buffers.as_ptr());
        }
    }

    fn set_viewports(&mut self, viewports: &[Viewport]) {
        let viewports: SmallVec<[winapi::D3D12_VIEWPORT; 16]> = viewports
            .iter()
            .map(|viewport| {
                winapi::D3D12_VIEWPORT {
                    TopLeftX: viewport.x as _,
                    TopLeftY: viewport.y as _,
                    Width: viewport.w as _,
                    Height: viewport.h as _,
                    MinDepth: viewport.near,
                    MaxDepth: viewport.far,
                }
            })
            .collect();

        unsafe {
            self.raw.RSSetViewports(
                viewports.len() as _,
                viewports.as_ptr(),
            );
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> = scissors.iter().map(get_rect).collect();
        unsafe {
            self.raw
                .RSSetScissorRects(rects.len() as _, rects.as_ptr())
        };
    }

    fn set_blend_constants(&mut self, color: target::ColorValue) {
        unsafe { self.raw.OMSetBlendFactor(&color); }
    }

    fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil) {
        if front != back {
            error!(
                "Unable to set different stencil ref values for front ({}) and back ({})",
                front,
                back,
            );
        }

        unsafe { self.raw.OMSetStencilRef(front as _); }
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
        sets: &[&n::DescriptorSet],
    ) {
        unsafe {
            self.raw.SetGraphicsRootSignature(layout.raw);

            // Bind descriptor heaps
            // TODO: Can we bind them always or only once?
            //       Resize while recording?
            let mut heaps = [
                sets[0].heap_srv_cbv_uav.as_mut() as *mut _,
                sets[0].heap_samplers.as_mut() as *mut _
            ];
            self.raw.SetDescriptorHeaps(2, heaps.as_mut_ptr())
        }

        let mut table_id = 0;
        for table in &layout.tables[.. first_set] {
            if table.contains(n::SRV_CBV_UAV) {
                table_id += 1;
            }
            if table.contains(n::SAMPLERS) {
                table_id += 1;
            }
        }
        for (set, table) in sets.iter().zip(layout.tables[first_set..].iter()) {
            set.first_gpu_view.map(|gpu| unsafe {
                assert!(table.contains(n::SRV_CBV_UAV));
                self.raw.SetGraphicsRootDescriptorTable(table_id, gpu);
                table_id += 1;
            });
            set.first_gpu_sampler.map(|gpu| unsafe {
                assert!(table.contains(n::SAMPLERS));
                self.raw.SetGraphicsRootDescriptorTable(table_id, gpu);
                table_id += 1;
            });
        }
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

    fn dispatch_indirect(&mut self, _buffer: &n::Buffer, _offset: u64) {
        unimplemented!()
    }

    fn fill_buffer(
        &mut self,
        _buffer: &n::Buffer,
        _range: Range<u64>,
        _data: u32,
    ) {
        unimplemented!()
    }

    fn update_buffer(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _data: &[u8],
    ) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        // copy each region
        for region in regions {
            unsafe {
                self.raw.CopyBufferRegion(
                    dst.resource,
                    region.dst as _,
                    src.resource,
                    region.src as _,
                    region.size as _,
                );
            }
        }

        // TODO: Optimization: Copy whole resource if possible
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        _: image::ImageLayout,
        dst: &n::Image,
        _: image::ImageLayout,
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
                    src.calc_subresource(region.src_subresource.0 as _, (region.src_subresource.1 + layer) as _);
                *unsafe { dst_image.SubresourceIndex_mut() } =
                    dst.calc_subresource(region.dst_subresource.0 as _, (region.dst_subresource.1 + layer) as _);

                let src_box = winapi::D3D12_BOX {
                    left: region.src_offset.x as _,
                    top: region.src_offset.y as _,
                    right: (region.src_offset.x + region.extent.width as i32) as _,
                    bottom: (region.src_offset.y + region.extent.height as i32) as _,
                    front: region.src_offset.z as _,
                    back: (region.src_offset.z + region.extent.depth as i32) as _,
                };
                unsafe {
                    self.raw.CopyTextureRegion(
                        &dst_image,
                        region.dst_offset.x as _,
                        region.dst_offset.y as _,
                        region.dst_offset.z as _,
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
        _: image::ImageLayout,
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

                let height = cmp::max(1, height as _);
                let depth = cmp::max(1, depth as _);

                // Advance buffer offset with each layer
                *unsafe { src.PlacedFootprint_mut() } = winapi::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: region.buffer_offset as UINT64 + (layer as u32 * region.buffer_row_pitch * height * depth) as UINT64,
                    Footprint: winapi::D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: image.dxgi_format,
                        Width: width as _,
                        Height: height,
                        Depth: depth,
                        RowPitch: region.buffer_row_pitch,
                    },
                };
                *unsafe { dst.SubresourceIndex_mut() } =
                    image.calc_subresource(region.image_subresource.0 as _, layer as _);
                let src_box = winapi::D3D12_BOX {
                    left: 0,
                    top: 0,
                    right: region.image_extent.width as _,
                    bottom: region.image_extent.height as _,
                    front: 0,
                    back: region.image_extent.depth as _,
                };
                unsafe {
                    self.raw.CopyTextureRegion(
                        &dst,
                        region.image_offset.x as _,
                        region.image_offset.y as _,
                        region.image_offset.z as _,
                        &src,
                        &src_box,
                    );
                }
            }
        }
    }

    fn copy_image_to_buffer(
        &mut self,
        _src: &n::Image,
        _: image::ImageLayout,
        _dst: &n::Buffer,
        _regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        unsafe {
            self.raw.DrawInstanced(
                vertices.end - vertices.start,
                instances.end - instances.start,
                vertices.start,
                instances.start,
            );
        }
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        unsafe {
            self.raw.DrawIndexedInstanced(
                indices.end - indices.start,
                instances.end - instances.start,
                indices.start,
                base_vertex,
                instances.start,
            );
        }
    }

    fn draw_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }
}

pub struct SubpassCommandBuffer {}
