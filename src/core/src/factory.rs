// Copyright 2015 The Gfx-rs Developers.
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

//! Resource factory.

use std::mem;
use {handle, format, mapping, pso, shade, target, tex};
use {Capabilities, Resources};
use {VertexShader, GeometryShader, PixelShader, ShaderSet};
use draw::CommandBuffer;

/// Generic error for features that are not supported
/// by the device capabilities.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct NotSupported;

/// A service trait used to get the raw data out of
/// strong types. Not meant for public use.
pub trait Phantom: Sized {
    /// The raw type behind the phantom.
    type Raw;
    /// Crete a new phantom from the raw type.
    fn new(raw: Self::Raw) -> Self;
    /// Get an internal reference to the raw type.
    fn raw(&self) -> &Self::Raw;
}


/// Cast a slice from one type to another.
pub fn cast_slice<A, B>(slice: &[A]) -> &[B] {
    use std::slice;
    let raw_len = mem::size_of::<A>() * slice.len();
    let len = raw_len / mem::size_of::<B>();
    assert_eq!(raw_len, len * mem::size_of::<B>());
    unsafe {
        slice::from_raw_parts(slice.as_ptr() as *const B, len)
    }
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
    Const,
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

bitflags!(
    /// Bind flags
    flags Bind: u8 {
        /// The resource can be bound to the shader for reading.
        const SHADER_RESOURCE  = 0x1,
        /// The resource can be rendered into.
        const RENDER_TARGET    = 0x2,
        /// The resource can be bound to the shader for writing.
        const UNORDERED_ACCESS = 0x4,
    }
);

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceViewError {
    /// The corresponding bind flag does not present in the texture.
    NoBindFlag,
    /// The backend refused for some reason.
    Unsupported,
}

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, PartialEq, Debug)]
pub enum TargetViewError {
    /// The `RENDER_TARGET` flag does not present in the texture.
    NoBindFlag,
    /// Tried to view more than there is.
    Size,
    /// The backend refused for some reason.
    Unsupported,
}

/// An error from creating textures with views at the same time.
#[derive(Clone, PartialEq, Debug)]
pub enum CombinedError {
    /// Failed to create the raw texture.
    Texture(tex::Error),
    /// Failed to create SRV or UAV.
    Resource(ResourceViewError),
    /// Failed to create RTV or DSV.
    Target(TargetViewError),
}

impl From<tex::Error> for CombinedError {
    fn from(e: tex::Error) -> CombinedError {
        CombinedError::Texture(e)
    }
}
impl From<ResourceViewError> for CombinedError {
    fn from(e: ResourceViewError) -> CombinedError {
        CombinedError::Resource(e)
    }
}
impl From<TargetViewError> for CombinedError {
    fn from(e: TargetViewError) -> CombinedError {
        CombinedError::Target(e)
    }
}

