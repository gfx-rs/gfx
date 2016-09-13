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

use gfx_core::{draw, pso, shade, state, target, tex};
use gfx_core::{IndexType, VertexCount};
use gfx_core::{MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS,
               MAX_RESOURCE_VIEWS,
               MAX_SAMPLERS, MAX_COLOR_TARGETS};


use {Resources, Buffer, Texture, Pipeline};

use encoder::MetalEncoder;

use native;
use native::{Rtv, Srv, Dsv};

use metal::*;

use std::collections::HashSet;
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
    pub fn _reset(&mut self) {
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
    pub fn _get(&self, ptr: DataPointer) -> &[u8] {
        &self.0[ptr.offset as usize..(ptr.offset + ptr.size) as usize]
    }
}

/// Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    BindPipeline(Pipeline),
    _BindIndex(Buffer),
    BindVertexBuffers([native::Buffer; MAX_VERTEX_ATTRIBUTES],
                      [u64; MAX_VERTEX_ATTRIBUTES],
                      [u64; MAX_VERTEX_ATTRIBUTES]),
    BindConstantBuffers(shade::Stage, [native::Buffer; MAX_CONSTANT_BUFFERS]),
    BindShaderResources(shade::Stage, [native::Srv; MAX_RESOURCE_VIEWS]),
    BindSamplers(shade::Stage, [native::Sampler; MAX_SAMPLERS]),
    _BindPixelTargets([native::Rtv; MAX_COLOR_TARGETS], native::Dsv),
    SetViewport(MTLViewport),
    SetScissor(MTLScissorRect),
    _SetBlend([f32; 4], u64),

    // TODO: can we skip storing these? should have no side-effect to process
    //       directly
    _UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture, tex::Kind, Option<tex::CubeFace>, DataPointer, tex::RawImageInfo),
    // GenerateMips(native::Srv),
    _ClearColor(native::Rtv, [f32; 4]),
    _ClearDepthStencil(native::Dsv, f32, u8),
}

#[derive(Clone, Copy, Debug)]
enum Draw {
    Normal(u64, u64),
    Instanced(u64, u64, u64, u64),
    Indexed(u64, u64, u64),
    IndexedInstanced(u64, u64, u64, u64, u64),
}

unsafe impl Send for Command {}

pub struct CommandBuffer {
    queue: MTLCommandQueue,
    device: MTLDevice,
    encoder: MetalEncoder,
    drawable: *mut CAMetalDrawable,
    should_restore: bool
}

unsafe impl Send for CommandBuffer {}

impl CommandBuffer {
    pub fn new(device: MTLDevice, queue: MTLCommandQueue, drawable: *mut CAMetalDrawable) -> Self {
        CommandBuffer {
            device: device,
            queue: queue,
            encoder: MetalEncoder::new(queue.new_command_buffer()),
            drawable: drawable,
            should_restore: false
        }
    }

    pub fn commit(&mut self, drawable: CAMetalDrawable) {
        self.encoder.end_encoding();
        self.encoder.commit_command_buffer(drawable, false);
        self.encoder.reset();

        self.encoder.start_command_buffer(self.queue.new_command_buffer());

        self.should_restore = false;
    }

    fn ensure_render_encoder(&mut self) {
        if !self.encoder.is_render_encoding() {
            self.encoder.begin_render_encoding();
        }

        if self.should_restore {
            self.encoder.restore_render_state();
            self.should_restore = false;
        }
    }
}

impl command::Buffer<Resources> for CommandBuffer {
    fn reset(&mut self) {
        self.encoder.reset();
    }

    fn bind_pipeline_state(&mut self, pso: Pipeline) {
        self.encoder.set_render_pipeline_state(pso.pipeline);
        self.encoder.set_front_facing_winding(pso.winding);
        self.encoder.set_cull_mode(pso.cull);
        self.encoder.set_triangle_fill_mode(pso.fill);

        self.encoder.set_depth_bias(pso.depth_bias as f32 ,
                                    pso.slope_scaled_depth_bias as f32,
                                    0f32);


        if let Some(depth_state) = pso.depth_stencil {
            self.encoder.set_depth_stencil_state(depth_state);
        }

        //self.buf.push(Command::BindPipeline(pso));
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        for i in 0..MAX_VERTEX_ATTRIBUTES {
            if let Some((buffer, offset)) = vbs.0[i] {
                self.encoder.set_vertex_buffer(i as u64, offset as u64, unsafe { *(buffer.0).0 });
            }
        }
    }

