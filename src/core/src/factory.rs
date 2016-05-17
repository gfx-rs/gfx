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

//! Resource factory
//!
//! This module exposes the `Factory` trait, used for creating and managing graphics resources, and
//! includes several items to facilitate this. 

use std::error::Error;
use std::fmt;
use std::mem;
use {handle, format, mapping, pso, shade, target, tex};
use {Capabilities, Resources, Pod};
use {VertexShader, GeometryShader, PixelShader, ShaderSet};


/// A service trait used to get the raw data out of
/// strong types. Not meant for public use.
pub trait Typed: Sized {
    /// The raw type behind the phantom.
    type Raw;
    /// Crete a new phantom from the raw type.
    fn new(raw: Self::Raw) -> Self;
    /// Get an internal reference to the raw type.
    fn raw(&self) -> &Self::Raw;
}


/// Cast a slice from one POD type to another.
pub fn cast_slice<A: Pod, B: Pod>(slice: &[A]) -> &[B] {
    use std::slice;
    let raw_len = mem::size_of::<A>().wrapping_mul(slice.len());
    let len = raw_len / mem::size_of::<B>();
    assert_eq!(raw_len, mem::size_of::<B>().wrapping_mul(len));
    unsafe {
        slice::from_raw_parts(slice.as_ptr() as *const B, len)
    }
}

/// Specifies the access allowed to a buffer mapping.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum MapAccess {
    /// Only allow reads.
    Readable,
    /// Only allow writes.
    Writable,
    /// Allow full access.
    RW
}

bitflags!(
    /// Bind flags
    pub flags Bind: u8 {
        /// The resource can be rendered into.
        const RENDER_TARGET    = 0x1,
        /// The resource can serve as a depth/stencil target.
        const DEPTH_STENCIL    = 0x2,
        /// The resource can be bound to the shader for reading.
        const SHADER_RESOURCE  = 0x4,
        /// The resource can be bound to the shader for writing.
        const UNORDERED_ACCESS = 0x8,
    }
);


/// Role of the memory buffer. GLES doesn't allow chaning bind points for buffers.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum BufferRole {
    /// Generic vertex buffer
    Vertex,
    /// Index buffer
    Index,
    /// Uniform block buffer //TODO: rename to `Constant`
    Uniform,
}

/// A hint as to how this buffer/texture will be used.
///
/// The nature of these hints make them very implementation specific. Different drivers on
/// different hardware will handle them differently. Only careful profiling will tell which is the
/// best to use for a specific buffer.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Usage {
    /// GPU: read + write, CPU: copy. Optimal for render targets.
    GpuOnly,
    /// GPU: read, CPU: none. Optimal for resourced textures/buffers.
    Const,
    /// GPU: read, CPU: write.
    Dynamic,
    /// GPU: copy, CPU: as specified. Used as a staging buffer,
    /// to be copied back and forth with on-GPU targets.
    CpuOnly(MapAccess),
}

/// An information block that is immutable and associated with each buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BufferInfo {
    /// Role
    pub role: BufferRole,
    /// Usage hint
    pub usage: Usage,
    /// Bind flags
    pub bind: Bind,
    /// Size in bytes
    pub size: usize,
    /// Stride of a single element, in bytes. Only used for structured buffers
    /// that you use via shader resource / unordered access views.
    pub stride: usize,
}

/// Error creating a buffer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BufferError {
    /// Some of the bind flags are not supported.
    UnsupportedBind(Bind),
    /// Unknown other error.
    Other,
    //todo: unsupported role
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let BufferError::UnsupportedBind(ref bind) = *self {
            write!(f, "{}: {:?}", self.description(), bind)
        } else {
            write!(f, "{}", self.description())
        }
    }
}

impl Error for BufferError {
    fn description(&self) -> &str {
        match *self {
            BufferError::UnsupportedBind(_) => "Bind flags are not supported",
            BufferError::Other => "An unknown error occurred",
        }
    }
}

/// An error happening on buffer updates.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BufferUpdateError {
    /// Trying to change the contents outside of the allocation.
    OutOfBounds,
}

impl fmt::Display for BufferUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for BufferUpdateError {
    fn description(&self) -> &str {
        match *self {
            BufferUpdateError::OutOfBounds =>
                "Tried to change the buffer contents outside of the allocation",
        }
    }
}

