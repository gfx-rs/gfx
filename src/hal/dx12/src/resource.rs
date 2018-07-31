use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::shared::minwindef::UINT;
use winapi::um::d3d12;

use bal_dx12;
use bal_dx12::native::query;
use hal::{format, image, pass, pso, DescriptorPool as HalDescriptorPool};
use range_alloc::RangeAllocator;
use root_constants::RootConstant;
use {Backend, MAX_VERTEX_BUFFERS};

use std::collections::BTreeMap;
use std::ops::Range;

// ShaderModule is either a precompiled if the source comes from HLSL or
// the SPIR-V module doesn't contain specialization constants or push constants
// because they need to be adjusted on pipeline creation.
#[derive(Debug, Hash)]
pub enum ShaderModule {
    Compiled(BTreeMap<String, bal_dx12::native::Blob>),
    Spirv(Vec<u8>),
}
unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

#[derive(Debug, Hash, Clone)]
pub struct BarrierDesc {
    pub(crate) attachment_id: pass::AttachmentId,
    pub(crate) states: Range<d3d12::D3D12_RESOURCE_STATES>,
    pub(crate) flags: d3d12::D3D12_RESOURCE_BARRIER_FLAGS,
}

impl BarrierDesc {
    pub(crate) fn new(
        attachment_id: pass::AttachmentId,
        states: Range<d3d12::D3D12_RESOURCE_STATES>,
    ) -> Self {
        BarrierDesc {
            attachment_id,
            states,
            flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_NONE,
        }
    }

    pub(crate) fn split(self) -> Range<Self> {
        BarrierDesc {
            flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY,
            ..self.clone()
        }..BarrierDesc {
            flags: d3d12::D3D12_RESOURCE_BARRIER_FLAG_END_ONLY,
            ..self
        }
    }
}

#[derive(Debug, Hash, Clone)]
pub struct SubpassDesc {
    pub(crate) color_attachments: Vec<pass::AttachmentRef>,
    pub(crate) depth_stencil_attachment: Option<pass::AttachmentRef>,
    pub(crate) input_attachments: Vec<pass::AttachmentRef>,
    pub(crate) resolve_attachments: Vec<pass::AttachmentRef>,
    pub(crate) pre_barriers: Vec<BarrierDesc>,
    pub(crate) post_barriers: Vec<BarrierDesc>,
}

impl SubpassDesc {
    /// Check if an attachment is used by this sub-pass.
    //Note: preserved attachment are not considered used.
    pub(crate) fn is_using(&self, at_id: pass::AttachmentId) -> bool {
        self.color_attachments
            .iter()
            .chain(self.depth_stencil_attachment.iter())
            .chain(self.input_attachments.iter())
            .chain(self.resolve_attachments.iter())
            .any(|&(id, _)| id == at_id)
    }
}

#[derive(Debug, Hash, Clone)]
pub struct RenderPass {
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) subpasses: Vec<SubpassDesc>,
    pub(crate) post_barriers: Vec<BarrierDesc>,
}

// Indirection layer attribute -> remap -> binding.
//
// Required as vulkan allows attribute offsets larger than the stride.
// Storing the stride specified in the pipeline required for vertex buffer binding.
#[derive(Copy, Clone, Debug)]
pub struct VertexBinding {
    // Map into the specified bindings on pipeline creation.
    pub mapped_binding: usize,
    pub stride: UINT,
    // Additional offset to rebase the attributes.
    pub offset: u32,
}

#[derive(Debug)]
pub struct GraphicsPipeline {
    pub(crate) raw: bal_dx12::native::PipelineState,
    pub(crate) signature: bal_dx12::native::RootSignature, // weak-ptr, owned by `PipelineLayout`
    pub(crate) num_parameter_slots: usize, // signature parameter slots, see `PipelineLayout`
    pub(crate) topology: d3d12::D3D12_PRIMITIVE_TOPOLOGY,
    pub(crate) constants: Vec<RootConstant>,
    pub(crate) vertex_bindings: [Option<VertexBinding>; MAX_VERTEX_BUFFERS],
    pub(crate) baked_states: pso::BakedStates,
}
unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) raw: bal_dx12::native::PipelineState,
    pub(crate) signature: bal_dx12::native::RootSignature, // weak-ptr, owned by `PipelineLayout`
    pub(crate) num_parameter_slots: usize, // signature parameter slots, see `PipelineLayout`
    pub(crate) constants: Vec<RootConstant>,
}

unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}

bitflags! {
    pub struct SetTableTypes: u8 {
        const SRV_CBV_UAV = 0x1;
        const SAMPLERS = 0x2;
    }
}

