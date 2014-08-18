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

use device;
use backend = device::back;
use device::draw::CommandBuffer;
use device::shade::{ProgramInfo, ShaderSource, Vertex, Fragment, CreateShaderError};
use device::attrib::{U8, U16, U32};
use mesh;
use shade;
use shade::{Program, ShaderParam};
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

/// An error with a defined Mesh.
#[deriving(Show)]
pub enum MeshError {
    /// A required attribute was missing.
    ErrorAttributeMissing,
    /// An attribute's type from the vertex format differed from the type used in the shader.
    ErrorAttributeType,
}

/// An error that can happen when trying to draw.
#[deriving(Show)]
pub enum DrawError {
    /// Error with a program.
    ErrorProgram,
    /// Error with the program shell.
    ErrorParameter(ParameterError),
    /// Error with the mesh.
    ErrorMesh(MeshError),
    /// Error with the mesh slice
    ErrorSlice,
}

/// Program linking error
#[deriving(Clone, PartialEq, Show)]
pub enum ProgramError<'a> {
    /// Unable to compile the vertex shader
    ErrorVertex(CreateShaderError),
    /// Unable to compile the fragment shader
    ErrorFragment(CreateShaderError),
    /// Unable to link
    ErrorLink(()),
    /// Unable to connect parameters
    ErrorParameters(shade::ParameterError<'a>),
}

/// Graphics state
struct State {
    frame: target::Frame,
    draw_state: state::DrawState,
}

