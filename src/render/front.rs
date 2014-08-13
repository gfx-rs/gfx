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

use backend = device::dev;
use device::draw::DrawList;
use device;
use mesh;
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

impl Manager {
	/// Create a new render front-end
	pub fn spawn(&self) -> FrontEnd {
		FrontEnd {
			list: device::DrawList::new(),
			common_array_buffer: self.common_array_buffer,
			common_frame_buffer: self.common_frame_buffer,
			default_frame_buffer: 0,
			state: State {
				frame: target::Frame::new(0, 0),
				//care: this doesn't match the HW state
				fixed_function: state::DrawState::new(),
			},
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

	/// Draw `slice` of `mesh` into `frame`, using a `bundle` of shader program and parameters, and
	/// a given draw state.
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
		self.list.set_depth_stencil(state.depth, state.stencil, state.primitive.get_cull_mode());
		self.list.set_blend(state.blend);
		self.list.set_color_mask(state.color_mask);
		// bind mesh data
		self.list.bind_array_buffer(self.common_array_buffer);
		match self.bind_mesh(mesh, prog_shell.get_program()) {
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

	fn bind_target(list: &mut device::DrawList, to: device::target::Target, plane: target::Plane) {
		match plane {
			target::PlaneEmpty => list.unbind_target(to),
			target::PlaneSurface(suf) => {
				list.bind_target_surface(to, suf);
			},
			target::PlaneTexture(tex, level, layer) => {
				list.bind_target_texture(to, tex, level, layer);
			},
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
		let meta = shell.get_program();
		self.list.bind_program(meta.name);
		/* TODO:
		for (var, value) in meta.uniforms.iter().zip(uniforms.iter()) {
			// unwrap() is safe since the errors were caught in prebind_shell()
			self.cast(device::BindUniform(var.location, value.unwrap()));
		}
		for (i, (var, option)) in meta.blocks.iter().zip(blocks.iter()).enumerate() {
			let BufferHandle(bh) = option.unwrap();
			match self.dispatcher.resource.buffers.get(bh) {
				Ok(&Loaded(block)) =>
					self.cast(device::BindUniformBlock(
						meta.name,
						i as device::UniformBufferSlot,
						i as device::UniformBlockIndex,
						block)),
				_ => return Err(ErrorShellBlock(var.name.clone())),
			}
		}
		for (i, (var, option)) in meta.textures.iter().zip(textures.iter()).enumerate() {
			let (TextureHandle(tex_handle), sampler) = option.unwrap();
			let sam = match sampler {
				Some(SamplerHandle(sam)) => match self.dispatcher.resource.samplers[sam] {
					(Loaded(sam), ref info) => Some((sam, info.clone())),
					_ => return Err(ErrorShellSampler(var.name.clone())),
				},
				None => None,
			};
			match self.dispatcher.resource.textures.get(tex_handle) {
				Ok(&(Loaded(tex), ref info)) => {
					self.cast(device::BindUniform(
						var.location,
						device::shade::ValueI32(i as i32)
						));
					self.cast(device::BindTexture(
						i as device::TextureSlot,
						info.kind,
						tex,
						sam));
				},
				_ => return Err(ErrorShellTexture(var.name.clone())),
			}
		}*/
		Ok(())
	}

	fn bind_mesh(&mut self, mesh: &mesh::Mesh, prog: &device::shade::ProgramMeta)
				 -> Result<(), MeshError> {
		for sat in prog.attributes.iter() {
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
