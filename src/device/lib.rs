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

#![feature(phase, unsafe_destructor)]
#![deny(missing_docs)]

//! Graphics device. Not meant for direct use.

#[phase(plugin, link)] extern crate log;
extern crate libc;

// TODO: Remove these exports once `gl_device` becomes a separate crate.
pub use self::gl_device as back;

use std::mem;
use std::slice;

pub mod attrib;
pub mod draw;
pub mod shade;
pub mod state;
pub mod target;
pub mod tex;

// TODO: This will become a separate crate once associated items are implemented
// in rustc and subsequently used in the `Device` trait.
/* #[cfg(gl)] */ #[path = "../gl_device/lib.rs"] pub mod gl_device;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw number of instances
pub type InstanceCount = u32;
/// Index of a uniform block.
pub type UniformBlockIndex = u8;
/// Slot for an attribute.
pub type AttributeSlot = u8;
/// Slot for a uniform buffer object.
pub type UniformBufferSlot = u8;
/// Slot a texture can be bound to.
pub type TextureSlot = u8;

/// Specifies the access allowed to a buffer mapping.
pub enum MapAccess {
    /// Only allow reads.
    Readable,
    /// Only allow writes.
    Writable,
    /// Allow full access.
    RW
}

/// A handle to a readable map, which can be sliced.
pub struct ReadableMapping<'a, T: Copy, C: draw::CommandBuffer, D: 'a + Device<C>> {
    raw: back::RawMapping,
    len: uint,
    device: &'a mut D,
}

impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> Deref<[T]> for ReadableMapping<'a, T, C, D> {
    fn deref(&self) -> &[T] {
        unsafe { mem::transmute(slice::from_raw_buf(&(self.raw.pointer as *const T), self.len)) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> Drop for ReadableMapping<'a, T, C, D> {
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw)
    }
}

/// A handle to a writable map, which only allows setting elements.
pub struct WritableMapping<'a, T: Copy, C: draw::CommandBuffer, D: 'a + Device<C>> {
    raw: back::RawMapping,
    len: uint,
    device: &'a mut D,
}

impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> WritableMapping<'a, T, C, D> {
    /// Set a value in the buffer
    pub fn set(&mut self, idx: uint, val: T) {
        if idx >= self.len {
            panic!("Tried to write out of bounds to a WritableMapping!")
        }
        unsafe { *(std::mem::transmute::<_, *mut T>(self.raw.pointer).offset(idx as int)) = val }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> Drop for WritableMapping<'a, T, C, D> {
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw)
    }
}

/// A handle to a complete readable/writable map, which can be sliced both ways.
pub struct RWMapping<'a, T: Copy, C: draw::CommandBuffer, D: 'a + Device<C>> {
    raw: back::RawMapping,
    len: uint,
    device: &'a mut D,
}

impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> Deref<[T]> for RWMapping<'a, T, C, D> {
    fn deref(&self) -> &[T] {
        unsafe { mem::transmute(slice::from_raw_buf(&(self.raw.pointer as *const T), self.len)) }
    }
}

impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> DerefMut<[T]> for RWMapping<'a, T, C, D> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { mem::transmute(slice::from_raw_mut_buf(&self.raw.pointer, self.len)) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, C: draw::CommandBuffer, D: Device<C>> Drop for RWMapping<'a, T, C, D> {
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw)
    }
}

/// A generic handle struct
#[deriving(Clone, PartialEq, Show)]
pub struct Handle<T, I>(T, I);

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

/// Type-safe buffer handle
#[deriving(Show, PartialEq, Clone)]
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

    /// Get the number of elements in the buffer.
    ///
    /// Fails if `T` is zero-sized.
    pub fn len(&self) -> uint {
        assert!(mem::size_of::<T>() != 0, "Cannot determine the length of zero-sized buffers.");
        self.get_info().size / mem::size_of::<T>()
    }
}

/// Raw (untyped) Buffer Handle
pub type RawBufferHandle = Handle<back::Buffer, BufferInfo>;
/// Array Buffer Handle
pub type ArrayBufferHandle = Handle<back::ArrayBuffer, ()>;
/// Shader Handle
pub type ShaderHandle  = Handle<back::Shader, shade::Stage>;
/// Program Handle
pub type ProgramHandle = Handle<back::Program, shade::ProgramInfo>;
/// Frame Buffer Handle
pub type FrameBufferHandle = Handle<back::FrameBuffer, ()>;
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
        usage: BufferUsage::Static,
        size: 0,
    };
    BufferHandle::from_raw(Handle(0, info))
}

