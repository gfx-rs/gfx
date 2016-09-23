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

use metal::*;

use std::mem;

use super::{MTL_MAX_BUFFER_BINDINGS, MTL_MAX_TEXTURE_BINDINGS,
            MTL_MAX_SAMPLER_BINDINGS};

const MTL_MAX_TEXTURE_BINDINGS_SZ: usize = MTL_MAX_TEXTURE_BINDINGS / 32;

// TODO(fkaa): can we use associated constants and `Bindings` trait instead?
const VS_IDX: usize = 0;
const FS_IDX: usize = 1;
const CS_IDX: usize = 2;

// TODO(fkaa): handle retain/release for bindings
pub struct MetalTextureBindings {
    textures: [MTLTexture; MTL_MAX_TEXTURE_BINDINGS],

    // TODO(fkaa): change to [[u32; 32]; 4] to avoid limitations of no
    //             type-level integers
    bound: [u32; MTL_MAX_TEXTURE_BINDINGS_SZ]
}

impl MetalTextureBindings {
    pub fn new() -> Self {
        MetalTextureBindings {
            textures: [MTLTexture::nil(); MTL_MAX_TEXTURE_BINDINGS],
            bound: [0; MTL_MAX_TEXTURE_BINDINGS_SZ]
        }
    }

    pub fn insert(&mut self, index: usize, texture: MTLTexture) {
        self.textures[index] = texture;
        self.bound[index / 32] |= 1 << (index % 32);
    }

    pub fn reset(&mut self) {
        self.textures = [MTLTexture::nil(); MTL_MAX_TEXTURE_BINDINGS];
        self.bound = [0; MTL_MAX_TEXTURE_BINDINGS_SZ];
    }
}

#[derive(Copy, Clone)]
pub struct MetalBufferBindings {
    buffers: [MTLBuffer; MTL_MAX_BUFFER_BINDINGS],
    offsets: [u64; MTL_MAX_BUFFER_BINDINGS],
    bound: u32
}

impl MetalBufferBindings {
    pub fn new() -> Self {
        MetalBufferBindings {
            buffers: [MTLBuffer::nil(); MTL_MAX_BUFFER_BINDINGS],
            offsets: [0u64; MTL_MAX_BUFFER_BINDINGS],
            bound: 0u32
        }
    }

    pub fn insert(&mut self, index: usize, offset: u64, buffer: MTLBuffer) {
        self.buffers[index] = buffer;
        self.offsets[index] = offset;
        self.bound |= 1 << index;
    }

    pub fn is_bound(&self, buffer: MTLBuffer) -> bool {
        match self.buffers.iter().position(|&b| b == buffer) {
            Some(idx) => self.bound & (1 << idx) != 0,
            None => false
        }
    }

    pub fn invalidate(&mut self, buffer: MTLBuffer) {
        self.buffers.iter().position(|&b| b == buffer).map(|idx | { self.bound &= !(1 << idx) });
    }

    pub fn reset(&mut self) {
        self.buffers = [MTLBuffer::nil(); MTL_MAX_BUFFER_BINDINGS];
        self.offsets = [0u64; MTL_MAX_BUFFER_BINDINGS];
        self.bound = 0;
    }

}

#[derive(Copy, Clone)]
pub struct MetalSamplerBindings {
    samplers: [MTLSamplerState; MTL_MAX_SAMPLER_BINDINGS],
    bound: u16
}

impl MetalSamplerBindings {
    pub fn new() -> Self {
        MetalSamplerBindings {
            samplers: [MTLSamplerState::nil(); MTL_MAX_SAMPLER_BINDINGS],
            bound: 0u16
        }
    }

    pub fn insert(&mut self, index: usize, sampler: MTLSamplerState) {
        self.samplers[index] = sampler;
        self.bound |= 1 << index;
    }

    pub fn reset(&mut self) {
        self.samplers = [MTLSamplerState::nil(); MTL_MAX_SAMPLER_BINDINGS];
        self.bound = 0;
    }
}

pub struct MetalEncoderCache {
    render: MTLRenderPipelineState,