/// Backend extension trait for convenience methods
pub trait DeviceHelper {
    /// Create a new draw list
    fn create_draw_list(&mut self) -> DrawList;
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from`.
    fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program<'a, L, T: ShaderParam<L>>(&mut self, data: T, vs_src: ShaderSource,
                   fs_src: ShaderSource) -> Result<shade::UserProgram<L, T>, ProgramError>;
}

impl<D: device::Device> DeviceHelper for D {
    fn create_draw_list(&mut self) -> DrawList {
        DrawList {
            buf: CommandBuffer::new(),
            common_array_buffer: self.create_array_buffer(),
            common_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: 0,
            //TODO: make sure this is HW default
            state: State {
                frame: target::Frame::new(0,0),
                draw_state: state::DrawState::new(),
            },
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
        mesh::Mesh::from::<T>(buf, nv as device::VertexCount)
    }

    fn link_program<'a, L, T: ShaderParam<L>>(&mut self, data: T,
                    vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<shade::UserProgram<L, T>, ProgramError> {
        let vs = match self.create_shader(Vertex, vs_src) {
            Ok(s) => s,
            Err(e) => return Err(ErrorVertex(e)),
        };
        let fs = match self.create_shader(Fragment, fs_src) {
            Ok(s) => s,
            Err(e) => return Err(ErrorFragment(e)),
        };
        let prog = match self.create_program([vs, fs]) {
            Ok(p) => p,
            Err(e) => return Err(ErrorLink(e)),
        };
        shade::UserProgram::connect(prog, data).map_err(|e| ErrorParameters(e))
    }
}

/// Renderer front-end
pub struct DrawList {
    buf: device::ActualCommandBuffer,
    common_array_buffer: Result<backend::ArrayBuffer, ()>,
    common_frame_buffer: backend::FrameBuffer,
    default_frame_buffer: backend::FrameBuffer,
    state: State,
}

impl DrawList {
    /// Reset all commands for draw list re-usal.
    pub fn reset(&mut self) {
        self.buf.clear();
    }

    /// Get the draw list to be submitted.
    pub fn as_slice(&self) -> &device::ActualCommandBuffer {
        &self.buf
    }

    /// Clone the draw list shared data but ignore the commands
    pub fn clone_empty(&self) -> DrawList {
        DrawList {
            buf: CommandBuffer::new(),
            common_array_buffer: self.common_array_buffer,
            common_frame_buffer: self.common_frame_buffer,
            default_frame_buffer: self.default_frame_buffer,
            state: State {
                frame: target::Frame::new(0,0),
                draw_state: state::DrawState::new(),
            },
        }
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: device::target::ClearData, frame: &target::Frame) {
        self.bind_frame(frame);
        self.buf.call_clear(data);
    }

    /// Draw `slice` of `mesh` into `frame`, using a program shell, and a given draw state.
    pub fn draw<P: Program>(&mut self, mesh: &mesh::Mesh, slice: mesh::Slice,
                            frame: &target::Frame, program: &P, state: &state::DrawState)
                            -> Result<(), DrawError> {
        self.bind_frame(frame);
        match self.bind_program(program) {
            Ok(_) => (),
            Err(e) => return Err(ErrorParameter(e)),
        }
        // bind fixed-function states
        self.buf.set_primitive(state.primitive);
        self.buf.set_scissor(state.scissor);
        self.buf.set_depth_stencil(state.depth, state.stencil,
            state.primitive.get_cull_mode());
        self.buf.set_blend(state.blend);
        self.buf.set_color_mask(state.color_mask);
        // bind mesh data
        match self.bind_mesh(mesh, program.get_handle().get_info()) {
            Ok(_) => (),
            Err(e) => return Err(ErrorMesh(e)),
        }
        // draw
        match slice {
            mesh::VertexSlice(start, end) => {
                self.buf.call_draw(mesh.prim_type, start, end);
            },
            mesh::IndexSlice8(buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(mesh.prim_type, U8, start, end);
            },
            mesh::IndexSlice16(buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(mesh.prim_type, U16, start, end);
            },
            mesh::IndexSlice32(buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(mesh.prim_type, U32, start, end);
            },
        }
        Ok(())
    }

    /// Update a buffer with data from a vector.
    pub fn update_buffer_vec<T: Send>(&mut self, buf: device::BufferHandle<T>,
                             data: Vec<T>) {
        self.buf.update_buffer(
            buf.get_name(),
            (box data) as Box<device::Blob<T> + Send>,
            buf.get_info().usage
        );
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: device::Blob<U>+Send>(&mut self,
                                buf: device::BufferHandle<U>, data: T) {
        self.buf.update_buffer(
            buf.get_name(),
            (box data) as Box<device::Blob<U> + Send>,
            buf.get_info().usage
        );
    }

    /// Update the contents of a texture.
    pub fn update_texture<T: Send>(&mut self, tex: device::TextureHandle,
                                   img: device::tex::ImageInfo, data: Vec<T>) {
        self.buf.update_texture(tex.get_info().kind, tex.get_name(), img,
                                 (box data) as Box<device::Blob<T> + Send>);
    }

    fn bind_target(buf: &mut device::ActualCommandBuffer,
                   to: device::target::Target, plane: target::Plane) {
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
        self.buf.set_viewport(device::target::Rect {
            x: 0,
            y: 0,
            w: frame.width,
            h: frame.height,
        });
        if frame.is_default() {
            // binding the default FBO, not touching our common one
            self.buf.bind_frame_buffer(self.default_frame_buffer);
        } else {
            self.buf.bind_frame_buffer(self.common_frame_buffer);
            for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    DrawList::bind_target(&mut self.buf, device::target::TargetColor(i as u8), *new);
                }
            }
            if self.state.frame.depth != frame.depth {
                DrawList::bind_target(&mut self.buf, device::target::TargetDepth, frame.depth);
            }
            if self.state.frame.stencil != frame.stencil {
                DrawList::bind_target(&mut self.buf, device::target::TargetStencil, frame.stencil);
            }
            self.state.frame = *frame;
        }
    }

    fn bind_program<P: Program>(&mut self, prog: &P) -> Result<(), ParameterError> {
        let handle = prog.get_handle();
        self.buf.bind_program(handle.get_name());
        let pinfo = handle.get_info();
        // gather parameters
        // this is a bit ugly, not sure how to make it more sound
        let mut uniforms = Vec::from_elem(pinfo.uniforms.len(), None);
        let mut blocks   = Vec::from_elem(pinfo.blocks  .len(), None);
        let mut textures = Vec::from_elem(pinfo.textures.len(), None);
        prog.fill_params(shade::ParamValues {
            uniforms: uniforms.as_mut_slice(),
            blocks: blocks.as_mut_slice(),
            textures: textures.as_mut_slice(),
        });
        // bind uniforms
        for (var, option) in pinfo.uniforms.iter().zip(uniforms.move_iter()) {
            match option {
                Some(v) => self.buf.bind_uniform(var.location, v),
                None => return Err(ErrorParamUniform(var.name.clone())),
            }
        }
        // bind uniform blocks
        for (i, (var, option)) in pinfo.blocks.iter().zip(blocks.move_iter()).enumerate() {
            match option {
                Some(buf) => self.buf.bind_uniform_block(
                    handle.get_name(),
                    i as device::UniformBufferSlot,
                    i as device::UniformBlockIndex,
                    buf.get_name()
                ),
                None => return Err(ErrorParamBlock(var.name.clone())),
            }
        }
        // bind textures and samplers
        for (i, (var, option)) in pinfo.textures.iter().zip(textures.move_iter()).enumerate() {
            match option {
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

    fn bind_mesh(&mut self, mesh: &mesh::Mesh, info: &ProgramInfo)
                 -> Result<(), MeshError> {
        // It's Ok the array buffer is not supported. If so we just ignore it.
        self.common_array_buffer.map(|ab| self.buf.bind_array_buffer(ab));
        for sat in info.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => {
                        self.buf.bind_attribute(
                            sat.location as device::AttributeSlot,
                            vat.buffer.get_name(), vat.elem_count, vat.elem_type,
                            vat.stride, vat.offset);
                    },
                    Err(_) => return Err(ErrorAttributeType)
                },
                None => return Err(ErrorAttributeMissing)
            }
        }
        Ok(())
    }
}
