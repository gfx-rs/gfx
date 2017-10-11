use core::{self, image, pass, pso, MemoryType};
use free_list;
use winapi::{self, UINT};
use wio::com::ComPtr;
use Backend;

use std::collections::BTreeMap;
use std::ops::Range;


#[derive(Debug, Hash)]
pub struct ShaderModule {
    pub(crate) shaders: BTreeMap<String, *mut winapi::ID3DBlob>,
}
unsafe impl Send for ShaderModule { }
unsafe impl Sync for ShaderModule { }

#[derive(Debug, Hash, Clone)]
pub struct BarrierDesc {
    pub(crate) attachment_id: pass::AttachmentId,
    pub(crate) states: Range<winapi::D3D12_RESOURCE_STATES>,
    pub(crate) flags: winapi::D3D12_RESOURCE_BARRIER_FLAGS,
}

impl BarrierDesc {
    pub(crate) fn new(
        attachment_id: pass::AttachmentId,
        states: Range<winapi::D3D12_RESOURCE_STATES>,
    ) -> Self {
        BarrierDesc {
            attachment_id,
            states,
            flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_NONE,
        }
    }

    pub(crate) fn split(self) -> Range<Self> {
        BarrierDesc {
            flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY,
            .. self.clone()
        }
        ..
        BarrierDesc {
            flags: winapi::D3D12_RESOURCE_BARRIER_FLAG_END_ONLY,
            .. self
        }
    }
}

#[derive(Debug, Hash, Clone)]
pub struct SubpassDesc {
    pub(crate) color_attachments: Vec<pass::AttachmentRef>,
    pub(crate) depth_stencil_attachment: Option<pass::AttachmentRef>,
    pub(crate) input_attachments: Vec<pass::AttachmentRef>,
    pub(crate) pre_barriers: Vec<BarrierDesc>,
}

#[derive(Debug, Hash, Clone)]
pub struct RenderPass {
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) subpasses: Vec<SubpassDesc>,
    pub(crate) post_barriers: Vec<BarrierDesc>,
}

#[derive(Debug, Hash)]
pub struct GraphicsPipeline {
    pub(crate) raw: *mut winapi::ID3D12PipelineState,
    pub(crate) topology: winapi::D3D12_PRIMITIVE_TOPOLOGY,
}
unsafe impl Send for GraphicsPipeline { }
unsafe impl Sync for GraphicsPipeline { }

#[derive(Debug, Hash)]
pub struct ComputePipeline {
    pub(crate) raw: *mut winapi::ID3D12PipelineState,
}

unsafe impl Send for ComputePipeline { }
unsafe impl Sync for ComputePipeline { }

bitflags! {
    pub flags SetTableTypes: u8 {
        const SRV_CBV_UAV = 0x1,
        const SAMPLERS = 0x2,
    }
}

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub(crate) raw: *mut winapi::ID3D12RootSignature,
    // Storing for each associated descriptor set layout, which tables we created
    // in the root signature. This is required for binding descriptor sets.
    pub(crate) tables: Vec<SetTableTypes>,
}
unsafe impl Send for PipelineLayout { }
unsafe impl Sync for PipelineLayout { }

#[derive(Debug, Hash, Clone)]
pub struct Framebuffer {
    pub(crate) attachments: Vec<ImageView>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Buffer {
    pub(crate) resource: *mut winapi::ID3D12Resource,
    pub(crate) size_in_bytes: u32,
    pub(crate) stride: u32,
}
unsafe impl Send for Buffer { }
unsafe impl Sync for Buffer { }

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct BufferView;

#[derive(Clone, Debug, Hash)]
pub struct Image {
    pub(crate) resource: *mut winapi::ID3D12Resource,
    pub(crate) kind: image::Kind,
    pub(crate) usage: image::Usage,
    pub(crate) dxgi_format: winapi::DXGI_FORMAT,
    pub(crate) bits_per_texel: u8,
    pub(crate) num_levels: image::Level,
    pub(crate) num_layers: image::Layer,
    pub(crate) clear_cv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) clear_dv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) clear_sv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
}
unsafe impl Send for Image { }
unsafe impl Sync for Image { }

impl Image {
    /// Get `SubresourceRange` of the whole image.
    pub fn to_subresource_range(&self, aspects: image::AspectFlags) -> image::SubresourceRange {
        image::SubresourceRange {
            aspects,
            levels: 0 .. self.num_levels,
            layers: 0 .. self.num_layers,
        }
    }

    pub fn calc_subresource(&self, mip_level: UINT, layer: UINT, plane: UINT) -> UINT {
        mip_level + (layer * self.num_levels as UINT) + (plane * self.num_levels as UINT * self.num_layers as UINT)
    }
}

#[derive(Copy, Debug, Hash, Clone)]
pub struct ImageView {
    pub(crate) resource: *mut winapi::ID3D12Resource,
    pub(crate) handle_srv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) handle_rtv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) handle_dsv: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) handle_uav: Option<winapi::D3D12_CPU_DESCRIPTOR_HANDLE>,
}
unsafe impl Send for ImageView { }
unsafe impl Sync for ImageView { }

