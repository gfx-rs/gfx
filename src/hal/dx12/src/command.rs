use hal::{DrawCount, IndexCount, VertexCount, VertexOffset, WorkGroupCount};

use hal::backend::FastHashMap;
use hal::format::Aspects;
use hal::range::RangeArg;
use hal::{buffer, command as com, format, image, memory, pass, pso, query};
use hal::{IndexType, InstanceCount};

use std::borrow::Borrow;
use std::ops::Range;
use std::sync::Arc;
use std::{cmp, iter, mem, ptr};

use winapi::shared::minwindef::{FALSE, TRUE, UINT};
use winapi::shared::{dxgiformat, winerror};
use winapi::um::{d3d12, d3dcommon};
use winapi::Interface;

use device::ViewInfo;
use root_constants::RootConstant;
use smallvec::SmallVec;
use {
    conv, descriptors_cpu, device, internal, resource as r, validate_line_width, Backend, Device,
    Shared, MAX_VERTEX_BUFFERS,
};

use bal_dx12;
use bal_dx12::native::descriptor::{HeapFlags, HeapType};

// Fixed size of the root signature.
// Limited by D3D12.
const ROOT_SIGNATURE_SIZE: usize = 64;

const NULL_VERTEX_BUFFER_VIEW: d3d12::D3D12_VERTEX_BUFFER_VIEW = d3d12::D3D12_VERTEX_BUFFER_VIEW {
    BufferLocation: 0,
    SizeInBytes: 0,
    StrideInBytes: 0,
};

fn get_rect(rect: &pso::Rect) -> d3d12::D3D12_RECT {
    d3d12::D3D12_RECT {
        left: rect.x as i32,
        top: rect.y as i32,
        right: (rect.x + rect.w) as i32,
        bottom: (rect.y + rect.h) as i32,
    }
}

#[derive(Clone)]
struct AttachmentClear {
    subpass_id: Option<pass::SubpassId>,
    value: Option<com::ClearValueRaw>,
    stencil_value: Option<u32>,
}

#[derive(Clone)]
pub struct RenderPassCache {
    render_pass: r::RenderPass,
    framebuffer: r::Framebuffer,
    target_rect: d3d12::D3D12_RECT,
    attachment_clears: Vec<AttachmentClear>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum OcclusionQuery {
    Binary(UINT),
    Precise(UINT),
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
    data: [RootElement; ROOT_SIGNATURE_SIZE],
    dirty_mask: u64,
}

impl UserData {
    fn new() -> Self {
        UserData {
            data: [RootElement::Undefined; ROOT_SIGNATURE_SIZE],
            dirty_mask: 0,
        }
    }

    /// Update root constant values. Changes are marked as dirty.
    fn set_constants(&mut self, offset: usize, data: &[u32]) {
        assert!(offset + data.len() <= ROOT_SIGNATURE_SIZE);
        // Each root constant occupies one DWORD
        for (i, val) in data.iter().enumerate() {
            self.data[offset + i] = RootElement::Constant(*val);
            self.dirty_mask |= 1u64 << (offset + i);
        }
    }

    /// Update descriptor table. Changes are marked as dirty.
    fn set_srv_cbv_uav_table(&mut self, offset: usize, table_start: u32) {
        assert!(offset < ROOT_SIGNATURE_SIZE);
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSrvCbvUav(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Update descriptor table. Changes are marked as dirty.
    fn set_sampler_table(&mut self, offset: usize, table_start: u32) {
        assert!(offset < ROOT_SIGNATURE_SIZE);
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSampler(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Clear dirty flag.
    fn clear_dirty(&mut self, i: usize) {
        self.dirty_mask &= !(1 << i);
    }

    /// Mark all entries as dirty.
    fn dirty_all(&mut self) {
        self.dirty_mask = !0;
    }
}

#[derive(Clone)]
struct PipelineCache {
    // Bound pipeline and root signature.
    // Changed on bind pipeline calls.
    pipeline: Option<(
        bal_dx12::native::PipelineState,
        bal_dx12::native::RootSignature,
    )>,
    // Paramter slots of the current root signature.
    num_parameter_slots: usize,
    //
    root_constants: Vec<RootConstant>,
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
            root_constants: Vec::new(),
            user_data: UserData::new(),
            srv_cbv_uav_start: 0,
            sampler_start: 0,
        }
    }

    fn bind_descriptor_sets<'a, I, J>(
        &mut self,
        layout: &r::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) -> [bal_dx12::native::DescriptorHeap; 2]
    where
        I: IntoIterator,
        I::Item: Borrow<r::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        assert!(offsets.into_iter().next().is_none()); //TODO

        let mut sets = sets.into_iter().peekable();
        let (srv_cbv_uav_start, sampler_start, heap_srv_cbv_uav, heap_sampler) =
            if let Some(set_0) = sets.peek().map(Borrow::borrow) {
                (
                    set_0.srv_cbv_uav_gpu_start().ptr,
                    set_0.sampler_gpu_start().ptr,
                    set_0.heap_srv_cbv_uav,
                    set_0.heap_samplers,
                )
            } else {
                return [bal_dx12::native::DescriptorHeap::null(); 2];
            };

        self.srv_cbv_uav_start = srv_cbv_uav_start;
        self.sampler_start = sampler_start;

        let mut table_id = 0;
        for table in &layout.tables[..first_set] {
            if table.contains(r::SRV_CBV_UAV) {
                table_id += 1;
            }
            if table.contains(r::SAMPLERS) {
                table_id += 1;
            }
        }

        let table_base_offset = layout
            .root_constants
            .iter()
            .fold(0, |sum, c| sum + c.range.end - c.range.start);

        for (set, table) in sets.zip(layout.tables[first_set..].iter()) {
            let set = set.borrow();
            set.first_gpu_view.map(|gpu| {
                assert!(table.contains(r::SRV_CBV_UAV));

                let root_offset = table_id + table_base_offset;
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - srv_cbv_uav_start) as u32;
                self.user_data
                    .set_srv_cbv_uav_table(root_offset as _, table_offset);

                table_id += 1;
            });
            set.first_gpu_sampler.map(|gpu| {
                assert!(table.contains(r::SAMPLERS));

                let root_offset = table_id + table_base_offset;
                // Cast is safe as offset **must** be in u32 range. Unable to
                // create heaps with more descriptors.
                let table_offset = (gpu.ptr - sampler_start) as u32;
                self.user_data
                    .set_sampler_table(root_offset as _, table_offset);

                table_id += 1;
            });
        }

        [heap_srv_cbv_uav, heap_sampler]
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BindPoint {
    Compute,
    Graphics {
        /// Internal pipelines used for blitting, copying, etc.
        internal: bool,
    },
}

#[derive(Clone)]
pub struct CommandBuffer {
    raw: bal_dx12::native::GraphicsCommandList,
    allocator: bal_dx12::native::CommandAllocator,
    shared: Arc<Shared>,

    // Cache renderpasses for graphics operations
    pass_cache: Option<RenderPassCache>,
    cur_subpass: usize,

    // Cache current graphics root signature and pipeline to minimize rebinding and support two
    // bindpoints.
    gr_pipeline: PipelineCache,
    // Primitive topology of the currently bound graphics pipeline.
    // Caching required for internal graphics pipelines.
    primitive_topology: d3d12::D3D12_PRIMITIVE_TOPOLOGY,
    // Cache current compute root signature and pipeline.
    comp_pipeline: PipelineCache,
    // D3D12 only has one slot for both bindpoints. Need to rebind everything if we want to switch
    // between different bind points (ie. calling draw or dispatch).
    active_bindpoint: BindPoint,
    // Current descriptor heaps heaps (CBV/SRV/UAV and Sampler).
    // Required for resetting due to internal descriptor heaps.
    active_descriptor_heaps: [bal_dx12::native::DescriptorHeap; 2],

    // Active queries in the command buffer.
    // Queries must begin and end in the same command buffer, which allows us to track them.
    // The query pool type on `begin_query` must differ from all currently active queries.
    // Therefore, only one query per query type can be active at the same time. Binary and precise
    // occlusion queries share one queue type in Vulkan.
    occlusion_query: Option<OcclusionQuery>,
    pipeline_stats_query: Option<UINT>,

    // Cached vertex buffer views to bind.
    // `Stride` values are not known at `bind_vertex_buffers` time because they are only stored
    // inside the pipeline state.
    vertex_bindings_remap: [Option<r::VertexBinding>; MAX_VERTEX_BUFFERS],
    vertex_buffer_views: [d3d12::D3D12_VERTEX_BUFFER_VIEW; MAX_VERTEX_BUFFERS],

