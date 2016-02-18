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

use std::ptr;
use winapi::{FLOAT, INT, UINT, UINT8, DXGI_FORMAT,
             DXGI_FORMAT_R8_UINT, DXGI_FORMAT_R16_UINT, DXGI_FORMAT_R32_UINT,
             D3D11_CLEAR_FLAG, D3D11_PRIMITIVE_TOPOLOGY, D3D11_VIEWPORT, D3D11_RECT,
             ID3D11RasterizerState, ID3D11DepthStencilState, ID3D11BlendState};
use gfx_core::{draw, pso, shade, state, target, tex};
use gfx_core::{IndexType, VertexCount};
use gfx_core::{MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS, MAX_RESOURCE_VIEWS, MAX_COLOR_TARGETS};
use {native, Resources, InputLayout, Texture, Pipeline, Program};

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    BindProgram(Program),
    BindInputLayout(InputLayout),
    BindIndex(native::Buffer, DXGI_FORMAT),
    BindVertexBuffers([native::Buffer; MAX_VERTEX_ATTRIBUTES], [UINT; MAX_VERTEX_ATTRIBUTES], [UINT; MAX_VERTEX_ATTRIBUTES]),
    BindConstantBuffers(shade::Stage, [native::Buffer; MAX_CONSTANT_BUFFERS]),
    BindShaderResources(shade::Stage, [native::Srv; MAX_RESOURCE_VIEWS]),
    BindPixelTargets([native::Rtv; MAX_COLOR_TARGETS], native::Dsv),
    SetPrimitive(D3D11_PRIMITIVE_TOPOLOGY),
    SetViewport(D3D11_VIEWPORT),
    SetScissor(D3D11_RECT),
    SetRasterizer(*const ID3D11RasterizerState),
    SetDepthStencil(*const ID3D11DepthStencilState, UINT),
    SetBlend(*const ID3D11BlendState, [FLOAT; 4], UINT),
    // resource updates
    // drawing
    ClearColor(native::Rtv, [f32; 4]),
    ClearDepthStencil(native::Dsv, D3D11_CLEAR_FLAG, FLOAT, UINT8),
    Draw(UINT, UINT),
    DrawInstanced(UINT, UINT, UINT, UINT),
    DrawIndexed(UINT, UINT, INT),
    DrawIndexedInstanced(UINT, UINT, UINT, INT, UINT),
}

unsafe impl Send for Command {}

struct Cache {
    attributes: [Option<pso::AttributeDesc>; MAX_VERTEX_ATTRIBUTES],
    rasterizer: *const ID3D11RasterizerState,
    depth_stencil: *const ID3D11DepthStencilState,
    stencil_ref: UINT,
    blend: *const ID3D11BlendState,
    blend_ref: [FLOAT; 4],
}

impl Cache {
    fn new() -> Cache {
        Cache {
            attributes: [None; MAX_VERTEX_ATTRIBUTES],
            rasterizer: ptr::null(),
            depth_stencil: ptr::null(),
            stencil_ref: 0,
            blend: ptr::null(),
            blend_ref: [0.0; 4],
        }
    }
}

pub struct CommandBuffer {
    pub buf: Vec<Command>,
    cache: Cache,
}

impl CommandBuffer {
    pub fn new() -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
            cache: Cache::new(),
        }
    }

    fn flush(&mut self) {
        let sample_mask = !0; //TODO
        self.buf.push(Command::SetDepthStencil(self.cache.depth_stencil, self.cache.stencil_ref));
        self.buf.push(Command::SetBlend(self.cache.blend, self.cache.blend_ref, sample_mask));
    }
}

impl draw::CommandBuffer<Resources> for CommandBuffer {
    fn clone_empty(&self) -> CommandBuffer {
        CommandBuffer::new()
    }

    fn reset(&mut self) {
        self.buf.clear();
        self.cache = Cache::new();
    }

    fn bind_pipeline_state(&mut self, pso: Pipeline) {
        use std::mem; //temporary
        self.buf.push(Command::SetPrimitive(unsafe{mem::transmute(pso.topology)}));
        if self.cache.rasterizer != pso.rasterizer {
            self.cache.rasterizer = pso.rasterizer;
            self.buf.push(Command::SetRasterizer(pso.rasterizer));
        }
        self.cache.depth_stencil = pso.depth_stencil;
        self.cache.blend = pso.blend;
        self.buf.push(Command::BindInputLayout(pso.layout));
        self.buf.push(Command::BindProgram(pso.program));
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        let mut buffers = [native::Buffer(ptr::null_mut()); MAX_VERTEX_ATTRIBUTES];
        let mut strides = [0; MAX_VERTEX_ATTRIBUTES];
        let mut offsets = [0; MAX_VERTEX_ATTRIBUTES];
        for i in 0 .. MAX_VERTEX_ATTRIBUTES {
            match (vbs.0[i], self.cache.attributes[i]) {
                (None, Some(fm)) => {
                    error!("No vertex input provided for slot {} of format {:?}", i, fm)
                },
                (Some((buffer, offset)), Some(ref format)) => {
                    buffers[i] = buffer;
                    strides[i] = format.0.stride as UINT;
                    offsets[i] = format.0.offset as UINT + (offset as UINT);
                },
                (_, None) => (),
            }
        }
        self.buf.push(Command::BindVertexBuffers(buffers, strides, offsets));
    }