    scissor: Option<MTLScissorRect>,
    viewport: Option<MTLViewport>,
    front_face_winding: Option<MTLWinding>,
    cull_mode: Option<MTLCullMode>,
    fill_mode: Option<MTLTriangleFillMode>,
    depth_clip_mode: Option<MTLDepthClipMode>,
    depth_bias: Option<(f32, f32)>,
    blend_color: Option<[f32; 4]>,
    stencil_front_back_ref: Option<(u32, u32)>,

    index_buffer: Option<(MTLBuffer, MTLIndexType)>,
    depth_stencil: MTLDepthStencilState,

    texture_bindings: [MetalTextureBindings; 3],
    buffer_bindings: [MetalBufferBindings; 3],
    sampler_bindings: [MetalSamplerBindings; 3],
}

impl MetalEncoderCache {
    pub fn new() -> Self {
        let tbs = {
            let mut tbs: [MetalTextureBindings; 3] = unsafe { mem::uninitialized() };
            for tb in &mut tbs {
                *tb = MetalTextureBindings::new();
            }
            tbs
        };

        MetalEncoderCache {
            render: MTLRenderPipelineState::nil(),

            scissor: None,
            viewport: None,
            front_face_winding: None,
            cull_mode: None,
            fill_mode: None,
            depth_clip_mode: None,
            depth_bias: None,
            blend_color: None,
            stencil_front_back_ref: None,

            index_buffer: None,
            depth_stencil: MTLDepthStencilState::nil(),

            texture_bindings: tbs,
            buffer_bindings: [MetalBufferBindings::new(); 3],
            sampler_bindings: [MetalSamplerBindings::new(); 3],
        }
    }

    pub fn reset(&mut self) {
        self.render = MTLRenderPipelineState::nil();

        self.scissor = None;
        self.viewport = None;
        self.front_face_winding = None;
        self.cull_mode = None;
        self.fill_mode = None;
        self.depth_clip_mode = None;
        self.depth_bias = None;
        self.blend_color = None;
        self.stencil_front_back_ref = None;

        // TODO(fkaa): retain/release? does encoder really have ownership?
        self.index_buffer = None;
        self.depth_stencil = MTLDepthStencilState::nil();

        self.texture_bindings.iter_mut().map(|binds| binds.reset());
        self.buffer_bindings.iter_mut().map(|binds| binds.reset());
        self.sampler_bindings.iter_mut().map(|binds| binds.reset());
    }
}

pub struct MetalEncoder {
    command_buffer: MTLCommandBuffer,

    render_desc:    MTLRenderPassDescriptor,
    render:         MTLRenderCommandEncoder,
    blit:           MTLBlitCommandEncoder,
    compute:        MTLComputeCommandEncoder,

    cache:          MetalEncoderCache
}

impl MetalEncoder {
    pub fn new(cb: MTLCommandBuffer) -> Self {
        MetalEncoder {
            command_buffer: cb,

            render_desc:    MTLRenderPassDescriptor::nil(),
            render:         MTLRenderCommandEncoder::nil(),
            blit:           MTLBlitCommandEncoder::nil(),
            compute:        MTLComputeCommandEncoder::nil(),

            cache:          MetalEncoderCache::new()

        }
    }

    pub fn reset(&mut self) {
        self.cache.reset();
    }

