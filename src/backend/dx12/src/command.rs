use core::{command, image, memory, pass, pso, target};
use core::{IndexCount, IndexType, InstanceCount, VertexCount, VertexOffset, Viewport};
use core::buffer::IndexBufferView;
use core::command::{AttachmentClear, BufferCopy, BufferImageCopy, ClearColor,
                    ClearValue, ImageCopy, ImageResolve, SubpassContents};
use winapi::{self, UINT64, UINT};
use {conv, native as n, Backend, ComPtr};
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
    render_area: winapi::D3D12_RECT,
    clear_values: Vec<ClearValue>,
}

impl RenderPassCache {
    fn get_resource(&self, att_id: pass::AttachmentId) -> *mut winapi::ID3D12Resource {
        match self.frame_buffer.color.get(att_id) {
            Some(view) => view.resource,
            None => self.frame_buffer.depth_stencil[att_id - self.frame_buffer.color.len()].resource,
        }
    }
}

#[derive(Clone)]
pub struct CommandBuffer {
    raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
    allocator: ComPtr<winapi::ID3D12CommandAllocator>,

    // Cache renderpasses for graphics operations
    pass_cache: Option<RenderPassCache>,
    cur_subpass: usize,
}

unsafe impl Send for CommandBuffer { }

impl CommandBuffer {
    pub(crate) fn new(
        raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
        allocator: ComPtr<winapi::ID3D12CommandAllocator>,
    ) -> Self {
        CommandBuffer {
            raw,
            allocator,
            pass_cache: None,
            cur_subpass: !0,
        }
    }

    pub(crate) unsafe fn as_raw_list(&self) -> *mut winapi::ID3D12CommandList {
        self.raw.as_mut() as *mut _ as *mut _
    }

    fn insert_subpass_barriers(&self) {
        let state = self.pass_cache.as_ref().unwrap();
        let proto_barriers = match state.render_pass.subpasses.get(self.cur_subpass) {
            Some(subpass) => &subpass.pre_barriers,
            None => &state.render_pass.post_barriers,
        };

        let transition_barriers = proto_barriers
            .iter()
            .map(|barrier| winapi::D3D12_RESOURCE_BARRIER {
                Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: barrier.flags,
                u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: state.get_resource(barrier.attachment_id),
                    Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: barrier.states.start,
                    StateAfter: barrier.states.end,
                },
            })
            .collect::<Vec<_>>();

