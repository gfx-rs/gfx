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

//! # Device
//!
//! This module exposes the `Device` trait, used for creating and managing graphics resources, and
//! includes several items to facilitate this.

use std::error::Error;
use std::{mem, fmt};
use {buffer, handle, format, mapping, pass, pso, shade, target, texture};
use {Capabilities, Backend};
use memory::{Usage, Typed, Pod, cast_slice};
use memory::{Bind, RENDER_TARGET, DEPTH_STENCIL, SHADER_RESOURCE, UNORDERED_ACCESS};

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceViewError {
    /// The corresponding bind flag is not present in the texture.
    NoBindFlag,
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// Selected layer can not be viewed for this texture.
    Layer(texture::LayerError),
    /// The backend was refused for some reason.
    Unsupported,
}

impl fmt::Display for ResourceViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ResourceViewError::Channel(ref channel_type) => write!(f, "{}: {:?}", self.description(), channel_type),
            ResourceViewError::Layer(ref le) => write!(f, "{}: {}", self.description(), le),
            _ => write!(f, "{}", self.description())
        }
    }
}

impl Error for ResourceViewError {
    fn description(&self) -> &str {
        match *self {
            ResourceViewError::NoBindFlag => "The corresponding bind flag is not present in the texture",
            ResourceViewError::Channel(_) => "Selected channel type is not supported for this texture",
            ResourceViewError::Layer(_) => "Selected layer can not be viewed for this texture",
            ResourceViewError::Unsupported => "The backend was refused for some reason",
        }
    }

    fn cause(&self) -> Option<&Error> {
        if let ResourceViewError::Layer(ref e) = *self {
            Some(e)
        } else {
            None
        }
    }
}

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, Debug, PartialEq)]
pub enum TargetViewError {
    /// The `RENDER_TARGET`/`DEPTH_STENCIL` flag is not present in the texture.
    NoBindFlag,
    /// Selected mip level doesn't exist.
    Level(target::Level),
    /// Selected array layer doesn't exist.
    Layer(texture::LayerError),
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// The backend was refused for some reason.
    Unsupported,
    /// The RTV cannot be changed due to the references to it existing.
    NotDetached
}

impl fmt::Display for TargetViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            TargetViewError::Level(ref level) => write!(f, "{}: {}", description, level),
            TargetViewError::Layer(ref layer) => write!(f, "{}: {}", description, layer),
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
            TargetViewError::Level(_) =>
                "Selected mip level doesn't exist",
            TargetViewError::Layer(_) =>
                "Selected array layer doesn't exist",
            TargetViewError::Channel(_) =>
                "Selected channel type is not supported for this texture",
            TargetViewError::Unsupported =>
                "The backend was refused for some reason",
            TargetViewError::NotDetached =>
                "The RTV cannot be changed due to the references to it existing",
        }
    }

    fn cause(&self) -> Option<&Error> {
        if let TargetViewError::Layer(ref e) = *self {
            Some(e)
        } else {
            None
        }
    }
}