    pub fn restore_render_state(&mut self) {
        debug_assert!(!self.render.is_null(), "Render encoder must be non-nil");

        if let Some(viewport) = self.cache.viewport {
            self.render.set_viewport(viewport);
        }

        if let Some(winding) = self.cache.front_face_winding {
            self.render.set_front_facing_winding(winding);
        }

        if let Some(cull) = self.cache.cull_mode {
            self.render.set_cull_mode(cull);
        }

        if let Some(fill) = self.cache.fill_mode {
            self.render.set_triangle_fill_mode(fill);
        }

        if let Some(clip) = self.cache.depth_clip_mode {
            self.render.set_depth_clip_mode(clip);
        }

        if let Some(blend) = self.cache.blend_color {
            self.render.set_blend_color(blend[0], blend[1], blend[2], blend[3]);
        }

        if let Some((bias, slope)) = self.cache.depth_bias {
            self.render.set_depth_bias(bias, slope, 16f32);
        }

        if let Some((front, back)) = self.cache.stencil_front_back_ref {
            self.render.set_stencil_front_back_reference_value(front, back);
        }

        if !self.cache.render.is_null() {
            self.render.set_render_pipeline_state(self.cache.render);
        }

        if !self.cache.depth_stencil.is_null() {
            self.render.set_depth_stencil_state(self.cache.depth_stencil);
        }

        for stage in 0..3 {
            for idx in 0..MTL_MAX_TEXTURE_BINDINGS {
                let tex = self.cache.texture_bindings[stage].textures[idx];

                if !tex.is_null() {
                    if stage == VS_IDX {
                        self.render.set_vertex_texture(idx as u64, tex);
                    } else {
                        self.render.set_fragment_texture(idx as u64, tex);
                    }
                }
            }

            for idx in 0..MTL_MAX_BUFFER_BINDINGS {
                let buf = self.cache.buffer_bindings[stage].buffers[idx];
                let offset = self.cache.buffer_bindings[stage].offsets[idx];

                if !buf.is_null() {
                    if stage == VS_IDX {
                        self.render.set_vertex_buffer(idx as u64, offset, buf);
                    } else {
                        self.render.set_fragment_buffer(idx as u64, offset, buf);
                    }
                }
            }

            for idx in 0..MTL_MAX_SAMPLER_BINDINGS {
                let sampler = self.cache.sampler_bindings[stage].samplers[idx];

                if !sampler.is_null() {
                    if stage == VS_IDX {
                        self.render.set_vertex_sampler_state(idx as u64, sampler);
                    } else {
                        self.render.set_fragment_sampler_state(idx as u64, sampler);
                    }
                }
            }
        }
    }

    pub fn set_render_pass_descriptor(&mut self, desc: MTLRenderPassDescriptor) {
        self.render_desc = desc;
    }

    pub fn begin_render_encoding(&mut self) -> MTLRenderCommandEncoder {
        debug_assert!(!self.render_desc.is_null(), "Render description must be non-nil");
        debug_assert!(!self.command_buffer.is_null(), "Command Buffer must be non-nil");
        debug_assert!(self.blit.is_null() && self.compute.is_null(), "Remaining encoders must be ended");

        self.render = self.command_buffer.new_render_command_encoder(self.render_desc);
        self.render
    }

    pub fn is_render_encoding(&self) -> bool {
        !self.render.is_null()
    }

    pub fn has_command_buffer(&self) -> bool {
        !self.command_buffer.is_null()
    }

    pub fn start_command_buffer(&mut self, buf: MTLCommandBuffer) {
        debug_assert!(!buf.is_null(), "New Command Buffer must be non-nil");

        self.command_buffer = buf;
    }

    pub fn commit_command_buffer(&mut self, drawable: CAMetalDrawable, wait: bool) {
        debug_assert!(!self.command_buffer.is_null(), "Command Buffer must be non-nil");

        if !drawable.is_null() {
            self.command_buffer.present_drawable(drawable);
        }

        self.command_buffer.commit();

        if wait {
            self.command_buffer.wait_until_completed();
        }

        self.command_buffer = MTLCommandBuffer::nil();
    }

    pub fn sync(&mut self, resource: MTLResource) {
        self.blit = self.command_buffer.new_blit_command_encoder();
        self.blit.synchronize_resource(resource);
        self.blit.end_encoding();
        self.blit = MTLBlitCommandEncoder::nil();
    }

    pub fn end_encoding(&mut self) {
        unsafe {
            if !self.render.is_null() {
                self.render.end_encoding();
                //self.render.release();
                self.render = MTLRenderCommandEncoder::nil();
            }

            if !self.blit.is_null() {
                self.blit.end_encoding();
                self.blit.release();
                self.blit = MTLBlitCommandEncoder::nil();

            }

            if !self.compute.is_null() {
                self.compute.end_encoding();
                self.compute.release();
                self.compute = MTLComputeCommandEncoder::nil();
            }
        }
    }

    pub fn is_buffer_bound(&mut self, buf: MTLBuffer) -> bool {
        self.cache.buffer_bindings.iter().any(|binds| binds.is_bound(buf))
    }

    pub fn invalidate_buffer(&mut self, buf: MTLBuffer) {
        for binds in self.cache.buffer_bindings.iter_mut() {
            binds.invalidate(buf);
        }
    }

    pub fn set_render_pipeline_state(&mut self, pso: MTLRenderPipelineState) {
        self.cache.render = pso;
    }

