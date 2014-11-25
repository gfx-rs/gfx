// Copyright 2014 The Gfx-rs Developers.
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

//! High-level, platform independent, bindless rendering API.

#![crate_name = "render"]
#![comment = "A platform independent renderer for gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]
#![deny(missing_docs)]
#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate device;

use std::mem;

use device::attrib;
use device::draw::CommandBuffer;
use device::shade::{ProgramInfo, UniformValue, ShaderSource, Stage, CreateShaderError};
use device::target::{Rect, ClearData, Mask, Access, Draw, Read,
    Target, TargetColor, TargetDepth, TargetStencil};
use batch::Batch;
use mesh::SliceKind;
use target::Plane;

/// Batches
pub mod batch;
/// Meshes
pub mod mesh;
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;

/// Program linking error
#[deriving(Clone, PartialEq, Show)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    Vertex(CreateShaderError),
    /// Unable to compile the fragment shader
    Fragment(CreateShaderError),
    /// Unable to link
    Link(()),
}

const TRACKED_ATTRIBUTES: uint = 8;
type CachedAttribute = (device::RawBufferHandle, attrib::Format);

/// The internal state of the renderer. This is used as a cache to eliminate
/// redundant state changes.
struct RenderState {
    is_frame_buffer_set: bool,
    frame: target::Frame,
    is_array_buffer_set: bool,
    program_name: device::back::Program,
    index: Option<device::RawBufferHandle>,
    attributes: [Option<CachedAttribute>, .. TRACKED_ATTRIBUTES],
    draw: state::DrawState,
}

impl RenderState {
    /// Generate the initial state matching `Device::reset_state`
    fn new() -> RenderState {
        RenderState {
            is_frame_buffer_set: false,
            frame: target::Frame::new(0,0),
            is_array_buffer_set: false,
            program_name: 0,
            index: None,
            attributes: [None, ..TRACKED_ATTRIBUTES],
            draw: state::DrawState::new(),
        }
    }
}

struct ParamStorage {
    uniforms: Vec<UniformValue>,
    blocks  : Vec<device::RawBufferHandle>,
    textures: Vec<shade::TextureParam>,
}

impl ParamStorage{
    fn new() -> ParamStorage {
        ParamStorage {
            uniforms: Vec::new(),
            blocks: Vec::new(),
            textures: Vec::new(),
        }
    }

    fn get_mut(&mut self) -> shade::ParamValues {
        self.uniforms.truncate(0);
        self.blocks.truncate(0);
        self.textures.truncate(0);
        shade::ParamValues {
            uniforms: &mut self.uniforms,
            blocks: &mut self.blocks,
            textures: &mut self.textures,
        }
    }
}

/// Helper routines for the command buffer
/// Useful when Renderer is borrowed, and we need to issue commands.
trait CommandBufferHelper {
    /// Bind a plane to some target
    fn bind_target(&mut self, Access, Target, Option<&Plane>);
}

impl<C: CommandBuffer> CommandBufferHelper for C {
    fn bind_target(&mut self, access: Access, to: Target,
                   plane: Option<&Plane>) {
        match plane {
            None => self.unbind_target(access, to),
            Some(&Plane::Surface(ref suf)) =>
                self.bind_target_surface(access, to, suf.get_name()),
            Some(&Plane::Texture(ref tex, level, layer)) =>
                self.bind_target_texture(access, to, tex.get_name(), level, layer),
        }
    }
}

/// Renderer front-end
pub struct Renderer<C: CommandBuffer> {
    command_buffer: C,
    data_buffer: device::draw::DataBuffer,
    common_array_buffer: Result<device::ArrayBufferHandle, ()>,
    draw_frame_buffer: device::FrameBufferHandle,
    read_frame_buffer: device::FrameBufferHandle,
    default_frame_buffer: device::FrameBufferHandle,
    render_state: RenderState,
    parameters: ParamStorage,
}

