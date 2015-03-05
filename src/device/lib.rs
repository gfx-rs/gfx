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

pub use draw_state::target;
pub use draw_state::block as state;

pub mod attrib;
pub mod draw;
pub mod handle;
pub mod shade;
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
pub struct ReadableMapping<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> {
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Deref for ReadableMapping<'a, T, R, F> where
    F::Mapper: 'a,
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for ReadableMapping<'a, T, R, F> where
    F::Mapper: 'a,
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a writable map, which only allows setting elements.
pub struct WritableMapping<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> {
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> WritableMapping<'a, T, R, F> {
    /// Set a value in the buffer
    pub fn set(&mut self, idx: usize, val: T) {
        if idx >= self.len {
            panic!("Tried to write out of bounds to a WritableMapping!")
        }
        unsafe { self.raw.set(idx, val); }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for WritableMapping<'a, T, R, F> where
    F::Mapper: 'a,
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}

/// A handle to a complete readable/writable map, which can be sliced both ways.
pub struct RWMapping<'a, T: Copy, R: 'a + Resources, F: 'a + Factory<R>> {
    raw: F::Mapper,
    len: usize,
    factory: &'a mut F,
    phantom_t: PhantomData<T>
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> Deref for RWMapping<'a, T, R, F> where
    F::Mapper: 'a
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { self.raw.to_slice(self.len) }
    }
}

impl<'a, T: Copy, R: Resources, F: Factory<R>> DerefMut for RWMapping<'a, T, R, F> where
    F::Mapper: 'a
{
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { self.raw.to_mut_slice(self.len) }
    }
}

#[unsafe_destructor]
impl<'a, T: Copy, R: Resources, F: Factory<R>> Drop for RWMapping<'a, T, R, F> where
    F::Mapper: 'a
{
    fn drop(&mut self) {
        self.factory.unmap_buffer_raw(self.raw.clone())
    }
}


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
#[allow(missing_docs)]
pub trait Resources: PhantomFn<Self> + Clone + PartialEq + fmt::Debug {
    type Buffer:        Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type ArrayBuffer:   Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Shader:        Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Program:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type FrameBuffer:   Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Surface:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Texture:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
    type Sampler:       Copy + Clone + fmt::Debug + PartialEq + Send + Sync;
}