/// Return the framebuffer handle for the screen.
pub fn get_main_frame_buffer() -> FrameBufferHandle {
    Handle(0, ())
}

/// Treat a given slice as `&[u8]` for the given function call
pub fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    let len = mem::size_of::<T>() * slice.len();
    let slice = std::raw::Slice { data: slice.as_ptr(), len: len };
    unsafe { mem::transmute(slice) }
}

/// Features that the device supports.
#[deriving(Show)]
#[allow(missing_docs)] // pretty self-explanatory fields!
pub struct Capabilities {
    pub shader_model: shade::ShaderModel,
    pub max_draw_buffers : uint,
    pub max_texture_size : uint,
    pub max_vertex_attributes: uint,
    pub uniform_block_supported: bool,
    pub array_buffer_supported: bool,
    pub sampler_objects_supported: bool,
    pub immutable_storage_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub render_targets_supported: bool,
    pub vertex_base_supported: bool,
    pub instance_base_supported: bool,
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
    Static,
    /// This buffer will be updated "frequently", and will be read from multiple times between
    /// updates.
    Dynamic,
    /// This buffer always or almost always be updated after each read.
    Stream,
}

/// An information block that is immutable and associated with each buffer
#[deriving(Clone, PartialEq, Show)]
pub struct BufferInfo {
    /// Usage hint
    pub usage: BufferUsage,
    /// Size in bytes
    pub size: uint,
}

/// Serialized device command.
/// While this is supposed to be an internal detail of a device,
/// this particular representation may be used by different backends,
/// such as OpenGL (prior to GLNG) and DirectX (prior to DX12)
#[allow(missing_docs)]
#[deriving(Show)]
pub enum Command {
    BindProgram(back::Program),
    BindArrayBuffer(back::ArrayBuffer),
    BindAttribute(AttributeSlot, back::Buffer, attrib::Format),
    BindIndex(back::Buffer),
    BindFrameBuffer(target::Access, back::FrameBuffer),
    /// Unbind any surface from the specified target slot
    UnbindTarget(target::Access, target::Target),
    /// Bind a surface to the specified target slot
    BindTargetSurface(target::Access, target::Target, back::Surface),
    /// Bind a level of the texture to the specified target slot
    BindTargetTexture(target::Access, target::Target, back::Texture,
                      target::Level, Option<target::Layer>),
    BindUniformBlock(back::Program, UniformBufferSlot, UniformBlockIndex, back::Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, tex::TextureKind, back::Texture, Option<SamplerHandle>),
    SetDrawColorBuffers(uint),
    SetPrimitiveState(state::Primitive),
    SetViewport(target::Rect),
    SetMultiSampleState(Option<state::MultiSample>),
    SetScissor(Option<target::Rect>),
    SetDepthStencilState(Option<state::Depth>, Option<state::Stencil>, state::CullMode),
    SetBlendState(Option<state::Blend>),
    SetColorMask(state::ColorMask),
    UpdateBuffer(back::Buffer, draw::DataPointer, uint),
    UpdateTexture(tex::TextureKind, back::Texture, tex::ImageInfo, draw::DataPointer),
    // drawing
    Clear(target::ClearData, target::Mask),
    Draw(PrimitiveType, VertexCount, VertexCount, Option<(InstanceCount, VertexCount)>),
    DrawIndexed(PrimitiveType, IndexType, VertexCount, VertexCount, VertexCount, Option<(InstanceCount, VertexCount)>),
    Blit(target::Rect, target::Rect, target::Mask),
}

// CommandBuffer is really an associated type, so will look much better when
// Rust supports this natively.
/// An interface for performing draw calls using a specific graphics API
#[allow(missing_docs)]
pub trait Device<C: draw::CommandBuffer> {
    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    /// Reset all the states to disabled/default
    fn reset_state(&mut self);
    /// Submit a command buffer for execution
    fn submit(&mut self, buffer: (&C, &draw::DataBuffer));

