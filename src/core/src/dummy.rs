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

use {Capabilities, Device, Resources, SubmitInfo};
use {AttributeSlot, ColorSlot, ConstantBufferSlot, ResourceViewSlot};
use {IndexType, Primitive, VertexCount};
use {attrib, draw, pso, shade, target, tex};
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
    fn new() -> DummyCommandBuffer { DummyCommandBuffer }
    fn clear(&mut self) {}
    fn bind_program(&mut self, _: ()) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<DummyResources>) {}
    fn bind_constant_buffers(&mut self, _: pso::ConstantBufferSet<DummyResources>) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: pso::ResourceViewSet<DummyResources>) {}
    fn bind_unordered_views(&mut self, _: pso::UnorderedViewSet<DummyResources>) {}
    fn bind_samplers(&mut self, _: pso::SamplerSet<DummyResources>) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<DummyResources>) {}
    fn bind_array_buffer(&mut self, _: ()) {}
    fn bind_attribute(&mut self, _: AttributeSlot, _: (), _: attrib::Format) {}
    fn bind_index(&mut self, _: ()) {}
    fn bind_frame_buffer(&mut self, _: draw::Access, _: (), _: draw::Gamma) {}
    fn unbind_target(&mut self, _: draw::Access, _: draw::Target) {}
    fn bind_target_surface(&mut self, _: draw::Access, _: draw::Target, _: ()) {}
    fn bind_target_texture(&mut self, _: draw::Access, _: draw::Target, _: (),
                           _: target::Level, _: Option<target::Layer>) {}
    fn bind_uniform_block(&mut self, _: ConstantBufferSlot, _: ()) {}
    fn bind_texture(&mut self, _: ResourceViewSlot, _: tex::Kind, _: (), _: Option<()>) {}
    fn set_draw_color_buffers(&mut self, _: ColorSlot) {}
    fn set_rasterizer(&mut self, _: s::Rasterizer) {}
    fn set_viewport(&mut self, _: target::Rect) {}
    fn set_scissor(&mut self, _: Option<target::Rect>) {}
    fn set_depth_stencil(&mut self, _: Option<s::Depth>, _: Option<s::Stencil>,
                         _: s::CullFace) {}
    fn set_blend(&mut self, _: ColorSlot, _: Option<s::Blend>) {}
    fn set_ref_values(&mut self, _: s::RefValues) {}
    fn update_buffer(&mut self, _: (), _: draw::DataPointer, _: usize) {}
    fn update_texture(&mut self, _: tex::Kind, _: (), _: tex::ImageInfo,
                      _: draw::DataPointer) {}
    fn set_primitive(&mut self, _: Primitive) {}
    fn call_clear(&mut self, _: target::ClearData, _: target::Mask) {}
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: IndexType,
                         _: VertexCount, _: VertexCount,
                         _: VertexCount, _: draw::InstanceOption) {}
    fn call_blit(&mut self, _: target::Rect, _: target::Rect,
                 _: target::Mirror, _: target::Mask) {}
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
