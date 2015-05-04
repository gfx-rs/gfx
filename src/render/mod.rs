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

use std::mem;
use draw_state::DrawState;
use draw_state::target::{ClearData, Mask, Mirror, Rect};

use device;
use device::Resources;
use device::{attrib, handle};
use device::attrib::IntSize;
use device::draw::{Access, Gamma, Target};
use device::draw::{CommandBuffer, DataBuffer, InstanceOption};
use device::shade::{ProgramInfo, UniformValue};
use render::batch::Batch;
use render::mesh::SliceKind;

/// Batches
pub mod batch;
/// Meshes
pub mod mesh;
/// Shaders
pub mod shade;
/// Render targets
pub mod target;


type CachedAttribute<R: Resources> = (handle::RawBuffer<R>, attrib::Format);

/// The internal state of the renderer.
/// This is used as a cache to eliminate redundant state changes.
struct RenderState<R: Resources> {
    frame_buffer: Option<handle::FrameBuffer<R>>,
    frame: target::Frame<R>,
    gamma: Gamma,
    is_array_buffer_set: bool,
    program: Option<handle::Program<R>>,
    index: Option<handle::RawBuffer<R>>,
    attributes: Vec<Option<CachedAttribute<R>>>,
    draw: DrawState,
}

impl<R: Resources> RenderState<R> {
    /// Generate the initial state matching `Device::reset_state`
    fn new() -> RenderState<R> {
        RenderState {
            frame_buffer: None,
            frame: target::Frame::empty(0,0),
            gamma: Gamma::Original,
            is_array_buffer_set: false,
            program: None,
            index: None,
            attributes: Vec::new(),
            draw: DrawState::new(),
        }
    }
}

/// Temporary parameter storage, used for shader activation.
pub struct ParamStorage<R: Resources> {
    /// uniform values to be provided
    pub uniforms: Vec<Option<UniformValue>>,
    /// uniform buffers to be provided
    pub blocks  : Vec<Option<handle::RawBuffer<R>>>,
    /// textures to be provided
    pub textures: Vec<Option<shade::TextureParam<R>>>,
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

    fn reserve(&mut self, pinfo: &ProgramInfo) {
        // clear
        self.uniforms.clear();
        self.blocks  .clear();
        self.textures.clear();
        // allocate
        self.uniforms.extend(pinfo.uniforms.iter().map(|_| None));
        self.blocks  .extend(pinfo.blocks  .iter().map(|_| None));
        self.textures.extend(pinfo.textures.iter().map(|_| None));
    }
}

/// Extension methods for the command buffer.
/// Useful when Renderer is borrowed, and we need to issue commands.
trait CommandBufferExt<R: Resources>: CommandBuffer<R> {
    /// Bind a plane to some target
    fn bind_target(&mut self, &mut handle::Manager<R>, Access, Target,
                   Option<&target::Plane<R>>);
}