impl<C: CommandBuffer> Renderer<C> {
    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.data_buffer.clear();
        self.render_state = RenderState::new();
    }

    /// Get a command buffer to be submitted to the device.
    pub fn as_buffer(&self) -> (&C, &device::draw::DataBuffer) {
        (&self.command_buffer, &self.data_buffer)
    }

    /// Clone the renderer shared data but ignore the commands.
    pub fn clone_empty(&self) -> Renderer<C> {
        Renderer {
            command_buffer: CommandBuffer::new(),
            data_buffer: device::draw::DataBuffer::new(),
            common_array_buffer: self.common_array_buffer,
            draw_frame_buffer: self.draw_frame_buffer,
            read_frame_buffer: self.read_frame_buffer,
            default_frame_buffer: self.default_frame_buffer,
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ClearData, mask: Mask, frame: &target::Frame) {
        self.bind_frame(frame);
        self.command_buffer.call_clear(data, mask);
    }

    /// Draw a `batch` into the specified `frame`
    pub fn draw<B: Batch>(&mut self, batch: &B, frame: &target::Frame) {
        self.bind_frame(frame);
        let (mesh, link, slice, program, state) = batch.get_data();
        self.bind_program(batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, link, program.get_info());
        self.draw_slice(slice, None);
    }

    /// Draw a `batch` multiple times using instancing
    pub fn draw_instanced<B: Batch>(&mut self, batch: B,
                          count: device::InstanceCount,
                          base: device::VertexCount,
                          frame: &target::Frame) {
        self.bind_frame(frame);
        let (mesh, link, slice, program, state) = batch.get_data();
        self.bind_program(&batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, link, program.get_info());
        self.draw_slice(slice, Some((count, base)));
    }

    /// Blit one frame onto another
    #[experimental]
    pub fn blit(&mut self, source: &target::Frame, source_rect: Rect,
                destination: &target::Frame, dest_rect: Rect, mask: Mask) {
        // verify as much as possible here
        if mask.intersects(device::target::COLOR) {
            debug_assert!(source.is_default() || !source.colors.is_empty());
            debug_assert!(destination.is_default() || !destination.colors.is_empty());
        }
        if mask.intersects(device::target::DEPTH) {
            debug_assert!(source.is_default() || source.depth.is_some());
            debug_assert!(destination.is_default() || destination.depth.is_some());
        }
        if mask.intersects(device::target::STENCIL) {
            debug_assert!(source.is_default() || source.stencil.is_some());
            debug_assert!(destination.is_default() || destination.stencil.is_some());
        }
        // actually blit
        self.bind_frame(destination);
        self.bind_read_frame(source);
        self.command_buffer.call_blit(source_rect, dest_rect, mask);
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Copy>(&mut self, buf: device::BufferHandle<T>,
                             data: &[T], offset_elements: uint) {
        let esize = mem::size_of::<T>();
        let offset_bytes = esize * offset_elements;
        debug_assert!(data.len() * esize + offset_bytes <= buf.get_info().size);
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_buffer(buf.get_name(), pointer, offset_bytes);
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: Copy>(&mut self,
                                buf: device::BufferHandle<U>, data: &T) {
        debug_assert!(mem::size_of::<T>() <= buf.get_info().size);
        let pointer = self.data_buffer.add_struct(data);
        self.command_buffer.update_buffer(buf.get_name(), pointer, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Copy>(&mut self, tex: device::TextureHandle,
                          img: device::tex::ImageInfo, data: &[T]) {
        debug_assert!(tex.get_info().contains(&img));
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_texture(tex.get_info().kind, tex.get_name(), img, pointer);
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        if self.render_state.frame.width != frame.width ||
                self.render_state.frame.height != frame.height {
            self.command_buffer.set_viewport(Rect {
                x: 0,
                y: 0,
                w: frame.width,
                h: frame.height,
            });
            self.render_state.frame.width = frame.width;
            self.render_state.frame.height = frame.height;
        }
        if frame.is_default() {
            if self.render_state.is_frame_buffer_set {
                // binding the default FBO, not touching our common one
                self.command_buffer.bind_frame_buffer(Draw, self.default_frame_buffer.get_name());
                self.render_state.is_frame_buffer_set = false;
            }
        } else {
            if !self.render_state.is_frame_buffer_set {
                self.command_buffer.bind_frame_buffer(Draw, self.draw_frame_buffer.get_name());
                self.render_state.is_frame_buffer_set = true;
            }
            // cut off excess color planes
            for (i, _) in self.render_state.frame.colors.iter().enumerate()
                                .skip(frame.colors.len()) {
                self.command_buffer.unbind_target(Draw, TargetColor(i as u8));
            }
            self.render_state.frame.colors.truncate(frame.colors.len());
            // bind intersecting subsets
            for (i, (cur, new)) in self.render_state.frame.colors.iter_mut()
                                       .zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    self.command_buffer.bind_target(Draw, TargetColor(i as u8), Some(new));
                    *cur = *new;
                }
            }
            // activate the color targets that were just bound
            self.command_buffer.set_draw_color_buffers(frame.colors.len());
            // append new planes
            for (i, new) in frame.colors.iter().enumerate()
                                 .skip(self.render_state.frame.colors.len()) {
                self.command_buffer.bind_target(Draw, TargetColor(i as u8), Some(new));
                self.render_state.frame.colors.push(*new);
            }
            // set depth
            if self.render_state.frame.depth != frame.depth {
                self.command_buffer.bind_target(Draw, TargetDepth, frame.depth.as_ref());
                self.render_state.frame.depth = frame.depth;
            }
            // set stencil
            if self.render_state.frame.stencil != frame.stencil {
                self.command_buffer.bind_target(Draw, TargetStencil, frame.stencil.as_ref());
                self.render_state.frame.stencil = frame.stencil;
            }
        }
    }

    fn bind_read_frame(&mut self, frame: &target::Frame) {
        self.command_buffer.bind_frame_buffer(Read, self.read_frame_buffer.get_name());
        // color
        if frame.colors.is_empty() {
            self.command_buffer.unbind_target(Read, TargetColor(0));
        }else {
            self.command_buffer.bind_target(Read, TargetColor(0), Some(&frame.colors[0]));
        }
        // depth/stencil
        self.command_buffer.bind_target(Read, TargetDepth, frame.depth.as_ref());
        self.command_buffer.bind_target(Read, TargetStencil, frame.stencil.as_ref());
    }

    fn bind_state(&mut self, state: &state::DrawState) {
        if self.render_state.draw.primitive != state.primitive {
            self.command_buffer.set_primitive(state.primitive);
        }
        if self.render_state.draw.multi_sample != state.multi_sample {
            self.command_buffer.set_multi_sample(state.multi_sample);
        }
        if self.render_state.draw.scissor != state.scissor {
            self.command_buffer.set_scissor(state.scissor);
        }
        if self.render_state.draw.depth != state.depth || self.render_state.draw.stencil != state.stencil ||
                self.render_state.draw.primitive.get_cull_mode() != state.primitive.get_cull_mode() {
            self.command_buffer.set_depth_stencil(state.depth, state.stencil,
                state.primitive.get_cull_mode());
        }
        if self.render_state.draw.blend != state.blend {
            self.command_buffer.set_blend(state.blend);
        }
        if self.render_state.draw.color_mask != state.color_mask {
            self.command_buffer.set_color_mask(state.color_mask);
        }
        self.render_state.draw = *state;
    }

    fn bind_program<B: Batch>(&mut self, batch: &B, program: &device::ProgramHandle) {
        //Warning: this is not protected against deleted resources in single-threaded mode
        if self.render_state.program_name != program.get_name() {
            self.command_buffer.bind_program(program.get_name());
            self.render_state.program_name = program.get_name();
        }
        batch.fill_params(self.parameters.get_mut());
        self.upload_parameters(program);
    }

    fn upload_parameters(&mut self, program: &device::ProgramHandle) {
        let info = program.get_info();
        if self.parameters.uniforms.len() != info.uniforms.len() ||
            self.parameters.blocks.len() != info.blocks.len() ||
            self.parameters.textures.len() != info.textures.len() {
            error!("Mismatching number of uniforms ({}), blocks ({}), or \
                    textures ({}) in `upload_parameters` for program: {}",
                    self.parameters.uniforms.len(),
                    self.parameters.blocks.len(),
                    self.parameters.textures.len(),
                    info);
        }
        // bind uniforms
        for (var, value) in info.uniforms.iter()
            .zip(self.parameters.uniforms.iter()) {
            self.command_buffer.bind_uniform(var.location, *value);
        }
        // bind uniform blocks
        for (i, (_, buf)) in info.blocks.iter()
            .zip(self.parameters.blocks.iter()).enumerate() {
            self.command_buffer.bind_uniform_block(
                program.get_name(),
                i as device::UniformBufferSlot,
                i as device::UniformBlockIndex,
                buf.get_name()
            );
        }
        // bind textures and samplers
        for (i, (var, &(tex, sampler))) in info.textures.iter()
            .zip(self.parameters.textures.iter()).enumerate() {
            if sampler.is_some() && tex.get_info().kind.get_aa_mode().is_some() {
                error!("A sampler provided for an AA texture: {}", var.name.clone());
            }
            self.command_buffer.bind_uniform(var.location, UniformValue::I32(i as i32));
            self.command_buffer.bind_texture(i as device::TextureSlot,
                tex.get_info().kind, tex.get_name(), sampler);
        }
    }

    fn bind_mesh(&mut self, mesh: &mesh::Mesh, link: &mesh::Link, info: &ProgramInfo) {
        if !self.render_state.is_array_buffer_set {
            // It's Ok if the array buffer is not supported. We can just ignore it.
            self.common_array_buffer.map(|ab|
                self.command_buffer.bind_array_buffer(ab.get_name())
            ).is_ok();
            self.render_state.is_array_buffer_set = true;
        }
        for (attr_index, sat) in link.attribute_indices().zip(info.attributes.iter()) {
            let vat = &mesh.attributes[attr_index];
            let loc = sat.location as uint;
            let need_update = loc >= self.render_state.attributes.len() ||
                match self.render_state.attributes[loc] {
                    Some((buf, fmt)) => buf != vat.buffer || fmt != vat.format,
                    None => true,
                };
            if need_update {
                self.command_buffer.bind_attribute(loc as device::AttributeSlot,
                    vat.buffer.get_name(), vat.format);
                if loc < self.render_state.attributes.len() {
                    self.render_state.attributes[loc] = Some((vat.buffer, vat.format));
                }
            }
        }
    }

    fn bind_index<T>(&mut self, buf: device::BufferHandle<T>) {
        if self.render_state.index != Some(buf.raw()) {
            self.command_buffer.bind_index(buf.get_name());
            self.render_state.index = Some(buf.raw());
        }
    }

    fn draw_slice(&mut self, slice: &mesh::Slice,
                  instances: Option<(device::InstanceCount, device::VertexCount)>) {
        let mesh::Slice { start, end, prim_type, kind } = *slice;
        match kind {
            SliceKind::Vertex => {
                self.command_buffer.call_draw(prim_type, start, end - start, instances);
            },
            SliceKind::Index8(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, attrib::U8, start, end - start, base, instances);
            },
            SliceKind::Index16(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, attrib::U16, start, end - start, base, instances);
            },
            SliceKind::Index32(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, attrib::U32, start, end - start, base, instances);
            },
        }
    }
}