/// An error associated with selected texture layer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LayerError {
    /// The source texture kind doesn't support array slices.
    NotExpected(tex::Kind),
    /// Selected layer is outside of the provided range.
    OutOfBounds(target::Layer, target::Layer),
}

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceViewError {
    /// The corresponding bind flag is not present in the texture.
    NoBindFlag,
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// Selected layer can not be viewed for this texture.
    Layer(LayerError),
    /// The backend was refused for some reason.
    Unsupported,
}

impl fmt::Display for ResourceViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let ResourceViewError::Channel(ref channel_type) = *self {
            write!(f, "{}: {:?}", self.description(), channel_type)
        } else {
            write!(f, "{}", self.description())
        }
    }
}

impl Error for ResourceViewError {
    fn description(&self) -> &str {
        match *self {
            ResourceViewError::NoBindFlag => "The corresponding bind flag is not present in the texture",
            ResourceViewError::Channel(_) => "Selected channel type is not supported for this texture",
            ResourceViewError::Unsupported => "The backend was refused for some reason",
        }
    }
}

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, PartialEq, Debug)]
pub enum TargetViewError {
    /// The `RENDER_TARGET`/`DEPTH_STENCIL` flag is not present in the texture.
    NoBindFlag,
    /// Selected mip level doesn't exist.
    BadLevel(target::Level),
    /// Selected array layer doesn't exist.
    Layer(LayerError),
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// The backend was refused for some reason.
    Unsupported,
}

impl fmt::Display for TargetViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            TargetViewError::BadLevel(ref level) => write!(f, "{}: {}", description, level),
            TargetViewError::BadLayer(ref layer) => write!(f, "{}: {}", description, layer),
            TargetViewError::Channel(ref channel)  => write!(f, "{}: {:?}", description, channel),
            _ => write!(f, "{}", description)
        }
    }
}

impl Error for TargetViewError {
    fn description(&self) -> &str {
        match *self {
            TargetViewError::NoBindFlag =>
                "The `RENDER_TARGET`/`DEPTH_STENCIL` flag is not present in the texture",
            TargetViewError::BadLevel(_) =>
                "Selected mip level doesn't exist",
            TargetViewError::BadLayer(_) =>
                "Selected array layer doesn't exist",
            TargetViewError::Channel(_) =>
                "Selected channel type is not supported for this texture",
            TargetViewError::Unsupported =>
                "The backend was refused for some reason",
        }
    }
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

impl fmt::Display for CombinedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CombinedError::Texture(ref e) => write!(f, "{}: {}", self.description(), e),
            CombinedError::Resource(ref e) => write!(f, "{}: {}", self.description(), e),
            CombinedError::Target(ref e) => write!(f, "{}: {}", self.description(), e),
        }
    }
}

impl Error for CombinedError {
    fn description(&self) -> &str {
        match *self {
            CombinedError::Texture(_) => "Failed to create the raw texture",
            CombinedError::Resource(_) => "Failed to create SRV or UAV",
            CombinedError::Target(_) => "Failed to create RTV or DSV",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            CombinedError::Texture(ref e) => Some(e),
            CombinedError::Resource(ref e) => Some(e),
            CombinedError::Target(ref e) => Some(e),
        }
    }
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

/// A `Factory` is responsible for creating and managing resources for the context it was created
/// with. 
///
/// # Construction and Handling
/// A `Factory` is typically created along with other objects using a helper function of the
/// appropriate gfx_window module (e.g. gfx_window_glutin::init()).
///
/// This factory structure can then be used to create and manage different resources, like buffers,
/// shader programs and textures. See the individual methods for more information.
///
/// Also see the `FactoryExt` trait inside the `gfx` module for additional methods.
#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// Associated mapper type
    type Mapper: Clone + mapping::Raw;

    /// Returns the capabilities of this `Factory`. This usually depends on the graphics API being
    /// used.
    fn get_capabilities(&self) -> &Capabilities;

