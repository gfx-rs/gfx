use wio::com::ComPtr;
use hal::{command as com, image, memory, pass, pso};
use hal::{IndexCount, IndexType, InstanceCount, VertexCount, VertexOffset};
use hal::buffer::IndexBufferView;
use winapi::{self, UINT64, UINT};
use {conv, native as n, Backend, CmdSignatures};
use smallvec::SmallVec;
use std::{mem, ptr};
use std::ops::Range;

fn get_rect(rect: &com::Rect) -> winapi::D3D12_RECT {
    winapi::D3D12_RECT {
        left: rect.x as i32,
        top: rect.y as i32,
        right: (rect.x + rect.w) as i32,
        bottom: (rect.y + rect.h) as i32,
    }
}

#[derive(Debug, Clone)]
struct AttachmentClear {
    subpass_id: Option<pass::SubpassId>,
    value: Option<com::ClearValue>,
    stencil_value: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RenderPassCache {
    render_pass: n::RenderPass,
    framebuffer: n::Framebuffer,
    target_rect: winapi::D3D12_RECT,
    attachment_clears: Vec<AttachmentClear>,
}

/// Strongly-typed root signature element
///
/// Could be removed for an unsafer variant to occupy less memory
#[derive(Debug, Copy, Clone)]
enum RootElement {
    /// Root constant in the signature
    Constant(u32),
    /// Descriptor table, storing table offset for the current descriptor heap
    TableSrvCbvUav(u32),
    /// Descriptor table, storing table offset for the current descriptor heap
    TableSampler(u32),
    /// Undefined value, implementation specific
    Undefined,
}

/// Virtual data storage for the current root signature memory.
#[derive(Clone)]
struct UserData {
    data: [RootElement; 64],
    dirty_mask: u64,
}

impl UserData {
    fn new() -> Self {
        UserData {
            data: [RootElement::Undefined; 64],
            dirty_mask: 0,
        }
    }

    /// Update root constant values. Changes are marked as dirty.
    fn set_constants(&mut self, offset: usize, data: &[u32]) {
        // Each root constant occupies one DWORD
        for (i, val) in data.iter().enumerate() {
            self.data[offset+i] = RootElement::Constant(*val);
            self.dirty_mask |= 1u64 << (offset + i);
        }
    }

