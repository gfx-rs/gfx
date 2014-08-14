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
use device::draw::DrawList;
use device::shade::{ProgramInfo, ShaderSource, Vertex, Fragment, CreateShaderError};
use mesh;
use shade;
use shade::{ProgramShell, ShaderParam};
use state;
use target;

/// An error with an invalid texture or uniform block.
//TODO: use slices when Rust allows
#[deriving(Show)]
pub enum ShellError {
	/// Error from a uniform value
	ErrorShellUniform(String),
	/// Error from a uniform block.
	ErrorShellBlock(String),
	/// Error from a texture.
	ErrorShellTexture(String),
	/// Error from a sampler
	ErrorShellSampler(String),
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
	ErrorShell(ShellError),
	/// Error with the mesh.
	ErrorMesh(MeshError),
	/// Error with the mesh slice
	ErrorSlice,
}

/// Graphics state
struct State {
	frame: target::Frame,
	fixed_function: state::DrawState,
}

/// Front-end manager
pub struct Manager {
	common_array_buffer: backend::ArrayBuffer,
	common_frame_buffer: backend::FrameBuffer,
	default_frame_buffer: backend::FrameBuffer,
}

#[repr(u8)]
#[deriving(Clone, PartialEq, Show)]
pub enum InitError {
	ErrorArrayBuffer,
	ErrorFramebuffer,
}

#[deriving(Clone, PartialEq, Show)]
pub enum ProgramError {
	ErrorVertex(CreateShaderError),
	ErrorFragment(CreateShaderError),
	ErrorLink(()),
}


impl<D, B: device::ApiBackEnd<D>> Manager {
	pub fn new(backend: &mut B) -> Result<Manager, InitError> {
		Ok(Manager {
			common_array_buffer: match backend.create_array_buffer() {
				Ok(vao) => vao,
				Err(_) => return Err(ErrorArrayBuffer),
			},
			common_frame_buffer: backend.create_frame_buffer(),
			default_frame_buffer: 0,
		})
	}

	/// Create a new render front-end
	pub fn spawn(&self) -> FrontEnd {
		FrontEnd {
			list: device::DrawList::new(),
			common_array_buffer: self.common_array_buffer,
			common_frame_buffer: self.common_frame_buffer,
			default_frame_buffer: self.default_frame_buffer,
			state: State {
				frame: target::Frame::new(0, 0),
				//TODO: force this to match the default HW state
				fixed_function: state::DrawState::new(),
			},
		}
	}

	/// Create a new mesh from the given vertex data.
	/// Convenience function around `create_buffer` and `Mesh::from`.
	pub fn create_mesh<T: mesh::VertexFormat + Send>(&mut self, data: Vec<T>,
					  backend: &mut B) -> mesh::Mesh {
		let nv = data.len();
		debug_assert!(nv < {
			use std::num::Bounded;
			let val: device::VertexCount = Bounded::max_value();
			val as uint
		});
		let buf = backend.create_buffer();
		backend.update_buffer(buf, &data, device::UsageStatic);
		mesh::Mesh::from::<T>(buf, nv as device::VertexCount)
	}

	/// Create a simple program given a vertex shader with a fragment one
	pub fn link_program(&mut self, vs_src: ShaderSource, fs_src: ShaderSource,
						backend: &mut B) -> Result<device::ProgramHandle, ProgramError> {
		//TODO: integrate connect_program here
		let vs = match backend.create_shader(Vertex, vs_src) {
			Ok(name) => name,
			Err(e) => return Err(ErrorVertex(e)),
		};
		let fs = match backend.create_shader(Fragment, fs_src) {
			Ok(name) => name,
			Err(e) => return Err(ErrorFragment(e)),
		};
		backend.create_program([vs, fs]).map_err(|e| ErrorLink(e))
	}

	/// Connect a shader program with a parameter structure
	pub fn connect_program<'a, L, T: ShaderParam<L>>
						  (&'a mut self, prog: device::ProgramHandle, data: T)
						  -> Result<shade::CustomShell<L, T>,
						  shade::ParameterLinkError<'a>> {
		let info = prog.get_info();
		let input = (info.uniforms.as_slice(), info.blocks.as_slice(),
			info.textures.as_slice());
		match data.create_link(input) {	//TODO: no clone
			Ok(link) => Ok(shade::CustomShell::new(prog.clone(), link, data)),
			Err(e) => Err(shade::ErrorUnusedParameter(e)),
		}
	}
}

pub struct FrontEnd {
	list: device::DrawList,
	common_array_buffer: backend::ArrayBuffer,
	common_frame_buffer: backend::FrameBuffer,
	default_frame_buffer: backend::FrameBuffer,
	state: State,
}

impl FrontEnd {
	/// Reset all commands for draw list re-usal.
	pub fn reset(&mut self) {
		self.list.clear();
	}

	/// Get the draw list to be submitted.
	pub fn as_slice(&self) -> &device::DrawList {
		&self.list
	}

	/// Clear the `Frame` as the `ClearData` specifies.
	pub fn clear(&mut self, data: device::target::ClearData, frame: &target::Frame) {
		self.bind_frame(frame);
		self.list.call_clear(data);
	}