/// An error from creating textures with views at the same time.
#[derive(Clone, Debug, PartialEq)]
pub enum CombinedError {
    /// Failed to create the raw texture.
    Texture(texture::CreationError),
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

impl From<texture::CreationError> for CombinedError {
    fn from(e: texture::CreationError) -> CombinedError {
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

/// Specifies the waiting targets.
#[derive(Clone, Copy, Debug)]
pub enum WaitFor {
    /// Wait for any target.
    Any,
    /// Wait for all targets at once.
    All,
}

/// # Overview
///
/// A `Device` is responsible for creating and managing resources for the physical device
/// it was created from.
///
/// ## Resource Construction and Handling
///
/// This device structure can then be used to create and manage different resources, like buffers,
/// shader programs and textures. See the individual methods for more information.
///
/// This trait is extended by the [`gfx::DeviceExt` trait](https://docs.rs/gfx/*/gfx/traits/trait.DeviceExt.html).
/// All types implementing `Device` also implement `DeviceExt`.
///
/// ## Immutable resources
///
/// Immutable buffers and textures can only be read by the GPU. They cannot be written by the GPU and
/// cannot be accessed at all by the CPU.
///
/// See:
///  - [`Device::create_texture_immutable`](trait.Device.html#tymethod.create_texture_immutable),
///  - [`Device::create_buffer_immutable`](trait.Device.html#tymethod.create_buffer_immutable).
///
/// ## Raw resources
///
/// The term "raw" is used in the context of types of functions that have a strongly typed and an
/// untyped equivalent, to refer to the untyped equivalent.
///
/// For example ['Device::create_buffer_raw'](trait.Device.html#tymethod.create_buffer_raw) and
/// ['Device::create_buffer'](trait.Device.html#tymethod.create_buffer)
///
/// ## Shader resource views and unordered access views
///
/// This terminology is borrowed from D3D.
///
/// Shader resource views typically wrap textures and buffers to provide read-only access in shaders.
/// An unordered access view provides similar functionality, but enables reading and writing to
/// the buffer or texture in any order.
///
/// See:
///
/// - [The gfx::UNORDERED_ACCESS bit in the gfx::Bind flags](../gfx/struct.Bind.html).
/// - [Device::view_buffer_as_unordered_access](trait.Device.html#method.view_buffer_as_unordered_access).
///
#[allow(missing_docs)]
pub trait Device<B: Backend> {
    /// Returns the capabilities of this `Device`. This usually depends on the graphics API being
    /// used.
    fn get_capabilities(&self) -> &Capabilities;

    // resource creation
    fn create_buffer_raw(&mut self, buffer::Info) -> Result<handle::RawBuffer<B>, buffer::CreationError>;
    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, buffer::Role, Bind)
                                   -> Result<handle::RawBuffer<B>, buffer::CreationError>;
    fn create_buffer_immutable<T: Pod>(&mut self, data: &[T], role: buffer::Role, bind: Bind)
                                       -> Result<handle::Buffer<B, T>, buffer::CreationError> {
        self.create_buffer_immutable_raw(cast_slice(data), mem::size_of::<T>(), role, bind)
            .map(|raw| Typed::new(raw))
    }
    fn create_buffer<T>(&mut self, num: usize, role: buffer::Role, usage: Usage, bind: Bind)
                        -> Result<handle::Buffer<B, T>, buffer::CreationError> {
        let stride = mem::size_of::<T>();
        let info = buffer::Info {
            role: role,
            usage: usage,
            bind: bind,
            size: num * stride,
            stride: stride,
        };
        self.create_buffer_raw(info).map(|raw| Typed::new(raw))
    }

    ///
    fn create_renderpass(
        &mut self,
        attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc],
        dependencies: &[pass::SubpassDependency]
    ) -> handle::RenderPass<B>;

    /// Create a descriptor heap.
    fn create_descriptor_heap(&mut self, num_srv_cbv_uav: usize, num_samplers: usize) -> handle::DescriptorHeap<B>;

    /// Create a descriptor set pool inside an heap.
    ///
    /// Descriptor set pools allow allocation of descriptor sets by allocating space inside the heap.
    /// The heap can't be modified directly, only trough updating descriptor sets.
    ///
    /// Pools reserve a contiguous range in the heap. The application _must_ keep track of the used ranges.
    /// Using overlapping ranges at the same time results in undefined behavior, depending on the backend implementation.
    fn create_descriptor_set_pool(&mut self, heap: &B::DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[pso::DescriptorPoolDesc]) -> handle::DescriptorSetPool<B>;

    /// Create one or multiple descriptor sets from a pool.
    ///
    /// Each descriptor set will be allocated from the pool according to the corresponding set layout.
    ///
    /// The descriptor pool _must_ have enough space in to allocate the required descriptors.
    // TODO: Handle allocation/reset in pools
    // fn create_descriptor_sets(&mut self, set_pool: &mut B::DescriptorSetPool, layout: &[&B::DescriptorSetLayout]) -> Vec<handle::DescriptorSet<B>>;

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout(&mut self, bindings: &[pso::DescriptorSetLayoutBinding]) -> handle::DescriptorSetLayout<B>;