#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// Associated command buffer type
    type CommandBuffer: CommandBuffer<R>;
    /// Associated mapper type
    type Mapper: Clone + mapping::Raw;

    /// Returns the capabilities available to the specific API implementation
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;

    fn create_command_buffer(&mut self) -> Self::CommandBuffer;

    // resource creation
    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, NotSupported>;
    fn create_buffer_raw(&mut self, size: usize, BufferRole, BufferUsage) -> handle::RawBuffer<R>;
    fn create_buffer_static_raw(&mut self, data: &[u8], BufferRole) -> handle::RawBuffer<R>;
    fn create_buffer_static<T>(&mut self, data: &[T], role: BufferRole) -> handle::Buffer<R, T> {
        let raw = self.create_buffer_static_raw(cast_slice(data), role);
        Phantom::new(raw)
    }
    fn create_buffer_dynamic<T>(&mut self, num: usize, role: BufferRole) -> handle::Buffer<R, T> {
        let raw = self.create_buffer_raw(num * mem::size_of::<T>(), role, BufferUsage::Stream);
        Phantom::new(raw)
    }

    fn create_pipeline_state_raw(&mut self, &handle::Program<R>, &pso::Descriptor)
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
        self.update_buffer_raw(buf.raw(), cast_slice(data), mem::size_of::<T>() * offset_elements)
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
                          img: &tex::ImageInfo, data: &[u8], face: Option<tex::CubeFace>)
                          -> Result<(), tex::TextureError>;

    fn update_texture<T>(&mut self, tex: &handle::Texture<R>,
                         img: &tex::ImageInfo, data: &[T], face: Option<tex::CubeFace>)
                         -> Result<(), tex::TextureError> {
        self.update_texture_raw(tex, img, cast_slice(data), face)
    }

    fn generate_mipmap(&mut self, &handle::Texture<R>);
    fn generate_mipmap_raw(&mut self, &handle::RawTexture<R>);

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

    fn create_new_texture_raw(&mut self, tex::Descriptor)
        -> Result<handle::RawTexture<R>, tex::Error>;
    fn create_new_texture_with_data(&mut self, tex::Descriptor, format::ChannelType, &[u8])
        -> Result<handle::RawTexture<R>, tex::Error>;
    fn view_buffer_as_shader_resource_raw(&mut self, &handle::RawBuffer<R>)
        -> Result<handle::RawShaderResourceView<R>, ResourceViewError>;
    fn view_buffer_as_unordered_access_raw(&mut self, &handle::RawBuffer<R>)
        -> Result<handle::RawUnorderedAccessView<R>, ResourceViewError>;
    fn view_texture_as_shader_resource_raw(&mut self, &handle::RawTexture<R>, tex::ViewDesc)
        -> Result<handle::RawShaderResourceView<R>, ResourceViewError>;
    fn view_texture_as_unordered_access_raw(&mut self, &handle::RawTexture<R>)
        -> Result<handle::RawUnorderedAccessView<R>, ResourceViewError>;
    fn view_texture_as_render_target_raw(&mut self, &handle::RawTexture<R>, target::Level, Option<target::Layer>)
        -> Result<handle::RawRenderTargetView<R>, TargetViewError>;
    fn view_texture_as_depth_stencil_raw(&mut self, &handle::RawTexture<R>, Option<target::Layer>)
        -> Result<handle::RawDepthStencilView<R>, TargetViewError>;

    fn create_new_texture<S: format::SurfaceTyped>(&mut self, kind: tex::Kind, levels: target::Level, bind: Bind)
                          -> Result<handle::NewTexture<R, S>, tex::Error>
    {
        let desc = tex::Descriptor {
            kind: kind,
            levels: levels,
            format: S::get_surface_type(),
            bind: bind,
        };
        let raw = try!(self.create_new_texture_raw(desc));
        Ok(Phantom::new(raw))
    }

    fn view_texture_as_shader_resource<T: format::Formatted>(&mut self,
                                       tex: &handle::NewTexture<R, T::Surface>, levels: (target::Level, target::Level))
                                       -> Result<handle::ShaderResourceView<R, T>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(SHADER_RESOURCE) {
            return Err(ResourceViewError::NoBindFlag)
        }
        assert!(levels.0 <= levels.1);
        let desc = tex::ViewDesc {
            channel: <T::Channel as format::ChannelTyped>::get_channel_type(),
            min: levels.0,
            max: levels.1,
        };
        let raw = try!(self.view_texture_as_shader_resource_raw(tex.raw(), desc));
        Ok(Phantom::new(raw))
    }

    fn view_texture_as_unordered_access<T: format::Formatted>(&mut self, tex: &handle::NewTexture<R, T::Surface>)
                                        -> Result<handle::UnorderedAccessView<R, T>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(UNORDERED_ACCESS) {
            return Err(ResourceViewError::NoBindFlag)
        }
        let raw = try!(self.view_texture_as_unordered_access_raw(tex.raw()));
        Ok(Phantom::new(raw))
    }

    fn view_texture_as_render_target<T: format::RenderFormat>(&mut self,
                                     tex: &handle::NewTexture<R, T::Surface>, level: target::Level, layer: Option<target::Layer>)
                                     -> Result<handle::RenderTargetView<R, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(RENDER_TARGET) {
            return Err(TargetViewError::NoBindFlag)
        }
        let raw = try!(self.view_texture_as_render_target_raw(tex.raw(), level, layer));
        Ok(Phantom::new(raw))
    }

    fn view_texture_as_depth_stencil<T: format::DepthStencilFormat>(&mut self,
                                     tex: &handle::NewTexture<R, T::Surface>, layer: Option<target::Layer>)
                                     -> Result<handle::DepthStencilView<R, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(RENDER_TARGET) {
            return Err(TargetViewError::NoBindFlag)
        }
        let raw = try!(self.view_texture_as_depth_stencil_raw(tex.raw(), layer));
        Ok(Phantom::new(raw))
    }

    fn create_texture_const<T: format::Formatted>(&mut self, kind: tex::Kind, data: &[T], mipmap: bool)
                            -> Result<(handle::NewTexture<R, T::Surface>, handle::ShaderResourceView<R, T>), CombinedError>
    {
        let desc = tex::Descriptor {
            kind: kind,
            levels: if mipmap {99} else {1},
            format: <T::Surface as format::SurfaceTyped>::get_surface_type(),
            bind: SHADER_RESOURCE,
        };
        //todo: check sizes
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let raw = try!(self.create_new_texture_with_data(desc, cty, cast_slice(data)));
        self.generate_mipmap_raw(&raw);
        let levels = (0, raw.get_info().levels - 1);
        let tex = Phantom::new(raw);
        let view = try!(self.view_texture_as_shader_resource(&tex, levels));
        Ok((tex, view))
    }

    fn create_render_target<T: format::RenderFormat>(&mut self, width: tex::Size, height: tex::Size, allocate_mipmap: bool)
                            -> Result<(handle::NewTexture<R, T::Surface>, handle::ShaderResourceView<R, T>, handle::RenderTargetView<R, T>), CombinedError>
    {
        let kind = tex::Kind::D2(width, height, tex::AaMode::Single);
        let levels = if allocate_mipmap {99} else {1};
        let tex = try!(self.create_new_texture(kind, levels, SHADER_RESOURCE | RENDER_TARGET));
        let resource = try!(self.view_texture_as_shader_resource(&tex, (0, levels)));
        let target = try!(self.view_texture_as_render_target(&tex, 0, None));
        Ok((tex, resource, target))
    }

    fn create_depth_stencil<T: format::DepthStencilFormat>(&mut self, width: tex::Size, height: tex::Size)
                            -> Result<(handle::NewTexture<R, T::Surface>, handle::ShaderResourceView<R, T>, handle::DepthStencilView<R, T>), CombinedError>
    {
        let kind = tex::Kind::D2(width, height, tex::AaMode::Single);
        let tex = try!(self.create_new_texture(kind, 1, SHADER_RESOURCE | RENDER_TARGET));
        let resource = try!(self.view_texture_as_shader_resource(&tex, (0,0)));
        let target = try!(self.view_texture_as_depth_stencil(&tex, None));
        Ok((tex, resource, target))
    }
}