    // Re-using allocation for the image-buffer copies.
    copies: Vec<bal_dx12::copy::Copy>,

    // D3D12 only allows setting all viewports or all scissors at once, not partial updates.
    // So we must cache the implied state for these partial updates.
    viewport_cache: SmallVec<
        [d3d12::D3D12_VIEWPORT;
            d3d12::D3D12_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize],
    >,
    scissor_cache: SmallVec<
        [d3d12::D3D12_RECT;
            d3d12::D3D12_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize],
    >,

    // HACK: renderdoc workaround for temporary RTVs
    rtv_pools: Vec<bal_dx12::native::DescriptorHeap>,
    // Temporary gpu descriptor heaps (internal).
    temporary_gpu_heaps: Vec<bal_dx12::native::DescriptorHeap>,
    // Resources that need to be alive till the end of the GPU execution.
    retained_resources: Vec<bal_dx12::native::Resource>,
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {}

// Insetion point for subpasses.
enum BarrierPoint {
    // Pre barriers are inserted of the beginning when switching into a new subpass.
    Pre,
    // Post barriers are applied after exectuing the user defined commands.
    Post,
}

impl CommandBuffer {
    pub(crate) fn new(
        raw: bal_dx12::native::GraphicsCommandList,
        allocator: bal_dx12::native::CommandAllocator,
        shared: Arc<Shared>,
    ) -> Self {
        CommandBuffer {
            raw,
            allocator,
            shared,
            pass_cache: None,
            cur_subpass: !0,
            gr_pipeline: PipelineCache::new(),
            primitive_topology: d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED,
            comp_pipeline: PipelineCache::new(),
            active_bindpoint: BindPoint::Graphics { internal: false },
            active_descriptor_heaps: [bal_dx12::native::DescriptorHeap::null(); 2],
            occlusion_query: None,
            pipeline_stats_query: None,
            vertex_bindings_remap: [None; MAX_VERTEX_BUFFERS],
            vertex_buffer_views: [NULL_VERTEX_BUFFER_VIEW; MAX_VERTEX_BUFFERS],
            copies: Vec::new(),
            viewport_cache: SmallVec::new(),
            scissor_cache: SmallVec::new(),
            rtv_pools: Vec::new(),
            temporary_gpu_heaps: Vec::new(),
            retained_resources: Vec::new(),
        }
    }

    pub(crate) fn destroy(&mut self) {
        self.raw.destroy();
        for heap in &self.rtv_pools {
            heap.destroy();
        }
        for heap in &self.temporary_gpu_heaps {
            heap.destroy();
        }
        for resource in &self.retained_resources {
            resource.destroy();
        }
    }

    pub(crate) unsafe fn as_raw_list(&self) -> *mut d3d12::ID3D12CommandList {
        self.raw.as_mut_ptr() as *mut _
    }

    fn reset(&mut self) {
        self.raw
            .reset(self.allocator, bal_dx12::native::PipelineState::null());
        self.pass_cache = None;
        self.cur_subpass = !0;
        self.gr_pipeline = PipelineCache::new();
        self.primitive_topology = d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED;
        self.comp_pipeline = PipelineCache::new();
        self.active_bindpoint = BindPoint::Graphics { internal: false };
        self.active_descriptor_heaps = [bal_dx12::native::DescriptorHeap::null(); 2];
        self.occlusion_query = None;
        self.pipeline_stats_query = None;
        self.vertex_bindings_remap = [None; MAX_VERTEX_BUFFERS];
        self.vertex_buffer_views = [NULL_VERTEX_BUFFER_VIEW; MAX_VERTEX_BUFFERS];
        for heap in self.rtv_pools.drain(..) {
            heap.destroy();
        }
        for heap in self.temporary_gpu_heaps.drain(..) {
            heap.destroy();
        }
        for resource in self.retained_resources.drain(..) {
            resource.destroy();
        }
    }

    // Indicates that the pipeline slot has been overriden with an internal pipeline.
    //
    // This only invalidates the slot and the user data!
    fn set_internal_graphics_pipeline(&mut self) {
        self.active_bindpoint = BindPoint::Graphics { internal: true };
        self.gr_pipeline.user_data.dirty_all();
    }

    fn bind_descriptor_heaps(&mut self) {
        self.raw.set_descriptor_heaps(&self.active_descriptor_heaps);
    }

    fn insert_subpass_barriers(&self, insertion: BarrierPoint) {
        let state = self.pass_cache.as_ref().unwrap();
        let proto_barriers = match state.render_pass.subpasses.get(self.cur_subpass) {
            Some(subpass) => match insertion {
                BarrierPoint::Pre => &subpass.pre_barriers,
                BarrierPoint::Post => &subpass.post_barriers,
            },
            None => &state.render_pass.post_barriers,
        };

        let transition_barriers = proto_barriers
            .iter()
            .map(|barrier| {
                let mut resource_barrier = d3d12::D3D12_RESOURCE_BARRIER {
                    Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                    Flags: barrier.flags,
                    u: unsafe { mem::zeroed() },
                };

                *unsafe { resource_barrier.u.Transition_mut() } =
                    d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: state.framebuffer.attachments[barrier.attachment_id]
                            .resource
                            .as_mut_ptr(),
                        Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: barrier.states.start,
                        StateAfter: barrier.states.end,
                    };

                resource_barrier
            })
            .collect::<Vec<_>>();

        if !transition_barriers.is_empty() {
            unsafe {
                self.raw
                    .clone()
                    .ResourceBarrier(transition_barriers.len() as _, transition_barriers.as_ptr());
            }
        }
    }

    fn bind_targets(&mut self) {
        let state = self.pass_cache.as_ref().unwrap();
        let subpass = &state.render_pass.subpasses[self.cur_subpass];

        // collect render targets
        let color_views = subpass
            .color_attachments
            .iter()
            .map(|&(id, _)| state.framebuffer.attachments[id].handle_rtv.unwrap())
            .collect::<Vec<_>>();
        let ds_view = match subpass.depth_stencil_attachment {
            Some((id, _)) => state.framebuffer.attachments[id]
                .handle_dsv
                .as_ref()
                .unwrap() as *const _,
            None => ptr::null(),
        };
        // set render targets
        unsafe {
            self.raw.OMSetRenderTargets(
                color_views.len() as UINT,
                color_views.as_ptr(),
                FALSE,
                ds_view,
            );
        }

        // performs clears for all the attachments first used in this subpass
        for (view, clear) in state
            .framebuffer
            .attachments
            .iter()
            .zip(state.attachment_clears.iter())
        {
            if clear.subpass_id != Some(self.cur_subpass) {
                continue;
            }

            if let (Some(handle), Some(cv)) = (view.handle_rtv, clear.value) {
                self.clear_render_target_view(handle, unsafe { cv.color }, &[state.target_rect]);
            }

            if let Some(handle) = view.handle_dsv {
                let depth = clear.value.map(|cv| unsafe { cv.depth_stencil.depth });
                let stencil = clear.stencil_value;

                if depth.is_some() || stencil.is_some() {
                    self.clear_depth_stencil_view(handle, depth, stencil, &[state.target_rect]);
                }
            }
        }
    }

    fn resolve_attachments(&self) {
        let state = self.pass_cache.as_ref().unwrap();
        let framebuffer = &state.framebuffer;
        let subpass = &state.render_pass.subpasses[self.cur_subpass];

        for (i, resolve_attachment) in subpass.resolve_attachments.iter().enumerate() {
            let (dst_attachment, _) = *resolve_attachment;
            let (src_attachment, _) = subpass.color_attachments[i];

            let resolve_src = state.framebuffer.attachments[src_attachment];
            let resolve_dst = state.framebuffer.attachments[dst_attachment];

            // The number of layers of the render area are given on framebuffer creation.
            for l in 0..framebuffer.layers {
                // Attachtments only have a single mip level by specification.
                let subresource_src = resolve_src.calc_subresource(
                    resolve_src.mip_levels.0 as _,
                    (resolve_src.layers.0 + l) as _,
                );
                let subresource_dst = resolve_dst.calc_subresource(
                    resolve_dst.mip_levels.0 as _,
                    (resolve_dst.layers.0 + l) as _,
                );

                // TODO: take width and height of render area into account.
                unsafe {
                    self.raw.ResolveSubresource(
                        resolve_dst.resource.as_mut_ptr(),
                        subresource_dst,
                        resolve_src.resource.as_mut_ptr(),
                        subresource_src,
                        resolve_dst.dxgi_format,
                    );
                }
            }
        }
    }