    ///
    fn create_pipeline_layout(&mut self, sets: &[&B::DescriptorSetLayout]) -> handle::PipelineLayout<B>;

    /// Create graphics pipelines.
    fn create_graphics_pipelines(&mut self, &[(&B::ShaderLib, &B::PipelineLayout, pass::SubPass<B>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<handle::GraphicsPipeline<B>, pso::CreationError>>;

    /// Create compute pipelines.
    fn create_compute_pipelines(&mut self, &[(&B::ShaderLib, pso::EntryPoint, &B::PipelineLayout)]) -> Vec<Result<handle::ComputePipeline<B>, pso::CreationError>>;

    ///
    fn create_sampler(&mut self, texture::SamplerInfo) -> handle::Sampler<B>;

    ///
    fn create_semaphore(&mut self) -> handle::Semaphore<B>;

    ///
    fn create_fence(&mut self, signalled: bool) -> handle::Fence<B>;

    ///
    fn reset_fences(&mut self, fences: &[&handle::Fence<B>]);

    /// Blocks until all or one of the given fences are signalled.
    /// Returns true if fences were signalled before the timeout.
    fn wait_for_fences(&mut self, fences: &[&handle::Fence<B>], wait: WaitFor, timeout_ms: u32) -> bool;

    /// Acquire a mapping Reader
    ///
    /// See `write_mapping` for more information.
    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<B, T>)
                               -> Result<mapping::Reader<'b, B, T>,
                                         mapping::Error>
        where T: Copy;

    /// Acquire a mapping Writer
    ///
    /// While holding this writer, you hold CPU-side exclusive access.
    /// Any access overlap will result in an error.
    /// Submitting commands involving this buffer to the device
    /// implicitly requires exclusive access. Additionally,
    /// further access will be stalled until execution completion.
    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b handle::Buffer<B, T>)
                                -> Result<mapping::Writer<'b, B, T>,
                                          mapping::Error>
        where T: Copy;

    /// Create a new empty raw texture with no data. The channel type parameter is a hint,
    /// required to assist backends that have no concept of typeless formats (OpenGL).
    /// The initial data, if given, has to be provided for all mip levels and slices:
    /// Slice0.Mip0, Slice0.Mip1, ..., Slice1.Mip0, ...
    fn create_texture_raw(&mut self, texture::Info, Option<format::ChannelType>, Option<&[&[u8]]>)
                          -> Result<handle::RawTexture<B>, texture::CreationError>;

    fn view_buffer_as_shader_resource_raw(&mut self, &handle::RawBuffer<B>, format::Format)
        -> Result<handle::RawShaderResourceView<B>, ResourceViewError>;
    fn view_buffer_as_unordered_access_raw(&mut self, &handle::RawBuffer<B>)
        -> Result<handle::RawUnorderedAccessView<B>, ResourceViewError>;
    fn view_texture_as_shader_resource_raw(&mut self, &handle::RawTexture<B>, texture::ResourceDesc)
        -> Result<handle::RawShaderResourceView<B>, ResourceViewError>;
    fn view_texture_as_unordered_access_raw(&mut self, &handle::RawTexture<B>)
        -> Result<handle::RawUnorderedAccessView<B>, ResourceViewError>;
    fn view_texture_as_render_target_raw(&mut self, &handle::RawTexture<B>, texture::RenderDesc)
        -> Result<handle::RawRenderTargetView<B>, TargetViewError>;
    fn view_texture_as_depth_stencil_raw(&mut self, &handle::RawTexture<B>, texture::DepthStencilDesc)
        -> Result<handle::RawDepthStencilView<B>, TargetViewError>;

    fn create_texture<S>(&mut self, kind: texture::Kind, levels: target::Level,
                      bind: Bind, usage: Usage, channel_hint: Option<format::ChannelType>)
                      -> Result<handle::Texture<B, S>, texture::CreationError>
    where S: format::SurfaceTyped
    {
        let desc = texture::Info {
            kind: kind,
            levels: levels,
            format: S::get_surface_type(),
            bind: bind,
            usage: usage,
        };
        let raw = try!(self.create_texture_raw(desc, channel_hint, None));
        Ok(Typed::new(raw))
    }

    fn view_buffer_as_shader_resource<T: format::Formatted>(&mut self, buf: &handle::Buffer<B, T>)
                                      -> Result<handle::ShaderResourceView<B, T>, ResourceViewError>
    {
        //TODO: check bind flags
        self.view_buffer_as_shader_resource_raw(buf.raw(), T::get_format()).map(Typed::new)
    }

    fn view_buffer_as_unordered_access<T>(&mut self, buf: &handle::Buffer<B, T>)
                                      -> Result<handle::UnorderedAccessView<B, T>, ResourceViewError>
    {
        //TODO: check bind flags
        self.view_buffer_as_unordered_access_raw(buf.raw()).map(Typed::new)
    }

    fn view_texture_as_shader_resource<T: format::TextureFormat>(&mut self, tex: &handle::Texture<B, T::Surface>,
                                       levels: (target::Level, target::Level), swizzle: format::Swizzle)
                                       -> Result<handle::ShaderResourceView<B, T::View>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(SHADER_RESOURCE) {
            return Err(ResourceViewError::NoBindFlag)
        }
        assert!(levels.0 <= levels.1);
        let desc = texture::ResourceDesc {
            channel: <T::Channel as format::ChannelTyped>::get_channel_type(),
            layer: None,
            min: levels.0,
            max: levels.1,
            swizzle: swizzle,
        };
        self.view_texture_as_shader_resource_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_unordered_access<T: format::TextureFormat>(&mut self, tex: &handle::Texture<B, T::Surface>)
                                        -> Result<handle::UnorderedAccessView<B, T::View>, ResourceViewError>
    {
        if !tex.get_info().bind.contains(UNORDERED_ACCESS) {
            return Err(ResourceViewError::NoBindFlag)
        }
        self.view_texture_as_unordered_access_raw(tex.raw())
            .map(Typed::new)
    }

    fn view_texture_as_render_target<T: format::RenderFormat>(&mut self, tex: &handle::Texture<B, T::Surface>,
                                     level: target::Level, layer: Option<target::Layer>)
                                     -> Result<handle::RenderTargetView<B, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(RENDER_TARGET) {
            return Err(TargetViewError::NoBindFlag)
        }
        let desc = texture::RenderDesc {
            channel: <T::Channel as format::ChannelTyped>::get_channel_type(),
            level: level,
            layer: layer,
        };
        self.view_texture_as_render_target_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_depth_stencil<T: format::DepthFormat>(&mut self, tex: &handle::Texture<B, T::Surface>,
                                     level: target::Level, layer: Option<target::Layer>, flags: texture::DepthStencilFlags)
                                     -> Result<handle::DepthStencilView<B, T>, TargetViewError>
    {
        if !tex.get_info().bind.contains(DEPTH_STENCIL) {
            return Err(TargetViewError::NoBindFlag)
        }
        let desc = texture::DepthStencilDesc {
            level: level,
            layer: layer,
            flags: flags,
        };
        self.view_texture_as_depth_stencil_raw(tex.raw(), desc)
            .map(Typed::new)
    }

    fn view_texture_as_depth_stencil_trivial<T: format::DepthFormat>(&mut self, tex: &handle::Texture<B, T::Surface>)
                                            -> Result<handle::DepthStencilView<B, T>, TargetViewError>
    {
        self.view_texture_as_depth_stencil(tex, 0, None, texture::DepthStencilFlags::empty())
    }

    fn create_texture_immutable_u8<T: format::TextureFormat>(&mut self, kind: texture::Kind, data: &[&[u8]])
                                   -> Result<(handle::Texture<B, T::Surface>,
                                              handle::ShaderResourceView<B, T::View>),
                                             CombinedError>
    {
        let surface = <T::Surface as format::SurfaceTyped>::get_surface_type();
        let num_slices = kind.get_num_slices().unwrap_or(1) as usize;
        let num_faces = if kind.is_cube() {6} else {1};
        let desc = texture::Info {
            kind: kind,
            levels: (data.len() / (num_slices * num_faces)) as texture::Level,
            format: surface,
            bind: SHADER_RESOURCE,
            usage: Usage::Data,
        };
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let raw = try!(self.create_texture_raw(desc, Some(cty), Some(data)));
        let levels = (0, raw.get_info().levels - 1);
        let tex = Typed::new(raw);
        let view = try!(self.view_texture_as_shader_resource::<T>(&tex, levels, format::Swizzle::new()));
        Ok((tex, view))
    }

    fn create_texture_immutable<T: format::TextureFormat>(
        &mut self,
        kind: texture::Kind,
        data: &[&[<T::Surface as format::SurfaceTyped>::DataType]])
        -> Result<(handle::Texture<B, T::Surface>, handle::ShaderResourceView<B, T::View>),
                  CombinedError>
    {
        // we can use cast_slice on a 2D slice, have to use a temporary array of slices
        let mut raw_data: [&[u8]; 0x100] = [&[]; 0x100];
        assert!(data.len() <= raw_data.len());
        for (rd, d) in raw_data.iter_mut().zip(data.iter()) {
            *rd = cast_slice(*d);
        }
        self.create_texture_immutable_u8::<T>(kind, &raw_data[.. data.len()])
    }

    fn create_render_target<T: format::RenderFormat + format::TextureFormat>
                           (&mut self, width: texture::Size, height: texture::Size)
                            -> Result<(handle::Texture<B, T::Surface>,
                                       handle::ShaderResourceView<B, T::View>,
                                       handle::RenderTargetView<B, T>
                                ), CombinedError>
    {
        let kind = texture::Kind::D2(width, height, texture::AaMode::Single);
        let levels = 1;
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, levels, SHADER_RESOURCE | RENDER_TARGET, Usage::Data, Some(cty)));
        let resource = try!(self.view_texture_as_shader_resource::<T>(&tex, (0, levels-1), format::Swizzle::new()));
        let target = try!(self.view_texture_as_render_target(&tex, 0, None));
        Ok((tex, resource, target))
    }