    /// Update descriptor table. Changes are marked as dirty.
    fn set_srv_cbv_uav_table(&mut self, offset: usize, table_start: u32) {
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSrvCbvUav(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Update descriptor table. Changes are marked as dirty.
    fn set_sampler_table(&mut self, offset: usize, table_start: u32) {
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSampler(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Clear dirty flag.
    fn clear_dirty(&mut self, i: usize) {
        self.dirty_mask &= !(1 << i);
    }
}

#[derive(Clone)]
struct PipelineCache {
    // Bound pipeline and root signature.
    // Changed on bind pipeline calls.
    pipeline: Option<(*mut winapi::ID3D12PipelineState, *mut winapi::ID3D12RootSignature)>,
    // Paramter slots of the current root signature.
    num_parameter_slots: usize,
    // Virtualized root signature user data of the shaders
    user_data: UserData,

    // Descriptor heap gpu handle offsets
    srv_cbv_uav_start: u64,
    sampler_start: u64,
}

impl PipelineCache {
    fn new() -> Self {
        PipelineCache {
            pipeline: None,
            num_parameter_slots: 0,
            user_data: UserData::new(),
            srv_cbv_uav_start: 0,
            sampler_start: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BindPoint {
    Compute,
    Graphics,
}

#[derive(Clone)]
pub struct CommandBuffer {
    raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
    allocator: ComPtr<winapi::ID3D12CommandAllocator>,
    signatures: CmdSignatures,

    // Cache renderpasses for graphics operations
    pass_cache: Option<RenderPassCache>,
    cur_subpass: usize,

    // Cache current graphics root signature and pipeline to minimize rebinding and support two
    // bindpoints.
    gr_pipeline: PipelineCache,
    // Cache current compute root signature and pipeline.
    comp_pipeline: PipelineCache,
    // D3D12 only has one slot for both bindpoints. Need to rebind everything if we want to switch
    // between different bind points (ie. calling draw or dispatch).
    active_bindpoint: BindPoint,
}

unsafe impl Send for CommandBuffer { }

impl CommandBuffer {
    pub(crate) fn new(
        raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
        allocator: ComPtr<winapi::ID3D12CommandAllocator>,
        signatures: CmdSignatures,
    ) -> Self {
        CommandBuffer {
            raw,
            allocator,
            signatures,
            pass_cache: None,
            cur_subpass: !0,
            gr_pipeline: PipelineCache::new(),
            comp_pipeline: PipelineCache::new(),
            active_bindpoint: BindPoint::Graphics,
        }
    }

    pub(crate) unsafe fn as_raw_list(&self) -> *mut winapi::ID3D12CommandList {
        self.raw.as_mut() as *mut _ as *mut _
    }

    fn reset(&mut self) {
        unsafe { self.raw.Reset(self.allocator.as_mut(), ptr::null_mut()); }
        self.pass_cache = None;
        self.cur_subpass = !0;
        self.gr_pipeline = PipelineCache::new();
        self.comp_pipeline = PipelineCache::new();
        self.active_bindpoint = BindPoint::Graphics;
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
                    pResource: state.framebuffer.attachments[barrier.attachment_id].resource,
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

    fn bind_targets(&mut self) {
        let state = self.pass_cache.as_ref().unwrap();
        let subpass = &state.render_pass.subpasses[self.cur_subpass];

        // collect render targets
        let color_views = subpass.color_attachments
            .iter()
            .map(|&(id, _)| state.framebuffer.attachments[id].handle_rtv.unwrap())
            .collect::<Vec<_>>();
        let ds_view = match subpass.depth_stencil_attachment {
            Some((id, _)) => state.framebuffer.attachments[id].handle_dsv.as_ref().unwrap() as *const _,
            None => ptr::null(),
        };
        // set render targets
        unsafe {
            self.raw.OMSetRenderTargets(
                color_views.len() as UINT,
                color_views.as_ptr(),
                winapi::FALSE,
                ds_view,
            );
        }

        // performs clears for all the attachments first used in this subpass
        for (view, clear) in state.framebuffer.attachments.iter().zip(state.attachment_clears.iter()) {
            if clear.subpass_id != Some(self.cur_subpass) {
                continue;
            }
            match clear.value {
                Some(com::ClearValue::Color(value)) => {
                    let handle = view.handle_rtv.unwrap();
                    self.clear_render_target_view(handle, value, &[state.target_rect]);
                }
                Some(com::ClearValue::DepthStencil(value)) => {
                    let handle = view.handle_dsv.unwrap();
                    self.clear_depth_stencil_view(handle, Some(value.0), None, &[state.target_rect]);
                }
                None => {}
            }
            if let Some(value) = clear.stencil_value {
                let handle = view.handle_dsv.unwrap();
                self.clear_depth_stencil_view(handle, None, Some(value), &[state.target_rect]);
            }
        }
    }

    fn clear_render_target_view(
        &self,
        rtv: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
        color: com::ClearColor,
        rects: &[winapi::D3D12_RECT],
    ) {
        let num_rects = rects.len() as _;
        let rects = if num_rects > 0 {
            rects.as_ptr()
        } else {
            ptr::null()
        };

        match color {
            com::ClearColor::Float(ref c) => unsafe {
                self.raw.clone().ClearRenderTargetView(rtv, c, num_rects, rects);
            },
            _ => {
                // TODO: Can we support uint/int?
                error!("Unable to clear int/uint target");
            }
        }
    }

    fn clear_depth_stencil_view(
        &self,
        dsv: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
        depth: Option<f32>,
        stencil: Option<u32>,
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
            self.raw.clone().ClearDepthStencilView(
                dsv,
                flags,
                depth.unwrap_or_default(),
                stencil.unwrap_or_default() as _,
                num_rects,
                rects,
            );
        }
    }

    fn set_graphics_bind_point(&mut self) {
        if self.active_bindpoint != BindPoint::Graphics {
            // Switch to graphics bind point
            let (pipeline, _) = self.gr_pipeline.pipeline.expect("No graphics pipeline bound");
            unsafe { self.raw.SetPipelineState(pipeline); }
            self.active_bindpoint = BindPoint::Graphics;
        }

        let cmd_buffer = &mut self.raw;
        Self::flush_user_data(
            &mut self.gr_pipeline,
            |slot, gpu| unsafe { cmd_buffer.SetGraphicsRootDescriptorTable(slot, gpu); },
        );
    }

    fn set_compute_bind_point(&mut self) {
        if self.active_bindpoint != BindPoint::Compute {
            // Switch to compute bind point
            assert!(self.comp_pipeline.pipeline.is_some(), "No compute pipeline bound");
            let (pipeline, _) = self.comp_pipeline.pipeline.unwrap();
            unsafe { self.raw.SetPipelineState(pipeline); }
            self.active_bindpoint = BindPoint::Compute;
        }

        let cmd_buffer = &mut self.raw;
        Self::flush_user_data(
            &mut self.comp_pipeline,
            |slot, gpu| unsafe { cmd_buffer.SetGraphicsRootDescriptorTable(slot, gpu); },
        );
    }

    fn flush_user_data<F>(
        pipeline: &mut PipelineCache,
        mut table_update: F,
    ) where
        F: FnMut(u32, winapi::D3D12_GPU_DESCRIPTOR_HANDLE),
    {
        let user_data = &mut pipeline.user_data;
        if user_data.dirty_mask == 0 {
            return
        }

        // TODO: root constant support

        // Flush descriptor tables
        // Index in the user data array where tables are starting
        let table_start = 0;
        let table_slot_start = 0;
        for i in 0..pipeline.num_parameter_slots {
            let table_index = i + table_start;
            if ((user_data.dirty_mask >> table_index) & 1) == 1 {
                let slot = (i + table_slot_start) as _;
                let ptr = match user_data.data[i] {
                    RootElement::TableSrvCbvUav(offset) =>
                        pipeline.srv_cbv_uav_start + offset as u64,
                    RootElement::TableSampler(offset) =>
                        pipeline.sampler_start + offset as u64,
                    _ => {
                        error!("Unexpected user data element in the root signature ({:?})", user_data.data[i]);
                        continue
                    }
                };
                let gpu = winapi::D3D12_GPU_DESCRIPTOR_HANDLE { ptr };
                table_update(slot, gpu);
                user_data.clear_dirty(table_index);
            }
        }
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self) {
        self.reset();
    }

    fn finish(&mut self) {
        unsafe { self.raw.Close(); }
    }

    fn reset(&mut self, _release_resources: bool) {
        self.reset();
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        framebuffer: &n::Framebuffer,
        target_rect: com::Rect,
        clear_values: &[com::ClearValue],
        _first_subpass: com::SubpassContents,
    ) {
        assert_eq!(framebuffer.attachments.len(), render_pass.attachments.len());
        // Make sure that no subpass works with Present as intermediate layout.
        // This wouldn't make much sense, and proceeding with this constraint
        // allows the state transitions generated from subpass dependencies
        // to ignore the layouts completely.
        assert!(!render_pass.subpasses.iter().any(|sp| {
            sp.color_attachments
                .iter()
                .chain(sp.depth_stencil_attachment.iter())
                .chain(sp.input_attachments.iter()).
                any(|aref| aref.1 == image::ImageLayout::Present)
        }));

        let mut clear_iter = clear_values.iter();
        let attachment_clears = render_pass.attachments.iter().enumerate().map(|(i, attachment)| {
            AttachmentClear {
                subpass_id: render_pass.subpasses.iter().position(|sp| sp.is_using(i)),
                value: if attachment.ops.load == pass::AttachmentLoadOp::Clear {
                    Some(*clear_iter.next().unwrap())
                } else {
                    None
                },
                stencil_value: if attachment.stencil_ops.load == pass::AttachmentLoadOp::Clear {
                    match clear_iter.next() {
                        Some(&com::ClearValue::DepthStencil(value)) => Some(value.1),
                        other => panic!("Unexpected clear value: {:?}", other),
                    }
                } else {
                    None
                },
            }
        }).collect();
        assert_eq!(clear_iter.next(), None);

        self.pass_cache = Some(RenderPassCache {
            render_pass: render_pass.clone(),
            framebuffer: framebuffer.clone(),
            target_rect: get_rect(&target_rect),
            attachment_clears,
        });
        self.cur_subpass = 0;
        self.insert_subpass_barriers();
        self.bind_targets();
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        self.cur_subpass += 1;
        self.insert_subpass_barriers();
        self.bind_targets();
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

                    if *range == target.to_subresource_range(range.aspects) {
                        // Only one barrier if it affects the whole image.
                        raw_barriers.push(barrier);
                    } else {
                        // Generate barrier for each layer/level combination.
                        for level in range.levels.clone() {
                            for layer in range.layers.clone() {
                                barrier.u.Subresource = target.calc_subresource(level as _, layer as _, 0);
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

    fn clear_color_image(
        &mut self,
        image: &n::Image,
        _: image::ImageLayout,
        range: image::SubresourceRange,
        value: com::ClearColor,
    ) {
        assert_eq!(range, image.to_subresource_range(image::AspectFlags::COLOR));
        let rtv = image.clear_cv.unwrap();
        self.clear_render_target_view(rtv, value, &[]);
    }

    fn clear_depth_stencil_image(
        &mut self,
        image: &n::Image,
        _layout: image::ImageLayout,
        range: image::SubresourceRange,
        value: com::ClearDepthStencil,
    ) {
        use self::image::AspectFlags;
        assert!((AspectFlags::DEPTH | AspectFlags::STENCIL).contains(range.aspects));
        assert_eq!(range, image.to_subresource_range(range.aspects));
        if range.aspects.contains(AspectFlags::DEPTH) {
            let dsv = image.clear_dv.unwrap();
            self.clear_depth_stencil_view(dsv, Some(value.0), None, &[]);
        }
        if range.aspects.contains(AspectFlags::STENCIL) {
            let dsv = image.clear_sv.unwrap();
            self.clear_depth_stencil_view(dsv, None, Some(value.1 as _), &[]);
        }
    }

    fn clear_attachments(
        &mut self,
        clears: &[com::AttachmentClear],
        rects: &[com::Rect],
    ) {
        assert!(self.pass_cache.is_some(), "`clear_attachments` can only be called inside a renderpass");
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> = rects.iter().map(get_rect).collect();
        for clear in clears {
            match *clear {
                com::AttachmentClear::Color(index, cv) => {
                    let rtv = {
                        let pass_cache = self.pass_cache.as_ref().unwrap();
                        let rtv_id = pass_cache
                            .render_pass
                            .subpasses[self.cur_subpass]
                            .color_attachments[index]
                            .0;

                        pass_cache
                            .framebuffer
                            .attachments[rtv_id]
                            .handle_rtv
                            .unwrap()
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

    fn resolve_image(
        &mut self,
        src: &n::Image,
        _: image::ImageLayout,
        dst: &n::Image,
        _: image::ImageLayout,
        regions: &[com::ImageResolve],
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
                        src.calc_subresource(region.src_subresource.0 as UINT, l + region.src_subresource.1 as UINT, 0),
                        dst.resource,
                        dst.calc_subresource(region.dst_subresource.0 as UINT, l + region.dst_subresource.1 as UINT, 0),
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

    fn set_viewports(&mut self, viewports: &[com::Viewport]) {
        let viewports: SmallVec<[winapi::D3D12_VIEWPORT; 16]> = viewports
            .iter()
            .map(|viewport| {
                winapi::D3D12_VIEWPORT {
                    TopLeftX: viewport.rect.x as _,
                    TopLeftY: viewport.rect.y as _,
                    Width: viewport.rect.w as _,
                    Height: viewport.rect.h as _,
                    MinDepth: viewport.depth.start,
                    MaxDepth: viewport.depth.end,
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

    fn set_scissors(&mut self, scissors: &[com::Rect]) {
        let rects: SmallVec<[winapi::D3D12_RECT; 16]> = scissors.iter().map(get_rect).collect();
        unsafe {
            self.raw
                .RSSetScissorRects(rects.len() as _, rects.as_ptr())
        };
    }

    fn set_blend_constants(&mut self, color: com::ColorValue) {
        unsafe { self.raw.OMSetBlendFactor(&color); }
    }

    fn set_stencil_reference(&mut self, front: com::StencilValue, back: com::StencilValue) {
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
            match self.gr_pipeline.pipeline {
                Some((_, signature)) if signature == pipeline.signature => {
                    // Same root signature, nothing to do
                },
                _ => {
                    self.raw.SetGraphicsRootSignature(pipeline.signature);
                    self.gr_pipeline.num_parameter_slots = pipeline.num_parameter_slots;
                    // All slots need to be rebound internally on signature change.
                    self.gr_pipeline.user_data.dirty_mask = !0;
                }
            }
            self.raw.SetPipelineState(pipeline.raw);
            self.raw.IASetPrimitiveTopology(pipeline.topology);
        };

        self.active_bindpoint = BindPoint::Graphics;
        self.gr_pipeline.pipeline = Some((pipeline.raw, pipeline.signature));
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        // Bind descriptor heaps
        unsafe {
            // TODO: Can we bind them always or only once?
            //       Resize while recording?
            let mut heaps = [
                sets[0].heap_srv_cbv_uav.as_mut() as *mut _,
                sets[0].heap_samplers.as_mut() as *mut _
            ];
            self.raw.SetDescriptorHeaps(2, heaps.as_mut_ptr())
        }

        let srv_cbv_uav_base = sets[0].srv_cbv_uav_gpu_start();
        let sampler_base = sets[0].sampler_gpu_start();

        self.gr_pipeline.srv_cbv_uav_start = srv_cbv_uav_base.ptr;
        self.gr_pipeline.sampler_start = sampler_base.ptr;

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
            set.first_gpu_view.map(|gpu| {
                assert!(table.contains(n::SRV_CBV_UAV));

                let root_offset = table_id; // TODO: take push constants into account
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - srv_cbv_uav_base.ptr) as u32;
                self
                    .gr_pipeline
                    .user_data
                    .set_srv_cbv_uav_table(root_offset, table_offset);

                table_id += 1;
            });
            set.first_gpu_sampler.map(|gpu| {
                assert!(table.contains(n::SAMPLERS));

                let root_offset = table_id; // TODO: take push constants into account
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - sampler_base.ptr) as u32;
                self
                    .gr_pipeline
                    .user_data
                    .set_sampler_table(root_offset, table_offset);

                table_id += 1;
            });
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unsafe {
            match self.comp_pipeline.pipeline {
                Some((_, signature)) if signature == pipeline.signature => {
                    // Same root signature, nothing to do
                },
                _ => {
                    self.raw.SetComputeRootSignature(pipeline.signature);
                    self.comp_pipeline.num_parameter_slots = pipeline.num_parameter_slots;
                    // All slots need to be rebound internally on signature change.
                    self.comp_pipeline.user_data.dirty_mask = !0;
                }
            }
            self.raw.SetPipelineState(pipeline.raw);
        }

        self.active_bindpoint = BindPoint::Compute;
        self.comp_pipeline.pipeline = Some((pipeline.raw, pipeline.signature));
    }

    fn bind_compute_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        unsafe {
            // Bind descriptor heaps
            // TODO: Can we bind them always or only once?
            //       Resize while recording?
            let mut heaps = [
                sets[0].heap_srv_cbv_uav.as_mut() as *mut _,
                sets[0].heap_samplers.as_mut() as *mut _
            ];
            self.raw.SetDescriptorHeaps(2, heaps.as_mut_ptr())
        }

        let srv_cbv_uav_base = sets[0].srv_cbv_uav_gpu_start();
        let sampler_base = sets[0].sampler_gpu_start();

        self.comp_pipeline.srv_cbv_uav_start = srv_cbv_uav_base.ptr;
        self.comp_pipeline.sampler_start = sampler_base.ptr;

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
            set.first_gpu_view.map(|gpu| {
                assert!(table.contains(n::SRV_CBV_UAV));

                let root_offset = table_id; // TODO: take push constants into account
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - srv_cbv_uav_base.ptr) as u32;
                self
                    .comp_pipeline
                    .user_data
                    .set_srv_cbv_uav_table(root_offset, table_offset);

                table_id += 1;
            });
            set.first_gpu_sampler.map(|gpu| {
                assert!(table.contains(n::SAMPLERS));

                let root_offset = table_id; // TODO: take push constants into account
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - sampler_base.ptr) as u32;
                self
                    .comp_pipeline
                    .user_data
                    .set_sampler_table(root_offset, table_offset);

                table_id += 1;
            });
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.set_compute_bind_point();
        unsafe {
            self.raw.Dispatch(x, y, z);
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        self.set_compute_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.signatures.dispatch.as_mut() as *mut _,
                1,
                buffer.resource,
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }

    fn fill_buffer(
        &mut self,
        buffer: &n::Buffer,
        range: Range<u64>,
        data: u32,
    ) {
        assert!(buffer.clear_uav.is_some(), "Buffer needs to be created with usage `TRANSFER_DST`");
        assert_eq!(range, 0..buffer.size_in_bytes as u64); // TODO: Need to dynamically create UAVs

        // Insert barrier for `COPY_DEST` to `UNORDERED_ACCESS` as we use
        // `TRANSFER_WRITE` for all clear commands.
        let transition_barrier = winapi::D3D12_RESOURCE_BARRIER {
            Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
            u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: buffer.resource,
                Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: winapi::D3D12_RESOURCE_STATE_COPY_DEST,
                StateAfter: winapi::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            },
        };

        unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };

        let handles = buffer.clear_uav.unwrap();
        unsafe {
            self.raw.ClearUnorderedAccessViewUint(
                handles.gpu,
                handles.cpu,
                buffer.resource,
                &[data as UINT; 4],
                0,
                ptr::null_mut(), // TODO: lift with the forementioned restriction
            );
        }

        // Transition back to original state
        let transition_barrier = winapi::D3D12_RESOURCE_BARRIER {
            Type: winapi::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
            u: winapi::D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: buffer.resource,
                Subresource: winapi::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: winapi::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                StateAfter: winapi::D3D12_RESOURCE_STATE_COPY_DEST,
            },
        };

        unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };
    }

    fn update_buffer(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _data: &[u8],
    ) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[com::BufferCopy]) {
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
        regions: &[com::ImageCopy],
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
                    src.calc_subresource(region.src_subresource.0 as _, (region.src_subresource.1 + layer) as _, 0);
                *unsafe { dst_image.SubresourceIndex_mut() } =
                    dst.calc_subresource(region.dst_subresource.0 as _, (region.dst_subresource.1 + layer) as _, 0);

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
        regions: &[com::BufferImageCopy],
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
            let layers = region.image_layers.layers.clone();
            for layer in layers {
                assert_eq!(region.buffer_offset % winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64, 0);
                assert_eq!(region.buffer_row_pitch % winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u32, 0);
                assert!(region.buffer_row_pitch >= width as u32 * image.bits_per_texel as u32 / 8);

                let height = height as _;
                let depth = depth as _;

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
                    image.calc_subresource(region.image_layers.level as _, layer as _, 0);
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
        regions: &[com::BufferImageCopy],
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
            let layers = region.image_layers.layers.clone();
            for layer in layers {
                assert_eq!(region.buffer_offset % winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64, 0);
                assert_eq!(region.buffer_row_pitch % winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u32, 0);
                assert!(region.buffer_row_pitch >= width as u32 * image.bits_per_texel as u32 / 8);

                let height = height as _;
                let depth = depth as _;

                // Advance buffer offset with each layer
                *unsafe { src.SubresourceIndex_mut() } =
                    image.calc_subresource(region.image_layers.level as _, layer as _, 0);
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
        self.set_graphics_bind_point();
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
        self.set_graphics_bind_point();
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
        buffer: &n::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        assert_eq!(stride, 16);
        self.set_graphics_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.signatures.draw.as_mut() as *mut _,
                draw_count,
                buffer.resource,
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &n::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        assert_eq!(stride, 20);
        self.set_graphics_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.signatures.draw_indexed.as_mut() as *mut _,
                draw_count,
                buffer.resource,
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }
}

pub struct SubpassCommandBuffer {}