        if !transition_barriers.is_empty() {
            unsafe {
                self.raw.clone().ResourceBarrier(
                    transition_barriers.len() as _,
                    transition_barriers.as_ptr(),
                );
            }
        }
    }

    fn clear_render_target_view(
        &mut self,
        rtv: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
        color: ClearColor,
        rects: &[winapi::D3D12_RECT],
    ) {
        let num_rects = rects.len() as _;
        let rects = if num_rects > 0 {
            rects.as_ptr()
        } else {
            ptr::null()
        };

        match color {
            ClearColor::Float(ref c) => unsafe {
                self.raw.ClearRenderTargetView(rtv, c, num_rects, rects);
            },
            _ => {
                // TODO: Can we support uint/int?
                error!("Unable to clear int/uint target");
            }
        }
    }

    fn clear_depth_stencil_view(
        &mut self,
        dsv: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
        depth: Option<target::Depth>,
        stencil: Option<target::Stencil>,
        rects: &[winapi::D3D12_RECT],
    ) {
        let mut flags = winapi::D3D12_CLEAR_FLAGS(0);
        if depth.is_some() {
            flags = flags | winapi::D3D12_CLEAR_FLAG_DEPTH;
        }
        if stencil.is_some() {
            flags = flags | winapi::D3D12_CLEAR_FLAG_STENCIL;
        }

        let num_rects = rects.len() as _;
        let rects = if num_rects > 0 {
            rects.as_ptr()
        } else {
            ptr::null()
        };

        unsafe {
            self.raw.ClearDepthStencilView(
                dsv,
                flags,
                depth.unwrap_or_default() as _,
                stencil.unwrap_or_default() as _,
                num_rects,
                rects,
            );
        }
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
        unsafe { self.raw.Reset(self.allocator.as_mut(), ptr::null_mut()); }
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        framebuffer: &n::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        _first_subpass: SubpassContents,
    ) {
        assert_eq!(framebuffer.color.len() + framebuffer.depth_stencil.len(), render_pass.attachments.len());
        // Make sure that no subpass works with Present as intermediate layout.
        // This wouldn't make much sense, and proceeding with this constraint
        // allows the state transitions generated from subpass dependencies
        // to ignore the layouts completely.
        assert!(!render_pass.subpasses.iter().any(|sp| {
            sp.color_attachments
                .iter()
                .chain(sp.input_attachments.iter()).
                any(|aref| aref.1 == image::ImageLayout::Present)
        }));

        let area = get_rect(&render_area);
        self.pass_cache = Some(RenderPassCache {
            render_pass: render_pass.clone(),
            frame_buffer: framebuffer.clone(),
            render_area: area,
            clear_values: clear_values.into(),
        });
        self.cur_subpass = 0;
        self.insert_subpass_barriers();

        let mut clear_iter = clear_values.iter();
        for (color, attachment) in framebuffer.color.iter().zip(render_pass.attachments.iter()) {
            if attachment.ops.load == pass::AttachmentLoadOp::Clear {
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
        if let (Some(depth), Some(&pass::Attachment{ ops: pass::AttachmentOps { load: pass::AttachmentLoadOp::Clear, .. }, ..})) = (framebuffer.depth_stencil.first(), render_pass.attachments.last()) {
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

        let color_views = framebuffer.color.iter().map(|view| view.handle).collect::<Vec<_>>();
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
    }

    fn next_subpass(&mut self, _contents: SubpassContents) {
        self.cur_subpass += 1;
        self.insert_subpass_barriers();
    }

    fn end_renderpass(&mut self) {
        self.cur_subpass = !0;
        self.insert_subpass_barriers();
        self.pass_cache = None;
    }

    fn pipeline_barrier(
        &mut self,
        _stages: Range<pso::PipelineStage>,
        barriers: &[memory::Barrier<Backend>],
    ) {
        let mut raw_barriers = Vec::new();

        // transition barriers
        for barrier in barriers {
            match *barrier {
                memory::Barrier::Buffer { ref states, target } => {
                    let state_src = conv::map_buffer_resource_state(states.start);
                    let state_dst = conv::map_buffer_resource_state(states.end);

                    if state_src == state_dst {
                        continue;
                    }

                    raw_barriers.push(
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
                memory::Barrier::Image { ref states, target, ref range } => {
                    let _ = range; //TODO: use subresource range
                    let state_src = conv::map_image_resource_state(states.start.0, states.start.1);
                    let state_dst = conv::map_image_resource_state(states.end.0, states.end.1);

                    if state_src == state_dst {
                        continue;
                    }

                    let mut barrier = winapi::D3D12_RESOURCE_BARRIER {
                        Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                        Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                        u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                            pResource: target.resource,
                            Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                            StateBefore: state_src,
                            StateAfter: state_dst,
                        },
                    };

                    if *range == target.as_subresource_range() {
                        // Only one barrier if it affects the whole image.
                        raw_barriers.push(barrier);
                    } else {
                        // Generate barrier for each layer/level combination.
                        let (levels, layers) = range.clone();
                        for level in levels.start .. levels.end {
                            for layer in layers.start .. layers.end {
                                barrier.u.Subresource = target.calc_subresource(level as _, layer as _);
                                raw_barriers.push(barrier);
                            }
                        }
                    }
                }
            }
        }

        // UAV barriers
        //
        // TODO: Currently always add a global UAV barrier.
        //       WAR only requires an execution barrier but D3D12 seems to need
        //       a UAV barrier for this according to docs. Can we make this better?
        {
            let mut barrier = winapi::D3D12_RESOURCE_BARRIER {
                Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_UAV,
                Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            *unsafe { barrier.UAV_mut() } = winapi::D3D12_RESOURCE_UAV_BARRIER {
                pResource: ptr::null_mut(),
            };
            raw_barriers.push(barrier);
        }

        // Alias barriers
        //
        // TODO: Optimize, don't always add an alias barrier
        {
            let mut barrier = winapi::D3D12_RESOURCE_BARRIER {
                Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            *unsafe { barrier.Aliasing_mut() } = winapi::D3D12_RESOURCE_ALIASING_BARRIER {
                pResourceBefore: ptr::null_mut(),
                        pResourceAfter: ptr::null_mut(),
            };
            raw_barriers.push(barrier);
        }

        unsafe {
            self.raw.ResourceBarrier(
                raw_barriers.len() as _,
                raw_barriers.as_ptr(),
            );
        }
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        _: image::ImageLayout,
        color: ClearColor,
    ) {
        self.clear_render_target_view(rtv.handle, color, &[]);
    }

    fn clear_attachments(
        &mut self,
        clears: &[AttachmentClear],
        rects: &[target::Rect],
    ) {
        assert!(self.pass_cache.is_some(), "`clear_attachments` can only be called inside a renderpass");
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> = rects.iter().map(get_rect).collect();
        for clear in clears {
            match *clear {
                AttachmentClear::Color(index, cv) => {
                    let rtv = {
                        let pass_cache = self.pass_cache.as_ref().unwrap();
                        let rtv_id = pass_cache
                            .render_pass
                            .subpasses[self.cur_subpass]
                            .color_attachments[index]
                            .0;

                        pass_cache
                            .frame_buffer
                            .color[rtv_id]
                            .handle
                    };

                    self.clear_render_target_view(
                        rtv,
                        cv,
                        &rects,
                    );
                }
                _ => unimplemented!(),
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
        self.clear_depth_stencil_view(
            dsv.handle,
            depth,
            stencil,
            &[],
        );
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        _: image::ImageLayout,
        dst: &n::Image,
        _: image::ImageLayout,
        regions: &[ImageResolve],
    ) {
        {
            // Insert barrier for `COPY_DEST` to `RESOLVE_DEST` as we only expose
            // `TRANSFER_WRITE` which is used for all copy commands.
            let transition_barrier = winapi::D3D12_RESOURCE_BARRIER {
                Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: dst.resource,
                    Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, // TODO: only affected ranges
                    StateBefore: winapi::D3D12_RESOURCE_STATE_COPY_DEST,
                    StateAfter: winapi::D3D12_RESOURCE_STATE_RESOLVE_DEST,
                },
            };

            unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };
        }

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

        {
            // Insert barrier for back transition from `RESOLVE_DEST` to `COPY_DEST`.
            let transition_barrier = winapi::D3D12_RESOURCE_BARRIER {
                Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: dst.resource,
                    Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, // TODO: only affected ranges
                    StateBefore: winapi::D3D12_RESOURCE_STATE_RESOLVE_DEST,
                    StateAfter: winapi::D3D12_RESOURCE_STATE_COPY_DEST,
                },
            };

            unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };
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
            let layers = &region.image_subresource.1;
            for layer in layers.start .. layers.end {
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
        image: &n::Image,
        _: image::ImageLayout,
        buffer: &n::Buffer,
        regions: &[BufferImageCopy],
    ) {
        let mut src = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: image.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };
        let mut dst = winapi::D3D12_TEXTURE_COPY_LOCATION {
            pResource: buffer.resource,
            Type: winapi::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            u: unsafe { mem::zeroed() },
        };
        let (width, height, depth, _) = image.kind.get_dimensions();
        for region in regions {
            // Copy each layer in the region
            let layers = &region.image_subresource.1;
            for layer in layers.start .. layers.end {
                assert_eq!(region.buffer_offset % winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64, 0);
                assert_eq!(region.buffer_row_pitch % winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u32, 0);
                assert!(region.buffer_row_pitch >= width as u32 * image.bits_per_texel as u32 / 8);

                let height = cmp::max(1, height as _);
                let depth = cmp::max(1, depth as _);

                // Advance buffer offset with each layer
                *unsafe { src.SubresourceIndex_mut() } =
                    image.calc_subresource(region.image_subresource.0 as _, layer as _);
                *unsafe { dst.PlacedFootprint_mut() } = winapi::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: region.buffer_offset as UINT64 + (layer as u32 * region.buffer_row_pitch * height * depth) as UINT64,
                    Footprint: winapi::D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: image.dxgi_format,
                        Width: width as _,
                        Height: height,
                        Depth: depth,
                        RowPitch: region.buffer_row_pitch,
                    },
                };
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