    fn create_depth_stencil<T: format::DepthFormat + format::TextureFormat>
                           (&mut self, width: texture::Size, height: texture::Size)
                            -> Result<(handle::Texture<B, T::Surface>,
                                       handle::ShaderResourceView<B, T::View>,
                                       handle::DepthStencilView<B, T>
                                ), CombinedError>
    {
        let kind = texture::Kind::D2(width, height, texture::AaMode::Single);
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, 1, SHADER_RESOURCE | DEPTH_STENCIL, Usage::Data, Some(cty)));
        let resource = try!(self.view_texture_as_shader_resource::<T>(&tex, (0, 0), format::Swizzle::new()));
        let target = try!(self.view_texture_as_depth_stencil_trivial(&tex));
        Ok((tex, resource, target))
    }

    fn create_depth_stencil_view_only<T: format::DepthFormat + format::TextureFormat>
                                     (&mut self, width: texture::Size, height: texture::Size)
                                      -> Result<handle::DepthStencilView<B, T>, CombinedError>
    {
        let kind = texture::Kind::D2(width, height, texture::AaMode::Single);
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let tex = try!(self.create_texture(kind, 1, DEPTH_STENCIL, Usage::Data, Some(cty)));
        let target = try!(self.view_texture_as_depth_stencil_trivial(&tex));
        Ok(target)
    }
}