    fn bind_constant_buffers(&mut self, cbs: pso::ConstantBufferSet<Resources>) {
        for &stage in shade::STAGES.iter() {
            let mut buffers = [native::Buffer(ptr::null_mut()); MAX_CONSTANT_BUFFERS];
            let mask = stage.into();
            let mut count = 0;
            for i in 0 .. MAX_CONSTANT_BUFFERS {
                match cbs.0[i] {
                    Some((buffer, usage)) if usage.contains(mask) => {
                        buffers[i] = buffer;
                        count += 1;
                    },
                    _ => ()
                }
            }
            if count != 0 {
                self.buf.push(Command::BindConstantBuffers(stage, buffers));
            }
        }
    }

    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {
        error!("Global constants are not supported");
    }

    fn bind_resource_views(&mut self, rvs: pso::ResourceViewSet<Resources>) {
        for &stage in shade::STAGES.iter() {
            let mut views = [native::Srv(ptr::null_mut()); MAX_RESOURCE_VIEWS];
            let mask = stage.into();
            let mut count = 0;
            for i in 0 .. MAX_RESOURCE_VIEWS {
                match rvs.0[i] {
                    Some((view, usage)) if usage.contains(mask) => {
                        views[i] = view;
                        count += 1;
                    },
                    _ => ()
                }
            }
            if count != 0 {
                self.buf.push(Command::BindShaderResources(stage, views));
            }
        }
    }

    fn bind_unordered_views(&mut self, _: pso::UnorderedViewSet<Resources>) {
        unimplemented!()
    }

    fn bind_samplers(&mut self, _: pso::SamplerSet<Resources>) {
        unimplemented!()
    }

    fn bind_pixel_targets(&mut self, pts: pso::PixelTargetSet<Resources>) {
        if let (Some(ref d), Some(ref s)) = (pts.depth, pts.stencil) {
            if d != s {
                error!("Depth and stencil views have to be the same");
            }
        }
        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0, TopLeftY: 0.0,
            Width: pts.size.0 as f32, Height: pts.size.1 as f32,
            MinDepth: 0.0, MaxDepth: 1.0,
        };
        let mut colors = [native::Rtv(ptr::null_mut()); MAX_COLOR_TARGETS];
        for i in 0 .. MAX_COLOR_TARGETS {
            if let Some(c) = pts.colors[i] {
                colors[i] = c;
            }
        }
        let ds = pts.depth.unwrap_or(native::Dsv(ptr::null_mut()));
        self.buf.push(Command::BindPixelTargets(colors, ds));
        self.buf.push(Command::SetViewport(viewport));
    }

    fn bind_index(&mut self, buf: native::Buffer, itype: IndexType) {
        let format = match itype {
            IndexType::U8  => DXGI_FORMAT_R8_UINT,
            IndexType::U16 => DXGI_FORMAT_R16_UINT,
            IndexType::U32 => DXGI_FORMAT_R32_UINT,
        };
        self.buf.push(Command::BindIndex(buf, format));
    }

    fn set_scissor(&mut self, rect: target::Rect) {
        self.buf.push(Command::SetScissor(D3D11_RECT {
            left: rect.x as INT,
            top: rect.y as INT,
            right: (rect.x + rect.w) as INT,
            bottom: (rect.y + rect.h) as INT,
        }));
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        if rv.stencil.0 != rv.stencil.1 {
            error!("Unable to set different stencil ref values for front ({}) and back ({})",
                rv.stencil.0, rv.stencil.1);
        }
        self.cache.stencil_ref = rv.stencil.0 as UINT;
        self.cache.blend_ref = rv.blend;
    }

    fn update_buffer(&mut self, _: native::Buffer, _: draw::DataPointer, _: usize) {
        unimplemented!()
    }

    fn update_texture(&mut self, _: Texture, _: tex::Kind, _: Option<tex::CubeFace>,
                      _: draw::DataPointer, _: tex::RawImageInfo) {
        unimplemented!()
    }

    fn clear_color(&mut self, target: native::Rtv, value: draw::ClearColor) {
        match value {
            draw::ClearColor::Float(data) => {
                self.buf.push(Command::ClearColor(target, data));
            },
            _ => {
                error!("Unable to clear int/uint target");
            },
        }
    }

    fn clear_depth_stencil(&mut self, target: native::Dsv, depth: Option<target::Depth>,
                           stencil: Option<target::Stencil>) {
        let flags = //warning: magic constants ahead
            D3D11_CLEAR_FLAG(if depth.is_some() {1} else {0}) |
            D3D11_CLEAR_FLAG(if stencil.is_some() {2} else {0});
        self.buf.push(Command::ClearDepthStencil(target, flags,
                      depth.unwrap_or_default() as FLOAT,
                      stencil.unwrap_or_default() as UINT8
        ));
    }

    fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: draw::InstanceOption) {
        self.flush();
        self.buf.push(match instances {
            Some((ninst, offset)) => Command::DrawInstanced(
                count as UINT, ninst as UINT, start as UINT, offset as UINT),
            None => Command::Draw(count as UINT, start as UINT),
        });
    }

    fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                         base: VertexCount, instances: draw::InstanceOption) {
        self.flush();
        self.buf.push(match instances {
            Some((ninst, offset)) => Command::DrawIndexedInstanced(
                count as UINT, ninst as UINT, start as UINT, base as INT, offset as UINT),
            None => Command::DrawIndexed(count as UINT, start as UINT, base as INT),
        });
    }
}
