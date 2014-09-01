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

//! Rendering front-end

use std::mem::size_of;
use device;
use device::blob::{Blob, BoxBlobCast};
use device::draw::CommandBuffer;
use device::shade::{ProgramInfo, UniformValue, ShaderSource,
    Vertex, Fragment, CreateShaderError};
use device::attrib;
use batch::Batch;
use mesh;
use mesh::ToSlice;
use shade;
use state;
use target;

/// An error with an invalid texture or uniform block.
//TODO: use slices when Rust allows
#[deriving(Show)]
pub enum ParameterError {
    /// Error from a uniform value
    ErrorParamUniform(String),
    /// Error from a uniform block.
    ErrorParamBlock(String),
    /// Error from a texture.
    ErrorParamTexture(String),
    /// Error from a sampler
    ErrorParamSampler(String),
}

/// Program linking error
#[deriving(Clone, PartialEq, Show)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    ErrorVertex(CreateShaderError),
    /// Unable to compile the fragment shader
    ErrorFragment(CreateShaderError),
    /// Unable to link
    ErrorLink(()),
}

static TRACKED_ATTRIBUTES: uint = 8;
type CachedAttribute = (device::RawBufferHandle, attrib::Format);

/// Graphics state. Used as a cache to figure out redundant state changes.
struct State {
    is_frame_buffer_set: bool,
    frame: target::Frame,
    is_array_buffer_set: bool,
    program_name: device::back::Program,
    index: Option<device::RawBufferHandle>,
    attributes: [Option<CachedAttribute>, .. TRACKED_ATTRIBUTES],
    draw: state::DrawState,
}

