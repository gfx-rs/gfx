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

#![crate_name = "device"]
#![comment = "Back-ends to abstract over the differences between low-level, \
              platform-specific graphics APIs"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(phase)]
#![deny(missing_doc)]

//! Graphics device. Not meant for direct use.

#[phase(plugin, link)] extern crate log;
extern crate libc;

// when cargo is ready, re-enable the cfgs
/* #[cfg(gl)] */ pub use gl::GlDevice;
/* #[cfg(gl)] */ pub use back = self::gl;
/* #[cfg(gl)] */ pub use gl::DrawList;
// #[cfg(d3d11)] ... // TODO

use std::fmt;
use std::kinds::marker;
use std::mem::size_of;

pub mod attrib;
pub mod draw;
pub mod shade;
pub mod state;
pub mod target;
pub mod tex;
/* #[cfg(gl)] */ mod gl;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw index count.
pub type IndexCount = u32;
/// Index of a uniform block.
pub type UniformBlockIndex = u8;
/// Slot for an attribute.
pub type AttributeSlot = u8;
/// Slot for a uniform buffer object.
pub type UniformBufferSlot = u8;
/// Slot a texture can be bound to.
pub type TextureSlot = u8;

/// A generic handle struct
#[deriving(Clone, Show)]
pub struct Handle<T, I>(T, I);

#[deriving(Clone, Show)]
impl<T: Copy, I> Handle<T, I> {
    /// Get the internal name
    pub fn get_name(&self) -> T {
        let Handle(name, _) = *self;
        name
    }

    /// Get the info reference
    pub fn get_info(&self) -> &I {
        let Handle(_, ref info) = *self;
        info
    }
}

impl<T: Copy + PartialEq, I: PartialEq> PartialEq for Handle<T, I> {
    fn eq(&self, other: &Handle<T,I>) -> bool {
        self.get_name().eq(&other.get_name()) && self.get_info().eq(other.get_info())
    }
}

/// Type-safe buffer handle
#[deriving(Show, Clone)]
pub struct BufferHandle<T> {
    raw: RawBufferHandle,
}

impl<T> BufferHandle<T> {
    /// Create a type-safe BufferHandle from a RawBufferHandle
    pub fn from_raw(handle: RawBufferHandle) -> BufferHandle<T> {
        BufferHandle {
            raw: handle,
        }
    }

    /// Cast the type this BufferHandle references
    pub fn cast<U>(self) -> BufferHandle<U> {
        BufferHandle::from_raw(self.raw)
    }

    /// Get the underlying GL name for this BufferHandle
    pub fn get_name(&self) -> back::Buffer {
        self.raw.get_name()
    }

    /// Get the associated information about the buffer
    pub fn get_info(&self) -> &BufferInfo {
        self.raw.get_info()
    }

    /// Get the underlying raw Handle
    pub fn raw(&self) -> RawBufferHandle {
        self.raw
    }
}


pub type RawBufferHandle = Handle<back::Buffer, BufferInfo>;
/// Shader Handle
pub type ShaderHandle  = Handle<back::Shader, shade::Stage>;
/// Program Handle
pub type ProgramHandle = Handle<back::Program, shade::ProgramInfo>;
/// Surface Handle
pub type SurfaceHandle = Handle<back::Surface, tex::SurfaceInfo>;
/// Texture Handle
pub type TextureHandle = Handle<back::Texture, tex::TextureInfo>;
/// Sampler Handle
pub type SamplerHandle = Handle<back::Sampler, tex::SamplerInfo>;

/// A helper method to test `#[vertex_format]` without GL context
//#[cfg(test)]
pub fn make_fake_buffer<T>() -> BufferHandle<T> {
    let info = BufferInfo {
        usage: UsageStatic,
        size: 0,
    };
    BufferHandle::from_raw(Handle(0, info))
}