    // resource creation
    fn create_buffer_raw(&mut self, size: uint, usage: BufferUsage) -> BufferHandle<()>;
    fn create_buffer<T>(&mut self, num: uint, usage: BufferUsage) -> BufferHandle<T> {
        self.create_buffer_raw(num * mem::size_of::<T>(), usage).cast()
    }
    fn create_buffer_static_raw(&mut self, data: &[u8]) -> BufferHandle<()>;
    fn create_buffer_static<T: Copy>(&mut self, data: &[T]) -> BufferHandle<T> {
        self.create_buffer_static_raw(as_byte_slice(data)).cast()
    }
    fn create_array_buffer(&mut self) -> Result<ArrayBufferHandle, ()>;
    fn create_shader(&mut self, stage: shade::Stage, code: shade::ShaderSource) ->
                     Result<ShaderHandle, shade::CreateShaderError>;
    fn shader_outputs<'a>(&mut self, code: &'a ::shade::ShaderSource) -> Vec<&'a str>;
    fn create_program(&mut self, shaders: &[ShaderHandle], outputs: &[&str]) -> Result<ProgramHandle, ()>;
    fn create_frame_buffer(&mut self) -> FrameBufferHandle;
    fn create_surface(&mut self, info: tex::SurfaceInfo) -> Result<SurfaceHandle, tex::SurfaceError>;
    fn create_texture(&mut self, info: tex::TextureInfo) -> Result<TextureHandle, tex::TextureError>;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> SamplerHandle;

    // resource deletion
    fn delete_buffer_raw(&mut self, buf: BufferHandle<()>);
    fn delete_buffer<T>(&mut self, buf: BufferHandle<T>) {
        self.delete_buffer_raw(buf.cast());
    }
    fn delete_shader(&mut self, ShaderHandle);
    fn delete_program(&mut self, ProgramHandle);
    fn delete_surface(&mut self, SurfaceHandle);
    fn delete_texture(&mut self, TextureHandle);
    fn delete_sampler(&mut self, SamplerHandle);

    /// Update the information stored in a specific buffer
    fn update_buffer_raw(&mut self, buf: BufferHandle<()>, data: &[u8],
                         offset_bytes: uint);
    fn update_buffer<T: Copy>(&mut self, buf: BufferHandle<T>, data: &[T],
                     offset_elements: uint) {
        self.update_buffer_raw(buf.cast(), as_byte_slice(data), mem::size_of::<T>() * offset_elements)
    }
    fn map_buffer_raw(&mut self, buf: BufferHandle<()>, access: MapAccess) -> back::RawMapping;
    fn unmap_buffer_raw(&mut self, map: back::RawMapping);
    fn map_buffer_readable<T: Copy>(&mut self, buf: BufferHandle<T>) -> ReadableMapping<T, C, Self>;
    fn map_buffer_writable<T: Copy>(&mut self, buf: BufferHandle<T>) -> WritableMapping<T, C, Self>;
    fn map_buffer_rw<T: Copy>(&mut self, buf: BufferHandle<T>) -> RWMapping<T, C, Self>;

    /// Update the information stored in a texture
    fn update_texture_raw(&mut self, tex: &TextureHandle, img: &tex::ImageInfo,
                          data: &[u8]) -> Result<(), tex::TextureError>;
    fn update_texture<T: Copy>(&mut self, tex: &TextureHandle,
                      img: &tex::ImageInfo, data: &[T])
                      -> Result<(), tex::TextureError> {
        self.update_texture_raw(tex, img, as_byte_slice(data))
    }
    fn generate_mipmap(&mut self, tex: &TextureHandle);
}

#[cfg(test)]
mod test {
    use std::mem;
    use super::{BufferHandle, Handle};
    use super::{BufferInfo, BufferUsage};

    fn mock_buffer<T>(usage: BufferUsage, len: uint) -> BufferHandle<T> {
        BufferHandle {
            raw: Handle(
                0,
                BufferInfo {
                    usage: usage,
                    size: mem::size_of::<T>() * len,
                },
            ),
        }
    }

    #[test]
    fn test_buffer_len() {
        assert_eq!(mock_buffer::<u8>(BufferUsage::Static, 8).len(), 8);
        assert_eq!(mock_buffer::<u16>(BufferUsage::Static, 8).len(), 8);
    }

    #[test]
    #[should_fail]
    fn test_buffer_zero_len() {
        let _ = mock_buffer::<()>(BufferUsage::Static, 0).len();
    }
}
