
use core::pass::{Attachment, AttachmentRef};
use core::pso::DescriptorSetLayoutBinding;
use core::{self, image, pso, HeapType};
use free_list;
use winapi::{self, UINT};
use wio::com::ComPtr;
use Backend;

use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug, Hash)]
pub struct ShaderModule {
    pub shaders: BTreeMap<String, *mut winapi::ID3DBlob>,
}
unsafe impl Send for ShaderModule { }
unsafe impl Sync for ShaderModule { }

#[derive(Debug, Hash, Clone)]
pub struct SubpassDesc {
    pub color_attachments: Vec<AttachmentRef>,
}

#[derive(Debug, Hash, Clone)]
pub struct RenderPass {
    pub attachments: Vec<Attachment>,
    pub subpasses: Vec<SubpassDesc>,
}

#[derive(Debug, Hash)]
pub struct GraphicsPipeline {
    pub raw: *mut winapi::ID3D12PipelineState,
    pub topology: winapi::D3D12_PRIMITIVE_TOPOLOGY,
}
unsafe impl Send for GraphicsPipeline { }
unsafe impl Sync for GraphicsPipeline { }

#[derive(Debug, Hash)]
pub struct ComputePipeline {
    pub raw: *mut winapi::ID3D12PipelineState,
}

unsafe impl Send for ComputePipeline { }
unsafe impl Sync for ComputePipeline { }

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub raw: *mut winapi::ID3D12RootSignature,
}
unsafe impl Send for PipelineLayout { }
unsafe impl Sync for PipelineLayout { }

#[derive(Debug, Hash, Clone)]
pub struct FrameBuffer {
    pub color: Vec<RenderTargetView>,
    pub depth_stencil: Vec<DepthStencilView>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Buffer {
    pub resource: *mut winapi::ID3D12Resource,
    pub size_in_bytes: u32,
    pub stride: u32,
}
unsafe impl Send for Buffer { }
unsafe impl Sync for Buffer { }

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub resource: *mut winapi::ID3D12Resource,
    pub kind: image::Kind,
    pub dxgi_format: winapi::DXGI_FORMAT,
    pub bits_per_texel: u8,
    pub levels: image::Level,
}
unsafe impl Send for Image { }
unsafe impl Sync for Image { }

impl Image {
    pub fn calc_subresource(&self, mip_level: UINT, layer: UINT) -> UINT {
        mip_level + layer * self.levels as UINT
    }
}

#[derive(Copy, Debug, Hash, Clone)]
pub struct RenderTargetView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Copy, Debug, Hash, Clone)]
pub struct DepthStencilView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub bindings: Vec<DescriptorSetLayoutBinding>,
}

#[derive(Debug)]
pub struct Fence {
    pub raw: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Debug)]
pub struct Semaphore {
    pub raw: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Heap {
    pub raw: ComPtr<winapi::ID3D12Heap>,
    pub ty: HeapType,
    pub size: u64,
    pub default_state: winapi::D3D12_RESOURCE_STATES,
}
#[derive(Debug)]
pub struct ConstantBufferView;
#[derive(Debug)]
pub struct ShaderResourceView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}
#[derive(Debug)]
pub struct UnorderedAccessView;
#[derive(Debug)]
pub struct DescriptorSet;

#[derive(Clone, Debug)]
pub struct DualHandle {
    pub cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
    pub gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct DescriptorHeap {
    pub raw: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub handle_size: u64,
    pub total_handles: u64,
    pub start: DualHandle,
    pub allocator: free_list::Allocator,
}

impl DescriptorHeap {
    pub fn at(&self, index: u64) -> DualHandle {
        assert!(index < self.total_handles);
        DualHandle {
            cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.start.cpu.ptr + self.handle_size * index },
            gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: self.start.gpu.ptr + self.handle_size * index },
        }
    }
}

#[derive(Debug)]
pub struct DescriptorCpuPool {
    pub heap: DescriptorHeap,
    pub offset: u64,
    pub size: u64,
    pub max_size: u64,
}

impl DescriptorCpuPool {
    pub fn alloc_handles(&mut self, count: u64) -> DualHandle {
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
    pub heap: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub range: Range<u64>,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub heap_srv_cbv_uav: DescriptorHeapSlice,
    pub heap_sampler: DescriptorHeapSlice,
    pub pools: Vec<pso::DescriptorRangeDesc>,
    pub max_size: u64,
}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        // TODO:
        layouts.iter().map(|_| DescriptorSet).collect()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Sampler {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}
