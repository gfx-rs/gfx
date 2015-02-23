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

#![deny(missing_docs)]

//#[macro_use]
//extern crate log;
extern crate "gfx_device_gl" as device;

use std::mem;

use device::{Device, Resources};
use device::attrib;
use device::attrib::IntSize;
use device::draw::CommandBuffer;
use device::shade::{ProgramInfo, UniformValue};
use device::target::{Rect, ClearData, Mirror, Mask, Access, Target};
use render::batch::Batch;
use render::mesh::SliceKind;

/// Batches
pub mod batch;
/// Device extensions
pub mod device_ext;
/// Meshes
pub mod mesh;
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;


const TRACKED_ATTRIBUTES: usize = 8;
type CachedAttribute<R: Resources> = (device::RawBufferHandle<R>, attrib::Format);
type Instancing = (device::InstanceCount, device::VertexCount);

/// The internal state of the renderer.
/// This is used as a cache to eliminate redundant state changes.
struct RenderState<R: Resources> {
    is_frame_buffer_set: bool,
    frame: target::Frame<R>,
    is_array_buffer_set: bool,
    program_name: Option<R::Program>,
    index: Option<device::RawBufferHandle<R>>,
    attributes: [Option<CachedAttribute<R>>; TRACKED_ATTRIBUTES],
    draw: state::DrawState,
}

impl<R: Resources> RenderState<R> {
    /// Generate the initial state matching `Device::reset_state`
    fn new() -> RenderState<R> {
        RenderState {
            is_frame_buffer_set: false,
            frame: target::Frame::new(0,0),
            is_array_buffer_set: false,
            program_name: None,
            index: None,
            attributes: [None; TRACKED_ATTRIBUTES],
            draw: state::DrawState::new(),
        }
    }
}

/// Temporary parameter storage, used for shader activation.
struct ParamStorage<R: Resources> {
    uniforms: Vec<UniformValue>,
    blocks  : Vec<device::RawBufferHandle<R>>,
    textures: Vec<shade::TextureParam<R>>,
}

impl<R: Resources> ParamStorage<R> {
    /// Create an empty parameter storage.
    fn new() -> ParamStorage<R> {
        ParamStorage {
            uniforms: Vec::new(),
            blocks: Vec::new(),
            textures: Vec::new(),
        }
    }