    fn clear_render_target_view(
        &self,
        rtv: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
        color: com::ClearColorRaw,
        rects: &[d3d12::D3D12_RECT],
    ) {
        let num_rects = rects.len() as _;
        let rects = if num_rects > 0 {
            rects.as_ptr()
        } else {
            ptr::null()
        };

        unsafe {
            self.raw
                .clone()
                .ClearRenderTargetView(rtv, &color.float32, num_rects, rects);
        }
    }

    fn clear_depth_stencil_view(
        &self,
        dsv: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
        depth: Option<f32>,
        stencil: Option<u32>,
        rects: &[d3d12::D3D12_RECT],
    ) {
        let mut flags = bal_dx12::native::command_list::ClearFlags::empty();
        if depth.is_some() {
            flags |= bal_dx12::native::command_list::ClearFlags::DEPTH;
        }
        if stencil.is_some() {
            flags |= bal_dx12::native::command_list::ClearFlags::STENCIL;
        }

        self.raw.clear_depth_stencil_view(
            dsv,
            flags,
            depth.unwrap_or_default(),
            stencil.unwrap_or_default() as _,
            rects,
        );
    }

    fn set_graphics_bind_point(&mut self) {
        match self.active_bindpoint {
            BindPoint::Compute => {
                // Switch to graphics bind point
                let (pipeline, _) = self.gr_pipeline
                    .pipeline
                    .expect("No graphics pipeline bound");
                self.raw.set_pipeline_state(pipeline);
            }
            BindPoint::Graphics { internal: true } => {
                // Switch to graphics bind point
                let (pipeline, signature) = self.gr_pipeline
                    .pipeline
                    .expect("No graphics pipeline bound");
                self.raw.set_pipeline_state(pipeline);
                self.raw.set_graphics_root_signature(signature);
                self.bind_descriptor_heaps();
            }
            BindPoint::Graphics { internal: false } => {}
        }

        self.active_bindpoint = BindPoint::Graphics { internal: false };
        let cmd_buffer = &mut self.raw;

        // Bind vertex buffers
        // Use needs_bind array to determine which buffers still need to be bound
        // and bind them one continuous group at a time.
        {
            let vbs_remap = &self.vertex_bindings_remap;
            let vbs = &self.vertex_buffer_views;
            let mut last_end_slot = 0;
            loop {
                match vbs_remap[last_end_slot..]
                    .iter()
                    .position(|remap| remap.is_some())
                {
                    Some(start_offset) => {
                        let start_slot = last_end_slot + start_offset;
                        let buffers = vbs_remap[start_slot..]
                            .iter()
                            .take_while(|x| x.is_some())
                            .filter_map(|x| *x)
                            .map(|mapping| {
                                let view = vbs[mapping.mapped_binding];

                                d3d12::D3D12_VERTEX_BUFFER_VIEW {
                                    BufferLocation: view.BufferLocation + mapping.offset as u64,
                                    SizeInBytes: view.SizeInBytes - mapping.offset,
                                    StrideInBytes: mapping.stride,
                                }
                            })
                            .collect::<SmallVec<[_; MAX_VERTEX_BUFFERS]>>();
                        let num_views = buffers.len();

                        unsafe {
                            cmd_buffer.IASetVertexBuffers(
                                start_slot as _,
                                buffers.len() as _,
                                buffers.as_ptr(),
                            );
                        }
                        last_end_slot = start_slot + num_views;
                    }
                    None => break,
                }
            }
        }
        // Don't re-bind vertex buffers again.
        self.vertex_bindings_remap = [None; MAX_VERTEX_BUFFERS];

        // Flush root signature data
        Self::flush_user_data(
            &mut self.gr_pipeline,
            |slot, data| unsafe {
                cmd_buffer.clone().SetGraphicsRoot32BitConstants(
                    slot,
                    data.len() as _,
                    data.as_ptr() as *const _,
                    0,
                )
            },
            |slot, gpu| cmd_buffer.set_graphics_root_descriptor_table(slot, gpu),
        );
    }

    fn set_compute_bind_point(&mut self) {
        match self.active_bindpoint {
            BindPoint::Graphics { internal } => {
                // Switch to compute bind point
                let (pipeline, _) = self.comp_pipeline
                    .pipeline
                    .expect("No compute pipeline bound");

                self.raw.set_pipeline_state(pipeline);

                self.active_bindpoint = BindPoint::Compute;

                if internal {
                    self.bind_descriptor_heaps();
                    // Rebind the graphics root signature as we come from an internal graphics.
                    // Issuing a draw call afterwards would hide the information that we internally
                    // changed the graphics root signature.
                    if let Some((_, signature)) = self.gr_pipeline.pipeline {
                        self.raw.set_graphics_root_signature(signature);
                    }
                }
            }
            BindPoint::Compute => {} // Nothing to do
        }

        let cmd_buffer = &mut self.raw;
        Self::flush_user_data(
            &mut self.comp_pipeline,
            |slot, data| unsafe {
                cmd_buffer.clone().SetComputeRoot32BitConstants(
                    slot,
                    data.len() as _,
                    data.as_ptr() as *const _,
                    0,
                )
            },
            |slot, gpu| cmd_buffer.set_compute_root_descriptor_table(slot, gpu),
        );
    }

    fn push_constants(
        user_data: &mut UserData,
        layout: &r::PipelineLayout,
        offset: u32,
        constants: &[u32],
    ) {
        let num = constants.len() as u32;
        for root_constant in &layout.root_constants {
            assert!(root_constant.range.start <= root_constant.range.end);
            if root_constant.range.start >= offset && root_constant.range.start < offset + num {
                let start = (root_constant.range.start - offset) as _;
                let end = num.min(root_constant.range.end - offset) as _;
                user_data.set_constants(offset as _, &constants[start..end]);
            }
        }
    }

    fn flush_user_data<F, G>(
        pipeline: &mut PipelineCache,
        mut constants_update: F,
        mut table_update: G,
    ) where
        F: FnMut(u32, &[u32]),
        G: FnMut(u32, d3d12::D3D12_GPU_DESCRIPTOR_HANDLE),
    {
        let user_data = &mut pipeline.user_data;
        if user_data.dirty_mask == 0 {
            return;
        }

        let num_root_constant = pipeline.root_constants.len();
        let mut cur_index = 0;
        // TODO: opt: Only set dirty root constants?
        for (i, root_constant) in pipeline.root_constants.iter().enumerate() {
            let num_constants = (root_constant.range.end - root_constant.range.start) as usize;
            let mut data = Vec::new();
            for c in cur_index..cur_index + num_constants {
                data.push(match user_data.data[c] {
                    RootElement::Constant(v) => v,
                    _ => {
                        warn!(
                            "Unset or mismatching root constant at index {:?} ({:?})",
                            c, user_data.data[c]
                        );
                        0
                    }
                });
                user_data.clear_dirty(c);
            }
            constants_update(i as _, &data);
            cur_index += num_constants;
        }

        // Flush descriptor tables
        // Index in the user data array where tables are starting
        let table_start = pipeline
            .root_constants
            .iter()
            .fold(0, |sum, c| sum + c.range.end - c.range.start) as usize;

        for i in num_root_constant..pipeline.num_parameter_slots {
            let table_index = i - num_root_constant + table_start;
            if ((user_data.dirty_mask >> table_index) & 1) == 1 {
                let ptr = match user_data.data[table_index] {
                    RootElement::TableSrvCbvUav(offset) => {
                        pipeline.srv_cbv_uav_start + offset as u64
                    }
                    RootElement::TableSampler(offset) => pipeline.sampler_start + offset as u64,
                    other => {
                        error!(
                            "Unexpected user data element in the root signature ({:?})",
                            other
                        );
                        continue;
                    }
                };
                let gpu = d3d12::D3D12_GPU_DESCRIPTOR_HANDLE { ptr };
                table_update(i as _, gpu);
                user_data.clear_dirty(table_index);
            }
        }
    }

