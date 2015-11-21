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

#![deny(missing_docs)]

//! Graphics device. Not meant for direct use.

use std::{fmt, mem};
use std::hash::Hash;

pub use draw_state::{MAX_COLOR_TARGETS, state, target};
use draw_state::DrawState;

pub mod attrib;
pub mod command;
pub mod draw;
pub mod dummy;
pub mod handle;
pub mod mapping;
pub mod pso;
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
/// Slot for an active color buffer.
pub type ColorSlot = u8;

/// Generic error for features that are not supported
/// by the device capabilities.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct NotSupported;

/// Treat a given slice as `&[u8]` for the given function call
pub fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    use std::slice;
    let len = mem::size_of::<T>() * slice.len();
    unsafe {
        slice::from_raw_parts(slice.as_ptr() as *const u8, len)
    }
}

macro_rules! define_shaders {
    ($($name:ident),+) => {$(
        #[allow(missing_docs)]
        pub struct $name<R: Resources>(handle::Shader<R>);
        impl<R: Resources> $name<R> {
            #[allow(missing_docs)]
            pub fn reference(&self, man: &mut handle::Manager<R>) -> R::Shader {
                man.ref_shader(&self.0)
            }
        }
    )+}
}

define_shaders!(VertexShader, HullShader, DomainShader, GeometryShader, PixelShader);

/// A complete set of shaders to link a program.
pub enum ShaderSet<R: Resources> {
    /// Simple program: Vs-Ps
    Simple(VertexShader<R>, PixelShader<R>),
    /// Geometry shader programs: Vs-Gs-Ps
    Geometry(VertexShader<R>, GeometryShader<R>, PixelShader<R>),
    //TODO: Tessellated, TessellatedGeometry, TransformFeedback
}

/// Features that the device supports.
#[derive(Copy, Clone, Debug)]
#[allow(missing_docs)] // pretty self-explanatory fields!
pub struct Capabilities {
    pub shader_model: shade::ShaderModel,

    pub max_vertex_count: usize,
    pub max_index_count: usize,
    pub max_draw_buffers: usize,
    pub max_texture_size: usize,
    pub max_vertex_attributes: usize,

    /// In GLES it is not allowed to re-bind a buffer to a different
    /// target than the one it was initialized with.
    pub buffer_role_change_allowed: bool,

    pub array_buffer_supported: bool,
    pub fragment_output_supported: bool,
    pub immutable_storage_supported: bool,
    pub instance_base_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub render_targets_supported: bool,
    pub sampler_objects_supported: bool,
    pub srgb_color_supported: bool,
    pub uniform_block_supported: bool,
    pub vertex_base_supported: bool,
    pub separate_blending_slots_supported: bool,
}

/// Specifies the access allowed to a buffer mapping.
#[derive(Copy, Clone)]
pub enum MapAccess {
    /// Only allow reads.
    Readable,
    /// Only allow writes.
    Writable,
    /// Allow full access.
    RW
}

/// Describes what geometric primitives are created from vertex data.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
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

/// Role of the memory buffer. GLES doesn't chaning bind points for buffers.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum BufferRole {
    /// Generic vertex buffer
    Vertex,
    /// Index buffer
    Index,
    /// Uniform block buffer
    Uniform,
}

/// A hint as to how this buffer will be used.
///
/// The nature of these hints make them very implementation specific. Different drivers on
/// different hardware will handle them differently. Only careful profiling will tell which is the
/// best to use for a specific buffer.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BufferInfo {
    /// Role
    pub role: BufferRole,
    /// Usage hint
    pub usage: BufferUsage,
    /// Size in bytes
    pub size: usize,
}

/// An error happening on buffer updates.
#[derive(Clone, PartialEq, Debug)]
pub enum BufferUpdateError {
    /// Trying to change the contents outside of the allocation.
    OutOfBounds,
}

/// Resources pertaining to a specific API.
#[allow(missing_docs)]
pub trait Resources:                 Clone + Hash + fmt::Debug + Eq + PartialEq {
    type Buffer:              Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type ArrayBuffer:         Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Shader:              Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Program:             Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type PipelineStateObject: Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type FrameBuffer:         Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Surface:             Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type RenderTargetView:    Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type DepthStencilView:    Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type ShaderResourceView:  Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type UnorderedAccessView: Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Texture:             Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Sampler:             Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
    type Fence:               Copy + Clone + Hash + fmt::Debug + Eq + PartialEq + Send + Sync;
}