/// Features that the device supports.
#[deriving(Show)]
pub struct Capabilities {
    shader_model: shade::ShaderModel,
    max_draw_buffers : uint,
    max_texture_size : uint,
    max_vertex_attributes: uint,
    uniform_block_supported: bool,
    array_buffer_supported: bool,
    sampler_objects_supported: bool,
    immutable_storage_supported: bool,
}

/// A trait that slice-like types implement.
pub trait Blob<T> {
    /// Get the address to the data this `Blob` stores.
    fn get_address(&self) -> uint;
    /// Get the number of bytes in this blob.
    fn get_size(&self) -> uint;
}

/// Helper trait for casting &Blob
pub trait RefBlobCast<'a> {
    /// Cast the type the blob references
    fn cast<U>(self) -> &'a Blob<U>;
}

/// Helper trait for casting Box<Blob>
pub trait BoxBlobCast {
    /// Cast the type the blob references
    fn cast<U>(self) -> Box<Blob<U> + Send>;
}

impl<'a, T> RefBlobCast<'a> for &'a Blob<T> {
    fn cast<U>(self) -> &'a Blob<U> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<T> BoxBlobCast for Box<Blob<T> + Send> {
    fn cast<U>(self) -> Box<Blob<U> + Send> {
        unsafe { std::mem::transmute(self) }
    }
}

impl<T: Send> Blob<T> for Vec<T> {
    fn get_address(&self) -> uint {
        self.as_ptr() as uint
    }
    fn get_size(&self) -> uint {
        self.len() * size_of::<T>()
    }
}

impl<T> fmt::Show for Box<Blob<T> + Send> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blob({:#x}, {})", self.get_address(), self.get_size())
    }
}

/// Describes what geometric primitives are created from vertex data.
#[deriving(Clone, PartialEq, Show)]
#[repr(u8)]
pub enum PrimitiveType {
    /// Each vertex represents a single point.
    Point,
    /// Each pair of vertices represent a single line segment. For example, with `[a, b, c, d,
    /// e]`, `a` and `b` form a line, `c` and `d` form a line, and `e` is discarded.
    Line,
    /// Every two consecutive vertices represent a single line segment. Visually forms a "path" of
    /// lines, as they are all connected. For example, with `[a, b, c]`, `a` and `b` form a line
    /// line, and `b` and `c` form a line.
    LineStrip,
    /// Each triplet of vertices represent a single triangle. For example, with `[a, b, c, d, e]`,
    /// `a`, `b`, and `c` form a triangle, `d` and `e` are discarded.
    TriangleList,
    /// Every three consecutive vertices represent a single triangle. For example, with `[a, b, c,
    /// d]`, `a`, `b`, and `c` form a triangle, and `b`, `c`, and `d` form a triangle.
    TriangleStrip,
    /// The first vertex with the last two are forming a triangle. For example, with `[a, b, c, d
    /// ]`, `a` , `b`, and `c` form a triangle, and `a`, `c`, and `d` form a triangle.
    TriangleFan,
    //Quad,
}

/// A type of each index value in the mesh's index buffer
pub type IndexType = attrib::IntSize;

/// A hint as to how this buffer will be used.
///
/// The nature of these hints make them very implementation specific. Different drivers on
/// different hardware will handle them differently. Only careful profiling will tell which is the
/// best to use for a specific buffer.
#[deriving(Clone, PartialEq, Show)]
#[repr(u8)]
pub enum BufferUsage {
    /// Once uploaded, this buffer will rarely change, but will be read from often.
    UsageStatic,
    /// This buffer will be updated "frequently", and will be read from multiple times between
    /// updates.
    UsageDynamic,
    /// This buffer always or almost always be updated after each read.
    UsageStream,
}

/// An information block that is immutable and associated with each buffer
#[deriving(Clone, PartialEq, Show)]
pub struct BufferInfo {
    /// Usage hint
    pub usage: BufferUsage,
    /// Size in bytes
    pub size: uint,
}

/// Surface creation/update error.
#[deriving(Clone, PartialEq, Show)]
pub enum SurfaceError {
    /// Failed to map a given format to the device
    UnsupportedSurfaceFormat,
}