    fn transition_barrier(
        transition: d3d12::D3D12_RESOURCE_TRANSITION_BARRIER,
    ) -> d3d12::D3D12_RESOURCE_BARRIER {
        let mut barrier = d3d12::D3D12_RESOURCE_BARRIER {
            Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
            u: unsafe { mem::zeroed() },
        };

        *unsafe { barrier.u.Transition_mut() } = transition;
        barrier
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(
        &mut self,
        _flags: com::CommandBufferFlags,
        _info: com::CommandBufferInheritanceInfo<Backend>,
    ) {
        // TODO: Implement flags and secondary command buffers (bundles).
        self.reset();
    }

    fn finish(&mut self) {
        unsafe {
            self.raw.Close();
        }
    }

    fn reset(&mut self, _release_resources: bool) {
        self.reset();
    }

    fn begin_render_pass<T>(
        &mut self,
        render_pass: &r::RenderPass,
        framebuffer: &r::Framebuffer,
        target_rect: pso::Rect,
        clear_values: T,
        _first_subpass: com::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ClearValueRaw>,
    {
        assert_eq!(framebuffer.attachments.len(), render_pass.attachments.len());
        // Make sure that no subpass works with Present as intermediate layout.
        // This wouldn't make much sense, and proceeding with this constraint
        // allows the state transitions generated from subpass dependencies
        // to ignore the layouts completely.
        assert!(!render_pass.subpasses.iter().any(|sp| sp.color_attachments
            .iter()
            .chain(sp.depth_stencil_attachment.iter())
            .chain(sp.input_attachments.iter())
            .any(|aref| aref.1 == image::Layout::Present)));

        let mut clear_iter = clear_values.into_iter();
        let attachment_clears = render_pass
            .attachments
            .iter()
            .enumerate()
            .map(|(i, attachment)| {
                let cv = if attachment.ops.load == pass::AttachmentLoadOp::Clear
                    || attachment.stencil_ops.load == pass::AttachmentLoadOp::Clear
                {
                    Some(*clear_iter.next().unwrap().borrow())
                } else {
                    None
                };

                AttachmentClear {
                    subpass_id: render_pass.subpasses.iter().position(|sp| sp.is_using(i)),
                    value: if attachment.ops.load == pass::AttachmentLoadOp::Clear {
                        assert!(cv.is_some());
                        cv
                    } else {
                        None
                    },
                    stencil_value: if attachment.stencil_ops.load == pass::AttachmentLoadOp::Clear {
                        Some(unsafe { cv.unwrap().depth_stencil.stencil })
                    } else {
                        None
                    },
                }
            })
            .collect();

        self.pass_cache = Some(RenderPassCache {
            render_pass: render_pass.clone(),
            framebuffer: framebuffer.clone(),
            target_rect: get_rect(&target_rect),
            attachment_clears,
        });
        self.cur_subpass = 0;
        self.insert_subpass_barriers(BarrierPoint::Pre);
        self.bind_targets();
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        self.insert_subpass_barriers(BarrierPoint::Post);
        self.resolve_attachments();

        self.cur_subpass += 1;
        self.insert_subpass_barriers(BarrierPoint::Pre);
        self.bind_targets();
    }

    fn end_render_pass(&mut self) {
        self.insert_subpass_barriers(BarrierPoint::Post);
        self.resolve_attachments();

        self.cur_subpass = !0;
        self.insert_subpass_barriers(BarrierPoint::Pre);
        self.pass_cache = None;
    }

    fn pipeline_barrier<'a, T>(
        &mut self,
        _stages: Range<pso::PipelineStage>,
        _dependencies: memory::Dependencies,
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        let mut raw_barriers = Vec::new();

        // transition barriers
        for barrier in barriers {
            match *barrier.borrow() {
                memory::Barrier::AllBuffers(_) | memory::Barrier::AllImages(_) => {
                    // Aliasing barrier with NULL resource is the closest we can get to
                    // a global memory barrier in Vulkan.
                    // Was suggested by a Microsoft representative as well as some of the IHVs.
                    let mut bar = d3d12::D3D12_RESOURCE_BARRIER {
                        Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_UAV,
                        Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                        u: unsafe { mem::zeroed() },
                    };
                    *unsafe { bar.u.UAV_mut() } = d3d12::D3D12_RESOURCE_UAV_BARRIER {
                        pResource: ptr::null_mut(),
                    };
                    raw_barriers.push(bar);
                }
                memory::Barrier::Buffer { ref states, target } => {
                    let state_src = conv::map_buffer_resource_state(states.start);
                    let state_dst = conv::map_buffer_resource_state(states.end);

                    if state_src == state_dst {
                        continue;
                    }

                    let bar = Self::transition_barrier(d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: target.resource.as_mut_ptr(),
                        Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: state_src,
                        StateAfter: state_dst,
                    });

                    raw_barriers.push(bar);
                }
                memory::Barrier::Image {
                    ref states,
                    target,
                    ref range,
                } => {
                    let _ = range; //TODO: use subresource range
                    let state_src = conv::map_image_resource_state(states.start.0, states.start.1);
                    let state_dst = conv::map_image_resource_state(states.end.0, states.end.1);

                    if state_src == state_dst {
                        continue;
                    }

                    let mut bar =
                        Self::transition_barrier(d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                            pResource: target.resource.as_mut_ptr(),
                            Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                            StateBefore: state_src,
                            StateAfter: state_dst,
                        });

                    if *range == target.to_subresource_range(range.aspects) {
                        // Only one barrier if it affects the whole image.
                        raw_barriers.push(bar);
                    } else {
                        // Generate barrier for each layer/level combination.
                        for level in range.levels.clone() {
                            for layer in range.layers.clone() {
                                {
                                    let transition_barrier =
                                        &mut *unsafe { bar.u.Transition_mut() };
                                    transition_barrier.Subresource =
                                        target.calc_subresource(level as _, layer as _, 0);
                                }
                                raw_barriers.push(bar);
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
            let mut barrier = d3d12::D3D12_RESOURCE_BARRIER {
                Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_UAV,
                Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            *unsafe { barrier.u.UAV_mut() } = d3d12::D3D12_RESOURCE_UAV_BARRIER {
                pResource: ptr::null_mut(),
            };
            raw_barriers.push(barrier);
        }

        // Alias barriers
        //
        // TODO: Optimize, don't always add an alias barrier
        {
            let mut barrier = d3d12::D3D12_RESOURCE_BARRIER {
                Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            *unsafe { barrier.u.Aliasing_mut() } = d3d12::D3D12_RESOURCE_ALIASING_BARRIER {
                pResourceBefore: ptr::null_mut(),
                pResourceAfter: ptr::null_mut(),
            };
            raw_barriers.push(barrier);
        }

        unsafe {
            self.raw
                .ResourceBarrier(raw_barriers.len() as _, raw_barriers.as_ptr());
        }
    }

    fn clear_image<T>(
        &mut self,
        image: &r::Image,
        _: image::Layout,
        color: com::ClearColorRaw,
        depth_stencil: com::ClearDepthStencilRaw,
        subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        for subresource_range in subresource_ranges {
            let sub = subresource_range.borrow();
            assert_eq!(sub.levels, 0..1); //TODO
            for layer in sub.layers.clone() {
                if sub.aspects.contains(Aspects::COLOR) {
                    let rtv = image.clear_cv[layer as usize];
                    self.clear_render_target_view(rtv, color, &[]);
                }
                if sub.aspects.contains(Aspects::DEPTH) {
                    let dsv = image.clear_dv[layer as usize];
                    self.clear_depth_stencil_view(dsv, Some(depth_stencil.depth), None, &[]);
                }
                if sub.aspects.contains(Aspects::STENCIL) {
                    let dsv = image.clear_sv[layer as usize];
                    self.clear_depth_stencil_view(dsv, None, Some(depth_stencil.stencil as _), &[]);
                }
            }
        }
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<com::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        let pass_cache = match self.pass_cache {
            Some(ref cache) => cache,
            None => panic!("`clear_attachments` can only be called inside a renderpass"),
        };
        let sub_pass = &pass_cache.render_pass.subpasses[self.cur_subpass];

        let clear_rects: SmallVec<[pso::ClearRect; 16]> = rects
            .into_iter()
            .map(|rect| rect.borrow().clone())
            .collect();

        let device = self.shared.service_pipes.device;

        for clear in clears {
            match *clear.borrow() {
                com::AttachmentClear::Color { index, value } => {
                    let attachment = {
                        let rtv_id = sub_pass.color_attachments[index];
                        pass_cache.framebuffer.attachments[rtv_id.0]
                    };

                    let mut rtv_pool =
                        descriptors_cpu::HeapLinear::new(device, HeapType::Rtv, clear_rects.len());

                    for clear_rect in &clear_rects {
                        let rect = [get_rect(&clear_rect.rect)];

                        let view_info = device::ViewInfo {
                            resource: attachment.resource,
                            kind: attachment.kind,
                            flags: image::StorageFlags::empty(),
                            view_kind: image::ViewKind::D2Array,
                            format: attachment.dxgi_format,
                            range: image::SubresourceRange {
                                aspects: Aspects::COLOR,
                                levels: attachment.mip_levels.0..attachment.mip_levels.1,
                                layers: clear_rect.layers.clone(),
                            },
                        };
                        let rtv = rtv_pool.alloc_handle();
                        Device::view_image_as_render_target_impl(device, rtv, view_info).unwrap();
                        self.clear_render_target_view(rtv, value.into(), &rect);
                    }
                }
                com::AttachmentClear::DepthStencil { depth, stencil } => {
                    let attachment = {
                        let dsv_id = sub_pass.depth_stencil_attachment.unwrap();
                        pass_cache.framebuffer.attachments[dsv_id.0]
                    };

                    let mut dsv_pool =
                        descriptors_cpu::HeapLinear::new(device, HeapType::Dsv, clear_rects.len());

                    for clear_rect in &clear_rects {
                        let rect = [get_rect(&clear_rect.rect)];

                        let view_info = device::ViewInfo {
                            resource: attachment.resource,
                            kind: attachment.kind,
                            flags: image::StorageFlags::empty(),
                            view_kind: image::ViewKind::D2Array,
                            format: attachment.dxgi_format,
                            range: image::SubresourceRange {
                                aspects: if depth.is_some() {
                                    Aspects::DEPTH
                                } else {
                                    Aspects::empty()
                                } | if stencil.is_some() {
                                    Aspects::STENCIL
                                } else {
                                    Aspects::empty()
                                },
                                levels: attachment.mip_levels.0..attachment.mip_levels.1,
                                layers: clear_rect.layers.clone(),
                            },
                        };
                        let dsv = dsv_pool.alloc_handle();
                        Device::view_image_as_depth_stencil_impl(device, dsv, view_info).unwrap();
                        self.clear_depth_stencil_view(dsv, depth, stencil, &rect);
                    }
                }
            }
        }
    }

    fn resolve_image<T>(
        &mut self,
        src: &r::Image,
        _src_layout: image::Layout,
        dst: &r::Image,
        _dst_layout: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageResolve>,
    {
        assert_eq!(src.descriptor.Format, dst.descriptor.Format);

        {
            // Insert barrier for `COPY_DEST` to `RESOLVE_DEST` as we only expose
            // `TRANSFER_WRITE` which is used for all copy commands.
            let transition_barrier = Self::transition_barrier(
                d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: dst.resource.as_mut_ptr(),
                    Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, // TODO: only affected ranges
                    StateBefore: d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
                    StateAfter: d3d12::D3D12_RESOURCE_STATE_RESOLVE_DEST,
                },
            );
            unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };
        }

        for region in regions {
            let r = region.borrow();
            for layer in 0..r.extent.depth as UINT {
                unsafe {
                    self.raw.ResolveSubresource(
                        src.resource.as_mut_ptr(),
                        src.calc_subresource(
                            r.src_subresource.level as UINT,
                            r.src_subresource.layers.start as UINT + layer,
                            0,
                        ),
                        dst.resource.as_mut_ptr(),
                        dst.calc_subresource(
                            r.dst_subresource.level as UINT,
                            r.dst_subresource.layers.start as UINT + layer,
                            0,
                        ),
                        src.descriptor.Format,
                    );
                }
            }
        }

        {
            // Insert barrier for back transition from `RESOLVE_DEST` to `COPY_DEST`.
            let transition_barrier = Self::transition_barrier(
                d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: dst.resource.as_mut_ptr(),
                    Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, // TODO: only affected ranges
                    StateBefore: d3d12::D3D12_RESOURCE_STATE_RESOLVE_DEST,
                    StateAfter: d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
                },
            );
            unsafe { self.raw.ResourceBarrier(1, &transition_barrier) };
        }
    }

    fn blit_image<T>(
        &mut self,
        src: &r::Image,
        _src_layout: image::Layout,
        dst: &r::Image,
        _dst_layout: image::Layout,
        filter: image::Filter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageBlit>,
    {
        let device = self.shared.service_pipes.device.clone();

        // TODO: Resource barriers for src.
        // TODO: depth or stencil images not supported so far

        // TODO: only supporting 2D images
        match (src.kind, dst.kind) {
            (image::Kind::D2(..), image::Kind::D2(..)) => {}
            _ => unimplemented!(),
        }

        // Descriptor heap for the current blit, only storing the src image
        let (srv_heap, _) =
            device.create_descriptor_heap(1, HeapType::CbvSrvUav, HeapFlags::SHADER_VISIBLE, 0);
        let srv_desc = Device::build_image_as_shader_resource_desc(&ViewInfo {
            resource: src.resource,
            kind: src.kind,
            flags: src.storage_flags,
            view_kind: image::ViewKind::D2Array, // TODO
            format: src.descriptor.Format,
            range: image::SubresourceRange {
                aspects: format::Aspects::COLOR, // TODO
                levels: 0..src.descriptor.MipLevels as _,
                layers: 0..src.kind.num_layers(),
            },
        }).unwrap();
        unsafe {
            device.CreateShaderResourceView(
                src.resource.as_mut_ptr(),
                &srv_desc,
                srv_heap.start_cpu_descriptor(),
            );
            self.raw.set_descriptor_heaps(&[srv_heap]);
        }
        self.temporary_gpu_heaps.push(srv_heap);

        let filter = match filter {
            image::Filter::Nearest => d3d12::D3D12_FILTER_MIN_MAG_MIP_POINT,
            image::Filter::Linear => d3d12::D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        };

        struct Instance {
            rtv: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
            viewport: d3d12::D3D12_VIEWPORT,
            data: internal::BlitData,
        };
        let mut instances = FastHashMap::<internal::BlitKey, Vec<Instance>>::default();
        let mut barriers = Vec::new();

        for region in regions {
            let r = region.borrow();

            let first_layer = r.dst_subresource.layers.start;
            let num_layers = r.dst_subresource.layers.end - first_layer;

            // WORKAROUND: renderdoc crashes if we destroy the pool too early
            let rtv_pool =
                Device::create_descriptor_heap_impl(device, HeapType::Rtv, false, num_layers as _);
            self.rtv_pools.push(rtv_pool.raw.clone());

            let key = match r.dst_subresource.aspects {
                format::Aspects::COLOR => {
                    // Create RTVs of the dst image for the miplevel of the current region
                    for i in 0..num_layers {
                        let mut desc = d3d12::D3D12_RENDER_TARGET_VIEW_DESC {
                            Format: dst.descriptor.Format,
                            ViewDimension: d3d12::D3D12_RTV_DIMENSION_TEXTURE2DARRAY,
                            u: unsafe { mem::zeroed() },
                        };

                        *unsafe { desc.u.Texture2DArray_mut() } = d3d12::D3D12_TEX2D_ARRAY_RTV {
                            MipSlice: r.dst_subresource.level as _,
                            FirstArraySlice: (i + first_layer) as u32,
                            ArraySize: 1,
                            PlaneSlice: 0, // TODO
                        };

                        let view = rtv_pool.at(i as _, 0).cpu;
                        unsafe {
                            device.CreateRenderTargetView(dst.resource.as_mut_ptr(), &desc, view);
                        }
                    }

                    (dst.descriptor.Format, filter)
                }
                _ => unimplemented!(),
            };

            // Take flipping into account
            let viewport = d3d12::D3D12_VIEWPORT {
                TopLeftX: cmp::min(r.dst_bounds.start.x, r.dst_bounds.end.x) as _,
                TopLeftY: cmp::min(r.dst_bounds.start.y, r.dst_bounds.end.y) as _,
                Width: (r.dst_bounds.end.x - r.dst_bounds.start.x).abs() as _,
                Height: (r.dst_bounds.end.y - r.dst_bounds.start.y).abs() as _,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };

            let mut list = instances.entry(key).or_insert(Vec::new());

            for i in 0..num_layers {
                let src_layer = r.src_subresource.layers.start + i;
                // Screen space triangle blitting
                let data = {
                    // Image extents, layers are treated as depth
                    let (sx, dx) = if r.dst_bounds.start.x > r.dst_bounds.end.x {
                        (
                            r.src_bounds.end.x,
                            r.src_bounds.start.x - r.src_bounds.end.x,
                        )
                    } else {
                        (
                            r.src_bounds.start.x,
                            r.src_bounds.end.x - r.src_bounds.start.x,
                        )
                    };
                    let (sy, dy) = if r.dst_bounds.start.y > r.dst_bounds.end.y {
                        (
                            r.src_bounds.end.y,
                            r.src_bounds.start.y - r.src_bounds.end.y,
                        )
                    } else {
                        (
                            r.src_bounds.start.y,
                            r.src_bounds.end.y - r.src_bounds.start.y,
                        )
                    };
                    let image::Extent { width, height, .. } =
                        src.kind.level_extent(r.src_subresource.level);

                    internal::BlitData {
                        src_offset: [sx as f32 / width as f32, sy as f32 / height as f32],
                        src_extent: [dx as f32 / width as f32, dy as f32 / height as f32],
                        layer: src_layer as f32,
                        level: r.src_subresource.level as _,
                    }
                };

                list.push(Instance {
                    rtv: rtv_pool.at(i as _, 0).cpu,
                    viewport,
                    data,
                });

                barriers.push(Self::transition_barrier(
                    d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: dst.resource.as_mut_ptr(),
                        Subresource: dst.calc_subresource(
                            r.dst_subresource.level as _,
                            (first_layer + i) as _,
                            0,
                        ),
                        StateBefore: d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
                        StateAfter: d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET,
                    },
                ));
            }
        }

        // pre barriers
        unsafe {
            self.raw
                .ResourceBarrier(barriers.len() as _, barriers.as_ptr());
        }
        // execute blits
        self.set_internal_graphics_pipeline();
        for (key, list) in instances {
            let blit = self.shared.service_pipes.get_blit_2d_color(key);
            unsafe {
                self.raw
                    .IASetPrimitiveTopology(d3dcommon::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                self.raw.set_pipeline_state(blit.pipeline);
                self.raw.set_graphics_root_signature(blit.signature);
                self.raw
                    .set_graphics_root_descriptor_table(0, srv_heap.start_gpu_descriptor());
            }
            for inst in list {
                let scissor = d3d12::D3D12_RECT {
                    left: inst.viewport.TopLeftX as _,
                    top: inst.viewport.TopLeftY as _,
                    right: (inst.viewport.TopLeftX + inst.viewport.Width) as _,
                    bottom: (inst.viewport.TopLeftY + inst.viewport.Height) as _,
                };
                unsafe {
                    self.raw.RSSetViewports(1, &inst.viewport);
                    self.raw.RSSetScissorRects(1, &scissor);
                    self.raw.SetGraphicsRoot32BitConstants(
                        1,
                        (mem::size_of::<internal::BlitData>() / 4) as _,
                        &inst.data as *const _ as *const _,
                        0,
                    );
                    self.raw.OMSetRenderTargets(1, &inst.rtv, TRUE, ptr::null());
                    self.raw.draw(3, 1, 0, 0);
                }
            }
        }
        // post barriers
        for bar in &mut barriers {
            let mut transition = *unsafe { bar.u.Transition_mut() };
            mem::swap(&mut transition.StateBefore, &mut transition.StateAfter);
        }
        unsafe {
            self.raw
                .ResourceBarrier(barriers.len() as _, barriers.as_ptr());
        }

        // Reset states
        unsafe {
            self.raw
                .RSSetViewports(self.viewport_cache.len() as _, self.viewport_cache.as_ptr());
            self.raw
                .RSSetScissorRects(self.scissor_cache.len() as _, self.scissor_cache.as_ptr());
            if self.primitive_topology != d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED {
                self.raw.IASetPrimitiveTopology(self.primitive_topology);
            }
        }
    }

    fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<Backend>) {
        let format = match ibv.index_type {
            IndexType::U16 => dxgiformat::DXGI_FORMAT_R16_UINT,
            IndexType::U32 => dxgiformat::DXGI_FORMAT_R32_UINT,
        };
        let location = ibv.buffer.resource.gpu_virtual_address();
        self.raw.set_index_buffer(
            location + ibv.offset,
            ibv.buffer.size_in_bytes - ibv.offset as u32,
            format,
        );
    }

    fn bind_vertex_buffers<I, T>(&mut self, first_binding: u32, buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<r::Buffer>,
    {
        // Only cache the vertex buffer views as we don't know the stride (PSO).
        assert!(first_binding as usize <= MAX_VERTEX_BUFFERS);
        for ((buffer, offset), view) in buffers
            .into_iter()
            .zip(self.vertex_buffer_views[first_binding as _..].iter_mut())
        {
            let b = buffer.borrow();
            let base = unsafe { (*b.resource).GetGPUVirtualAddress() };
            view.BufferLocation = base + offset;
            view.SizeInBytes = b.size_in_bytes - offset as u32;
        }
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        let viewports = viewports
            .into_iter()
            .map(|viewport| {
                let viewport = viewport.borrow();
                d3d12::D3D12_VIEWPORT {
                    TopLeftX: viewport.rect.x as _,
                    TopLeftY: viewport.rect.y as _,
                    Width: viewport.rect.w as _,
                    Height: viewport.rect.h as _,
                    MinDepth: viewport.depth.start,
                    MaxDepth: viewport.depth.end,
                }
            })
            .enumerate();

        for (i, viewport) in viewports {
            if i + first_viewport as usize >= self.viewport_cache.len() {
                self.viewport_cache.push(viewport);
            } else {
                self.viewport_cache[i + first_viewport as usize] = viewport;
            }
        }

        unsafe {
            self.raw
                .RSSetViewports(self.viewport_cache.len() as _, self.viewport_cache.as_ptr());
        }
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        let rects = scissors
            .into_iter()
            .map(|rect| get_rect(rect.borrow()))
            .enumerate();

        for (i, rect) in rects {
            if i + first_scissor as usize >= self.scissor_cache.len() {
                self.scissor_cache.push(rect);
            } else {
                self.scissor_cache[i + first_scissor as usize] = rect;
            }
        }

        unsafe {
            self.raw
                .RSSetScissorRects(self.scissor_cache.len() as _, self.scissor_cache.as_ptr())
        };
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        self.raw.set_blend_factor(color);
    }

    fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        assert!(!faces.is_empty());

        if !faces.is_all() {
            warn!(
                "Stencil ref values set for both faces but only one was requested ({})",
                faces.bits(),
            );
        }

        self.raw.set_stencil_reference(value as _);
    }

    fn set_stencil_read_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
    }

    fn set_stencil_write_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
    }

    fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        let (cmd_list1, hr) = self.raw.cast::<d3d12::ID3D12GraphicsCommandList1>();
        if winerror::SUCCEEDED(hr) {
            unsafe {
                cmd_list1.OMSetDepthBounds(bounds.start, bounds.end);
            }
            cmd_list1.destroy();
        } else {
            warn!("Depth bounds test is not supported");
        }
    }

    fn set_line_width(&mut self, width: f32) {
        validate_line_width(width);
    }

    fn set_depth_bias(&mut self, _depth_bias: pso::DepthBias) {
        unimplemented!()
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &r::GraphicsPipeline) {
        unsafe {
            match self.gr_pipeline.pipeline {
                Some((_, signature)) if signature == pipeline.signature => {
                    // Same root signature, nothing to do
                }
                _ => {
                    self.raw.set_graphics_root_signature(pipeline.signature);
                    self.gr_pipeline.num_parameter_slots = pipeline.num_parameter_slots;
                    self.gr_pipeline.root_constants = pipeline.constants.clone();
                    // All slots need to be rebound internally on signature change.
                    self.gr_pipeline.user_data.dirty_all();
                }
            }
            self.raw.set_pipeline_state(pipeline.raw);
            self.raw.IASetPrimitiveTopology(pipeline.topology);
            self.primitive_topology = pipeline.topology;
        };

        self.active_bindpoint = BindPoint::Graphics { internal: false };
        self.gr_pipeline.pipeline = Some((pipeline.raw, pipeline.signature));
        self.vertex_bindings_remap = pipeline.vertex_bindings;

        if let Some(ref vp) = pipeline.baked_states.viewport {
            self.set_viewports(0, iter::once(vp));
        }
        if let Some(ref rect) = pipeline.baked_states.scissor {
            self.set_scissors(0, iter::once(rect));
        }
        if let Some(color) = pipeline.baked_states.blend_color {
            self.set_blend_constants(color);
        }
        if let Some(ref bounds) = pipeline.baked_states.depth_bounds {
            self.set_depth_bounds(bounds.clone());
        }
    }

    fn bind_graphics_descriptor_sets<'a, I, J>(
        &mut self,
        layout: &r::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<r::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        self.active_descriptor_heaps = self.gr_pipeline
            .bind_descriptor_sets(layout, first_set, sets, offsets);
        self.bind_descriptor_heaps();
    }