    pub fn set_viewport(&mut self, viewport: MTLViewport) {
        self.cache.viewport = Some(viewport);
    }

    pub fn set_front_facing_winding(&mut self, winding: MTLWinding) {
        self.cache.front_face_winding = Some(winding);
    }

    pub fn set_cull_mode(&mut self, mode: MTLCullMode) {
        self.cache.cull_mode = Some(mode);
    }

    pub fn set_depth_clip_mode(&mut self, mode: MTLDepthClipMode) {
        self.cache.depth_clip_mode = Some(mode);
    }

    pub fn set_scissor_rect(&mut self, rect: MTLScissorRect) {
        self.cache.scissor = Some(rect);
    }

    pub fn set_triangle_fill_mode(&mut self, mode: MTLTriangleFillMode) {
        self.cache.fill_mode = Some(mode);
    }

    pub fn set_blend_color(&mut self, color: [f32; 4]) {
        self.cache.blend_color = Some(color);
    }

    pub fn set_depth_bias(&mut self, bias: f32, slope: f32, clamp: f32) {
        self.cache.depth_bias = Some((bias, slope));
    }

    pub fn set_depth_stencil_state(&mut self, depth_stencil: MTLDepthStencilState) {
        self.cache.depth_stencil = depth_stencil;
    }

    pub fn set_stencil_reference_value(&mut self, value: u32) {
        self.cache.stencil_front_back_ref = Some((value, value));
    }

    pub fn set_stencil_front_back_reference_value(&mut self, front: u32, back: u32) {
        self.cache.stencil_front_back_ref = Some((front, back));
    }

    pub fn set_vertex_texture(&mut self, index: u64, texture: MTLTexture) {
        self.cache.texture_bindings[VS_IDX].insert(index as usize, texture);
    }

    pub fn set_fragment_texture(&mut self, index: u64, texture: MTLTexture) {
        self.cache.texture_bindings[FS_IDX].insert(index as usize, texture);
    }

    pub fn set_vertex_buffer(&mut self, index: u64, offset: u64, buffer: MTLBuffer) {
        self.cache.buffer_bindings[VS_IDX].insert(index as usize, offset, buffer);
    }

    pub fn set_fragment_buffer(&mut self, index: u64, offset: u64, buffer: MTLBuffer) {
        self.cache.buffer_bindings[FS_IDX].insert(index as usize, offset, buffer);
    }

    pub fn set_vertex_sampler_state(&mut self, index: u64, sampler: MTLSamplerState) {
        self.cache.sampler_bindings[VS_IDX].insert(index as usize, sampler);
    }

    pub fn set_fragment_sampler_state(&mut self, index: u64, sampler: MTLSamplerState) {
        self.cache.sampler_bindings[FS_IDX].insert(index as usize, sampler);
    }

    pub fn set_index_buffer(&mut self, buf: MTLBuffer, idx_type: MTLIndexType) {
        self.cache.index_buffer = Some((buf, idx_type));
    }

    pub fn draw(&mut self, start: u64, count: u64) {
        self.render.draw_primitives(MTLPrimitiveType::Triangle, start, count);
    }

    pub fn draw_instanced(&self, start: u64, count: u64, instance_count: u64) {
        self.render.draw_primitives_instanced(MTLPrimitiveType::Triangle, start, count, instance_count);
    }

    pub fn draw_indexed(&self, index_count: u64, index_buffer_offset: u64) {
        if let Some((buf, ty)) = self.cache.index_buffer {
            self.render.draw_indexed_primitives(MTLPrimitiveType::Triangle,
                                                index_count,
                                                ty,
                                                buf,
                                                index_buffer_offset);
        } else {
            error!("Cannot draw indexed primitives without a index buffer bound");
        }
    }

    pub fn draw_indexed_instanced(&self, index_count: u64, index_buffer_offset: u64, instance_count: u64, base_vertex: i64, base_instance: u64) {
        if let Some((buf, ty)) = self.cache.index_buffer {
            self.render.draw_indexed_primitives_instanced(MTLPrimitiveType::Triangle,
                                                          index_count,
                                                          ty,
                                                          buf,
                                                          index_buffer_offset,
                                                          instance_count,
                                                          base_vertex,
                                                          base_instance);
        } else {
            error!("Cannot draw indexed primitives without a index buffer bound");
        }
    }

}