impl State {
    /// Generate the initial state matching `Device::reset_state`
    fn new() -> State {
        State {
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
    uniforms: Vec<Option<UniformValue>>,
    blocks  : Vec<Option<device::RawBufferHandle>>,
    textures: Vec<Option<shade::TextureParam>>,
}

impl ParamStorage{
    fn new() -> ParamStorage {
        ParamStorage {
            uniforms: Vec::new(),
            blocks: Vec::new(),
            textures: Vec::new(),
        }
    }

    fn resize(&mut self, nu: uint, nb: uint, nt: uint) {
        self.uniforms.truncate(0);
        self.uniforms.grow(nu, &None);
        self.blocks.truncate(0);
        self.blocks.grow(nb, &None);
        self.textures.truncate(0);
        self.textures.grow(nt, &None);
    }

    fn as_mut_slice(&mut self) -> shade::ParamValues {
        shade::ParamValues {
            uniforms: self.uniforms.as_mut_slice(),
            blocks: self.blocks.as_mut_slice(),
            textures: self.textures.as_mut_slice(),
        }
    }
}

/// Renderer front-end
pub struct Renderer<C: device::draw::CommandBuffer> {
    buf: C,
    common_array_buffer: Result<device::ArrayBufferHandle, ()>,
    common_frame_buffer: device::FrameBufferHandle,
    default_frame_buffer: device::FrameBufferHandle,
    state: State,
    parameters: ParamStorage,
}

impl<C: device::draw::CommandBuffer> Renderer<C> {
    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.state = State::new();
    }

    /// Get a command buffer to be submitted
    pub fn as_buffer(&self) -> &C {
        &self.buf
    }

    /// Clone the renderer shared data but ignore the commands
    pub fn clone_empty(&self) -> Renderer<C> {
        Renderer {
            buf: CommandBuffer::new(),
            common_array_buffer: self.common_array_buffer,
            common_frame_buffer: self.common_frame_buffer,
            default_frame_buffer: self.default_frame_buffer,
            state: State::new(),
            parameters: ParamStorage::new(),
        }
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: device::target::ClearData, frame: &target::Frame) {
        self.bind_frame(frame);
        self.buf.call_clear(data);
    }

    /// Draw a `batch` into the specified `frame`
    pub fn draw<B: Batch>(&mut self, batch: B, frame: &target::Frame) {
        self.bind_frame(frame);
        let (mesh, link, slice, program, state) = batch.get_data();
        self.bind_program(&batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, link, program.get_info());
        self.draw_slice(slice, None);
    }

    /// Draw a `batch` multiple times using instancing
    pub fn draw_instanced<B: Batch>(&mut self, batch: B,
                          count: device::InstanceCount, frame: &target::Frame) {
        self.bind_frame(frame);
        let (mesh, link, slice, program, state) = batch.get_data();
        self.bind_program(&batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, link, program.get_info());
        self.draw_slice(slice, Some(count));
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Send>(&mut self, buf: device::BufferHandle<T>,
                             data: Vec<T>, offset_elements: uint) {
        let esize = size_of::<T>();
        let offset_bytes = esize * offset_elements;
        debug_assert!(data.len() * esize + offset_bytes <= buf.get_info().size);
        self.buf.update_buffer(
            buf.get_name(),
            ((box data) as Box<Blob<T> + Send>).cast(),
            offset_bytes
        );
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: Blob<U>+Send>(&mut self,
                                buf: device::BufferHandle<U>, data: T) {
        debug_assert!(size_of::<T>() <= buf.get_info().size);
        self.buf.update_buffer(
            buf.get_name(),
            ((box data) as Box<Blob<U> + Send>).cast(),
            0
        );
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Send>(&mut self, tex: device::TextureHandle,
                                   img: device::tex::ImageInfo, data: Vec<T>) {
        debug_assert!(tex.get_info().contains(&img));
        self.buf.update_texture(
            tex.get_info().kind,
            tex.get_name(),
            img,
            ((box data) as Box<Blob<T> + Send>).cast()
        );
    }

    fn bind_target<C: device::draw::CommandBuffer>(
        buf: &mut C,
        to: device::target::Target,
        plane: target::Plane
    ) {
        match plane {
            target::PlaneEmpty =>
                buf.unbind_target(to),
            target::PlaneSurface(suf) =>
                buf.bind_target_surface(to, suf),
            target::PlaneTexture(tex, level, layer) =>
                buf.bind_target_texture(to, tex, level, layer),
        }
    }

    fn bind_frame(&mut self, frame: &target::Frame) {
        if self.state.frame.width != frame.width || self.state.frame.height != frame.height {
            self.buf.set_viewport(device::target::Rect {
                x: 0,
                y: 0,
                w: frame.width,
                h: frame.height,
            });
            self.state.frame.width = frame.width;
            self.state.frame.height = frame.height;
        }
        if frame.is_default() {
            if self.state.is_frame_buffer_set {
                // binding the default FBO, not touching our common one
                self.buf.bind_frame_buffer(self.default_frame_buffer.get_name());
                self.state.is_frame_buffer_set = false;
            }
        } else {
            if !self.state.is_frame_buffer_set {
                self.buf.bind_frame_buffer(self.common_frame_buffer.get_name());
                self.state.is_frame_buffer_set = true;
            }
            for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    Renderer::<C>::bind_target(&mut self.buf, device::target::TargetColor(i as u8), *new);
                }
            }
            if self.state.frame.depth != frame.depth {
                Renderer::<C>::bind_target(&mut self.buf, device::target::TargetDepth, frame.depth);
            }
            if self.state.frame.stencil != frame.stencil {
                Renderer::<C>::bind_target(&mut self.buf, device::target::TargetStencil, frame.stencil);
            }
            self.state.frame = *frame;
        }
    }

    fn bind_state(&mut self, state: &state::DrawState) {
        if self.state.draw.primitive != state.primitive {
            self.buf.set_primitive(state.primitive);
        }
		if self.state.draw.multi_sample != state.multi_sample {
			self.buf.set_multi_sample(state.multi_sample);
        }
        if self.state.draw.scissor != state.scissor {
            self.buf.set_scissor(state.scissor);
        }
        if self.state.draw.depth != state.depth || self.state.draw.stencil != state.stencil ||
                self.state.draw.primitive.get_cull_mode() != state.primitive.get_cull_mode() {
            self.buf.set_depth_stencil(state.depth, state.stencil,
                state.primitive.get_cull_mode());
        }
        if self.state.draw.blend != state.blend {
            self.buf.set_blend(state.blend);
        }
        if self.state.draw.color_mask != state.color_mask {
            self.buf.set_color_mask(state.color_mask);
        }
        self.state.draw = *state;
    }

    fn bind_program<B: Batch>(&mut self, batch: &B, program: &device::ProgramHandle) {
        //Warning: this is not protected against deleted resources in single-threaded mode
        if self.state.program_name != program.get_name() {
            self.buf.bind_program(program.get_name());
            self.state.program_name = program.get_name();
        }
        let pinfo = program.get_info();
        self.parameters.resize(pinfo.uniforms.len(), pinfo.blocks.len(),
            pinfo.textures.len());
        batch.fill_params(self.parameters.as_mut_slice());
        self.upload_parameters(program).unwrap();
    }

    fn upload_parameters(&mut self, program: &device::ProgramHandle) -> Result<(), ParameterError> {
        // bind uniforms
        for (var, &option) in program.get_info().uniforms.iter()
            .zip(self.parameters.uniforms.iter()) {
            match option {
                Some(v) => self.buf.bind_uniform(var.location, v),
                None => return Err(ErrorParamUniform(var.name.clone())),
            }
        }
        // bind uniform blocks
        for (i, (var, &option)) in program.get_info().blocks.iter()
            .zip(self.parameters.blocks.iter()).enumerate() {
            match option {
                Some(buf) => self.buf.bind_uniform_block(
                    program.get_name(),
                    i as device::UniformBufferSlot,
                    i as device::UniformBlockIndex,
                    buf.get_name()
                ),
                None => return Err(ErrorParamBlock(var.name.clone())),
            }
        }
        // bind textures and samplers
        for (i, (var, &option)) in program.get_info().textures.iter()
            .zip(self.parameters.textures.iter()).enumerate() {
            match option {
                Some((tex, Some(_))) if tex.get_info().kind.get_aa_mode().is_some() =>
                    return Err(ErrorParamSampler(var.name.clone())),
                Some((tex, sampler)) => {
                    self.buf.bind_uniform(var.location, device::shade::ValueI32(i as i32));
                    self.buf.bind_texture(i as device::TextureSlot,
                        tex.get_info().kind, tex.get_name(), sampler);
                },
                None => return Err(ErrorParamTexture(var.name.clone())),
            }
        }
        Ok(())
    }

    fn bind_mesh(&mut self, mesh: &mesh::Mesh, link: &mesh::Link, info: &ProgramInfo) {
        if !self.state.is_array_buffer_set {
            // It's Ok if the array buffer is not supported. We can just ignore it.
            self.common_array_buffer.map(|ab|
                self.buf.bind_array_buffer(ab.get_name())
            ).is_ok();
            self.state.is_array_buffer_set = true;
        }
        for (attr_index, sat) in link.attribute_indices().zip(info.attributes.iter()) {
            let vat = &mesh.attributes[attr_index];
            let loc = sat.location as uint;
            let need_update = loc >= self.state.attributes.len() ||
                match self.state.attributes[loc] {
                    Some((buf, fmt)) => buf != vat.buffer || fmt != vat.format,
                    None => true,
                };
            if need_update {
                self.buf.bind_attribute(loc as device::AttributeSlot,
                    vat.buffer.get_name(), vat.format);
                if loc < self.state.attributes.len() {
                    self.state.attributes[loc] = Some((vat.buffer, vat.format));
                }
            }
        }
    }

    fn bind_index<T>(&mut self, buf: device::BufferHandle<T>) {
        if self.state.index != Some(buf.raw()) {
            self.buf.bind_index(buf.get_name());
            self.state.index = Some(buf.raw());
        }
    }

    fn draw_slice(&mut self, slice: &mesh::Slice,
                  instances: Option<device::InstanceCount>) {
        match *slice {
            mesh::VertexSlice(prim_type, start, end) => {
                self.buf.call_draw(prim_type, start, end, instances);
            },
            mesh::IndexSlice8(prim_type, buf, start, end) => {
                self.bind_index(buf);
                self.buf.call_draw_indexed(prim_type, attrib::U8, start, end, instances);
            },
            mesh::IndexSlice16(prim_type, buf, start, end) => {
                self.bind_index(buf);
                self.buf.call_draw_indexed(prim_type, attrib::U16, start, end, instances);
            },
            mesh::IndexSlice32(prim_type, buf, start, end) => {
                self.bind_index(buf);
                self.buf.call_draw_indexed(prim_type, attrib::U32, start, end, instances);
            },
        }
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceHelper<C: device::draw::CommandBuffer> {
    /// Create a new renderer
    fn create_renderer(&mut self) -> Renderer<C>;
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from_format`.
    fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<device::ProgramHandle, ProgramError>;
}

impl<D: device::Device<C>,
     C: device::draw::CommandBuffer> DeviceHelper<C> for D {
    fn create_renderer(&mut self) -> Renderer<C> {
        Renderer {
            buf: CommandBuffer::new(),
            common_array_buffer: self.create_array_buffer(),
            common_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: device::get_main_frame_buffer(),
            state: State::new(),
            parameters: ParamStorage::new(),
        }
    }

    fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh {
        let nv = data.len();
        debug_assert!(nv < {
            use std::num::Bounded;
            let val: device::VertexCount = Bounded::max_value();
            val as uint
        });
        let buf = self.create_buffer_static(&data);
        mesh::Mesh::from_format(buf, nv as device::VertexCount)
    }

    fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<device::ProgramHandle, ProgramError> {
        let vs = match self.create_shader(Vertex, vs_src) {
            Ok(s) => s,
            Err(e) => return Err(ErrorVertex(e)),
        };
        let fs = match self.create_shader(Fragment, fs_src) {
            Ok(s) => s,
            Err(e) => return Err(ErrorFragment(e)),
        };
        self.create_program([vs, fs]).map_err(|e| ErrorLink(e))
    }
}