#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// Associated mapper type
    type Mapper: Clone + mapping::Raw;

    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;

    // resource creation
    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, NotSupported>;
    fn create_buffer_raw(&mut self, size: usize, BufferRole, BufferUsage) -> handle::RawBuffer<R>;
    fn create_buffer_static_raw(&mut self, data: &[u8], BufferRole) -> handle::RawBuffer<R>;
    fn create_buffer_static<T>(&mut self, data: &[T], role: BufferRole) -> handle::Buffer<R, T> {
        handle::Buffer::from_raw(
            self.create_buffer_static_raw(as_byte_slice(data), role))
    }
    fn create_buffer_dynamic<T>(&mut self, num: usize, role: BufferRole) -> handle::Buffer<R, T> {
        handle::Buffer::from_raw(
            self.create_buffer_raw(num * mem::size_of::<T>(), role, BufferUsage::Stream))
    }

    fn create_pipeline_state_raw<'a>(&mut self, PrimitiveType, &ShaderSet<R>, &DrawState,
                                 &pso::LinkMap<'a>, &mut pso::RegisterMap<'a>)
                                 -> Result<handle::RawPipelineState<R>, pso::CreationError>;
    fn create_program(&mut self, shader_set: &ShaderSet<R>)
                      -> Result<handle::Program<R>, shade::CreateProgramError>;
    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<handle::Shader<R>, shade::CreateShaderError>;
    fn create_shader_vertex(&mut self, code: &[u8]) -> Result<VertexShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Vertex, code).map(|s| VertexShader(s))
    }
    fn create_shader_geometry(&mut self, code: &[u8]) -> Result<GeometryShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Geometry, code).map(|s| GeometryShader(s))
    }
    fn create_shader_pixel(&mut self, code: &[u8]) -> Result<PixelShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Pixel, code).map(|s| PixelShader(s))
    }

    fn create_frame_buffer(&mut self) -> Result<handle::FrameBuffer<R>, NotSupported>;
    fn create_surface(&mut self, tex::SurfaceInfo) -> Result<handle::Surface<R>, tex::SurfaceError>;
    fn create_texture(&mut self, tex::TextureInfo) -> Result<handle::Texture<R>, tex::TextureError>;
    fn create_sampler(&mut self, tex::SamplerInfo) -> handle::Sampler<R>;

    /// Update the information stored in a specific buffer
    fn update_buffer_raw(&mut self, buf: &handle::RawBuffer<R>, data: &[u8], offset_bytes: usize)
                         -> Result<(), BufferUpdateError>;
    fn update_buffer<T>(&mut self, buf: &handle::Buffer<R, T>, data: &[T], offset_elements: usize)
                        -> Result<(), BufferUpdateError> {
        self.update_buffer_raw(buf.raw(), as_byte_slice(data), mem::size_of::<T>() * offset_elements)
    }
    fn map_buffer_raw(&mut self, &handle::RawBuffer<R>, MapAccess) -> Self::Mapper;
    fn unmap_buffer_raw(&mut self, Self::Mapper);
    fn map_buffer_readable<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::Readable<T, R, Self> where
        Self: Sized;
    fn map_buffer_writable<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::Writable<T, R, Self> where
        Self: Sized;
    fn map_buffer_rw<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::RW<T, R, Self> where
        Self: Sized;

    /// Update the information stored in a texture
    fn update_texture_raw(&mut self, tex: &handle::Texture<R>,
                          img: &tex::ImageInfo, data: &[u8],
                          kind: Option<tex::Kind>) -> Result<(), tex::TextureError>;

    fn update_texture<T>(&mut self, tex: &handle::Texture<R>,
                         img: &tex::ImageInfo, data: &[T],
                         kind: Option<tex::Kind>) -> Result<(), tex::TextureError> {
        self.update_texture_raw(tex, img, as_byte_slice(data), kind)
    }

    fn generate_mipmap(&mut self, &handle::Texture<R>);

    /// Create a new texture with given data
    fn create_texture_static<T>(&mut self, info: tex::TextureInfo, data: &[T])
                             -> Result<handle::Texture<R>, tex::TextureError> {
        let image_info = info.into();
        match self.create_texture(info) {
            Ok(handle) => self.update_texture(&handle, &image_info, data, None)
                              .map(|_| handle),
            Err(e) => Err(e),
        }
    }
}

/// All the data needed simultaneously for submitting a command buffer for
/// execution on a device.
pub type SubmitInfo<'a, D: Device> = (
    &'a D::CommandBuffer,
    &'a draw::DataBuffer,
    &'a handle::Manager<D::Resources>
);

/// An interface for performing draw calls using a specific graphics API
pub trait Device {
    /// Associated resources type.
    type Resources: Resources;
    /// Associated command buffer type.
    type CommandBuffer: draw::CommandBuffer<Self::Resources>;

    /// Returns the capabilities available to the specific API implementation.
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;

    /// Reset all the states to disabled/default.
    fn reset_state(&mut self);

    /// Submit a command buffer for execution.
    fn submit(&mut self, SubmitInfo<Self>);

    /// Cleanup unused resources, to be called between frames.
    fn cleanup(&mut self);
}

/// Extension to the Device that allows for submitting of commands
/// around a fence
pub trait DeviceFence<R: Resources>: Device<Resources=R> where
    <Self as Device>::CommandBuffer: draw::CommandBuffer<R> {
    /// Submit a command buffer to the stream creating a fence
    /// the fence is signaled after the GPU has executed all commands
    /// in the buffer
    fn fenced_submit(&mut self, SubmitInfo<Self>, after: Option<handle::Fence<R>>) -> handle::Fence<R>;

    /// Wait on the supplied fence stalling the current thread until
    /// the fence is satisfied
    fn fence_wait(&mut self, fence: &handle::Fence<R>);
}
