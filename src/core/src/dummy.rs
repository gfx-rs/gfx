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

#![allow(missing_docs)]

use {Capabilities, Device, Resources, SubmitInfo, IndexType, VertexCount};
use {draw, pso, shade, target};
use state as s;

///Dummy device which does minimal work, just to allow testing gfx-rs apps for
///compilation.
pub struct DummyDevice {
    capabilities: Capabilities,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources {}

impl Resources for DummyResources {
    type Buffer               = ();
    type ArrayBuffer          = ();
    type Shader               = ();
    type Program              = ();
    type PipelineStateObject  = ();
    type NewTexture           = ();
    type ShaderResourceView   = ();
    type UnorderedAccessView  = ();
    type FrameBuffer          = ();
    type Surface              = ();
    type RenderTargetView     = ();
    type DepthStencilView     = ();
    type Texture              = ();
    type Sampler              = ();
    type Fence                = ();
}

impl DummyDevice {
    pub fn new() -> DummyDevice {
        let caps = Capabilities {
            shader_model: shade::ShaderModel::Unsupported,
            max_vertex_count: 0,
            max_index_count: 0,
            max_draw_buffers: 0,
            max_texture_size: 0,
            max_vertex_attributes: 0,
            buffer_role_change_allowed: false,
            array_buffer_supported: false,
            fragment_output_supported: false,
            immutable_storage_supported: false,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            render_targets_supported: false,
            sampler_objects_supported: false,
            srgb_color_supported: false,
            uniform_block_supported: false,
            vertex_base_supported: false,
            separate_blending_slots_supported: false,
        };
        DummyDevice {
            capabilities: caps,
        }
    }
}

pub struct DummyCommandBuffer;
impl draw::CommandBuffer<DummyResources> for DummyCommandBuffer {
    fn clone_empty(&self) -> DummyCommandBuffer { DummyCommandBuffer }
    fn clear(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<DummyResources>) {}
    fn bind_constant_buffers(&mut self, _: pso::ConstantBufferSet<DummyResources>) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: pso::ResourceViewSet<DummyResources>) {}
    fn bind_unordered_views(&mut self, _: pso::UnorderedViewSet<DummyResources>) {}
    fn bind_samplers(&mut self, _: pso::SamplerSet<DummyResources>) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<DummyResources>) {}
    fn bind_index(&mut self, _: ()) {}
    fn set_scissor(&mut self, _: Option<target::Rect>) {}
    fn set_ref_values(&mut self, _: s::RefValues) {}
    fn update_buffer(&mut self, _: (), _: draw::DataPointer, _: usize) {}
    fn call_clear(&mut self, _: target::ClearData, _: target::Mask) {}
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: IndexType,
                         _: VertexCount, _: VertexCount,
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
