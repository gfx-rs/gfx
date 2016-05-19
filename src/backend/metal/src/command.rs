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

use cocoa::foundation::NSRange;

use gfx_core::{draw, pso, shade, state, target, tex};
use gfx_core::{IndexType, VertexCount};
use gfx_core::{MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS,
               MAX_RESOURCE_VIEWS, MAX_UNORDERED_VIEWS,
               MAX_SAMPLERS, MAX_COLOR_TARGETS};

use gfx_core::shade::Stage;

use {Resources, InputLayout, Buffer, Texture, Pipeline, Program};

use native;
use native::{Rtv, Srv, Dsv};

use metal::*;

use std::ptr;

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
    BindPipeline(Pipeline),
    BindIndex(Buffer),
    BindVertexBuffers([native::Buffer; MAX_VERTEX_ATTRIBUTES], [u64; MAX_VERTEX_ATTRIBUTES], [u64; MAX_VERTEX_ATTRIBUTES]),
    BindConstantBuffers(shade::Stage, [native::Buffer; MAX_CONSTANT_BUFFERS]),
    BindShaderResources(shade::Stage, [native::Srv; MAX_RESOURCE_VIEWS]),
    BindSamplers(shade::Stage, [native::Sampler; MAX_SAMPLERS]),
    BindPixelTargets([native::Rtv; MAX_COLOR_TARGETS], native::Dsv),
    SetViewport(MTLViewport),
    SetScissor(MTLScissorRect),
    SetBlend([f32; 4], u64),

    // TODO: can we skip storing these? should have no side-effect to process
    //       directly
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture, tex::Kind, Option<tex::CubeFace>, DataPointer, tex::RawImageInfo),
    // GenerateMips(native::Srv),
    ClearColor(native::Rtv, [f32; 4]),
    ClearDepthStencil(native::Dsv, f32, u8),
}

pub enum Draw {
    Normal(u64, u64),
    Instanced(u64, u64, u64, u64),
    Indexed(u64, u64, u64),
    IndexedInstanced(u64, u64, u64, u64, u64),
}

unsafe impl Send for Command {}

struct Cache {
    targets: Option<pso::PixelTargetSet<Resources>>,
    clear: draw::ClearColor
}

unsafe impl Send for Cache {}

impl Cache {
    fn new() -> Cache {
        Cache {
            targets: None,
            clear: draw::ClearColor::Float([0.0f32; 4])
        }
    }
}

pub struct CommandBuffer {
    mtl_queue: MTLCommandQueue,
    mtl_buf: MTLCommandBuffer,
    render_pass_descriptor: MTLRenderPassDescriptor,
    render_encoder: MTLRenderCommandEncoder,

    buf: Vec<Command>,
    data: DataBuffer,

    cache: Cache,
    encoding: bool
}

impl CommandBuffer {
    pub fn new(queue: MTLCommandQueue) -> Self {
        CommandBuffer {
            mtl_queue: queue,
            mtl_buf: queue.new_command_buffer(),
            render_pass_descriptor: MTLRenderPassDescriptor::new(),
            render_encoder: MTLRenderCommandEncoder::nil(),
            buf: Vec::new(),
            data: DataBuffer::new(),
            cache: Cache::new(),
            encoding: false
        }
    }

    pub fn commit(&mut self, drawable: CAMetalDrawable) {
        if self.encoding {
            self.render_encoder.end_encoding();
            self.encoding = false;
        }

        self.mtl_buf.present_drawable(drawable);
        self.mtl_buf.commit();
        self.mtl_buf.wait_until_completed();

        unsafe {
            self.mtl_buf.release();
            self.render_encoder.release();
        }

        self.mtl_buf = MTLCommandBuffer::nil();
        self.render_encoder = MTLRenderCommandEncoder::nil();
    }

    pub fn render_encoder(&mut self) -> Option<MTLRenderCommandEncoder> {
        if self.render_encoder.is_null() {
            if let Some(targets) = self.cache.targets {
                let render_pass_descriptor = MTLRenderPassDescriptor::new();

                for i in 0..MAX_COLOR_TARGETS {
                    if let Some(color) = targets.colors[i] {
                        let attachment = render_pass_descriptor.color_attachments().object_at(i);
                        attachment.set_texture(unsafe { *(color.0) });

                        attachment.set_store_action(MTLStoreAction::Store);
                        attachment.set_load_action(MTLLoadAction::Clear);

                        if let draw::ClearColor::Float(vals) = self.cache.clear {
                            attachment.set_clear_color(MTLClearColor::new(
                                vals[0] as f64,
                                vals[1] as f64,
                                vals[2] as f64,
                                vals[3] as f64));
                        }
                    }
                }

                let enc = self.mtl_buf.new_render_command_encoder(render_pass_descriptor);
                self.render_encoder = enc;
                self.encoding = true;
                unsafe {
                    render_pass_descriptor.release();
                }

                Some(enc)
            } else {
                None
            }
        } else {
            Some(self.render_encoder)
        }
    }

