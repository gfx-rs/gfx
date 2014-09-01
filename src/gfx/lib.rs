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
pub use render::Graphics;
pub use render::batch;
pub use render::front;
pub use render::front::{DeviceHelper, Renderer};
pub use render::mesh::{Attribute, Mesh, VertexFormat};
pub use render::mesh::{Slice, ToSlice};
pub use render::mesh::{VertexSlice, IndexSlice8, IndexSlice16, IndexSlice32};
pub use render::state::{DrawState, BlendAdditive, BlendAlpha};
pub use render::shade;
pub use render::target::{Frame, Plane, PlaneEmpty, PlaneSurface, PlaneTexture};
pub use device::Device;
// when cargo is ready, re-enable the cfgs
/* #[cfg(gl)] */ pub use device::GlDevice;
pub use device::{attrib, state, tex};
pub use device::{BufferHandle, BufferInfo, RawBufferHandle, ShaderHandle,
    ProgramHandle, SurfaceHandle, TextureHandle};
pub use device::{BufferUsage, UsageStatic, UsageDynamic, UsageStream};
pub use device::{VertexCount, IndexCount, InstanceCount};
pub use device::{Point, Line, LineStrip, TriangleList, TriangleStrip, TriangleFan};
pub use device::blob::Blob;
pub use device::shade::{UniformValue,
    ValueI32, ValueF32,
    ValueI32Vector2, ValueI32Vector3, ValueI32Vector4,
    ValueF32Vector2, ValueF32Vector3, ValueF32Vector4,
    ValueF32Matrix2, ValueF32Matrix3, ValueF32Matrix4};
pub use device::shade::{ShaderSource, StaticBytes, OwnedBytes, ProgramInfo};
pub use device::target::{Color, ClearData, Layer, Level};
