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

use core::{factory as f, image, pso, HeapType};
use core::pass::Attachment;
use comptr::ComPtr;
use winapi;

use std::collections::BTreeMap;

#[derive(Clone, Debug, Hash)]
pub struct ShaderLib {
    pub shaders: BTreeMap<pso::EntryPoint, ComPtr<winapi::ID3DBlob>>,
}
unsafe impl Send for ShaderLib {}
unsafe impl Sync for ShaderLib {}

#[derive(Clone, Debug, Hash)]
pub struct RenderPass {
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Hash)]
pub struct GraphicsPipeline {
    pub inner: ComPtr<winapi::ID3D12PipelineState>,
    pub topology: winapi::D3D12_PRIMITIVE_TOPOLOGY,
}
unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Clone, Debug, Hash)]
pub struct ComputePipeline {
    pub inner: ComPtr<winapi::ID3D12PipelineState>,
}
unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}

#[derive(Clone, Debug, Hash)]
pub struct PipelineLayout {
    pub inner: ComPtr<winapi::ID3D12RootSignature>,
}
unsafe impl Send for PipelineLayout {}
unsafe impl Sync for PipelineLayout {}

pub struct CommandBuffer {
    pub inner: ComPtr<winapi::ID3D12GraphicsCommandList>,
}

pub struct GeneralCommandBuffer(pub CommandBuffer);

pub struct GraphicsCommandBuffer(pub CommandBuffer);

pub struct ComputeCommandBuffer(pub CommandBuffer);

pub struct TransferCommandBuffer(pub CommandBuffer);

pub struct SubpassCommandBuffer(pub CommandBuffer);

#[derive(Debug)]
pub struct Heap {
    pub inner: ComPtr<winapi::ID3D12Heap>,
    pub ty: HeapType,
    pub size: u64,
    pub default_state: winapi::D3D12_RESOURCE_STATES,
}

#[derive(Clone, Debug, Hash)]
pub struct Buffer {
    pub resource: ComPtr<winapi::ID3D12Resource>,
    pub size_in_bytes: u32,
    pub stride: u32,
}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Clone, Debug, Hash)]
pub struct Image {
    pub resource: ComPtr<winapi::ID3D12Resource>,
    pub kind: image::Kind,
    pub dxgi_format: winapi::DXGI_FORMAT,
    pub bits_per_texel: u8,
}
unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Debug)]
pub struct Sampler {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug, Hash)]
pub struct ConstantBufferView {
    //TODO
}

#[derive(Clone, Debug)]
pub struct ShaderResourceView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug, Hash)]
pub struct UnorderedAccessView {
}


#[derive(Clone, Debug)]
pub struct RenderTargetView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Clone, Debug)]
pub struct DepthStencilView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct Semaphore {
    pub fence: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

#[derive(Debug)]
pub struct Fence {
    pub inner: ComPtr<winapi::ID3D12Fence>,
}
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Debug)]
pub struct FrameBuffer {
    pub color: Vec<RenderTargetView>,
    pub depth_stencil: Vec<DepthStencilView>,
}

#[derive(Clone, Debug)]
pub struct DualHandle {
    pub cpu: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
    pub gpu: winapi::D3D12_GPU_DESCRIPTOR_HANDLE,
}

#[derive(Clone, Debug)]
pub struct DescriptorHeap {
    pub inner: ComPtr<winapi::ID3D12DescriptorHeap>,
    pub handle_size: u64,
    pub total_handles: u64,
    pub start: DualHandle,
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
pub struct DescriptorSetPool {
    pub heap: DescriptorHeap,
    pub pools: Vec<f::DescriptorPoolDesc>,
    pub offset: u64,
    pub size: u64,
    pub max_size: u64,
}

impl DescriptorSetPool {
    pub fn alloc_handles(&mut self, count: u64) -> DualHandle {
        assert!(self.size + count <= self.max_size);
        let index = self.offset + self.size;
        self.size += count;
        self.heap.at(index)
    }
}

#[derive(Debug)]
pub struct DescriptorRange {
    pub handle: DualHandle,
    pub ty: f::DescriptorType,
    pub handle_size: u64,
    pub count: usize,
}

impl DescriptorRange {
    pub fn at(&self, index: usize) -> winapi::D3D12_CPU_DESCRIPTOR_HANDLE {
        assert!(index < self.count);
        let ptr = self.handle.cpu.ptr + self.handle_size * index as u64;
        winapi::D3D12_CPU_DESCRIPTOR_HANDLE { ptr }
    }
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub ranges: Vec<DescriptorRange>,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub bindings: Vec<f::DescriptorSetLayoutBinding>,
}

gfx_impl_resources!();
