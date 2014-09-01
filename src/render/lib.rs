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
#![deny(missing_doc)]
#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate device;

/// Batches
pub mod batch;
/// Frontend
pub mod front;
/// Meshes
pub mod mesh;
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;

/// A convenient wrapper suitable for single-threaded operation
pub struct Graphics<D, C: device::draw::CommandBuffer> {
    /// Graphics device
    pub device: D,
    /// Renderer front-end
    pub renderer: front::Renderer<C>,
    /// Hidden batch context
    context: batch::Context,
}

impl<D: device::Device<C>,
     C: device::draw::CommandBuffer> Graphics<D, C> {
    /// Create a new graphics wrapper
    pub fn new(mut device: D) -> Graphics<D, C> {
        use front::DeviceHelper;
        let rend = device.create_renderer();
        Graphics {
            device: device,
            renderer: rend,
            context: batch::Context::new(),
        }
    }

    /// Create a new ref batch
    pub fn make_batch<L, T: shade::ShaderParam<L>>(&mut self,
                      program: &device::ProgramHandle,
                      mesh: &mesh::Mesh,
                      slice: mesh::Slice,
                      state: &state::DrawState)
                      -> Result<batch::RefBatch<L, T>, batch::BatchError> {
        self.context.batch(mesh, slice, program, state)
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: device::target::ClearData, frame: &target::Frame) {
        self.renderer.clear(data, frame)
    }

    /// Draw a ref batch
    pub fn draw<'a, L, T: shade::ShaderParam<L>>(&'a mut self,
        batch: &'a batch::RefBatch<L, T>, data: &'a T, frame: &target::Frame) {
        self.renderer.draw((batch, data, &self.context), frame)
    }

    /// Submit the internal command buffer and reset for the next frame
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }
}
