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
use state;
use target;

/// Graphics state
struct State {
	frame: target::Frame,
	fixed_function: state::DrawState,
}

/// Front-end manager
pub struct Manager {
	common_frame_buffer: backend::FrameBuffer,
}

impl Manager {
	/// Create a new render front-end
	pub fn spawn(&self) -> FrontEnd {
		FrontEnd {
			list: device::DrawList::new(),
			default_frame_buffer: 0,
			common_frame_buffer: self.common_frame_buffer,
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
	default_frame_buffer: backend::FrameBuffer,
	common_frame_buffer: backend::FrameBuffer,
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

	fn bind_target(list: &mut device::DrawList, to: device::target::Target, plane: target::Plane) {
		match plane {
			target::PlaneEmpty => list.unbind_target(to),
			target::PlaneSurface(super::SurfaceHandle(suf)) => {
				//let name = *dp.get_any(|res| res.surfaces[suf].ref0());
				//device::BindTargetSurface(to, name)
			},
			target::PlaneTexture(super::TextureHandle(tex), level, layer) => {
				//let name = *dp.get_any(|res| res.textures[tex].ref0());
				//device::BindTargetTexture(to, name, level, layer)
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
}
