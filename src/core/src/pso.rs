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

//! Pipeline State Objects

use {MAX_COLOR_TARGETS, MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS,
     MAX_RESOURCE_VIEWS, MAX_UNORDERED_VIEWS, MAX_SAMPLERS};
use {Primitive, Resources};
use {attrib, tex};
use state as s;

/// An offset inside a vertex buffer, in bytes.
pub type BufferOffset = usize;

/// Error types happening upon PSO creation on the device side.
#[derive(Clone, PartialEq, Debug)]
pub struct CreationError;

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BlendInfo {
    pub mask: s::ColorMask,
    pub color: Option<s::BlendChannel>,
    pub alpha: Option<s::BlendChannel>,
}
impl From<s::ColorMask> for BlendInfo {
    fn from(mask: s::ColorMask) -> BlendInfo {
        BlendInfo {
            mask: mask,
            color: None,
            alpha: None,
        }
    }
}
impl From<s::Blend> for BlendInfo {
    fn from(blend: s::Blend) -> BlendInfo {
        BlendInfo {
            mask: blend.mask,
            color: Some(blend.color),
            alpha: Some(blend.alpha),
        }
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DepthStencilInfo {
    pub depth: Option<s::Depth>,
    pub front: Option<s::StencilSide>,
    pub back: Option<s::StencilSide>,
}
impl From<s::Depth> for DepthStencilInfo {
    fn from(depth: s::Depth) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(depth),
            front: None,
            back: None,
        }
    }
}
impl From<s::Stencil> for DepthStencilInfo {
    fn from(stencil: s::Stencil) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: None,
            front: Some(stencil.front),
            back: Some(stencil.back),
        }
    }
}
impl From<(s::Depth, s::Stencil)> for DepthStencilInfo {
    fn from(ds: (s::Depth, s::Stencil)) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(ds.0),
            front: Some(ds.1.front),
            back: Some(ds.1.back),
        }
    }
}

/// PSO vertex attribute descriptor
pub type AttributeDesc = (attrib::Format, attrib::InstanceRate);
/// PSO color target descriptor
pub type ColorTargetDesc = (tex::Format, BlendInfo);
/// PSO depth-stencil target descriptor
pub type DepthStencilDesc = (tex::Format, DepthStencilInfo);

/// All the information surrounding a shader program that is required
/// for PSO creation, including the formats of vertex buffers and pixel targets;
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Descriptor {
    /// Type of the primitive
    pub primitive: Primitive,
    /// Rasterizer setup
    pub rasterizer: s::Rasterizer,
    /// Vertex attributes
    pub attributes: [Option<AttributeDesc>; MAX_VERTEX_ATTRIBUTES],
    /// Render target views (RTV)
    pub color_targets: [Option<ColorTargetDesc>; MAX_COLOR_TARGETS],
    /// Depth stencil view (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
}

impl Descriptor {
    /// Create a new empty PSO descriptor.
    pub fn new(primitive: Primitive, rast: s::Rasterizer) -> Descriptor {
        Descriptor {
            primitive: primitive,
            rasterizer: rast,
            attributes: [None; MAX_VERTEX_ATTRIBUTES],
            color_targets: [None; MAX_COLOR_TARGETS],
            depth_stencil: None,
        }
    }
}

/// A complete set of vertex buffers to be used for vertex import in PSO.
#[derive(Copy, Clone, Debug)]
pub struct VertexBufferSet<R: Resources>(
    /// Array of buffer handles with offsets in them
    pub [Option<(R::Buffer, BufferOffset)>; MAX_VERTEX_ATTRIBUTES]
);

impl<R: Resources> VertexBufferSet<R> {
    /// Create an empty set
    pub fn new() -> VertexBufferSet<R> {
        VertexBufferSet([None; MAX_VERTEX_ATTRIBUTES])
    }
}

/// A complete set of constant buffers to be used for the constants binding in PSO.
#[derive(Copy, Clone, Debug)]
pub struct ConstantBufferSet<R: Resources>(
    /// Array of buffer handles
    pub [Option<R::Buffer>; MAX_CONSTANT_BUFFERS]
);

impl<R: Resources> ConstantBufferSet<R> {
    /// Create an empty set
    pub fn new() -> ConstantBufferSet<R> {
        ConstantBufferSet([None; MAX_CONSTANT_BUFFERS])
    }
}

/// A complete set of shader resource views to be used in PSO.
#[derive(Copy, Clone, Debug)]
pub struct ResourceViewSet<R: Resources>(
    /// Array of SRVs
    pub [Option<R::ShaderResourceView>; MAX_RESOURCE_VIEWS],
);

impl<R: Resources> ResourceViewSet<R> {
    /// Create an empty set
    pub fn new() -> ResourceViewSet<R> {
        ResourceViewSet([None; MAX_RESOURCE_VIEWS])
    }
}

/// A complete set of unordered access views to be used in PSO.
#[derive(Copy, Clone, Debug)]
pub struct UnorderedViewSet<R: Resources>(
    /// Array of UAVs
    pub [Option<R::UnorderedAccessView>; MAX_UNORDERED_VIEWS],
);

impl<R: Resources> UnorderedViewSet<R> {
    /// Create an empty set
    pub fn new() -> UnorderedViewSet<R> {
        UnorderedViewSet([None; MAX_UNORDERED_VIEWS])
    }
}

/// A complete set of samplers to be used for PSO.
#[derive(Copy, Clone, Debug)]
pub struct SamplerSet<R: Resources>(
    /// Array of samplers
    pub [Option<R::Sampler>; MAX_SAMPLERS]
);

impl<R: Resources> SamplerSet<R> {
    /// Create an empty set
    pub fn new() -> SamplerSet<R> {
        SamplerSet([None; MAX_SAMPLERS])
    }
}

/// A complete set of render targets to be used for pixel export in PSO.
#[derive(Copy, Clone, Debug)]
pub struct PixelTargetSet<R: Resources> {
    /// Array of color target views
    pub colors: [Option<R::RenderTargetView>; MAX_COLOR_TARGETS],
    /// Depth-stencil target view
    pub depth_stencil: Option<R::DepthStencilView>,
}

impl<R: Resources> PixelTargetSet<R> {
    /// Create an empty set
    pub fn new() -> PixelTargetSet<R> {
        PixelTargetSet {
            colors: [None; MAX_COLOR_TARGETS],
            depth_stencil: None,
        }
    }
}
