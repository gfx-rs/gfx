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
use std::ops::Deref;

#[derive(Clone, Debug, Hash)]
pub struct ShaderLib {
    pub shaders: BTreeMap<pso::EntryPoint, ComPtr<winapi::ID3DBlob>>,
}

unsafe impl Send for ShaderLib {}
unsafe impl Sync for ShaderLib {}

#[derive(Clone, Debug, Hash)]
pub struct Pipeline {
    pub inner: ComPtr<winapi::ID3D12PipelineState>,
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Clone, Debug, Hash)]
pub struct PipelineSignature {
    pub inner: ComPtr<winapi::ID3D12RootSignature>,
}
unsafe impl Send for PipelineSignature {}
unsafe impl Sync for PipelineSignature {}

pub struct CommandBuffer {
    pub inner: ComPtr<winapi::ID3D12GraphicsCommandList>,
}

pub struct GeneralCommandBuffer(CommandBuffer);
impl Deref for GeneralCommandBuffer {
    type Target = CommandBuffer;
    fn deref(&self) -> &CommandBuffer {
        &self.0
    }
}

pub struct GraphicsCommandBuffer(CommandBuffer);
impl Deref for GraphicsCommandBuffer {
    type Target = CommandBuffer;
    fn deref(&self) -> &CommandBuffer {
        &self.0
    }
}

pub struct ComputeCommandBuffer(CommandBuffer);
impl Deref for ComputeCommandBuffer {
    type Target = CommandBuffer;
    fn deref(&self) -> &CommandBuffer {
        &self.0
    }
}

pub struct TransferCommandBuffer(CommandBuffer);
impl Deref for TransferCommandBuffer {
    type Target = CommandBuffer;
    fn deref(&self) -> &CommandBuffer {
        &self.0
    }
}

pub struct SubpassCommandBuffer(CommandBuffer);
impl Deref for SubpassCommandBuffer {
    type Target = CommandBuffer;
    fn deref(&self) -> &CommandBuffer {
        &self.0
    }
}
