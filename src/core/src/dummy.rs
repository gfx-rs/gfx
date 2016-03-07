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

use {Capabilities, Device, Resources, SubmitInfo, IndexType, VertexCount};
use {draw, pso, shade, target, tex};
use state as s;

/// Dummy device which does minimal work, just to allow testing
/// gfx-rs apps for compilation.
pub struct DummyDevice {
    capabilities: Capabilities,
}

/// Dummy resources phantom type
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
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
    type Fence                = ();
}

impl DummyDevice {
    /// Create a new dummy device
    pub fn new() -> DummyDevice {
        let caps = Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: false,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
        };
        DummyDevice {
            capabilities: caps,
        }
    }
}

/// Dummy command buffer, which ignores all the calls.
pub struct DummyCommandBuffer;
impl draw::CommandBuffer<DummyResources> for DummyCommandBuffer {
    fn clone_empty(&self) -> DummyCommandBuffer { DummyCommandBuffer }
    fn reset(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<DummyResources>) {}
    fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<DummyResources>]) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: pso::ResourceViewSet<DummyResources>) {}
    fn bind_unordered_views(&mut self, _: pso::UnorderedViewSet<DummyResources>) {}
    fn bind_samplers(&mut self, _: pso::SamplerSet<DummyResources>) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<DummyResources>) {}
    fn bind_index(&mut self, _: (), _: IndexType) {}
    fn set_scissor(&mut self, _: target::Rect) {}
    fn set_ref_values(&mut self, _: s::RefValues) {}
    fn update_buffer(&mut self, _: (), _: draw::DataPointer, _: usize) {}
    fn update_texture(&mut self, _: (), _: tex::Kind, _: Option<tex::CubeFace>,
                      _: draw::DataPointer, _: tex::RawImageInfo) {}
    fn clear_color(&mut self, _: (), _: draw::ClearColor) {}
    fn clear_depth_stencil(&mut self, _: (), _: Option<target::Depth>,
                           _: Option<target::Stencil>) {}
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: VertexCount, _: VertexCount,
                         _: VertexCount, _: draw::InstanceOption) {}
}

impl Device for DummyDevice {
    type Resources = DummyResources;
    type CommandBuffer = DummyCommandBuffer;

    fn get_capabilities<'a>(&'a self) -> &'a Capabilities {
        &self.capabilities
    }
    fn reset_state(&mut self) {}
    fn submit(&mut self, _: SubmitInfo<Self>) {}
    fn cleanup(&mut self) {}
}