    fn draw(&mut self, draw: Draw) {
        if self.mtl_buf.is_null() {
            self.mtl_buf = self.mtl_queue.new_command_buffer();
        }

        let encoder = self.render_encoder().unwrap();

        for &cmd in self.buf.iter() {
            match cmd {
                Command::BindPipeline(pso) => {
                    encoder.set_render_pipeline_state(pso.pipeline);
                },
                Command::BindIndex(buf) => {
                },
                Command::BindVertexBuffers(bufs, offsets, indices) => {
                    for i in 0..MAX_VERTEX_ATTRIBUTES {
                        encoder.set_vertex_buffer(indices[i], offsets[i], bufs[i].0);
                    }
                },
                Command::BindConstantBuffers(stage, cbufs) => {
                    if let Stage::Vertex = stage {
                        for i in 0..MAX_CONSTANT_BUFFERS {
                            if !cbufs[i].0.is_null() {
                                encoder.set_vertex_buffer(i as u64, 0, cbufs[i].0);
                            }
                        }
                    }

                    if let Stage::Pixel = stage {
                        for i in 0..MAX_CONSTANT_BUFFERS {
                            if !cbufs[i].0.is_null() {
                                encoder.set_fragment_buffer(i as u64, 0, cbufs[i].0);
                            }
                        }
                    }
                },
                Command::BindShaderResources(stage, srvs) => {
                },
                Command::BindSamplers(stage, samplers) => {
                },
                Command::BindPixelTargets(rtvs, dsv) => {
                },
                Command::SetViewport(viewport) => {
                    encoder.set_viewport(viewport);
                },
                Command::SetScissor(rect) => {
                    encoder.set_scissor_rect(rect);
                },
                Command::SetBlend(blend, mask) => {
                    // TODO: do stencil mask

                    encoder.set_blend_color(blend[0], blend[1], blend[2], blend[3]);
                },
                Command::UpdateBuffer(buf, data, offset) => {

                },
                Command::UpdateTexture(tex, kind, face, data, info) => {
                },
                // GenerateMips(native::Srv),
                Command::ClearColor(target, value) => {
                },
                Command::ClearDepthStencil(target, depth, stencil) => {
                },
            }
        }

        match draw {
            Draw::Normal(count, start) => {
                encoder.draw_primitives(MTLPrimitiveType::Triangle, start, count)
            },
            Draw::Instanced(count, ninst, start, offset) => {
                encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, start, count, ninst);
            },
            Draw::Indexed(count, start, base) => {},
            Draw::IndexedInstanced(count, ninst, start, base, offset) => {}
        }

        self.buf.clear();
    }
}

impl draw::CommandBuffer<Resources> for CommandBuffer {
    fn clone_empty(&self) -> Self {
        unimplemented!()
    }

    fn reset(&mut self) {

    }

    fn bind_pipeline_state(&mut self, pso: Pipeline) {
        self.buf.push(Command::BindPipeline(pso));
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        let mut buffers = [native::Buffer(MTLBuffer::nil()); MAX_VERTEX_ATTRIBUTES];
        let mut offsets = [0; MAX_VERTEX_ATTRIBUTES];
        let mut indices = [0; MAX_VERTEX_ATTRIBUTES];

        for i in 0 .. MAX_VERTEX_ATTRIBUTES {
            if let Some((buffer, offset)) = vbs.0[i] {
                buffers[i] = buffer.0;
                offsets[i] = offset as u64;
                indices[i] = i as u64;
            }
        }
        self.buf.push(Command::BindVertexBuffers(buffers, offsets, indices));
    }

