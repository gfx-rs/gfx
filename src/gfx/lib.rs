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

#![crate_name = "gfx"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]

//! An efficient, low-level, bindless graphics API for Rust. See [the
//! blog](http://gfx-rs.github.io/) for explanations and annotated examples.

#[phase(plugin, link)] extern crate log;
extern crate libc;

extern crate device;
extern crate render;

// public re-exports
pub use render::{DeviceHelper, Renderer};
pub use render::batch;
pub use render::mesh::{Attribute, Mesh, VertexFormat};
pub use render::mesh::{Slice, ToSlice};
pub use render::mesh::SliceKind;
pub use render::mesh::SliceKind::Vertex as VertexSlice;
pub use render::mesh::SliceKind::Index8 as IndexSlice8;
pub use render::mesh::SliceKind::Index16 as IndexSlice16;
pub use render::mesh::SliceKind::Index32 as IndexSlice32;
pub use render::state::{BlendPreset, DrawState};
pub use render::state::BlendPreset::Additive as BlendAdditive;
pub use render::state::BlendPreset::Alpha as BlendAlpha;
pub use render::shade;
pub use render::target::{Frame, Plane};
pub use render::target::Plane::Surface as PlaneSurface;
pub use render::target::Plane::Texture as PlaneTexture;
pub use device::Device;
pub use device::{attrib, state, tex};
pub use device::as_byte_slice;
pub use device::{BufferHandle, BufferInfo, RawBufferHandle, ShaderHandle};
pub use device::{ProgramHandle, SurfaceHandle, TextureHandle};
pub use device::BufferUsage;
pub use device::BufferUsage::Static as UsageStatic;
pub use device::BufferUsage::Dynamic as UsageDynamic;
pub use device::BufferUsage::Stream as UsageStream;
pub use device::{VertexCount, InstanceCount};
pub use device::PrimitiveType;
pub use device::draw::CommandBuffer;
pub use device::shade::UniformValue;
pub use device::shade::UniformValue::I32 as ValueI32;
pub use device::shade::UniformValue::F32 as ValueF32;
pub use device::shade::UniformValue::I32Vector2 as ValueI32Vector2;
pub use device::shade::UniformValue::I32Vector3 as ValueI32Vector3;
pub use device::shade::UniformValue::I32Vector4 as ValueI32Vector4;
pub use device::shade::UniformValue::F32Vector2 as ValueF32Vector2;
pub use device::shade::UniformValue::F32Vector3 as ValueF32Vector3;
pub use device::shade::UniformValue::F32Vector4 as ValueF32Vector4;
pub use device::shade::UniformValue::F32Matrix2 as ValueF32Matrix2;
pub use device::shade::UniformValue::F32Matrix3 as ValueF32Matrix3;
pub use device::shade::UniformValue::F32Matrix4 as ValueF32Matrix4;
pub use device::shade::{ShaderSource, ProgramInfo};
pub use device::target::{ColorValue, ClearData, Mask, Layer, Level, Rect, Target};
pub use device::target::{COLOR, DEPTH, STENCIL};

// TODO: Remove this re-export once `gl_device` becomes a separate crate.
pub use device::gl_device::{GlDevice, GlCommandBuffer};

use render::batch::Context as BatchContext;
use render::batch::RefBatch;

/// A convenient wrapper suitable for single-threaded operation.
pub struct Graphics<D, C: device::draw::CommandBuffer> {
    /// Graphics device.
    pub device: D,
    /// Renderer front-end.
    pub renderer: Renderer<C>,
    /// Hidden batch context.
    context: BatchContext,
}

impl<D: device::Device<C>, C: device::draw::CommandBuffer> Graphics<D, C> {
    /// Create a new graphics wrapper.
    pub fn new(mut device: D) -> Graphics<D, C> {
        let rend = device.create_renderer();
        Graphics {
            device: device,
            renderer: rend,
            context: BatchContext::new(),
        }
    }

    /// Create a new ref batch.
    pub fn make_batch<L, T: shade::ShaderParam<L>>(&mut self,
                      program: &ProgramHandle,
                      mesh: &Mesh,
                      slice: Slice,
                      state: &DrawState)
                      -> Result<RefBatch<L, T>, batch::BatchError> {
        self.context.make_batch(program, mesh, slice, state)
    }

    /// Clear the `Frame` as the `ClearData` specifies.
    pub fn clear(&mut self, data: ClearData, mask: Mask, frame: &Frame) {
        self.renderer.clear(data, mask, frame)
    }

    /// Draw a ref batch.
    pub fn draw<'a, L, T: shade::ShaderParam<L>>(&'a mut self,
        batch: &'a RefBatch<L, T>, data: &'a T, frame: &Frame) {
        self.renderer.draw((batch, data, &self.context), frame)
    }

    /// Submit the internal command buffer and reset for the next frame.
    pub fn end_frame(&mut self) {
        self.device.submit(self.renderer.as_buffer());
        self.renderer.reset();
    }
}