    // resource creation
    fn create_buffer_raw(&mut self, BufferInfo) -> Result<handle::RawBuffer<R>, BufferError>;
    fn create_buffer_const_raw(&mut self, data: &[u8], stride: usize, BufferRole, Bind)
                                -> Result<handle::RawBuffer<R>, BufferError>;
    fn create_buffer_const<T: Pod>(&mut self, data: &[T], role: BufferRole, bind: Bind) -> Result<handle::Buffer<R, T>, BufferError> {
        self.create_buffer_const_raw(cast_slice(data), mem::size_of::<T>(), role, bind)
            .map(|raw| Typed::new(raw))
    }
    fn create_buffer_dynamic<T>(&mut self, num: usize, role: BufferRole, bind: Bind)
                                -> Result<handle::Buffer<R, T>, BufferError> {
        let stride = mem::size_of::<T>();
        let info = BufferInfo {
            role: role,
            usage: Usage::Dynamic,
            bind: bind,
            size: num * stride,
            stride: stride,
        };
        self.create_buffer_raw(info).map(|raw| Typed::new(raw))
    }
    fn create_buffer_staging<T>(&mut self, num: usize, role: BufferRole, bind: Bind, map: MapAccess)
                             -> Result<handle::Buffer<R, T>, BufferError> {
        let stride = mem::size_of::<T>();
        let info = BufferInfo {
            role: role,
            usage: Usage::CpuOnly(map),
            bind: bind,
            size: num * stride,
            stride: stride,
        };
        self.create_buffer_raw(info).map(|raw| Typed::new(raw))
    }

    /// Creates a new `RawPipelineState`. To create a safely typed `PipelineState`, see the
    /// `FactoryExt` trait and `pso` module, both in the `gfx` crate.
    fn create_pipeline_state_raw(&mut self, &handle::Program<R>, &pso::Descriptor)
                                 -> Result<handle::RawPipelineState<R>, pso::CreationError>;
                                 
    /// Creates a new shader `Program` for the supplied `ShaderSet`.
    fn create_program(&mut self, shader_set: &ShaderSet<R>)
                      -> Result<handle::Program<R>, shade::CreateProgramError>;
    
