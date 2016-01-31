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

use {native, Resources};
use gfx_core::{draw, pso, shade, state, target, tex};
use gfx_core::{IndexType, VertexCount};
use gfx_core::{MAX_COLOR_TARGETS};
use winapi::{D3D11_VIEWPORT};

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    SetViewport(D3D11_VIEWPORT),
    // resource updates
    // drawing
    ClearColor(native::Rtv, [f32; 4]),
}

pub struct CommandBuffer {
    pub buf: Vec<Command>,
    cur_pts: pso::PixelTargetSet<Resources>,
}

impl CommandBuffer {
    pub fn new() -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
            cur_pts: pso::PixelTargetSet::new(),
        }
    }
}

impl draw::CommandBuffer<Resources> for CommandBuffer {
    fn clone_empty(&self) -> CommandBuffer { CommandBuffer::new() }
    fn reset(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Resources>) {}
    fn bind_constant_buffers(&mut self, _: pso::ConstantBufferSet<Resources>) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: pso::ResourceViewSet<Resources>) {}
    fn bind_unordered_views(&mut self, _: pso::UnorderedViewSet<Resources>) {}
    fn bind_samplers(&mut self, _: pso::SamplerSet<Resources>) {}

    fn bind_pixel_targets(&mut self, pts: pso::PixelTargetSet<Resources>) {
        //TODO: OMSetRenderTargets
        self.buf.push(Command::SetViewport(D3D11_VIEWPORT {
            TopLeftX: 0.0, TopLeftY: 0.0,
            Width: pts.size.0 as f32, Height: pts.size.1 as f32,
            MinDepth: 0.0, MaxDepth: 1.0,
        }));
        self.cur_pts = pts;
    }

    fn bind_index(&mut self, _: ()) {}
    fn set_scissor(&mut self, _: Option<target::Rect>) {}
    fn set_ref_values(&mut self, _: state::RefValues) {}
    fn update_buffer(&mut self, _: (), _: draw::DataPointer, _: usize) {}
    fn update_texture(&mut self, _: native::Texture, _: tex::Kind, _: Option<tex::CubeFace>,
                      _: draw::DataPointer, _: tex::RawImageInfo) {}

    fn clear(&mut self, set: draw::ClearSet) {
        for i in 0 .. MAX_COLOR_TARGETS {
            match (self.cur_pts.colors[i], set.0[i]) {
                (Some(target), Some(draw::ClearColor::Float(data))) => {
                    self.buf.push(Command::ClearColor(target, data));
                },
                (Some(_), Some(_)) => {
                    error!("Unable to clear int/uint surface for slot {}", i);
                },
                (None, Some(_)) => {
                    error!("Color value provided for slot {} but there is no target there", i);
                },
                (_, None) => (),
            }
        }
        //TODO: depth clear
    }

    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: IndexType,
                         _: VertexCount, _: VertexCount,
                         _: VertexCount, _: draw::InstanceOption) {}
}