    fn bind_constant_buffers(&mut self, cbs: &[pso::ConstantBufferParam<Resources>]) {
        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mut buffers = [native::Buffer(MTLBuffer::nil()); MAX_CONSTANT_BUFFERS];
            let mask = stage.into();
            let mut count = 0;
            for cbuf in cbs.iter() {
                if cbuf.1.contains(mask) {
                    buffers[cbuf.2 as usize] = (cbuf.0).0;
                    count += 1;
                }
            }
            if count != 0 {
                self.buf.push(Command::BindConstantBuffers(stage, buffers));
            }
        }
    }

    fn bind_global_constant(&mut self, gc: shade::Location, value: shade::UniformValue) {
        unimplemented!()
    }

    fn bind_resource_views(&mut self, rvs: &[pso::ResourceViewParam<Resources>]) {
        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mut views = [native::Srv(ptr::null_mut()); MAX_RESOURCE_VIEWS];
            let mask = stage.into();
            let mut count = 0;
            for view in rvs.iter() {
                if view.1.contains(mask) {
                    views[view.2 as usize] = view.0;
                    count += 1;
                }
            }
            if count != 0 {
                self.buf.push(Command::BindShaderResources(stage, views));
            }
        }
    }

    fn bind_unordered_views(&mut self, uvs: &[pso::UnorderedViewParam<Resources>]) {
        // TODO: UAVs
    }

    fn bind_samplers(&mut self, ss: &[pso::SamplerParam<Resources>]) {
        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mut samplers = [native::Sampler(MTLSamplerState::nil()); MAX_SAMPLERS];
            let mask = stage.into();
            let mut count = 0;
            for sm in ss.iter() {
                if sm.1.contains(mask) {
                    samplers[sm.2 as usize] = sm.0;
                    count += 1;
                }
            }
            if count != 0 {
                self.buf.push(Command::BindSamplers(stage, samplers));
            }
        }
    }

    fn bind_pixel_targets(&mut self, targets: pso::PixelTargetSet<Resources>) {
        if let Some(targets) = self.cache.targets {
            self.cache.targets = Some(targets);

            unsafe {
                if self.encoding {
                    self.render_encoder.end_encoding();
                    self.encoding = false;
                }
                self.render_encoder.release();
                self.render_encoder = MTLRenderCommandEncoder::nil();
            }
        } else {
            self.cache.targets = Some(targets);
        }
    }

    fn bind_index(&mut self, buf: Buffer, idx_type: IndexType) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn set_scissor(&mut self, rect: target::Rect) {
        self.buf.push(Command::SetScissor(MTLScissorRect {
            x: rect.x as u64,
            y: rect.y as u64,
            width: (rect.x + rect.w) as u64,
            height: (rect.y + rect.h) as u64,
        }));
    }

    fn set_ref_values(&mut self, vals: state::RefValues) {
        if vals.stencil.0 != vals.stencil.1 {
            error!("Unable to set different stencil ref values for front ({}) and back ({})",
                vals.stencil.0, vals.stencil.1);
        }
        // TODO: blend/stencil
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        let contents = (buf.0).0.contents();

        unsafe {
            let dst = (contents as *mut u8).offset(offset as isize);
            ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());

            // (buf.0).0.invalidate_range(NSRange::new(offset as u64, data.len() as u64));
        }
        // let ptr = self.data.add(data);
        // self.buf.push(Command::UpdateBuffer(buf, ptr, offset));
    }

    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>,
                      data: &[u8], info: tex::RawImageInfo) {
        let ptr = self.data.add(data);
        self.buf.push(Command::UpdateTexture(tex, kind, face, ptr, info));
    }

    fn generate_mipmap(&mut self, srv: Srv) {
        unimplemented!()
    }

    fn clear_color(&mut self, target: Rtv, value: draw::ClearColor) {
        self.cache.clear = value;
    }

    fn clear_depth_stencil(&mut self, target: Dsv,
                           depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        self.buf.push(Command::ClearDepthStencil(
            target,
            depth.unwrap_or_default() as f32,
            stencil.unwrap_or_default() as u8
        ));
    }

    fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: draw::InstanceOption) {
        self.draw(match instances {
            Some((ninst, offset)) => Draw::Instanced(
                count as u64, ninst as u64, start as u64, offset as u64),
            None => Draw::Normal(count as u64, start as u64),
        });
    }

    fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                         base: VertexCount, instances: draw::InstanceOption) {
        self.draw(match instances {
            Some((ninst, offset)) => Draw::IndexedInstanced(
                count as u64, ninst as u64, start as u64, base as u64, offset as u64),
            None => Draw::Indexed(count as u64, start as u64, base as u64),
        });
    }
}
