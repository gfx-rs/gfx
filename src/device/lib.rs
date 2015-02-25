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

#![deny(missing_docs, missing_copy_implementations)]

//! Graphics device. Not meant for direct use.

use std::{fmt, mem, raw};
use std::ops::{Deref, DerefMut};
use std::marker::{PhantomData, PhantomFn};

pub mod attrib;
pub mod draw;
pub mod shade;
pub mod state;
pub mod target;
pub mod tex;

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
#[derive(Copy)]
pub enum MapAccess {
    /// Only allow reads.
    Readable,
    /// Only allow writes.
    Writable,
    /// Allow full access.
    RW
}

/// Unsafe operations for a buffer mapping
pub trait RawMapping {
    /// Set the element at `index` to `val`. Not bounds-checked.
    unsafe fn set<T>(&self, index: usize, val: T);
    /// Returns a slice of the specified length.
    unsafe fn to_slice<T>(&self, len: usize) -> &[T];
    /// Returns a mutable slice of the specified length.
    unsafe fn to_mut_slice<T>(&self, len: usize) -> &mut [T];
}

/// A handle to a readable map, which can be sliced.
pub struct ReadableMapping<'a, T: Copy, D: 'a + Device> {
    raw: <D::Resources as Resources>::RawMapping,
    len: usize,
    device: &'a mut D,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, D: Device> Deref for ReadableMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, D: Device> Drop for ReadableMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a writable map, which only allows setting elements.
pub struct WritableMapping<'a, T: Copy, D: 'a + Device> {
    raw: <D::Resources as Resources>::RawMapping,
    len: usize,
    device: &'a mut D,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, D: Device> WritableMapping<'a, T, D> {
    /// Set a value in the buffer
    pub fn set(&mut self, idx: usize, val: T) {
        if idx >= self.len {
            panic!("Tried to write out of bounds to a WritableMapping!")
        }
        unsafe { self.raw.set(idx, val); }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, D: Device> Drop for WritableMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a complete readable/writable map, which can be sliced both ways.
pub struct RWMapping<'a, T: Copy, D: 'a + Device> {
    raw: <D::Resources as Resources>::RawMapping,
    len: usize,
    device: &'a mut D,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, D: Device> Deref for RWMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

impl<'a, T: Copy, D: Device> DerefMut for RWMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { self.raw.to_mut_slice(self.len) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, D: Device> Drop for RWMapping<'a, T, D> where
    <D::Resources as Resources>::RawMapping: 'a,
{
    fn drop(&mut self) {
        self.device.unmap_buffer_raw(self.raw.clone())
    }
}

/// A generic handle struct
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Handle<T, I>(T, I);

impl<T: Copy, I> Handle<T, I> {
    /// Get the internal name
    pub fn get_name(&self) -> T {
        let Handle(name, _) = *self;
        name
    }
}

impl<T, I> Handle<T, I> {
    /// Get the info reference
    pub fn get_info(&self) -> &I {
        let Handle(_, ref info) = *self;
        info
    }
}

/// Type-safe buffer handle
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct BufferHandle<R: Resources, T> {
    raw: RawBufferHandle<R>,
    phantom_t: PhantomData<T>,
}

impl<R: Resources, T> BufferHandle<R, T> {
    /// Create a type-safe BufferHandle from a RawBufferHandle
    pub fn from_raw(handle: RawBufferHandle<R>) -> BufferHandle<R, T> {
        BufferHandle {
            raw: handle,
            phantom_t: PhantomData,
        }
    }

    /// Cast the type this BufferHandle references
    pub fn cast<U>(self) -> BufferHandle<R, U> {
        BufferHandle::from_raw(self.raw)
    }

    /// Get the underlying name for this BufferHandle
    pub fn get_name(&self) -> <R as Resources>::Buffer {
        self.raw.get_name()
    }

    /// Get the underlying raw Handle
    pub fn raw(&self) -> RawBufferHandle<R> {
        self.raw
    }

    /// Get the associated information about the buffer
    pub fn get_info(&self) -> &BufferInfo {
        self.raw.get_info()
    }

    /// Get the number of elements in the buffer.
    ///
    /// Fails if `T` is zero-sized.
    pub fn len(&self) -> usize {
        assert!(mem::size_of::<T>() != 0, "Cannot determine the length of zero-sized buffers.");
        self.get_info().size / mem::size_of::<T>()
    }
}

/// A helper method to test `#[vertex_format]` without GL context
/// Not to be used by user code!
/// Will be replaced by a proper dummy device in the future.
//#[cfg(test)]
pub fn make_dummy_buffer<R: Resources, T>(value: R::Buffer)
                         -> BufferHandle<R, T> {
    let info = BufferInfo {
        usage: BufferUsage::Static,
        size: 0,
    };
    BufferHandle::from_raw(Handle(value, info))
}

/// Raw (untyped) Buffer Handle
pub type RawBufferHandle<R: Resources> = Handle<<R as Resources>::Buffer, BufferInfo>;
/// Array Buffer Handle
pub type ArrayBufferHandle<R: Resources> = Handle<<R as Resources>::ArrayBuffer, ()>;
/// Shader Handle
pub type ShaderHandle<R: Resources>  = Handle<<R as Resources>::Shader, shade::Stage>;
/// Program Handle
pub type ProgramHandle<R: Resources> = Handle<<R as Resources>::Program, shade::ProgramInfo>;
/// Frame Buffer Handle
pub type FrameBufferHandle<R: Resources> = Handle<<R as Resources>::FrameBuffer, ()>;
/// Surface Handle
pub type SurfaceHandle<R: Resources> = Handle<<R as Resources>::Surface, tex::SurfaceInfo>;
/// Texture Handle
pub type TextureHandle<R: Resources> = Handle<<R as Resources>::Texture, tex::TextureInfo>;
/// Sampler Handle
pub type SamplerHandle<R: Resources> = Handle<<R as Resources>::Sampler, tex::SamplerInfo>;

/// Treat a given slice as `&[u8]` for the given function call
pub fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    let len = mem::size_of::<T>() * slice.len();
    let slice = raw::Slice { data: slice.as_ptr(), len: len };
    unsafe { mem::transmute(slice) }
}

/// Features that the device supports.
#[derive(Copy, Debug)]
#[allow(missing_docs)] // pretty self-explanatory fields!
pub struct Capabilities {
    pub shader_model: shade::ShaderModel,

    pub max_draw_buffers: usize,
    pub max_texture_size: usize,
    pub max_vertex_attributes: usize,

    pub array_buffer_supported: bool,
    pub fragment_output_supported: bool,
    pub immutable_storage_supported: bool,
    pub instance_base_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub render_targets_supported: bool,
    pub sampler_objects_supported: bool,
    pub uniform_block_supported: bool,
    pub vertex_base_supported: bool,
}

/// Describes what geometric primitives are created from vertex data.
#[derive(Copy, Clone, PartialEq, Debug)]
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
#[derive(Copy, Clone, PartialEq, Debug)]
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
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct BufferInfo {
    /// Usage hint
    pub usage: BufferUsage,
    /// Size in bytes
    pub size: usize,
}

/// Resources pertaining to a specific API.
pub trait Resources: PhantomFn<Self> + Copy + Clone + PartialEq + fmt::Debug {
    type RawMapping:    Clone + RawMapping;
    type Buffer:        Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type ArrayBuffer:   Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Shader:        Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Program:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type FrameBuffer:   Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Surface:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Texture:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Sampler:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
}

/// An interface for performing draw calls using a specific graphics API
#[allow(missing_docs)]
pub trait Device {
    type Resources: Resources;
    type CommandBuffer: draw::CommandBuffer<Resources = Self::Resources>;

    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    /// Reset all the states to disabled/default
    fn reset_state(&mut self);
    /// Submit a command buffer for execution
    fn submit(&mut self, buffer: (&Self::CommandBuffer, &draw::DataBuffer));

    // resource creation
    fn create_buffer_raw(&mut self, size: usize, usage: BufferUsage) -> BufferHandle<Self::Resources, ()>;
    fn create_buffer<T>(&mut self, num: usize, usage: BufferUsage) -> BufferHandle<Self::Resources, T> {
        self.create_buffer_raw(num * mem::size_of::<T>(), usage).cast()
    }
    fn create_buffer_static_raw(&mut self, data: &[u8]) -> BufferHandle<Self::Resources, ()>;
    fn create_buffer_static<T: Copy>(&mut self, data: &[T]) -> BufferHandle<Self::Resources, T> {
        self.create_buffer_static_raw(as_byte_slice(data)).cast()
    }
    fn create_array_buffer(&mut self) -> Result<ArrayBufferHandle<Self::Resources>, ()>;
    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<ShaderHandle<Self::Resources>, shade::CreateShaderError>;
    fn create_program(&mut self, shaders: &[ShaderHandle<Self::Resources>], targets: Option<&[&str]>) -> Result<ProgramHandle<Self::Resources>, ()>;
    fn create_frame_buffer(&mut self) -> FrameBufferHandle<Self::Resources>;
    fn create_surface(&mut self, info: tex::SurfaceInfo) -> Result<SurfaceHandle<Self::Resources>, tex::SurfaceError>;
    fn create_texture(&mut self, info: tex::TextureInfo) -> Result<TextureHandle<Self::Resources>, tex::TextureError>;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> SamplerHandle<Self::Resources>;

    /// Return the framebuffer handle for the screen.
    fn get_main_frame_buffer(&self) -> FrameBufferHandle<Self::Resources>;

    // resource deletion
    fn delete_buffer_raw(&mut self, buf: BufferHandle<Self::Resources, ()>);
    fn delete_buffer<T>(&mut self, buf: BufferHandle<Self::Resources, T>) {
        self.delete_buffer_raw(buf.cast());
    }
    fn delete_shader(&mut self, ShaderHandle<Self::Resources>);
    fn delete_program(&mut self, ProgramHandle<Self::Resources>);
    fn delete_surface(&mut self, SurfaceHandle<Self::Resources>);
    fn delete_texture(&mut self, TextureHandle<Self::Resources>);
    fn delete_sampler(&mut self, SamplerHandle<Self::Resources>);

    /// Update the information stored in a specific buffer
    fn update_buffer_raw(&mut self, buf: BufferHandle<Self::Resources, ()>, data: &[u8],
                         offset_bytes: usize);
    fn update_buffer<T: Copy>(&mut self, buf: BufferHandle<Self::Resources, T>, data: &[T],
                     offset_elements: usize) {
        self.update_buffer_raw(buf.cast(), as_byte_slice(data), mem::size_of::<T>() * offset_elements)
    }
    fn map_buffer_raw(&mut self, buf: BufferHandle<Self::Resources, ()>, access: MapAccess) -> <Self::Resources as Resources>::RawMapping;
    fn unmap_buffer_raw(&mut self, map: <Self::Resources as Resources>::RawMapping);
    fn map_buffer_readable<T: Copy>(&mut self, buf: BufferHandle<Self::Resources, T>) -> ReadableMapping<T, Self>;
    fn map_buffer_writable<T: Copy>(&mut self, buf: BufferHandle<Self::Resources, T>) -> WritableMapping<T, Self>;
    fn map_buffer_rw<T: Copy>(&mut self, buf: BufferHandle<Self::Resources, T>) -> RWMapping<T, Self>;

    /// Update the information stored in a texture
    fn update_texture_raw(&mut self, tex: &TextureHandle<Self::Resources>, img: &tex::ImageInfo,
                          data: &[u8]) -> Result<(), tex::TextureError>;
    fn update_texture<T: Copy>(&mut self, tex: &TextureHandle<Self::Resources>,
                      img: &tex::ImageInfo, data: &[T])
                      -> Result<(), tex::TextureError> {
        self.update_texture_raw(tex, img, as_byte_slice(data))
    }
    fn generate_mipmap(&mut self, tex: &TextureHandle<Self::Resources>);
}

/// A service trait with methods for handle creation already implemented.
/// To be used by device back ends.
#[allow(missing_docs)]
pub trait DeviceInternal {
    type RawMapping: RawMapping;

    fn make_handle<T, I>(&self, T, I) -> Handle<T, I>;
    fn map_readable<T: Copy>(&mut self, Self::RawMapping, usize)
                    -> ReadableMapping<T, Self>;
    fn map_writable<T: Copy>(&mut self, Self::RawMapping, usize)
                    -> WritableMapping<T, Self>;
    fn map_read_write<T: Copy>(&mut self, Self::RawMapping, usize)
                      -> RWMapping<T, Self>;
}

impl<D: Device> DeviceInternal for D {
    type RawMapping = <D::Resources as Resources>::RawMapping;

    fn make_handle<T, I>(&self, value: T, info: I) -> Handle<T, I> {
        Handle(value, info)
    }

    fn map_readable<T: Copy>(&mut self, map: <Self as DeviceInternal>::RawMapping,
                    length: usize) -> ReadableMapping<T, Self> {
        ReadableMapping {
            raw: map,
            len: length,
            device: self,
            phantom_t: PhantomData,
        }
    }

    fn map_writable<T: Copy>(&mut self, map: <Self as DeviceInternal>::RawMapping,
                    length: usize) -> WritableMapping<T, Self> {
        WritableMapping {
            raw: map,
            len: length,
            device: self,
            phantom_t: PhantomData,
        }
    }

    fn map_read_write<T: Copy>(&mut self, map: <Self as DeviceInternal>::RawMapping,
                      length: usize) -> RWMapping<T, Self> {
        RWMapping {
            raw: map,
            len: length,
            device: self,
            phantom_t: PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use std::mem;
    use super::{BufferHandle, Handle};
    use super::{BufferInfo, BufferUsage};
    use super::back;

    fn mock_buffer<T>(usage: BufferUsage, len: usize) -> BufferHandle<back::GlResources, T> {
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