    /// Compiles a shader source into a `Shader` object that can be used to create a shader
    /// `Program`.
    fn create_shader(&mut self, stage: shade::Stage, code: &[u8]) ->
                     Result<handle::Shader<R>, shade::CreateShaderError>;
    /// Compiles a `VertexShader` from source.
    fn create_shader_vertex(&mut self, code: &[u8]) -> Result<VertexShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Vertex, code).map(|s| VertexShader(s))
    }
    /// Compiles a `GeometryShader` from source.
    fn create_shader_geometry(&mut self, code: &[u8]) -> Result<GeometryShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Geometry, code).map(|s| GeometryShader(s))
    }
    /// Compiles a `PixelShader` from source. This is the same as what some APIs call a fragment
    /// shader.
    fn create_shader_pixel(&mut self, code: &[u8]) -> Result<PixelShader<R>, shade::CreateShaderError> {
        self.create_shader(shade::Stage::Pixel, code).map(|s| PixelShader(s))
    }

    fn create_sampler(&mut self, tex::SamplerInfo) -> handle::Sampler<R>;

    fn map_buffer_raw(&mut self, &handle::RawBuffer<R>, MapAccess) -> Self::Mapper;
    fn unmap_buffer_raw(&mut self, Self::Mapper);
    fn map_buffer_readable<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::Readable<T, R, Self> where
        Self: Sized;
    fn map_buffer_writable<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::Writable<T, R, Self> where
        Self: Sized;
    fn map_buffer_rw<T: Copy>(&mut self, &handle::Buffer<R, T>) -> mapping::RW<T, R, Self> where
        Self: Sized;

    /// Create a new empty raw texture with no data. The channel type parameter is a hint,
    /// required to assist backends that have no concept of typeless formats (OpenGL).
    /// The initial data, if given, has to be provided for all mip levels and slices:
    /// Slice0.Mip0, Slice0.Mip1, ..., Slice1.Mip0, ...
    fn create_texture_raw(&mut self, tex::Descriptor, Option<format::ChannelType>, Option<&[&[u8]]>)
                          -> Result<handle::RawTexture<R>, tex::Error>;

    fn view_buffer_as_shader_resource_raw(&mut self, &handle::RawBuffer<R>)
        -> Result<handle::RawShaderResourceView<R>, ResourceViewError>;
    fn view_buffer_as_unordered_access_raw(&mut self, &handle::RawBuffer<R>)
        -> Result<handle::RawUnorderedAccessView<R>, ResourceViewError>;
    fn view_texture_as_shader_resource_raw(&mut self, &handle::RawTexture<R>, tex::ResourceDesc)
        -> Result<handle::RawShaderResourceView<R>, ResourceViewError>;
    fn view_texture_as_unordered_access_raw(&mut self, &handle::RawTexture<R>)
        -> Result<handle::RawUnorderedAccessView<R>, ResourceViewError>;
    fn view_texture_as_render_target_raw(&mut self, &handle::RawTexture<R>, tex::RenderDesc)
        -> Result<handle::RawRenderTargetView<R>, TargetViewError>;
    fn view_texture_as_depth_stencil_raw(&mut self, &handle::RawTexture<R>, tex::DepthStencilDesc)
        -> Result<handle::RawDepthStencilView<R>, TargetViewError>;

    fn create_texture<S>(&mut self, kind: tex::Kind, levels: target::Level,
                      bind: Bind, usage: Usage, channel_hint: Option<format::ChannelType>)
                      -> Result<handle::Texture<R, S>, tex::Error>
    where S: format::SurfaceTyped
    {
        let desc = tex::Descriptor {
            kind: kind,
            levels: levels,
            format: S::get_surface_type(),
            bind: bind,
            usage: usage,
        };
        let raw = try!(self.create_texture_raw(desc, channel_hint, None));
        Ok(Typed::new(raw))
    }

    fn view_buffer_as_shader_resource<T>(&mut self, buf: &handle::Buffer<R, T>)
                                      -> Result<handle::ShaderResourceView<R, T>, ResourceViewError>
    {
        //TODO: check bind flags
        self.view_buffer_as_shader_resource_raw(buf.raw()).map(Typed::new)
    }

    fn view_buffer_as_unordered_access<T>(&mut self, buf: &handle::Buffer<R, T>)
                                      -> Result<handle::UnorderedAccessView<R, T>, ResourceViewError>
    {
        //TODO: check bind flags
        self.view_buffer_as_unordered_access_raw(buf.raw()).map(Typed::new)
    }

    fn view_texture_as_shader_resource<T: format::TextureFormat>(&mut self, tex: &handle::Texture<R, T::Surface>,
                                       levels: (target::Level, target::Level), swizzle: format::Swizzle)
                                       -> Result<handle::ShaderResourceView<R, T::View>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(SHADER_RESOURCE) {
            return Err(ResourceViewError::NoBindFlag)
        }
        assert!(levels.0 <= levels.1);
        let desc = tex::ResourceDesc {
            channel: <T::Channel as format::ChannelTyped>::get_channel_type(),
            layer: None,
            min: levels.0,
            max: levels.1,
            swizzle: swizzle,
        };
        self.view_texture_as_shader_resource_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_unordered_access<T: format::TextureFormat>(&mut self, tex: &handle::Texture<R, T::Surface>)
                                        -> Result<handle::UnorderedAccessView<R, T::View>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(UNORDERED_ACCESS) {
            return Err(ResourceViewError::NoBindFlag)
        }
        self.view_texture_as_unordered_access_raw(tex.raw())
            .map(Typed::new)
    }

    fn view_texture_as_render_target<T: format::RenderFormat>(&mut self, tex: &handle::Texture<R, T::Surface>,
                                     level: target::Level, layer: Option<target::Layer>)
                                     -> Result<handle::RenderTargetView<R, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(RENDER_TARGET) {
            return Err(TargetViewError::NoBindFlag)
        }
        let desc = tex::RenderDesc {
            channel: <T::Channel as format::ChannelTyped>::get_channel_type(),
            level: level,
            layer: layer,
        };
        self.view_texture_as_render_target_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_depth_stencil<T: format::DepthFormat>(&mut self, tex: &handle::Texture<R, T::Surface>,
                                     level: target::Level, layer: Option<target::Layer>, flags: tex::DepthStencilFlags)
                                     -> Result<handle::DepthStencilView<R, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(DEPTH_STENCIL) {
            return Err(TargetViewError::NoBindFlag)
        }
        let desc = tex::DepthStencilDesc {
            level: level,
            layer: layer,
            flags: flags,
        };
        self.view_texture_as_depth_stencil_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_depth_stencil_trivial<T: format::DepthFormat>(&mut self, tex: &handle::Texture<R, T::Surface>)
                                            -> Result<handle::DepthStencilView<R, T>, TargetViewError>
    {
        self.view_texture_as_depth_stencil(tex, 0, None, tex::DepthStencilFlags::empty())
    }

    fn create_texture_const_u8<T: format::TextureFormat>(&mut self, kind: tex::Kind, data: &[&[u8]])
                               -> Result<(handle::Texture<R, T::Surface>, handle::ShaderResourceView<R, T::View>), CombinedError>
    {
        let surface = <T::Surface as format::SurfaceTyped>::get_surface_type();
        let num_slices = kind.get_num_slices().unwrap_or(1) as usize;
        let num_faces = if kind.is_cube() {6} else {1};
        let desc = tex::Descriptor {
            kind: kind,
            levels: (data.len() / (num_slices * num_faces)) as tex::Level,
            format: surface,
            bind: SHADER_RESOURCE,
            usage: Usage::Const,
        };
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let raw = try!(self.create_texture_raw(desc, Some(cty), Some(data)));
        let levels = (0, raw.get_info().levels - 1);
        let tex = Typed::new(raw);
        let view = try!(self.view_texture_as_shader_resource::<T>(&tex, levels, format::Swizzle::new()));
        Ok((tex, view))
    }

    fn create_texture_const<T: format::TextureFormat>(&mut self, kind: tex::Kind,
                            data: &[&[<T::Surface as format::SurfaceTyped>::DataType]])
                            -> Result<(handle::Texture<R, T::Surface>, handle::ShaderResourceView<R, T::View>), CombinedError>
    {
        // we can use cast_slice on a 2D slice, have to use a temporary array of slices
        let mut raw_data: [&[u8]; 0x100] = [&[]; 0x100];
        assert!(data.len() <= raw_data.len());
        for (rd, d) in raw_data.iter_mut().zip(data.iter()) {
            *rd = cast_slice(*d);
        }
        self.create_texture_const_u8::<T>(kind, &raw_data[.. data.len()])
    }

    fn create_render_target<T: format::RenderFormat + format::TextureFormat>
                           (&mut self, width: tex::Size, height: tex::Size)
                            -> Result<(handle::Texture<R, T::Surface>,
                                       handle::ShaderResourceView<R, T::View>,
                                       handle::RenderTargetView<R, T>
                                ), CombinedError>
    {
        let kind = tex::Kind::D2(width, height, tex::AaMode::Single);
        let levels = 1;
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, levels, SHADER_RESOURCE | RENDER_TARGET, Usage::GpuOnly, Some(cty)));
        let resource = try!(self.view_texture_as_shader_resource::<T>(&tex, (0, levels-1), format::Swizzle::new()));
        let target = try!(self.view_texture_as_render_target(&tex, 0, None));
        Ok((tex, resource, target))
    }

    fn create_depth_stencil<T: format::DepthFormat + format::TextureFormat>
                           (&mut self, width: tex::Size, height: tex::Size)
                            -> Result<(handle::Texture<R, T::Surface>,
                                       handle::ShaderResourceView<R, T::View>,
                                       handle::DepthStencilView<R, T>
                                ), CombinedError>
    {
        let kind = tex::Kind::D2(width, height, tex::AaMode::Single);
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, 1, SHADER_RESOURCE | DEPTH_STENCIL, Usage::GpuOnly, Some(cty)));
        let resource = try!(self.view_texture_as_shader_resource::<T>(&tex, (0, 0), format::Swizzle::new()));
        let target = try!(self.view_texture_as_depth_stencil_trivial(&tex));
        Ok((tex, resource, target))
    }

    fn create_depth_stencil_view_only<T: format::DepthFormat + format::TextureFormat>
                                     (&mut self, width: tex::Size, height: tex::Size)
                                      -> Result<handle::DepthStencilView<R, T>, CombinedError>
    {
        let kind = tex::Kind::D2(width, height, tex::AaMode::Single);
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, 1, DEPTH_STENCIL, Usage::GpuOnly, Some(cty)));
        let target = try!(self.view_texture_as_depth_stencil_trivial(&tex));
        Ok(target)
    }
}