pub const SRV_CBV_UAV: SetTableTypes = SetTableTypes::SRV_CBV_UAV;
pub const SAMPLERS: SetTableTypes = SetTableTypes::SAMPLERS;

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub(crate) raw: bal_dx12::native::RootSignature,
    // Storing for each associated descriptor set layout, which tables we created
    // in the root signature. This is required for binding descriptor sets.
    pub(crate) tables: Vec<SetTableTypes>,
    // Disjunct, sorted vector of root constant ranges.
    pub(crate) root_constants: Vec<RootConstant>,
    // Number of parameter slots in this layout, can be larger than number of tables.
    // Required for updating the root signature when flusing user data.
    pub(crate) num_parameter_slots: usize,
}
unsafe impl Send for PipelineLayout {}
unsafe impl Sync for PipelineLayout {}

#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub(crate) attachments: Vec<ImageView>,
    // Number of layers in the render area. Required for subpass resolves.
    pub(crate) layers: image::Layer,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    pub(crate) resource: bal_dx12::native::Resource,
    pub(crate) size_in_bytes: u32,
    #[derivative(Debug = "ignore")]
    pub(crate) clear_uav: Option<bal_dx12::native::CpuDescriptor>,
}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Copy, Clone, Derivative)]
#[derivative(Debug)]
pub struct BufferView {
    // Descriptor handle for uniform texel buffers.
    #[derivative(Debug = "ignore")]
    pub(crate) handle_srv: bal_dx12::native::CpuDescriptor,
    // Descriptor handle for storage texel buffers.
    #[derivative(Debug = "ignore")]
    pub(crate) handle_uav: bal_dx12::native::CpuDescriptor,
}
unsafe impl Send for BufferView {}
unsafe impl Sync for BufferView {}

#[derive(Clone)]
pub enum Place {
    SwapChain,
    Heap {
        raw: bal_dx12::native::Heap,
        offset: u64,
    },
}

#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct Image {
    pub(crate) resource: bal_dx12::native::Resource,
    #[derivative(Debug = "ignore")]
    pub(crate) place: Place,
    pub(crate) surface_type: format::SurfaceType,
    pub(crate) kind: image::Kind,
    pub(crate) usage: image::Usage,
    pub(crate) storage_flags: image::StorageFlags,
    #[derivative(Debug = "ignore")]
    pub(crate) descriptor: d3d12::D3D12_RESOURCE_DESC,
    pub(crate) bytes_per_block: u8,
    // Dimension of a texel block (compressed formats).
    pub(crate) block_dim: (u8, u8),
    #[derivative(Debug = "ignore")]
    pub(crate) clear_cv: Vec<bal_dx12::native::CpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) clear_dv: Vec<bal_dx12::native::CpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) clear_sv: Vec<bal_dx12::native::CpuDescriptor>,
}
unsafe impl Send for Image {}
unsafe impl Sync for Image {}

impl Image {
    /// Get `SubresourceRange` of the whole image.
    pub fn to_subresource_range(&self, aspects: format::Aspects) -> image::SubresourceRange {
        image::SubresourceRange {
            aspects,
            levels: 0..self.descriptor.MipLevels as _,
            layers: 0..self.kind.num_layers(),
        }
    }

    pub fn calc_subresource(&self, mip_level: UINT, layer: UINT, plane: UINT) -> UINT {
        mip_level
            + (layer * self.descriptor.MipLevels as UINT)
            + (plane * self.descriptor.MipLevels as UINT * self.kind.num_layers() as UINT)
    }
}

#[derive(Copy, Derivative, Clone)]
#[derivative(Debug)]
pub struct ImageView {
    #[derivative(Debug = "ignore")]
    pub(crate) resource: bal_dx12::native::Resource, // weak-ptr owned by image.
    #[derivative(Debug = "ignore")]
    pub(crate) handle_srv: Option<bal_dx12::native::CpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) handle_rtv: Option<bal_dx12::native::CpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) handle_dsv: Option<bal_dx12::native::CpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) handle_uav: Option<bal_dx12::native::CpuDescriptor>,
    // Required for attachment resolves.
    pub(crate) dxgi_format: DXGI_FORMAT,
    pub(crate) num_levels: image::Level,
    pub(crate) mip_levels: (image::Level, image::Level),
    pub(crate) layers: (image::Layer, image::Layer),
    pub(crate) kind: image::Kind,
}
unsafe impl Send for ImageView {}
unsafe impl Sync for ImageView {}