/// Texture creation/update error.
#[deriving(Clone, PartialEq, Show)]
pub enum TextureError {
    /// Failed to map a given format to the device
    UnsupportedTextureFormat,
}

/// Serialized device command.
/// While this is supposed to be an internal detail of a device,
/// this particular representation may be used by different backends,
/// such as OpenGL (prior to GLNG) and DirectX (prior to DX12)
#[allow(missing_doc)]
#[deriving(Show)]
enum Command {
    BindProgram(back::Program),
    BindArrayBuffer(back::ArrayBuffer),
    BindAttribute(AttributeSlot, back::Buffer, attrib::Count,
        attrib::Type, attrib::Stride, attrib::Offset),
    BindIndex(back::Buffer),
    BindFrameBuffer(back::FrameBuffer),
    /// Unbind any surface from the specified target slot
    UnbindTarget(target::Target),
    /// Bind a surface to the specified target slot
    BindTargetSurface(target::Target, back::Surface),
    /// Bind a level of the texture to the specified target slot
    BindTargetTexture(target::Target, back::Texture, target::Level, Option<target::Layer>),
    BindUniformBlock(back::Program, UniformBufferSlot, UniformBlockIndex, back::Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, tex::TextureKind, back::Texture, Option<SamplerHandle>),
    SetPrimitiveState(state::Primitive),
    SetViewport(target::Rect),
    SetScissor(Option<target::Rect>),
    SetDepthStencilState(Option<state::Depth>, Option<state::Stencil>, state::CullMode),
    SetBlendState(Option<state::Blend>),
    SetColorMask(state::ColorMask),
    UpdateBuffer(back::Buffer, Box<Blob<()> + Send>, BufferUsage),
    UpdateTexture(tex::TextureKind, back::Texture, tex::ImageInfo, Box<Blob<()> + Send>),
    // drawing
    Clear(target::ClearData),
    Draw(PrimitiveType, VertexCount, VertexCount),
    DrawIndexed(PrimitiveType, IndexType, IndexCount, IndexCount),
}

/// An interface for performing draw calls using a specific graphics API
#[allow(missing_doc)]
pub trait Device<D: draw::DrawList> {
    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    // resource creation
    fn create_buffer<T>(&mut self, num: uint, usage: BufferUsage) -> BufferHandle<T>;
    fn create_buffer_static<T>(&mut self, &Blob<T>) -> BufferHandle<T>;
    fn create_array_buffer(&mut self) -> Result<back::ArrayBuffer, ()>;
    fn create_shader(&mut self, stage: shade::Stage, code: shade::ShaderSource) ->
                     Result<ShaderHandle, shade::CreateShaderError>;
    fn create_program(&mut self, shaders: &[ShaderHandle]) -> Result<ProgramHandle, ()>;
    fn create_frame_buffer(&mut self) -> back::FrameBuffer;
    fn create_surface(&mut self, info: tex::SurfaceInfo) -> Result<SurfaceHandle, SurfaceError>;
    fn create_texture(&mut self, info: tex::TextureInfo) -> Result<TextureHandle, TextureError>;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> SamplerHandle;
    // resource deletion
    fn delete_buffer<T>(&mut self, BufferHandle<T>);
    fn delete_shader(&mut self, ShaderHandle);
    fn delete_program(&mut self, ProgramHandle);
    fn delete_surface(&mut self, SurfaceHandle);
    fn delete_texture(&mut self, TextureHandle);
    fn delete_sampler(&mut self, SamplerHandle);
    /// Update the information stored in a specific buffer
    fn update_buffer<T>(&mut self, BufferHandle<T>, &Blob<T>);
    /// Update the information stored in a texture
    fn update_texture<T>(&mut self, &TextureHandle, &tex::ImageInfo, &Blob<T>)
                      -> Result<(), TextureError>;
    /// Submit a draw list for execution
    fn submit(&mut self, list: &D);
}
