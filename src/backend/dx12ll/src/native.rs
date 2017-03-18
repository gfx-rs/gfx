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

use core::pso;
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
pub struct RenderPass;
unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

#[derive(Clone, Debug, Hash)]
pub struct GraphicsPipeline {
    pub inner: ComPtr<winapi::ID3D12PipelineState>,
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

#[derive(Clone, Debug, Hash)]
pub struct Buffer {
    pub resource: ComPtr<winapi::ID3D12Resource>,
    pub size: u32,
}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Clone, Debug, Hash)]
pub struct Image {
    pub resource: ComPtr<winapi::ID3D12Resource>,
}
unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Clone, Debug)]
pub struct RenderTargetView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Clone, Debug)]
pub struct DepthStencilView {
    pub handle: winapi::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[derive(Debug)]
pub struct Fence;

#[derive(Debug)]
pub struct Semaphore;

#[derive(Debug)]
pub struct FrameBuffer;