	/// Draw `slice` of `mesh` into `frame`, using a program shell, and a given draw state.
	pub fn draw<P: ProgramShell>(&mut self, mesh: &mesh::Mesh, slice: mesh::Slice,
								frame: &target::Frame, prog_shell: &P, state: &state::DrawState)
								-> Result<(), DrawError> {
		self.bind_frame(frame);
		match self.bind_shell(prog_shell) {
			Ok(_) => (),
			Err(e) => return Err(ErrorShell(e)),
		}
		// bind fixed-function states
		self.list.set_primitive(state.primitive);
		self.list.set_scissor(state.scissor);
		self.list.set_depth_stencil(state.depth, state.stencil,
			state.primitive.get_cull_mode());
		self.list.set_blend(state.blend);
		self.list.set_color_mask(state.color_mask);
		// bind mesh data
		match self.bind_mesh(mesh, prog_shell.get_program().get_info()) {
			Ok(_) => (),
			Err(e) => return Err(ErrorMesh(e)),
		}
		// draw
		match slice {
			mesh::VertexSlice(start, end) => {
				self.list.call_draw(mesh.prim_type, start, end);
			},
			mesh::IndexSlice(buf, index, start, end) => {
				self.list.bind_index(buf);
				self.list.call_draw_indexed(mesh.prim_type, index, start, end);
			},
		}
		Ok(())
	}

	/// Update a buffer with data from a vector.
	pub fn update_buffer_vec<T: Send>(&mut self, buf: backend::Buffer, data: Vec<T>) {
		self.list.update_buffer(buf, (box data) as Box<device::Blob + Send>);
	}

	/// Update a buffer with data from a single type.
	pub fn update_buffer_struct<T: device::Blob+Send>(&mut self, buf: backend::Buffer, data: T) {
		self.list.update_buffer(buf, (box data) as Box<device::Blob + Send>);
	}

	/// Update the contents of a texture.
	pub fn update_texture<T: Send>(&mut self, tex: device::TextureHandle,
								   img: device::tex::ImageInfo, data: Vec<T>) {
		self.list.update_texture(tex.get_info().kind, tex.get_name(), img,
								 (box data) as Box<device::Blob + Send>);
	}

	fn bind_target(list: &mut device::DrawList, to: device::target::Target, plane: target::Plane) {
		match plane {
			target::PlaneEmpty =>
				list.unbind_target(to),
			target::PlaneSurface(suf) =>
				list.bind_target_surface(to, suf),
			target::PlaneTexture(tex, level, layer) =>
				list.bind_target_texture(to, tex, level, layer),
		}
	}

	fn bind_frame(&mut self, frame: &target::Frame) {
		self.list.set_viewport(device::target::Rect {
			x: 0,
			y: 0,
			w: frame.width,
			h: frame.height,
		});
		if frame.is_default() {
			// binding the default FBO, not touching our common one
			self.list.bind_frame_buffer(self.default_frame_buffer);
		} else {
			self.list.bind_frame_buffer(self.common_frame_buffer);
			for (i, (cur, new)) in self.state.frame.colors.iter().zip(frame.colors.iter()).enumerate() {
				if *cur != *new {
					FrontEnd::bind_target(&mut self.list, device::target::TargetColor(i as u8), *new);
				}
			}
			if self.state.frame.depth != frame.depth {
				FrontEnd::bind_target(&mut self.list, device::target::TargetDepth, frame.depth);
			}
			if self.state.frame.stencil != frame.stencil {
				FrontEnd::bind_target(&mut self.list, device::target::TargetStencil, frame.stencil);
			}
			self.state.frame = *frame;
		}
	}

	fn bind_shell<P: ProgramShell>(&mut self, shell: &P) -> Result<(), ShellError> {
		let prog = shell.get_program();
		self.list.bind_program(prog.get_name());
		let pinfo = prog.get_info();
		// gather parameters
		// this is a bit ugly, need to re-think the interface with `#[shader_program]`
		let mut uniforms = Vec::from_elem(pinfo.uniforms.len(), None);
		let mut blocks   = Vec::from_elem(pinfo.blocks  .len(), None);
		let mut textures = Vec::from_elem(pinfo.textures.len(), None);
		shell.fill_params(shade::ParamValues {
			uniforms: uniforms.as_mut_slice(),
			blocks: blocks.as_mut_slice(),
			textures: textures.as_mut_slice(),
		});
		// bind uniforms
		for (var, option) in pinfo.uniforms.iter().zip(uniforms.move_iter()) {
			match option {
				Some(v) => self.list.bind_uniform(var.location, v),
				None => return Err(ErrorShellUniform(var.name.clone())),
			}
		}
		// bind uniform blocks
		for (i, (var, option)) in pinfo.blocks.iter().zip(blocks.move_iter()).enumerate() {
			match option {
				Some(buf) => self.list.bind_uniform_block(
					prog.get_name(),
					i as device::UniformBufferSlot,
					i as device::UniformBlockIndex,
					buf),
				None => return Err(ErrorShellBlock(var.name.clone())),
			}
		}
		// bind textures and samplers
		for (i, (var, option)) in pinfo.textures.iter().zip(textures.move_iter()).enumerate() {
			match option {
				Some((tex, sampler)) => {
					self.list.bind_uniform(var.location, device::shade::ValueI32(i as i32));
					self.list.bind_texture(i as device::TextureSlot,
						tex.get_info().kind, tex.get_name(), sampler);
				},
				None => return Err(ErrorShellTexture(var.name.clone())),
			}
		}
		Ok(())
	}

	fn bind_mesh(&mut self, mesh: &mesh::Mesh, info: &ProgramInfo)
				 -> Result<(), MeshError> {
		self.list.bind_array_buffer(self.common_array_buffer);
		for sat in info.attributes.iter() {
			match mesh.attributes.iter().find(|a| a.name.as_slice() == sat.name.as_slice()) {
				Some(vat) => match vat.elem_type.is_compatible(sat.base_type) {
					Ok(_) => {
						self.list.bind_attribute(
							sat.location as device::AttributeSlot,
							vat.buffer, vat.elem_count, vat.elem_type,
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
