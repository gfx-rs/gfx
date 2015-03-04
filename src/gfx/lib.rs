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

//! An efficient, low-level, bindless graphics API for Rust. See [the
//! blog](http://gfx-rs.github.io/) for explanations and annotated examples.

#![feature(core, libc, unsafe_destructor)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate libc;
extern crate draw_state;

// draw state re-exports
pub use draw_state::{DrawState, BlendPreset};

// public re-exports
pub use render::{Renderer, DrawError};
pub use render::batch;
pub use render::device_ext::{DeviceExt, ShaderSource, ProgramError};
pub use render::mesh::{Attribute, Mesh, VertexFormat};
pub use render::mesh::Error as MeshError;
pub use render::mesh::{Slice, ToSlice, SliceKind};
pub use render::shade;
pub use render::target::{Frame, Plane};
pub use device::{Device, Resources};
pub use device::{attrib, tex};
pub use device::as_byte_slice;
pub use device::{BufferHandle, BufferInfo, RawBufferHandle, ShaderHandle};
pub use device::{ProgramHandle, SurfaceHandle, TextureHandle, SamplerHandle};
pub use device::BufferUsage;
pub use device::{VertexCount, InstanceCount};
pub use device::PrimitiveType;
pub use device::draw::CommandBuffer;
pub use device::shade::{ProgramInfo, UniformValue};
pub use draw_state::target::*;
pub use draw_state::block as state;

#[path = "../render/lib.rs"] pub mod render;
#[path = "../device/lib.rs"] pub mod device;

/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<D: device::Device> {
    /// Graphics device.
    pub device: D,
    /// Renderer front-end.
    pub renderer: Renderer<D::CommandBuffer>,
    /// Hidden batch context.
    context: batch::Context<D::Resources>,
}

impl<D: device::Device> std::ops::Deref for Graphics<D> {
    type Target = batch::Context<D::Resources>;

    fn deref(&self) -> &batch::Context<D::Resources> {
        &self.context
    }
}

impl<D: device::Device> Graphics<D> {
    /// Create a new graphics wrapper.
    pub fn new(mut device: D) -> Graphics<D> {
        let rend = device.create_renderer();
        Graphics {
            device: device,
            renderer: rend,
            context: batch::Context::new(),
        }
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ClearData, mask: Mask, frame: &Frame<D::Resources>) {
        self.renderer.clear(data, mask, frame)
    }

    /// Draw a `RefBatch` batch.
    pub fn draw<'a, T: shade::ShaderParam<Resources = D::Resources>>(&'a mut self,
                batch: &'a batch::RefBatch<T>, frame: &Frame<D::Resources>)
                -> Result<(), DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&(batch, &self.context), frame)
    }

    /// Draw a `RefCore` batch.
    pub fn draw_core<'a, T: shade::ShaderParam<Resources = D::Resources>>(&'a mut self,
                     core: &'a batch::RefCore<T>, slice: &'a Slice<D::Resources>,
                     params: &'a T, frame: &Frame<D::Resources>)
                     -> Result<(), DrawError<batch::OutOfBounds>> {
        self.renderer.draw(&self.context.bind(core, slice, params), frame)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }
}