impl ImageView {
    pub fn calc_subresource(&self, mip_level: UINT, layer: UINT) -> UINT {
        mip_level + (layer * self.num_levels as UINT)
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Sampler {
    #[derivative(Debug = "ignore")]
    pub(crate) handle: bal_dx12::native::CpuDescriptor,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub(crate) bindings: Vec<pso::DescriptorSetLayoutBinding>,
}

#[derive(Debug)]
pub struct Fence {
    pub(crate) raw: bal_dx12::native::Fence,
}
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Debug)]
pub struct Semaphore {
    pub(crate) raw: bal_dx12::native::Fence,
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Memory {
    pub(crate) heap: bal_dx12::native::Heap,
    pub(crate) type_id: usize,
    pub(crate) size: u64,
    // Buffer containing the whole memory for mapping (only for host visible heaps)
    pub(crate) resource: Option<bal_dx12::native::Resource>,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

#[derive(Debug)]
pub struct DescriptorRange {
    pub(crate) handle: DualHandle,
    pub(crate) ty: pso::DescriptorType,
    pub(crate) handle_size: u64,
    pub(crate) count: u64,
}

impl DescriptorRange {
    pub(crate) fn at(&self, index: u64) -> bal_dx12::native::CpuDescriptor {
        assert!(index < self.count);
        let ptr = self.handle.cpu.ptr + (self.handle_size * index) as usize;
        bal_dx12::native::CpuDescriptor { ptr }
    }
}

#[derive(Debug, Default)]
pub struct DescriptorBindingInfo {
    pub(crate) count: u64,
    pub(crate) view_range: Option<DescriptorRange>,
    pub(crate) sampler_range: Option<DescriptorRange>,
    pub(crate) is_uav: bool,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DescriptorSet {
    // Required for binding at command buffer
    #[derivative(Debug = "ignore")]
    pub(crate) heap_srv_cbv_uav: bal_dx12::native::DescriptorHeap,
    #[derivative(Debug = "ignore")]
    pub(crate) heap_samplers: bal_dx12::native::DescriptorHeap,

    pub(crate) binding_infos: Vec<DescriptorBindingInfo>,

    #[derivative(Debug = "ignore")]
    pub(crate) first_gpu_sampler: Option<bal_dx12::native::GpuDescriptor>,
    #[derivative(Debug = "ignore")]
    pub(crate) first_gpu_view: Option<bal_dx12::native::GpuDescriptor>,
}

// TODO: is this really safe?
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}

impl DescriptorSet {
    pub fn srv_cbv_uav_gpu_start(&self) -> bal_dx12::native::GpuDescriptor {
        self.heap_srv_cbv_uav.start_gpu_descriptor()
    }

    pub fn sampler_gpu_start(&self) -> bal_dx12::native::GpuDescriptor {
        self.heap_samplers.start_gpu_descriptor()
    }
}

#[derive(Copy, Clone, Derivative)]
#[derivative(Debug)]
pub struct DualHandle {
    #[derivative(Debug = "ignore")]
    pub(crate) cpu: bal_dx12::native::CpuDescriptor,
    #[derivative(Debug = "ignore")]
    pub(crate) gpu: bal_dx12::native::GpuDescriptor,
    /// How large the block allocated to this handle is.
    pub(crate) size: u64,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DescriptorHeap {
    #[derivative(Debug = "ignore")]
    pub(crate) raw: bal_dx12::native::DescriptorHeap,
    pub(crate) handle_size: u64,
    pub(crate) total_handles: u64,
    pub(crate) start: DualHandle,
    pub(crate) range_allocator: RangeAllocator<u64>,
}

impl DescriptorHeap {
    pub(crate) fn at(&self, index: u64, size: u64) -> DualHandle {
        assert!(index < self.total_handles);
        DualHandle {
            cpu: bal_dx12::native::CpuDescriptor {
                ptr: self.start.cpu.ptr + (self.handle_size * index) as usize,
            },
            gpu: bal_dx12::native::GpuDescriptor {
                ptr: self.start.gpu.ptr + self.handle_size * index,
            },
            size,
        }
    }

    pub(crate) unsafe fn destroy(&self) {
        self.raw.destroy();
    }
}

/// Slice of an descriptor heap, which is allocated for a pool.
/// Pools will create descriptor sets inside this slice.
#[derive(Debug)]
pub struct DescriptorHeapSlice {
    pub(crate) heap: bal_dx12::native::DescriptorHeap, // Weak reference, owned by descriptor heap.
    pub(crate) start: DualHandle,
    pub(crate) handle_size: u64,
    pub(crate) range_allocator: RangeAllocator<u64>,
}

impl DescriptorHeapSlice {
    pub(crate) fn alloc_handles(&mut self, count: u64) -> Option<DualHandle> {
        self.range_allocator
            .allocate_range(count)
            .ok()
            .map(|range| DualHandle {
                cpu: bal_dx12::native::CpuDescriptor {
                    ptr: self.start.cpu.ptr + (self.handle_size * range.start) as usize,
                },
                gpu: bal_dx12::native::GpuDescriptor {
                    ptr: self.start.gpu.ptr + (self.handle_size * range.start) as u64,
                },
                size: count,
            })
    }

    /// Free handles previously given out by this `DescriptorHeapSlice`.  Do not use this with handles not given out by this `DescriptorHeapSlice`.
    pub(crate) fn free_handles(&mut self, handle: DualHandle) {
        let start = (handle.gpu.ptr - self.start.gpu.ptr) / self.handle_size;
        let handle_range = start..start + handle.size as u64;
        self.range_allocator.free_range(handle_range);
    }

    pub(crate) fn clear(&mut self) {
        self.range_allocator.reset();
    }
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub(crate) heap_srv_cbv_uav: DescriptorHeapSlice,
    pub(crate) heap_sampler: DescriptorHeapSlice,
    pub(crate) pools: Vec<pso::DescriptorRangeDesc>,
    pub(crate) max_size: u64,
}
unsafe impl Send for DescriptorPool {}
unsafe impl Sync for DescriptorPool {}

impl HalDescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(
        &mut self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, pso::AllocationError> {
        let mut binding_infos = Vec::new();
        let mut first_gpu_sampler = None;
        let mut first_gpu_view = None;

        for binding in &layout.bindings {
            let HeapProperties {
                has_view,
                has_sampler,
                is_uav,
            } = HeapProperties::from(binding.ty);
            while binding_infos.len() <= binding.binding as usize {
                binding_infos.push(DescriptorBindingInfo::default());
            }
            binding_infos[binding.binding as usize] = DescriptorBindingInfo {
                count: binding.count as _,
                view_range: if has_view {
                    let handle = self.heap_srv_cbv_uav
                        .alloc_handles(binding.count as u64)
                        .ok_or(pso::AllocationError::OutOfPoolMemory)?;
                    if first_gpu_view.is_none() {
                        first_gpu_view = Some(handle.gpu);
                    }
                    Some(DescriptorRange {
                        handle,
                        ty: binding.ty,
                        count: binding.count as _,
                        handle_size: self.heap_srv_cbv_uav.handle_size,
                    })
                } else {
                    None
                },
                sampler_range: if has_sampler {
                    let handle = self.heap_sampler
                        .alloc_handles(binding.count as u64)
                        .ok_or(pso::AllocationError::OutOfPoolMemory)?;
                    if first_gpu_sampler.is_none() {
                        first_gpu_sampler = Some(handle.gpu);
                    }
                    Some(DescriptorRange {
                        handle,
                        ty: binding.ty,
                        count: binding.count as _,
                        handle_size: self.heap_sampler.handle_size,
                    })
                } else {
                    None
                },
                is_uav,
            };
        }

        Ok(DescriptorSet {
            heap_srv_cbv_uav: self.heap_srv_cbv_uav.heap.clone(),
            heap_samplers: self.heap_sampler.heap.clone(),
            binding_infos,
            first_gpu_sampler,
            first_gpu_view,
        })
    }

    fn free_sets<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSet>,
    {
        for descriptor_set in descriptor_sets {
            for binding_info in &descriptor_set.binding_infos {
                if let Some(ref view_range) = binding_info.view_range {
                    if HeapProperties::from(view_range.ty).has_view {
                        self.heap_srv_cbv_uav.free_handles(view_range.handle);
                    }
                }
                if let Some(ref sampler_range) = binding_info.sampler_range {
                    if HeapProperties::from(sampler_range.ty).has_sampler {
                        self.heap_sampler.free_handles(sampler_range.handle);
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        self.heap_srv_cbv_uav.clear();
        self.heap_sampler.clear();
    }
}

struct HeapProperties {
    has_view: bool,
    has_sampler: bool,
    is_uav: bool,
}

impl HeapProperties {
    pub fn new(has_view: bool, has_sampler: bool, is_uav: bool) -> Self {
        HeapProperties {
            has_view,
            has_sampler,
            is_uav,
        }
    }

    /// Returns DescriptorType properties for DX12.
    fn from(ty: pso::DescriptorType) -> HeapProperties {
        match ty {
            pso::DescriptorType::Sampler => HeapProperties::new(false, true, false),
            pso::DescriptorType::CombinedImageSampler => HeapProperties::new(true, true, false),
            pso::DescriptorType::InputAttachment
            | pso::DescriptorType::SampledImage
            | pso::DescriptorType::UniformTexelBuffer
            | pso::DescriptorType::UniformBufferDynamic
            | pso::DescriptorType::UniformBuffer => HeapProperties::new(true, false, false),
            pso::DescriptorType::StorageImage
            | pso::DescriptorType::StorageTexelBuffer
            | pso::DescriptorType::StorageBufferDynamic
            | pso::DescriptorType::StorageBuffer => HeapProperties::new(true, false, true),
        }
    }
}

#[derive(Debug)]
pub struct QueryPool {
    pub(crate) raw: bal_dx12::native::QueryHeap,
    pub(crate) ty: query::HeapType,
}

unsafe impl Send for QueryPool {}
unsafe impl Sync for QueryPool {}