    fn get_mut(&mut self) -> shade::ParamValues<R> {
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

/// Extension methods for the command buffer.
/// Useful when Renderer is borrowed, and we need to issue commands.
trait CommandBufferExt: CommandBuffer {
    /// Bind a plane to some target
    fn bind_target(&mut self, Access, Target, Option<&target::Plane<Self::Resources>>);
}

impl<C: CommandBuffer> CommandBufferExt for C {
    fn bind_target(&mut self, access: Access, to: Target,
                   plane: Option<&target::Plane<C::Resources>>) {
        match plane {
            None => self.unbind_target(access, to),
            Some(&target::Plane::Surface(ref suf)) =>
                self.bind_target_surface(access, to, suf.get_name()),
            Some(&target::Plane::Texture(ref tex, level, layer)) =>
                self.bind_target_texture(access, to, tex.get_name(), level, layer),
        }
    }
}

/// Draw-time error, showing inconsistencies in draw parameters and data
#[derive(Clone, Debug, PartialEq)]
pub enum DrawError<E> {
    /// Tha batch is not valid
    InvalidBatch(E),
    /// The `DrawState` interacts with a target that does not present in the
    /// frame. For example, the depth test is enabled while there is no depth.
    MissingTarget(Mask),
    /// The viewport either covers zero space or exceeds HW limitations.
    BadViewport,
    /// Vertex count exceeds HW limitations.
    BadVertexCount,
    /// Index count exceeds HW limitations.
    BadIndexCount,
}

/// Renderer front-end
pub struct Renderer<C: CommandBuffer> {
    command_buffer: C,
    data_buffer: device::draw::DataBuffer,
    common_array_buffer: Result<device::ArrayBufferHandle<C::Resources>, ()>,
    draw_frame_buffer: device::FrameBufferHandle<C::Resources>,
    read_frame_buffer: device::FrameBufferHandle<C::Resources>,
    default_frame_buffer: device::FrameBufferHandle<C::Resources>,
    render_state: RenderState<C::Resources>,
    parameters: ParamStorage<C::Resources>,
}

impl<C: CommandBuffer> Renderer<C> {
    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.data_buffer.clear();
        self.render_state = RenderState::new();
    }

    /// Get command and data buffers to be submitted to the device.
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
    pub fn clear(&mut self, data: ClearData, mask: Mask, frame: &target::Frame<C::Resources>) {
        self.bind_frame(frame);
        self.command_buffer.call_clear(data, mask);
    }

    /// Draw a `batch` into the specified `frame`
    pub fn draw<B: Batch<Resources = C::Resources>>(&mut self, batch: &B, frame: &target::Frame<C::Resources>)
                -> Result<(), DrawError<B::Error>> {
        self.draw_all(batch, None, frame)
    }

    /// Draw a `batch` multiple times using instancing
    pub fn draw_instanced<B: Batch<Resources = C::Resources>>(&mut self, batch: &B,
                          count: device::InstanceCount,
                          base: device::VertexCount,
                          frame: &target::Frame<C::Resources>)
                          -> Result<(), DrawError<B::Error>> {
        self.draw_all(batch, Some((count, base)), frame)
    }

    /// Draw a 'batch' with all known parameters specified, internal use only.
    fn draw_all<B: Batch<Resources = C::Resources>>(&mut self, batch: &B, instances: Option<Instancing>,
                frame: &target::Frame<C::Resources>) -> Result<(), DrawError<B::Error>> {
        let (mesh, attrib_iter, slice, state) = match batch.get_data() {
            Ok(data) => data,
            Err(e) => return Err(DrawError::InvalidBatch(e)),
        };
        let target_missing = state.get_target_mask() - frame.get_mask();
        if !target_missing.is_empty() {
            return Err(DrawError::MissingTarget(target_missing))
        }
        self.bind_frame(frame);
        let program = match self.bind_program(batch) {
            Ok(p) => p,
            Err(e) => return Err(DrawError::InvalidBatch(e)),
        };
        self.bind_state(state);
        self.bind_mesh(mesh, attrib_iter, program.get_info());
        self.draw_slice(slice, instances);
        Ok(())
    }

    /// Blit one frame onto another
    pub fn blit(&mut self, source: &target::Frame<C::Resources>, source_rect: Rect,
                destination: &target::Frame<C::Resources>, dest_rect: Rect,
                mirror: Mirror, mask: Mask) {
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
        self.command_buffer.call_blit(source_rect, dest_rect, mirror, mask);
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Copy>(&mut self, buf: device::BufferHandle<C::Resources, T>,
                             data: &[T], offset_elements: usize) {
        let esize = mem::size_of::<T>();
        let offset_bytes = esize * offset_elements;
        debug_assert!(data.len() * esize + offset_bytes <= buf.get_info().size);
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_buffer(buf.get_name(), pointer, offset_bytes);
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: Copy>(&mut self,
                                buf: device::BufferHandle<C::Resources, U>, data: &T) {
        debug_assert!(mem::size_of::<T>() <= buf.get_info().size);
        let pointer = self.data_buffer.add_struct(data);
        self.command_buffer.update_buffer(buf.get_name(), pointer, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Copy>(&mut self, tex: device::TextureHandle<C::Resources>,
                          img: device::tex::ImageInfo, data: &[T]) {
        debug_assert!(tex.get_info().contains(&img));
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_texture(tex.get_info().kind, tex.get_name(), img, pointer);
    }

    fn bind_frame(&mut self, frame: &target::Frame<C::Resources>) {
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
                self.command_buffer.bind_frame_buffer(Access::Draw, self.default_frame_buffer.get_name());
                self.render_state.is_frame_buffer_set = false;
            }
        } else {
            if !self.render_state.is_frame_buffer_set {
                self.command_buffer.bind_frame_buffer(Access::Draw, self.draw_frame_buffer.get_name());
                self.render_state.is_frame_buffer_set = true;
            }
            // cut off excess color planes
            for (i, _) in self.render_state.frame.colors.iter().enumerate()
                                .skip(frame.colors.len()) {
                self.command_buffer.unbind_target(Access::Draw, Target::Color(i as u8));
            }
            self.render_state.frame.colors.truncate(frame.colors.len());
            // bind intersecting subsets
            for (i, (cur, new)) in self.render_state.frame.colors.iter_mut()
                                       .zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    self.command_buffer.bind_target(Access::Draw, Target::Color(i as u8), Some(new));
                    *cur = *new;
                }
            }
            // activate the color targets that were just bound
            self.command_buffer.set_draw_color_buffers(frame.colors.len());
            // append new planes
            for (i, new) in frame.colors.iter().enumerate()
                                 .skip(self.render_state.frame.colors.len()) {
                self.command_buffer.bind_target(Access::Draw, Target::Color(i as u8), Some(new));
                self.render_state.frame.colors.push(*new);
            }
            // set depth
            if self.render_state.frame.depth != frame.depth {
                self.command_buffer.bind_target(Access::Draw, Target::Depth, frame.depth.as_ref());
                self.render_state.frame.depth = frame.depth;
            }
            // set stencil
            if self.render_state.frame.stencil != frame.stencil {
                self.command_buffer.bind_target(Access::Draw, Target::Stencil, frame.stencil.as_ref());
                self.render_state.frame.stencil = frame.stencil;
            }
        }
    }

    fn bind_read_frame(&mut self, frame: &target::Frame<C::Resources>) {
        self.command_buffer.bind_frame_buffer(Access::Read, self.read_frame_buffer.get_name());
        // color
        if frame.colors.is_empty() {
            self.command_buffer.unbind_target(Access::Read, Target::Color(0));
        }else {
            self.command_buffer.bind_target(Access::Read, Target::Color(0), Some(&frame.colors[0]));
        }
        // depth/stencil
        self.command_buffer.bind_target(Access::Read, Target::Depth, frame.depth.as_ref());
        self.command_buffer.bind_target(Access::Read, Target::Stencil, frame.stencil.as_ref());
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

    fn bind_program<'a, B: Batch<Resources = C::Resources>>(&mut self, batch: &'a B)
                    -> Result<&'a device::ProgramHandle<C::Resources>, B::Error> {
        let program = match batch.fill_params(self.parameters.get_mut()) {
            Ok(p) => p,
            Err(e) => return Err(e),
        };
        //Warning: this is not protected against deleted resources in single-threaded mode
        if self.render_state.program_name != Some(program.get_name()) {
            self.command_buffer.bind_program(program.get_name());
            self.render_state.program_name = Some(program.get_name());
        }
        self.upload_parameters(program);
        Ok(program)
    }