    fn bind_compute_pipeline(&mut self, pipeline: &r::ComputePipeline) {
        match self.comp_pipeline.pipeline {
            Some((_, signature)) if signature == pipeline.signature => {
                // Same root signature, nothing to do
            }
            _ => {
                self.raw.set_compute_root_signature(pipeline.signature);
                self.comp_pipeline.num_parameter_slots = pipeline.num_parameter_slots;
                self.comp_pipeline.root_constants = pipeline.constants.clone();
                // All slots need to be rebound internally on signature change.
                self.comp_pipeline.user_data.dirty_all();
            }
        }
        self.raw.set_pipeline_state(pipeline.raw);

        self.active_bindpoint = BindPoint::Compute;
        self.comp_pipeline.pipeline = Some((pipeline.raw, pipeline.signature));
    }

    fn bind_compute_descriptor_sets<I, J>(
        &mut self,
        layout: &r::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<r::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        self.active_descriptor_heaps = self.comp_pipeline
            .bind_descriptor_sets(layout, first_set, sets, offsets);
        self.bind_descriptor_heaps();
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        self.set_compute_bind_point();
        self.raw.dispatch(count);
    }

    fn dispatch_indirect(&mut self, buffer: &r::Buffer, offset: buffer::Offset) {
        self.set_compute_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.shared.signatures.dispatch.as_mut_ptr(),
                1,
                buffer.resource.as_mut_ptr(),
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }

    fn fill_buffer<R>(&mut self, buffer: &r::Buffer, range: R, data: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        assert!(
            buffer.clear_uav.is_some(),
            "Buffer needs to be created with usage `TRANSFER_DST`"
        );
        let bytes_per_unit = 4;
        let start = *range.start().unwrap_or(&0) as i32;
        let end = *range.end().unwrap_or(&(buffer.size_in_bytes as u64)) as i32;
        if start % 4 != 0 || end % 4 != 0 {
            warn!("Fill buffer bounds have to be multiples of 4");
        }
        let rect = d3d12::D3D12_RECT {
            left: start / bytes_per_unit,
            top: 0,
            right: end / bytes_per_unit,
            bottom: 1,
        };

        // Insert barrier for `COPY_DEST` to `UNORDERED_ACCESS` as we use
        // `TRANSFER_WRITE` for all clear commands.
        let pre_barrier = Self::transition_barrier(d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
            pResource: buffer.resource.as_mut_ptr(),
            Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            StateBefore: d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
            StateAfter: d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        });
        unsafe { self.raw.ResourceBarrier(1, &pre_barrier) };

        error!("fill_buffer currently unimplemented");
        // TODO: GPU handle must be in the current heap. Atm we use a CPU descriptor heap for allocation
        //       which is not shader visible.
        /*
        let handle = buffer.clear_uav.unwrap();
        unsafe {
            self.raw.ClearUnorderedAccessViewUint(
                handle.gpu,
                handle.cpu,
                buffer.resource,
                &[data as UINT; 4],
                1,
                &rect as *const _,
            );
        }
        */