#[derive(Debug)]
pub struct Sampler {
    pub(crate) handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub(crate) bindings: Vec<pso::DescriptorSetLayoutBinding>,
}

#[derive(Debug)]
pub struct Fence {
    pub(crate) raw: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Debug)]
pub struct Semaphore {
    pub(crate) raw: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Memory {
    pub(crate) heap: ComPtr<winapi::ID3D12Heap>,
    pub(crate) ty: MemoryType,
    pub(crate) size: u64,
    pub(crate) default_state: winapi::D3D12_RESOURCE_STATES,
}

#[derive(Debug)]
pub struct DescriptorRange {
    pub(crate) handle: DualHandle,
    pub(crate) ty: pso::DescriptorType,
    pub(crate) handle_size: u64,
    pub(crate) count: usize,
}

impl DescriptorRange {
    pub(crate) fn at(&self, index: usize) -> winapi::D3D12_CPU_DESCRIPTOR_HANDLE {
        assert!(index < self.count);
        let ptr = self.handle.cpu.ptr + self.handle_size * index as u64;
        winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr }
    }
}

#[derive(Debug)]
pub enum DescriptorRangeBinding {
    Sampler(DescriptorRange),
    View(DescriptorRange),
    Empty,
}

#[derive(Debug)]
pub struct DescriptorSet {
    // Required for binding at command buffer
    pub(crate) heap_srv_cbv_uav: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub(crate) heap_samplers: ComPtr<winapi::ID3D12DescriptorHeap>,

    pub(crate) ranges: Vec<DescriptorRangeBinding>,

    pub(crate) first_gpu_sampler: Option<winapi::D3D12_GPU_DESCRIPTOR_HANDLE>,
    pub(crate) first_gpu_view: Option<winapi::D3D12_GPU_DESCRIPTOR_HANDLE>,
}

// TODO: is this really safe?
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}

#[derive(Copy, Clone, Debug)]
pub struct DualHandle {
    pub(crate) cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
    pub(crate) gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct DescriptorHeap {
    pub(crate) raw: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub(crate) handle_size: u64,
    pub(crate) total_handles: u64,
    pub(crate) start: DualHandle,
    pub(crate) allocator: free_list::Allocator,
}

impl DescriptorHeap {
    pub(crate) fn at(&self, index: u64) -> DualHandle {
        assert!(index < self.total_handles);
        DualHandle {
            cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.start.cpu.ptr + self.handle_size * index },
            gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: self.start.gpu.ptr + self.handle_size * index },
        }
    }
}

#[derive(Debug)]
pub struct DescriptorCpuPool {
    pub(crate) heap: DescriptorHeap,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) max_size: u64,
}

impl DescriptorCpuPool {
    pub(crate) fn alloc_handles(&mut self, count: u64) -> DualHandle {
        assert!(self.size + count <= self.max_size);
        let index = self.offset + self.size;
        self.size += count;
        self.heap.at(index)
    }
}

/// Slice of an descriptor heap, which is allocated for a pool.
/// Pools will create descriptor sets inside this slice.
#[derive(Debug)]
pub struct DescriptorHeapSlice {
    pub(crate) heap: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub(crate) range: Range<u64>,
    pub(crate) start: DualHandle,
    pub(crate) handle_size: u64,
    pub(crate) next: u64,
}

impl DescriptorHeapSlice {
    pub(crate) fn alloc_handles(&mut self, count: u64) -> DualHandle {
        assert!(self.next + count <= self.range.end);
        let index = self.next;
        self.next += count;
        DualHandle {
            cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.start.cpu.ptr + self.handle_size * index },
            gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: self.start.gpu.ptr + self.handle_size * index },
        }
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

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        layouts
            .iter()
            .map(|layout| {
                let mut ranges = Vec::new();
                let mut first_gpu_sampler = None;
                let mut first_gpu_view = None;

                for binding in &layout.bindings {
                    let range = match binding.ty {
                        pso::DescriptorType::Sampler => {
                            let handle = self.heap_sampler.alloc_handles(binding.count as u64);
                            if first_gpu_sampler.is_none() {
                                first_gpu_sampler = Some(handle.gpu);
                            }
                            DescriptorRangeBinding::Sampler(DescriptorRange {
                                handle,
                                ty: binding.ty,
                                count: binding.count,
                                handle_size: self.heap_sampler.handle_size,
                            })
                        },
                        _ => {
                            let handle = self.heap_srv_cbv_uav.alloc_handles(binding.count as u64);
                            if first_gpu_view.is_none() {
                                first_gpu_view = Some(handle.gpu);
                            }
                            DescriptorRangeBinding::View(DescriptorRange {
                                handle,
                                ty: binding.ty,
                                count: binding.count,
                                handle_size: self.heap_sampler.handle_size,
                            })
                        }
                    };

                    while ranges.len() <= binding.binding as usize {
                        ranges.push(DescriptorRangeBinding::Empty);
                    }
                    ranges[binding.binding as usize] = range;
                }

                DescriptorSet {
                    heap_srv_cbv_uav: self.heap_srv_cbv_uav.heap.clone(),
                    heap_samplers: self.heap_sampler.heap.clone(),
                    ranges,
                    first_gpu_sampler,
                    first_gpu_view,
                }
            })
            .collect()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}