impl<R: Resources, C: CommandBuffer<R>> CommandBufferExt<R> for C {
    fn bind_target(&mut self, handles: &mut handle::Manager<R>, access: Access,
                   to: Target, plane: Option<&target::Plane<R>>) {
        match plane {
            None => self.unbind_target(access, to),
            Some(&target::Plane::Surface(ref suf)) =>
                self.bind_target_surface(access, to, handles.ref_surface(suf)),
            Some(&target::Plane::Texture(ref tex, level, layer)) =>
                self.bind_target_texture(access, to, handles.ref_texture(tex), level, layer),
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
pub struct Renderer<R: Resources, C: CommandBuffer<R>> {
    command_buffer: C,
    data_buffer: DataBuffer,
    handles: handle::Manager<R>,
    common_array_buffer: Result<handle::ArrayBuffer<R>, ()>,
    draw_frame_buffer: handle::FrameBuffer<R>,
    read_frame_buffer: handle::FrameBuffer<R>,
    render_state: RenderState<R>,
    parameters: ParamStorage<R>,
}

impl<R: Resources, C: CommandBuffer<R>> Renderer<R, C> {
    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.data_buffer.clear();
        self.handles.clear();
        self.render_state = RenderState::new();
    }

    /// Get command and data buffers to be submitted to the device.
    pub fn as_buffer(&self) -> (&C, &DataBuffer, &handle::Manager<R>) {
        (&self.command_buffer, &self.data_buffer, &self.handles)
    }

    /// Clone the renderer shared data but ignore the commands.
    pub fn clone_empty(&self) -> Renderer<R, C> {
        Renderer {
            command_buffer: CommandBuffer::new(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: self.common_array_buffer.clone(),
            draw_frame_buffer: self.draw_frame_buffer.clone(),
            read_frame_buffer: self.read_frame_buffer.clone(),
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }

    /// Clear the output with given `ClearData`.
    pub fn clear<O: target::Output<R>>(&mut self, data: ClearData, mask: Mask, output: &O) {
        let has_mask = output.get_mask();
        if has_mask.is_empty() {
            panic!("Clearing a frame without any attachments is not possible!
                    If you are using `Frame::empty` in place of a real output window,
                    please see https://github.com/gfx-rs/gfx-rs/pull/682");
        }
        debug_assert!(has_mask.contains(mask));
        self.bind_output(output);
        self.command_buffer.call_clear(data, mask);
    }

    /// Draw a `batch` into the specified output.
    pub fn draw<B: Batch<R>, O: target::Output<R>>(&mut self, batch: &B, output: &O)
                -> Result<(), DrawError<B::Error>> {
        self.draw_all(batch, None, output)
    }

    /// Draw a `batch` multiple times using instancing.
    pub fn draw_instanced<B: Batch<R>, O: target::Output<R>>(&mut self, batch: &B,
                          count: device::InstanceCount, base: device::VertexCount,
                          output: &O) -> Result<(), DrawError<B::Error>> {
        self.draw_all(batch, Some((count, base)), output)
    }

    /// Draw a 'batch' with all known parameters specified, internal use only.
    fn draw_all<B: Batch<R>, O: target::Output<R>>(&mut self, batch: &B,
                instances: InstanceOption, output: &O)
                -> Result<(), DrawError<B::Error>> {
        let (mesh, attrib_iter, slice, state) = match batch.get_data() {
            Ok(data) => data,
            Err(e) => return Err(DrawError::InvalidBatch(e)),
        };
        let target_missing = state.get_target_mask() - output.get_mask();
        if !target_missing.is_empty() {
            error!("Error drawing to the output {:?}. ", output.get_handle());
            error!("Output mask: {:?}, State mask: {:?}, difference: {:?}",
                output.get_mask(), state.get_target_mask(), target_missing);
            return Err(DrawError::MissingTarget(target_missing))
        }
        self.bind_output(output);
        let program = match self.bind_program(batch) {
            Ok(p) => p,
            Err(e) => return Err(DrawError::InvalidBatch(e)),
        };
        self.bind_state(state);
        self.bind_mesh(mesh, attrib_iter, program.get_info());
        self.draw_slice(slice, instances);
        Ok(())
    }

    /// Blit one frame onto another.
    pub fn blit<I: target::Output<R>, O: target::Output<R>>(&mut self,
                source: &I, source_rect: Rect,
                destination: &O, dest_rect: Rect,
                mirror: Mirror, mask: Mask) {
        debug_assert!(source.get_mask().contains(mask));
        debug_assert!(destination.get_mask().contains(mask));
        self.bind_output(destination);
        self.bind_pixel_input(source);
        self.command_buffer.call_blit(source_rect, dest_rect, mirror, mask);
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Copy>(&mut self, buf: &handle::Buffer<R, T>,
                             data: &[T], offset_elements: usize) {
        let esize = mem::size_of::<T>();
        let offset_bytes = esize * offset_elements;
        debug_assert!(data.len() * esize + offset_bytes <= buf.get_info().size);
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_buffer(
            self.handles.ref_buffer(buf.raw()), pointer, offset_bytes);
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: Copy>(&mut self,
                                buf: &handle::Buffer<R, U>, data: &T) {
        assert!(mem::size_of::<T>() <= buf.get_info().size);
        let pointer = self.data_buffer.add_struct(data);
        self.command_buffer.update_buffer(
            self.handles.ref_buffer(buf.raw()), pointer, 0);
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Copy>(&mut self, tex: &handle::Texture<R>,
                          img: device::tex::ImageInfo, data: &[T]) {
        debug_assert!(tex.get_info().contains(&img));
        let pointer = self.data_buffer.add_vec(data);
        self.command_buffer.update_texture(tex.get_info().kind, self.handles.ref_texture(tex), img, pointer);
    }

    fn bind_output<O: target::Output<R>>(&mut self, output: &O) {
        let (width, height) = output.get_size();
        if self.render_state.frame.width != width ||
                self.render_state.frame.height != height {
            self.command_buffer.set_viewport(Rect {x: 0, y: 0, w: width, h: height});
            self.render_state.frame.width = width;
            self.render_state.frame.height = height;
        }
        let gamma = output.get_gamma();
        let change_gamma = self.render_state.gamma != gamma;

        match output.get_handle() {
            Some(ref handle) => {
                if self.render_state.frame_buffer.as_ref() != Some(handle) || change_gamma {
                    self.command_buffer.bind_frame_buffer(Access::Draw,
                        self.handles.ref_frame_buffer(handle),
                        gamma);
                    self.render_state.frame_buffer = Some((*handle).clone());
                    self.render_state.gamma = gamma;
                }
            },
            None => {
                if self.render_state.frame_buffer.as_ref() != Some(&self.draw_frame_buffer) || change_gamma {
                    self.command_buffer.bind_frame_buffer(Access::Draw,
                        self.handles.ref_frame_buffer(&self.draw_frame_buffer),
                        gamma);
                    self.render_state.frame_buffer = Some(self.draw_frame_buffer.clone());
                    self.render_state.gamma = gamma;
                }
                let colors = output.get_colors();
                // cut off excess color planes
                for (i, _) in self.render_state.frame.colors.iter().enumerate()
                                    .skip(colors.len()) {
                    self.command_buffer.unbind_target(Access::Draw, Target::Color(i as u8));
                }
                self.render_state.frame.colors.truncate(colors.len());
                // bind intersecting subsets
                for (i, (cur, new)) in self.render_state.frame.colors.iter_mut()
                                           .zip(colors.iter()).enumerate() {
                    if *cur != *new {
                        self.command_buffer.bind_target(&mut self.handles,
                            Access::Draw, Target::Color(i as u8), Some(new));
                        *cur = new.clone();
                    }
                }
                // activate the color targets that were just bound
                self.command_buffer.set_draw_color_buffers(colors.len());
                // append new planes
                for (i, new) in colors.iter().enumerate()
                                      .skip(self.render_state.frame.colors.len()) {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Color(i as u8), Some(new));
                    self.render_state.frame.colors.push(new.clone());
                }
                // set depth
                let depth = output.get_depth();
                if self.render_state.frame.depth.as_ref() != depth {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Depth, depth);
                    self.render_state.frame.depth = depth.map(|p| p.clone());
                }
                // set stencil
                let stencil = output.get_stencil();
                if self.render_state.frame.stencil.as_ref() != stencil {
                    self.command_buffer.bind_target(&mut self.handles,
                        Access::Draw, Target::Stencil, stencil);
                    self.render_state.frame.stencil = stencil.map(|p| p.clone());
                }
            },
        }
    }

    fn bind_pixel_input<I: target::Output<R>>(&mut self, input: &I) {
        self.command_buffer.bind_frame_buffer(Access::Read,
            self.handles.ref_frame_buffer(&self.read_frame_buffer),
            Gamma::Original);
        // color
        match input.get_colors().first() {
            Some(ref color) => {
                self.command_buffer.bind_target(&mut self.handles,
                    Access::Read, Target::Color(0), Some(color));
            },
            None => {
                self.command_buffer.unbind_target(Access::Read, Target::Color(0));
            },
        }
        // depth/stencil
        self.command_buffer.bind_target(&mut self.handles,
            Access::Read, Target::Depth, input.get_depth());
        self.command_buffer.bind_target(&mut self.handles,
            Access::Read, Target::Stencil, input.get_stencil());
    }

    fn bind_state(&mut self, state: &DrawState) {
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
                self.render_state.draw.primitive.get_cull_face() != state.primitive.get_cull_face() {
            self.command_buffer.set_depth_stencil(state.depth, state.stencil,
                state.primitive.get_cull_face());
        }
        if self.render_state.draw.blend != state.blend {
            self.command_buffer.set_blend(state.blend);
        }
        if self.render_state.draw.color_mask != state.color_mask {
            self.command_buffer.set_color_mask(state.color_mask);
        }
        self.render_state.draw = *state;
    }

    fn bind_program<'a, B: Batch<R>>(&mut self, batch: &'a B)
                    -> Result<&'a handle::Program<R>, B::Error> {
        let program = match batch.fill_params(&mut self.parameters) {
            Ok(p) => p,
            Err(e) => return Err(e),
        };
        //Warning: this is not protected against deleted resources in single-threaded mode
        if self.render_state.program.as_ref() != Some(&program) {
            self.render_state.program = Some(program.clone());
            self.command_buffer.bind_program(
                self.handles.ref_program(&program));
        }
        self.upload_parameters(program);
        Ok(program)
    }

    fn upload_parameters(&mut self, program: &handle::Program<R>) {
        let info = program.get_info();
        // bind uniforms
        for (var, value) in info.uniforms.iter()
            .zip(self.parameters.uniforms.iter()) {
            match value {
                &Some(v) => self.command_buffer.bind_uniform(var.location, v),
                &None => error!("Missed uniform {}", var.name),
            }
        }
        // bind uniform blocks
        for (i, (var, value)) in info.blocks.iter()
            .zip(self.parameters.blocks.iter()).enumerate() {
            match value {
                &Some(ref buf) => self.command_buffer.bind_uniform_block(
                    self.handles.ref_program(program),
                    i as device::UniformBufferSlot,
                    i as device::UniformBlockIndex,
                    self.handles.ref_buffer(buf)
                ),
                &None => error!("Missed block {}", var.name),
            }
        }
        // bind textures and samplers
        for (i, (var, value)) in info.textures.iter()
            .zip(self.parameters.textures.iter()).enumerate() {
            match value {
                &Some((ref tex, ref sampler)) => {
                    let sam = match sampler {
                        &Some(ref s) => {
                            if tex.get_info().kind.get_aa_mode().is_some() {
                                error!("A sampler provided for an AA texture: {}", var.name);
                            }
                            Some((self.handles.ref_sampler(s), *s.get_info()))
                        },
                        &None => None,
                    };
                    self.command_buffer.bind_uniform(var.location, UniformValue::I32(i as i32));
                    self.command_buffer.bind_texture(i as device::TextureSlot, tex.get_info().kind,
                        self.handles.ref_texture(tex), sam);
                },
                &None => error!("Missed texture {}", var.name),
            }
        }
    }

    fn bind_mesh<I: Iterator<Item = mesh::AttributeIndex>>(&mut self,
                 mesh: &mesh::Mesh<R>, attrib_iter: I, info: &ProgramInfo) {
        if !self.render_state.is_array_buffer_set {
            // It's Ok if the array buffer is not supported. We can just ignore it.
            match self.common_array_buffer {
                Ok(ref ab) => self.command_buffer.bind_array_buffer(
                    self.handles.ref_array_buffer(ab)
                ),
                Err(()) => (),
            };
            self.render_state.is_array_buffer_set = true;
        }
        for (attr_index, sat) in attrib_iter.zip(info.attributes.iter()) {
            let vat = &mesh.attributes[attr_index];
            let loc = sat.location;
            if loc >= self.render_state.attributes.len() {
                let range = self.render_state.attributes.len() .. loc+1;
                self.render_state.attributes.extend(range.map(|_| None));
            }
            let need_update = match self.render_state.attributes[loc] {
                Some((ref buf, fmt)) => *buf != vat.buffer || fmt != vat.format,
                None => true,
            };
            if need_update {
                self.command_buffer.bind_attribute(loc as device::AttributeSlot,
                    self.handles.ref_buffer(&vat.buffer), vat.format);
                self.render_state.attributes[loc] = Some((vat.buffer.clone(), vat.format));
            }
        }
    }

    fn bind_index<T>(&mut self, buf: &handle::IndexBuffer<R, T>) {
        if self.render_state.index.as_ref() != Some(buf.raw()) {
            self.render_state.index = Some(buf.raw().clone());
            self.command_buffer.bind_index(self.handles.ref_buffer(buf.raw()));
        }
    }

    fn draw_slice(&mut self, slice: &mesh::Slice<R>, instances: InstanceOption) {
        let &mesh::Slice { start, end, prim_type, ref kind } = slice;
        match *kind {
            SliceKind::Vertex => {
                self.command_buffer.call_draw(prim_type, start, end - start, instances);
            },
            SliceKind::Index8(ref buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U8,
                    start, end - start, base, instances);
            },
            SliceKind::Index16(ref buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U16,
                    start, end - start, base, instances);
            },
            SliceKind::Index32(ref buf, base) => {
                self.bind_index(buf);
                self.command_buffer.call_draw_indexed(prim_type, IntSize::U32,
                    start, end - start, base, instances);
            },
        }
    }
}

/// Factory extension that allows creating new renderers.
pub trait RenderFactory<R: device::Resources, C: CommandBuffer<R>> {
    /// Create a new renderer
    fn create_renderer(&mut self) -> Renderer<R, C>;
}

impl<
    R: Resources,
    C: CommandBuffer<R>,
    F: device::Factory<R>,
> RenderFactory<R, C> for F {
    fn create_renderer(&mut self) -> Renderer<R, C> {
        Renderer {
            command_buffer: CommandBuffer::new(),
            data_buffer: DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: self.create_array_buffer(),
            draw_frame_buffer: self.create_frame_buffer(),
            read_frame_buffer: self.create_frame_buffer(),
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }
}