    fn bind_constant_buffers(&mut self, cbs: &[pso::ConstantBufferParam<Resources>]) {
        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mask = stage.into();
            for cb in cbs.iter() {
                if cb.1.contains(mask) {
                    if let Stage::Vertex = stage {
                        self.encoder.set_vertex_buffer(cb.2 as u64, 0, unsafe { *((cb.0).0).0 });
                    } else {
                        self.encoder.set_fragment_buffer(cb.2 as u64, 0, unsafe { *((cb.0).0).0 });
                    }
                }
            }
        }
    }

    fn bind_global_constant(&mut self, _gc: shade::Location, _value: shade::UniformValue) {
        unimplemented!()
    }

    fn bind_resource_views(&mut self, rvs: &[pso::ResourceViewParam<Resources>]) {
        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mask = stage.into();
            for view in rvs.iter() {
                if view.1.contains(mask) {
                    if let Stage::Vertex = stage {
                        self.encoder.set_vertex_texture(view.2 as u64, unsafe { *(view.0).0 });
                    } else {
                        self.encoder.set_fragment_texture(view.2 as u64, unsafe { *(view.0).0 });
                    }
                }
            }
        }
    }

    fn bind_unordered_views(&mut self, _uvs: &[pso::UnorderedViewParam<Resources>]) {
        // TODO: UAVs
    }

    fn bind_samplers(&mut self, ss: &[pso::SamplerParam<Resources>]) {
        use std::f32;

        for &stage in [Stage::Vertex, Stage::Pixel].iter() {
            let mask = stage.into();
            for sampler in ss.iter() {
                if sampler.1.contains(mask) {
                    if let Stage::Vertex = stage {
                        self.encoder.set_vertex_sampler_state(sampler.2 as u64, 0f32, f32::MAX, (sampler.0).0);
                    } else {
                        self.encoder.set_fragment_sampler_state(sampler.2 as u64, 0f32, f32::MAX, (sampler.0).0)
                    }
                }
            }
        }
    }

    fn bind_pixel_targets(&mut self, targets: pso::PixelTargetSet<Resources>) {
        // TODO(fkaa): cache here to see if we're actually changing targets!
        self.encoder.end_encoding();
        self.should_restore = true;

        let render_pass_descriptor = MTLRenderPassDescriptor::new();

        for i in 0..MAX_COLOR_TARGETS {
            if let Some(color) = targets.colors[i] {
                let attachment = render_pass_descriptor.color_attachments().object_at(i);
                attachment.set_texture(unsafe { *(color.0) });
                attachment.set_store_action(MTLStoreAction::Store);

                let clear = unsafe { *color.1 };

                if let Some(vals) = clear {
                    attachment.set_load_action(MTLLoadAction::Clear);
                    attachment.set_clear_color(
                        MTLClearColor::new(vals[0] as f64 / 255.0,
                                           vals[1] as f64 / 255.0,
                                           vals[2] as f64 / 255.0,
                                           vals[3] as f64 / 255.0)
                    );

                    // we reset the clear color to give the effect of being a
                    // "per-frame" operation
                    unsafe { *color.1 = None; }
                } else {
                    // if no clear has been specified, we simply load the
                    // previous content
                    attachment.set_load_action(MTLLoadAction::Load);
                }

                //unsafe { *color.1 = [0; 4]; }
            }
        }

        if let Some(depth) = targets.depth {
            let attachment = render_pass_descriptor.depth_attachment();
            attachment.set_texture(unsafe { *(depth.0) });

            if let Some(layer) = depth.1 {
                attachment.set_slice(layer as u64);
            }

            attachment.set_store_action(MTLStoreAction::Store);

            let clear = unsafe { *depth.2 };

            if let Some(val) = clear {
                attachment.set_load_action(MTLLoadAction::Clear);
                attachment.set_clear_depth(val as f64);

                // same here, clear should not be persistent across frames
                unsafe { *depth.2 = None; }
            } else {
                // see above
                attachment.set_load_action(MTLLoadAction::Load);
            }
        }

        self.encoder.set_render_pass_descriptor(render_pass_descriptor);
        //self.encoder.begin_render_encoding();
        //self.encoder.restore_render_state();
        self.encoder.set_viewport(MTLViewport {
            originX: 0f64,
            originY: 0f64,
            width: targets.size.0 as f64,
            height: targets.size.1 as f64,
            znear: 0f64,
            zfar: 1f64
        });
    }

    fn bind_index(&mut self, buf: Buffer, idx_type: IndexType) {
        use map::map_index_type;

        self.encoder.set_index_buffer(unsafe { *(buf.0).0 }, map_index_type(idx_type));
    }

    fn set_scissor(&mut self, rect: target::Rect) {
        // TODO(fkaa): why are getting 1x1 scissor?
        /*self.encoder.set_scissor_rect(MTLScissorRect {
            x: rect.x as u64,
            y: rect.y as u64,
            width: (rect.x + rect.w) as u64,
            height: (rect.y + rect.h) as u64,
        });*/
    }

    fn set_ref_values(&mut self, vals: state::RefValues) {
        //self.encoder.set_stencil_front_back_reference_value(vals.stencil.0, vals.stencil.1);
        /*if vals.stencil.0 != vals.stencil.1 {
            error!("Unable to set different stencil ref values for front ({}) and back ({})",
                vals.stencil.0, vals.stencil.1);
        }*/
        // TODO: blend/stencil
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        use map::{map_buffer_usage};

        debug_assert!(!unsafe { *(*(buf.0).0) }.is_null(), "Buffer must be non-nil");

        let b = unsafe { *(buf.0).0 };

        if offset == 0 && data.len() == b.length() as usize {
            unsafe {
                //b.release();
                *(buf.0).0 = self.device.new_buffer_with_data(
                    data.as_ptr() as _,
                    data.len() as _,
                    map_buffer_usage(buf.1));
            }
        } else {
            if self.encoder.is_buffer_bound(b) {
                self.encoder.end_encoding();
                //self.encoder.commit_command_buffer(CAMetalDrawable::nil(), true);
                self.should_restore = true;
            }

            let contents = b.contents();

            unsafe {
                let dst = (contents as *mut u8).offset(offset as isize);
                ptr::copy(data.as_ptr(), dst, data.len());
                //(buf.0).0.invalidate_range(NSRange::new(offset as u64, data.len() as u64));
            }
        }
    }

    fn update_texture(&mut self,
                      tex: Texture,
                      kind: tex::Kind,
                      face: Option<tex::CubeFace>,
                      data: &[u8],
                      info: tex::RawImageInfo) {
        //let ptr = self.data.add(data);
        //self.buf.push(Command::UpdateTexture(tex, kind, face, ptr, info));
    }

    fn generate_mipmap(&mut self, _srv: Srv) {
        unimplemented!()
    }

    fn clear_color(&mut self, target: Rtv, value: draw::ClearColor) {
        match value {
            draw::ClearColor::Float(val) => {
                unsafe {
                    *target.1 = Some([(val[0] * 255.0) as u8,
                                      (val[1] * 255.0) as u8,
                                      (val[2] * 255.0) as u8,
                                      (val[3] * 255.0) as u8]);
                }
            },
            _ => { unimplemented!(); }
        }
    }

    fn clear_depth_stencil(&mut self, target: Dsv,
                           depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unsafe {
            *target.2 = Some(depth.unwrap_or_default());
        }
        //self.cache.clear_depth = depth.unwrap_or_default();
        //self.cache.clear_stencil = stencil.unwrap_or_default();
    }

    fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: draw::InstanceOption) {
        self.encoder.begin_render_encoding();
        self.encoder.restore_render_state();

        match instances {
            Some((ninst, offset)) => {
                self.encoder.draw_instanced(count as u64,
                                            ninst as u64,
                                            start as u64);
            },
            None => {
                self.encoder.draw(start as u64,
                                  count as u64);
            }
        }
    }

    fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                         base: VertexCount, instances: draw::InstanceOption) {
        self.encoder.begin_render_encoding();
        self.encoder.restore_render_state();

        match instances {
            Some((ninst, offset)) => {
                self.encoder.draw_indexed_instanced(count as u64,
                                                    start as u64,
                                                    ninst as u64,
                                                    base as i64,
                                                    offset as u64);
            },
            None => {
                self.encoder.draw_indexed(count as u64,
                                          start as u64);
            }
        }
    }
}
