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
use device::BoxBlobCast;
use device::draw::CommandBuffer;
use device::shade::{ProgramInfo, UniformValue, ShaderSource,
    Vertex, Fragment, CreateShaderError};
use device::attrib::{U8, U16, U32};
use batch::Batch;
use mesh;
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

/// An error with a defined Mesh.
#[deriving(Show)]
pub enum MeshError {
    /// A required attribute was missing.
    ErrorAttributeMissing(String),
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
pub enum ProgramError {
    /// Unable to compile the vertex shader
    ErrorVertex(CreateShaderError),
    /// Unable to compile the fragment shader
    ErrorFragment(CreateShaderError),
    /// Unable to link
    ErrorLink(()),
}

/// Graphics state
#[allow(dead_code)]
// This is going to be used to do minimal state transfers between draw calls. Not yet implemented!
struct State {
    frame: target::Frame,
    draw_state: state::DrawState,
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
pub struct Renderer {
    buf: device::ActualCommandBuffer,
    common_array_buffer: Result<device::ArrayBufferHandle, ()>,
    common_frame_buffer: device::FrameBufferHandle,
    default_frame_buffer: device::FrameBufferHandle,
    state: State,
    parameters: ParamStorage,
}

impl Renderer {
    /// Reset all commands for the command buffer re-usal.
    pub fn reset(&mut self) {
        self.buf.clear();
    }

    /// Get a command buffer to be submitted
    pub fn as_buffer(&self) -> &device::ActualCommandBuffer {
        &self.buf
    }

    /// Clone the renderer shared data but ignore the commands
    pub fn clone_empty(&self) -> Renderer {
        Renderer {
            buf: CommandBuffer::new(),
            common_array_buffer: self.common_array_buffer,
            common_frame_buffer: self.common_frame_buffer,
            default_frame_buffer: self.default_frame_buffer,
            state: State {
                frame: target::Frame::new(0,0),
                draw_state: state::DrawState::new(),
            },
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
        let (mesh, slice, program, state) = batch.get_data();
        self.bind_program(&batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, program.get_info()).unwrap();  //TODO: mesh link
        self.draw_slice(slice, None);
    }

    /// Draw a `batch` multiple times using instancing
    pub fn draw_instanced<B: Batch>(&mut self, batch: B,
                          count: device::InstanceCount, frame: &target::Frame) {
        self.bind_frame(frame);
        let (mesh, slice, program, state) = batch.get_data();
        self.bind_program(&batch, program);
        self.bind_state(state);
        self.bind_mesh(mesh, program.get_info()).unwrap();  //TODO: mesh link
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
            ((box data) as Box<device::Blob<T> + Send>).cast(),
            offset_bytes
        );
    }

    /// Update a buffer with data from a single type.
    pub fn update_buffer_struct<U, T: device::Blob<U>+Send>(&mut self,
                                buf: device::BufferHandle<U>, data: T) {
        debug_assert!(size_of::<T>() <= buf.get_info().size);
        self.buf.update_buffer(
            buf.get_name(),
            ((box data) as Box<device::Blob<U> + Send>).cast(),
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
            ((box data) as Box<device::Blob<T> + Send>).cast()
        );
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
            self.buf.bind_frame_buffer(self.default_frame_buffer.get_name());
        } else {
            self.buf.bind_frame_buffer(self.common_frame_buffer.get_name());
            for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
                if *cur != *new {
                    Renderer::bind_target(&mut self.buf, device::target::TargetColor(i as u8), *new);
                }
            }
            if self.state.frame.depth != frame.depth {
                Renderer::bind_target(&mut self.buf, device::target::TargetDepth, frame.depth);
            }
            if self.state.frame.stencil != frame.stencil {
                Renderer::bind_target(&mut self.buf, device::target::TargetStencil, frame.stencil);
            }
            self.state.frame = *frame;
        }
    }

    fn bind_state(&mut self, state: &state::DrawState) {
        self.buf.set_primitive(state.primitive);
        self.buf.set_scissor(state.scissor);
        self.buf.set_depth_stencil(state.depth, state.stencil,
            state.primitive.get_cull_mode());
        self.buf.set_blend(state.blend);
        self.buf.set_color_mask(state.color_mask);
    }

    fn bind_program<B: Batch>(&mut self, batch: &B, program: &device::ProgramHandle) {
        self.buf.bind_program(program.get_name());
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
        self.common_array_buffer.map(|ab| self.buf.bind_array_buffer(ab.get_name())).is_ok();
        for sat in info.attributes.iter() {
            match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
                Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => {
                        self.buf.bind_attribute(
                            sat.location as device::AttributeSlot,
                            vat.buffer.get_name(), vat.elem_count, vat.elem_type,
                            vat.stride, vat.offset, vat.instance_rate);
                    },
                    Err(_) => return Err(ErrorAttributeType)
                },
                None => return Err(ErrorAttributeMissing(sat.name.clone()))
            }
        }
        Ok(())
    }

    fn draw_slice(&mut self, slice: mesh::Slice,
                  instances: Option<device::InstanceCount>) {
        match slice {
            mesh::VertexSlice(prim_type, start, end) => {
                self.buf.call_draw(prim_type, start, end, instances);
            },
            mesh::IndexSlice8(prim_type, buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(prim_type, U8, start, end, instances);
            },
            mesh::IndexSlice16(prim_type, buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(prim_type, U16, start, end, instances);
            },
            mesh::IndexSlice32(prim_type, buf, start, end) => {
                self.buf.bind_index(buf.get_name());
                self.buf.call_draw_indexed(prim_type, U32, start, end, instances);
            },
        }
    }
}


/// Backend extension trait for convenience methods
pub trait DeviceHelper {
    /// Create a new renderer
    fn create_renderer(&mut self) -> Renderer;
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from_format`.
    fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>) -> mesh::Mesh;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource)
                    -> Result<device::ProgramHandle, ProgramError>;
}

impl<D: device::Device> DeviceHelper for D {
    fn create_renderer(&mut self) -> Renderer {
        Renderer {
            buf: CommandBuffer::new(),
            common_array_buffer: self.create_array_buffer(),
            common_frame_buffer: self.create_frame_buffer(),
            default_frame_buffer: device::get_main_frame_buffer(),
            //TODO: make sure this is HW default
            state: State {
                frame: target::Frame::new(0,0),
                draw_state: state::DrawState::new(),
            },
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