    fn upload_parameters(&mut self, program: &device::ProgramHandle<C::Resources>) {
        let info = program.get_info();
        if self.parameters.uniforms.len() != info.uniforms.len() ||
            self.parameters.blocks.len() != info.blocks.len() ||
            self.parameters.textures.len() != info.textures.len() {
            error!("Mismatching number of uniforms ({:?}), blocks ({:?}), or \
                    textures ({:?}) in `upload_parameters` for program: {:?}",
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

    fn bind_mesh<I: Iterator<Item = mesh::AttributeIndex>>(&mut self,
                 mesh: &mesh::Mesh<C::Resources>, attrib_iter: I, info: &ProgramInfo) {
        if !self.render_state.is_array_buffer_set {
            // It's Ok if the array buffer is not supported. We can just ignore it.
            self.common_array_buffer.map(|ab|
                self.command_buffer.bind_array_buffer(ab.get_name())
            ).is_ok();
            self.render_state.is_array_buffer_set = true;
        }
        for (attr_index, sat) in attrib_iter.zip(info.attributes.iter()) {
            let vat = &mesh.attributes[attr_index as usize];
            let loc = sat.location as usize;
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

    fn bind_index<T>(&mut self, buf: device::BufferHandle<C::Resources, T>) {
        if self.render_state.index != Some(buf.raw()) {
            self.command_buffer.bind_index(buf.get_name());
            self.render_state.index = Some(buf.raw());
        }
    }

    fn draw_slice(&mut self, slice: &mesh::Slice<C::Resources>,
                  instances: Option<(device::InstanceCount, device::VertexCount)>) {
        let mesh::Slice { start, end, prim_type, kind } = slice.clone();
        match kind {
            SliceKind::Vertex => {
                self.command_buffer.call_draw(prim_type, start, end - start, instances);
            },
            SliceKind::Index8(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U8, start, end - start, base, instances);
            },
            SliceKind::Index16(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U16, start, end - start, base, instances);
            },
            SliceKind::Index32(buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U32, start, end - start, base, instances);
            },
        }
    }
}
