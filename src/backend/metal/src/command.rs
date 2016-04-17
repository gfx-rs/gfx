// Copyright 2016 The Gfx-rs Developers.
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

#![allow(missing_docs)]

use gfx_core::tex::{Kind, CubeFace, RawImageInfo};

use {Resources, InputLayout, Buffer, Texture, Pipeline, Program};

use metal::*;

/// The place of some data in the data buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DataPointer {
    offset: u32,
    size: u32,
}

pub struct DataBuffer(Vec<u8>);

impl DataBuffer {
    /// Create a new empty data buffer.
    pub fn new() -> DataBuffer {
        DataBuffer(Vec::new())
    }

    /// Reset the contents.
    pub fn reset(&mut self) {
        self.0.clear();
    }

    /// Copy a given vector slice into the buffer.
    pub fn add(&mut self, data: &[u8]) -> DataPointer {
        self.0.extend_from_slice(data);
        DataPointer {
            offset: (self.0.len() - data.len()) as u32,
            size: data.len() as u32,
        }
    }

    /// Return a reference to a stored data object.
    pub fn get(&self, ptr: DataPointer) -> &[u8] {
        &self.0[ptr.offset as usize .. (ptr.offset + ptr.size) as usize]
    }
}

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    BindProgram(Program),
    BindInputLayout(InputLayout),
    BindIndex(Buffer),
    BindVertexBuffers([MTLBuffer; MAX_VERTEX_ATTRIBUTES], [u64; MAX_VERTEX_ATTRIBUTES], [u64; MAX_VERTEX_ATTRIBUTES]),
    // BindConstantBuffers(shade::Stage, [native::Buffer; MAX_CONSTANT_BUFFERS]),
    // BindShaderResources(shade::Stage, [native::Srv; MAX_RESOURCE_VIEWS]),
    // BindSamplers(shade::Stage, [MTLSamplerState; MAX_SAMPLERS]),
    // BindPixelTargets([native::Rtv; MAX_COLOR_TARGETS], native::Dsv),
    SetPrimitive(MTLTriangleFillMode),
    SetViewport(MTLViewport),
    SetScissor(MTLScissorRect),
    // SetRasterizer(*const ID3D11RasterizerState),
    SetDepthStencil(MTLDepthStencilState, u32),
    // SetBlend(*const ID3D11BlendState, [f32; 4]),
    // resource updates
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture, Kind, Option<CubeFace>, DataPointer, RawImageInfo),
    // GenerateMips(native::Srv),
    // drawing
    // ClearColor(native::Rtv, [f32; 4]),
    // ClearDepthStencil(native::Dsv, D3D11_CLEAR_FLAG, FLOAT, UINT8),
    Draw(u64, u64),
    // DrawInstanced(UINT, UINT, UINT, UINT),
    // DrawIndexed(UINT, UINT, INT),
    // DrawIndexedInstanced(UINT, UINT, UINT, INT, UINT),
}