/// Backend extension trait for convenience methods
pub trait DeviceHelper<C: CommandBuffer> {
    /// Create a new renderer
    fn create_renderer(&mut self) -> Renderer<C>;
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from_format`.
    fn create_mesh<T: mesh::VertexFormat + Copy>(&mut self, data: &[T]) -> mesh::Mesh;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<device::ProgramHandle, ProgramError>;
}

impl<D: device::Device<C>, C: CommandBuffer> DeviceHelper<C> for D {
    fn create_renderer(&mut self) -> Renderer<C> {
        Renderer {
            command_buffer: CommandBuffer::new(),
            data_buffer: device::draw::DataBuffer::new(),
            common_array_buffer: self.create_array_buffer(),
            draw_frame_buffer: self.create_frame_buffer(),
            read_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: device::get_main_frame_buffer(),
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }

    fn create_mesh<T: mesh::VertexFormat + Copy>(&mut self, data: &[T]) -> mesh::Mesh {
        let nv = data.len();
        debug_assert!(nv < {
            use std::num::Int;
            let val: device::VertexCount = Int::max_value();
            val as uint
        });
        let buf = self.create_buffer_static(data);
        mesh::Mesh::from_format(buf, nv as device::VertexCount)
    }

    fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<device::ProgramHandle, ProgramError> {
        let vs = match self.create_shader(Stage::Vertex, vs_src) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let fs = match self.create_shader(Stage::Fragment, fs_src) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Fragment(e)),
        };
        self.create_program(&[vs, fs]).map_err(|e| ProgramError::Link(e))
    }
}