        let post_barrier = Self::transition_barrier(d3d12::D3D12_RESOURCE_TRANSITION_BARRIER {
            pResource: buffer.resource.as_mut_ptr(),
            Subresource: d3d12::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            StateBefore: d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            StateAfter: d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
        });
        unsafe { self.raw.ResourceBarrier(1, &post_barrier) };
    }

    fn update_buffer(&mut self, _buffer: &r::Buffer, _offset: buffer::Offset, _data: &[u8]) {
        unimplemented!()
    }

    fn copy_buffer<T>(&mut self, src: &r::Buffer, dst: &r::Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::BufferCopy>,
    {
        // copy each region
        for region in regions {
            let region = region.borrow();
            unsafe {
                self.raw.CopyBufferRegion(
                    dst.resource.as_mut_ptr(),
                    region.dst as _,
                    src.resource.as_mut_ptr(),
                    region.src as _,
                    region.size as _,
                );
            }
        }

        // TODO: Optimization: Copy whole resource if possible
    }

    fn copy_image<T>(
        &mut self,
        src: &r::Image,
        _: image::Layout,
        dst: &r::Image,
        _: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageCopy>,
    {
        let mut src_image = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: src.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };
        let mut dst_image = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: dst.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };

        let device = self.shared.service_pipes.device.clone();
        let src_desc = src.surface_type.desc();
        let dst_desc = dst.surface_type.desc();
        assert_eq!(src_desc.bits, dst_desc.bits);
        //Note: Direct3D 10.1 enables copies between prestructured-typed textures
        // and block-compressed textures of the same bit widths.
        let do_alias = src.surface_type != dst.surface_type
            && src_desc.is_compressed() == dst_desc.is_compressed();

        if do_alias {
            // D3D12 only permits changing the channel type for copies,
            // similarly to how it allows the views to be created.

            // create an aliased resource to the source
            let mut alias = ptr::null_mut();
            let desc = d3d12::D3D12_RESOURCE_DESC {
                Format: dst.descriptor.Format,
                ..src.descriptor.clone()
            };
            let (heap_ptr, offset) = match src.place {
                r::Place::SwapChain => {
                    error!("Unable to copy from a swapchain image with format conversion: {:?} -> {:?}",
                        src.descriptor.Format, dst.descriptor.Format);
                    return;
                }
                r::Place::Heap { ref raw, offset } => (raw.as_mut_ptr(), offset),
            };
            assert_eq!(winerror::S_OK, unsafe {
                device.CreatePlacedResource(
                    heap_ptr,
                    offset,
                    &desc,
                    d3d12::D3D12_RESOURCE_STATE_COMMON,
                    ptr::null(),
                    &d3d12::ID3D12Resource::uuidof(),
                    &mut alias,
                )
            });
            src_image.pResource = alias as _;
            self.retained_resources
                .push(bal_dx12::native::Resource::from_raw(alias as _));

            // signal the aliasing transition
            let sub_barrier = d3d12::D3D12_RESOURCE_ALIASING_BARRIER {
                pResourceBefore: src.resource.as_mut_ptr(),
                pResourceAfter: src_image.pResource,
            };
            let mut barrier = d3d12::D3D12_RESOURCE_BARRIER {
                Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            unsafe {
                *barrier.u.Aliasing_mut() = sub_barrier;
                self.raw.ResourceBarrier(1, &barrier as *const _);
            }
        }

        for region in regions {
            let r = region.borrow();
            debug_assert_eq!(
                r.src_subresource.layers.len(),
                r.dst_subresource.layers.len()
            );
            let src_box = d3d12::D3D12_BOX {
                left: r.src_offset.x as _,
                top: r.src_offset.y as _,
                right: (r.src_offset.x + r.extent.width as i32) as _,
                bottom: (r.src_offset.y + r.extent.height as i32) as _,
                front: r.src_offset.z as _,
                back: (r.src_offset.z + r.extent.depth as i32) as _,
            };

            for (src_layer, dst_layer) in r.src_subresource
                .layers
                .clone()
                .zip(r.dst_subresource.layers.clone())
            {
                *unsafe { src_image.u.SubresourceIndex_mut() } =
                    src.calc_subresource(r.src_subresource.level as _, src_layer as _, 0);
                *unsafe { dst_image.u.SubresourceIndex_mut() } =
                    dst.calc_subresource(r.dst_subresource.level as _, dst_layer as _, 0);
                unsafe {
                    self.raw.CopyTextureRegion(
                        &dst_image,
                        r.dst_offset.x as _,
                        r.dst_offset.y as _,
                        r.dst_offset.z as _,
                        &src_image,
                        &src_box,
                    );
                }
            }
        }

        if do_alias {
            // signal the aliasing transition - back to the original
            let sub_barrier = d3d12::D3D12_RESOURCE_ALIASING_BARRIER {
                pResourceBefore: src_image.pResource,
                pResourceAfter: src.resource.as_mut_ptr(),
            };
            let mut barrier = d3d12::D3D12_RESOURCE_BARRIER {
                Type: d3d12::D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                Flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
                u: unsafe { mem::zeroed() },
            };
            unsafe {
                *barrier.u.Aliasing_mut() = sub_barrier;
                self.raw.ResourceBarrier(1, &barrier as *const _);
            }
        }
    }

    fn copy_buffer_to_image<T>(
        &mut self,
        buffer: &r::Buffer,
        image: &r::Image,
        _: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
        assert!(self.copies.is_empty());

        let dst_image = bal_dx12::copy::Image {
            levels: image.kind.num_levels(),
            layers: image.kind.num_layers(),
            bytes_per_block: image.bytes_per_block as _,
            block_dim: (image.block_dim.0 as _, image.block_dim.1 as _),
        };

        for region in regions {
            let r = region.borrow();
            let image_copy = bal_dx12::copy::BufferImageCopy {
                buffer_offset: r.buffer_offset,
                buffer_width: r.buffer_width,
                buffer_height: r.buffer_height,
                image_offset: r.image_offset,
                image_extent: r.image_extent,
                image_level: r.image_layers.level,
                image_layers: r.image_layers.layers.clone(),
            };
            bal_dx12::copy::split_buffer_copy(&mut self.copies, &image_copy, dst_image);
        }

        if self.copies.is_empty() {
            return;
        }

        let mut src = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: buffer.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            u: unsafe { mem::zeroed() },
        };
        let mut dst = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: image.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };

        for c in self.copies.drain(..) {
            let src_box = d3d12::D3D12_BOX {
                left: c.buf_offset.x as u32,
                top: c.buf_offset.y as u32,
                right: c.buf_offset.x as u32 + c.copy_extent.width,
                bottom: c.buf_offset.y as u32 + c.copy_extent.height,
                front: c.buf_offset.z as u32,
                back: c.buf_offset.z as u32 + c.copy_extent.depth,
            };
            let footprint = d3d12::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                Offset: c.footprint_offset,
                Footprint: d3d12::D3D12_SUBRESOURCE_FOOTPRINT {
                    Format: image.descriptor.Format,
                    Width: c.footprint.width,
                    Height: c.footprint.height,
                    Depth: c.footprint.depth,
                    RowPitch: c.row_pitch,
                },
            };
            unsafe {
                *src.u.PlacedFootprint_mut() = footprint;
                *dst.u.SubresourceIndex_mut() = c.img_subresource;
                self.raw.CopyTextureRegion(
                    &dst,
                    c.img_offset.x as _,
                    c.img_offset.y as _,
                    c.img_offset.z as _,
                    &src,
                    &src_box,
                );
            }
        }
    }

    fn copy_image_to_buffer<T>(
        &mut self,
        image: &r::Image,
        _: image::Layout,
        buffer: &r::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
        assert!(self.copies.is_empty());

        // TODO: function for these
        let dst_image = bal_dx12::copy::Image {
            levels: image.kind.num_levels(),
            layers: image.kind.num_layers(),
            bytes_per_block: image.bytes_per_block as _,
            block_dim: (image.block_dim.0 as _, image.block_dim.1 as _),
        };

        for region in regions {
            let r = region.borrow();
            let image_copy = bal_dx12::copy::BufferImageCopy {
                buffer_offset: r.buffer_offset,
                buffer_width: r.buffer_width,
                buffer_height: r.buffer_height,
                image_offset: r.image_offset,
                image_extent: r.image_extent,
                image_level: r.image_layers.level,
                image_layers: r.image_layers.layers.clone(),
            };
            bal_dx12::copy::split_buffer_copy(&mut self.copies, &image_copy, dst_image);
        }

        if self.copies.is_empty() {
            return;
        }

        let mut src = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: image.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            u: unsafe { mem::zeroed() },
        };
        let mut dst = d3d12::D3D12_TEXTURE_COPY_LOCATION {
            pResource: buffer.resource.as_mut_ptr(),
            Type: d3d12::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            u: unsafe { mem::zeroed() },
        };

        for c in self.copies.drain(..) {
            let src_box = d3d12::D3D12_BOX {
                left: c.img_offset.x as u32,
                top: c.img_offset.y as u32,
                right: c.img_offset.x as u32 + c.copy_extent.width,
                bottom: c.img_offset.y as u32 + c.copy_extent.height,
                front: c.img_offset.z as u32,
                back: c.img_offset.z as u32 + c.copy_extent.depth,
            };
            let footprint = d3d12::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                Offset: c.footprint_offset,
                Footprint: d3d12::D3D12_SUBRESOURCE_FOOTPRINT {
                    Format: image.descriptor.Format,
                    Width: c.footprint.width,
                    Height: c.footprint.height,
                    Depth: c.footprint.depth,
                    RowPitch: c.row_pitch,
                },
            };
            unsafe {
                *dst.u.PlacedFootprint_mut() = footprint;
                *src.u.SubresourceIndex_mut() = c.img_subresource;
                self.raw.CopyTextureRegion(
                    &dst,
                    c.buf_offset.x as _,
                    c.buf_offset.y as _,
                    c.buf_offset.z as _,
                    &src,
                    &src_box,
                );
            }
        }
    }

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        self.set_graphics_bind_point();
        self.raw.draw(
            vertices.end - vertices.start,
            instances.end - instances.start,
            vertices.start,
            instances.start,
        );
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        self.set_graphics_bind_point();
        self.raw.draw_indexed(
            indices.end - indices.start,
            instances.end - instances.start,
            indices.start,
            base_vertex,
            instances.start,
        );
    }

    fn draw_indirect(
        &mut self,
        buffer: &r::Buffer,
        offset: buffer::Offset,
        draw_count: DrawCount,
        stride: u32,
    ) {
        assert_eq!(stride, 16);
        self.set_graphics_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.shared.signatures.draw.as_mut_ptr(),
                draw_count,
                buffer.resource.as_mut_ptr(),
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &r::Buffer,
        offset: buffer::Offset,
        draw_count: DrawCount,
        stride: u32,
    ) {
        assert_eq!(stride, 20);
        self.set_graphics_bind_point();
        unsafe {
            self.raw.ExecuteIndirect(
                self.shared.signatures.draw_indexed.as_mut_ptr(),
                draw_count,
                buffer.resource.as_mut_ptr(),
                offset,
                ptr::null_mut(),
                0,
            );
        }
    }

    fn begin_query(&mut self, query: query::Query<Backend>, flags: query::QueryControl) {
        let query_ty = match query.pool.ty {
            bal_dx12::native::query::HeapType::Occlusion => {
                if flags.contains(query::QueryControl::PRECISE) {
                    self.occlusion_query = Some(OcclusionQuery::Precise(query.id));
                    d3d12::D3D12_QUERY_TYPE_OCCLUSION
                } else {
                    // Default to binary occlusion as it might be faster due to early depth/stencil
                    // tests.
                    self.occlusion_query = Some(OcclusionQuery::Binary(query.id));
                    d3d12::D3D12_QUERY_TYPE_BINARY_OCCLUSION
                }
            }
            bal_dx12::native::query::HeapType::Timestamp => {
                panic!("Timestap queries are issued via ")
            }
            bal_dx12::native::query::HeapType::PipelineStatistics => {
                self.pipeline_stats_query = Some(query.id);
                d3d12::D3D12_QUERY_TYPE_PIPELINE_STATISTICS
            }
            _ => unreachable!(),
        };

        unsafe {
            self.raw
                .BeginQuery(query.pool.raw.as_mut_ptr(), query_ty, query.id);
        }
    }

    fn end_query(&mut self, query: query::Query<Backend>) {
        let id = query.id;
        let query_ty = match query.pool.ty {
            bal_dx12::native::query::HeapType::Occlusion
                if self.occlusion_query == Some(OcclusionQuery::Precise(id)) =>
            {
                self.occlusion_query = None;
                d3d12::D3D12_QUERY_TYPE_OCCLUSION
            }
            bal_dx12::native::query::HeapType::Occlusion
                if self.occlusion_query == Some(OcclusionQuery::Binary(id)) =>
            {
                self.occlusion_query = None;
                d3d12::D3D12_QUERY_TYPE_BINARY_OCCLUSION
            }
            bal_dx12::native::query::HeapType::PipelineStatistics
                if self.pipeline_stats_query == Some(id) =>
            {
                self.pipeline_stats_query = None;
                d3d12::D3D12_QUERY_TYPE_PIPELINE_STATISTICS
            }
            _ => panic!("Missing `begin_query` call for query: {:?}", query),
        };

        unsafe {
            self.raw.EndQuery(query.pool.raw.as_mut_ptr(), query_ty, id);
        }
    }

    fn reset_query_pool(&mut self, _pool: &r::QueryPool, _queries: Range<query::QueryId>) {
        // Nothing to do here
        // vkCmdResetQueryPool sets the queries to `unavailable` but the specification
        // doesn't state an affect on the `active` state. Every queries at the end of the command
        // buffer must be made inactive, which can only be done with EndQuery.
        // Therefore, every `begin_query` must follow a `end_query` state, the resulting values
        // after calling are undefined.
    }

    fn write_timestamp(&mut self, _: pso::PipelineStage, query: query::Query<Backend>) {
        unsafe {
            self.raw.EndQuery(
                query.pool.raw.as_mut_ptr(),
                d3d12::D3D12_QUERY_TYPE_TIMESTAMP,
                query.id,
            );
        }
    }

    fn push_graphics_constants(
        &mut self,
        layout: &r::PipelineLayout,
        _stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    ) {
        Self::push_constants(&mut self.gr_pipeline.user_data, layout, offset, constants);
    }

    fn push_compute_constants(
        &mut self,
        layout: &r::PipelineLayout,
        offset: u32,
        constants: &[u32],
    ) {
        Self::push_constants(&mut self.comp_pipeline.user_data, layout, offset, constants);
    }

    fn execute_commands<I>(&mut self, buffers: I)
    where
        I: IntoIterator,
        I::Item: Borrow<CommandBuffer>,
    {
        for _cmd_buf in buffers {
            error!("TODO: execute_commands");
        }
    }
}