#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// Associated mapper type
    type Mapper: Clone + RawMapping;
    // resource creation
    fn create_buffer_raw(&mut self, size: usize, usage: BufferUsage) -> handle::Buffer<R, ()>;
    fn create_buffer<T>(&mut self, num: usize, usage: BufferUsage) -> handle::Buffer<R, T> {
        self.create_buffer_raw(num * mem::size_of::<T>(), usage).cast()
    }
    fn create_buffer_static_raw(&mut self, data: &[u8]) -> handle::Buffer<R, ()>;
    fn create_buffer_static<T: Copy>(&mut self, data: &[T]) -> handle::Buffer<R, T> {
        self.create_buffer_static_raw(as_byte_slice(data)).cast()
    }
    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, ()>;
    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<handle::Shader<R>, shade::CreateShaderError>;
    fn create_program(&mut self, shaders: &[handle::Shader<R>], targets: Option<&[&str]>)
                      -> Result<handle::Program<R>, ()>;
    fn create_frame_buffer(&mut self) -> handle::FrameBuffer<R>;
    fn create_surface(&mut self, info: tex::SurfaceInfo) -> Result<handle::Surface<R>, tex::SurfaceError>;
    fn create_texture(&mut self, info: tex::TextureInfo) -> Result<handle::Texture<R>, tex::TextureError>;
    fn create_sampler(&mut self, info: tex::SamplerInfo) -> handle::Sampler<R>;

    /// Return the framebuffer handle for the screen.
    fn get_main_frame_buffer(&self) -> handle::FrameBuffer<R>;

    // resource deletion
    fn delete_buffer_raw(&mut self, buf: handle::Buffer<R, ()>);
    fn delete_buffer<T>(&mut self, buf: handle::Buffer<R, T>) {
        self.delete_buffer_raw(buf.cast());
    }
    fn delete_shader(&mut self, handle::Shader<R>);
    fn delete_program(&mut self, handle::Program<R>);
    fn delete_surface(&mut self, handle::Surface<R>);
    fn delete_texture(&mut self, handle::Texture<R>);
    fn delete_sampler(&mut self, handle::Sampler<R>);

    /// Update the information stored in a specific buffer
    fn update_buffer_raw(&mut self, buf: handle::Buffer<R, ()>, data: &[u8],
                         offset_bytes: usize);
    fn update_buffer<T: Copy>(&mut self, buf: handle::Buffer<R, T>, data: &[T],
                     offset_elements: usize) {
        self.update_buffer_raw(buf.cast(), as_byte_slice(data), mem::size_of::<T>() * offset_elements)
    }
    fn map_buffer_raw(&mut self, buf: handle::Buffer<R, ()>, access: MapAccess) -> Self::Mapper;
    fn unmap_buffer_raw(&mut self, map: Self::Mapper);
    fn map_buffer_readable<T: Copy>(&mut self, buf: handle::Buffer<R, T>) -> ReadableMapping<T, R, Self>;
    fn map_buffer_writable<T: Copy>(&mut self, buf: handle::Buffer<R, T>) -> WritableMapping<T, R, Self>;
    fn map_buffer_rw<T: Copy>(&mut self, buf: handle::Buffer<R, T>) -> RWMapping<T, R, Self>;

    /// Update the information stored in a texture
    fn update_texture_raw(&mut self, tex: &handle::Texture<R>, img: &tex::ImageInfo,
                          data: &[u8]) -> Result<(), tex::TextureError>;
    fn update_texture<T: Copy>(&mut self, tex: &handle::Texture<R>,
                      img: &tex::ImageInfo, data: &[T])
                      -> Result<(), tex::TextureError> {
        self.update_texture_raw(tex, img, as_byte_slice(data))
    }
    fn generate_mipmap(&mut self, tex: &handle::Texture<R>);
}

/// A service trait with methods for mapping already implemented.
/// To be used by device back ends.
#[allow(missing_docs)]
pub trait MapFactory<R: Resources> {
    type RawMapping: RawMapping;

    fn map_readable<T: Copy>(&mut self, Self::RawMapping, usize)
                    -> ReadableMapping<T, R, Self>;
    fn map_writable<T: Copy>(&mut self, Self::RawMapping, usize)
                    -> WritableMapping<T, R, Self>;
    fn map_read_write<T: Copy>(&mut self, Self::RawMapping, usize)
                      -> RWMapping<T, R, Self>;
}


impl<R: Resources, F: Factory<R>> MapFactory<R> for F {
    type RawMapping = F::Mapper;

    fn map_readable<T: Copy>(&mut self, map: <Self as MapFactory<R>>::RawMapping,
                    length: usize) -> ReadableMapping<T, R, Self> {
        ReadableMapping {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }

    fn map_writable<T: Copy>(&mut self, map: <Self as MapFactory<R>>::RawMapping,
                    length: usize) -> WritableMapping<T, R, Self> {
        WritableMapping {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }

    fn map_read_write<T: Copy>(&mut self, map: <Self as MapFactory<R>>::RawMapping,
                      length: usize) -> RWMapping<T, R, Self> {
        RWMapping {
            raw: map,
            len: length,
            factory: self,
            phantom_t: PhantomData,
        }
    }
}

/// An interface for performing draw calls using a specific graphics API
pub trait Device {
    /// Associated resources type
    type Resources: Resources;
    /// Associated command buffer type
    type CommandBuffer: draw::CommandBuffer<Self::Resources>;

    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;
    /// Reset all the states to disabled/default
    fn reset_state(&mut self);
    /// Submit a command buffer for execution
    fn submit(&mut self, buffer: (&Self::CommandBuffer, &draw::DataBuffer));
}
