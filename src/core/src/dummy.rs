// Copyright 2015 The Gfx-rs Developers.
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

//! Dummy backend implementation to test the code for compile errors
//! outside of the graphics development environment.

use {Capabilities, Device, SubmissionResult, Resources, IndexType, VertexCount};
use {state, target, handle, mapping, pso, shade, texture};
use command::{self, AccessInfo};

/// Dummy device which does minimal work, just to allow testing
/// gfx-rs apps for compilation.
pub struct DummyDevice {
    capabilities: Capabilities,
}

/// Dummy resources phantom type
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources {}

impl Resources for DummyResources {
    type Buffer               = ();
    type Shader               = ();
    type Program              = ();
    type PipelineStateObject  = ();
    type Texture              = ();
    type ShaderResourceView   = ();
    type UnorderedAccessView  = ();
    type RenderTargetView     = ();
    type DepthStencilView     = ();
    type Sampler              = ();
    type Fence                = DummyFence;
    type Mapping              = DummyMapping;
}

/// Dummy fence that does nothing.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DummyFence;

/// Dummy mapping which will crash on use.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DummyMapping;

impl mapping::Gate<DummyResources> for DummyMapping {
    unsafe fn set<T>(&self, _index: usize, _val: T) { unimplemented!() }
    unsafe fn slice<'a, 'b, T>(&'a self, _len: usize) -> &'b [T] { unimplemented!() }
    unsafe fn mut_slice<'a, 'b, T>(&'a self, _len: usize) -> &'b mut [T] { unimplemented!() }
}

impl DummyDevice {
    /// Create a new dummy device
    pub fn new() -> DummyDevice {
        let caps = Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            max_patch_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: false,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
            copy_buffer_supported: false,
        };
        DummyDevice {
            capabilities: caps,
        }
    }
}

/// Dummy command buffer, which ignores all the calls.
pub struct DummyCommandBuffer;
impl command::Buffer<DummyResources> for DummyCommandBuffer {
    fn reset(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<DummyResources>) {}
    fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<DummyResources>]) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: &[pso::ResourceViewParam<DummyResources>]) {}
    fn bind_unordered_views(&mut self, _: &[pso::UnorderedViewParam<DummyResources>]) {}
    fn bind_samplers(&mut self, _: &[pso::SamplerParam<DummyResources>]) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<DummyResources>) {}
    fn bind_index(&mut self, _: (), _: IndexType) {}
    fn set_scissor(&mut self, _: target::Rect) {}
    fn set_ref_values(&mut self, _: state::RefValues) {}
    fn copy_buffer(&mut self, _: (), _: (),
                   _: usize, _: usize,
                   _: usize) {}
    fn copy_buffer_to_texture(&mut self,
                              _: (), _: usize,
                              _: (), _: texture::Kind,
                              _: Option<texture::CubeFace>, _: texture::RawImageInfo) {}
    fn copy_texture_to_buffer(&mut self,
                              _: (), _: texture::Kind,
                              _: Option<texture::CubeFace>, _: texture::RawImageInfo,
                              _: (), _: usize) {}
    fn update_buffer(&mut self, _: (), _: &[u8], _: usize) {}
    fn update_texture(&mut self, _: (), _: texture::Kind, _: Option<texture::CubeFace>,
                      _: &[u8], _: texture::RawImageInfo) {}
    fn generate_mipmap(&mut self, _: ()) {}
    fn clear_color(&mut self, _: (), _: command::ClearColor) {}
    fn clear_depth_stencil(&mut self, _: (), _: Option<target::Depth>,
                           _: Option<target::Stencil>) {}
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: Option<command::InstanceParams>) {}
    fn call_draw_indexed(&mut self, _: VertexCount, _: VertexCount,
                         _: VertexCount, _: Option<command::InstanceParams>) {}
}

impl Device for DummyDevice {
    type Resources = DummyResources;
    type CommandBuffer = DummyCommandBuffer;

    fn get_capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn pin_submitted_resources(&mut self, _: &handle::Manager<DummyResources>) {}
    fn submit(&mut self,
              _: &mut DummyCommandBuffer,
              _: &AccessInfo<Self::Resources>)
              -> SubmissionResult<()> {
        unimplemented!()
    }

    fn fenced_submit(&mut self,
                     _: &mut Self::CommandBuffer,
                     _: &AccessInfo<Self::Resources>,
                     _after: Option<handle::Fence<Self::Resources>>)
                     -> SubmissionResult<handle::Fence<Self::Resources>> {
        unimplemented!()
    }

    fn wait_fence(&mut self, _: &handle::Fence<Self::Resources>) {
        unimplemented!()
    }

    fn cleanup(&mut self) {}
}
