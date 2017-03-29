// Copyright 2017 The Gfx-rs Developers.
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

use bitflags;
use std::error::Error;
use std::fmt;

use {format, state};
use {Primitive, Resources};
use MAX_COLOR_TARGETS;

/// An offset inside a vertex buffer, in bytes.
pub type BufferOffset = usize;

/// Error types happening upon PSO creation on the device side.
#[derive(Clone, PartialEq, Debug)]
pub struct CreationError;

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        "Could not create PSO on device."
    }
}

/// Color output configuration of the PSO.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ColorInfo {
    /// Color channel mask
    pub mask: state::ColorMask,
    /// Optional color blending
    pub color: Option<state::BlendChannel>,
    /// Optional alpha blending
    pub alpha: Option<state::BlendChannel>,
}
impl From<state::ColorMask> for ColorInfo {
    fn from(mask: state::ColorMask) -> ColorInfo {
        ColorInfo {
            mask: mask,
            color: None,
            alpha: None,
        }
    }
}
impl From<state::Blend> for ColorInfo {
    fn from(blend: state::Blend) -> ColorInfo {
        ColorInfo {
            mask: state::MASK_ALL,
            color: Some(blend.color),
            alpha: Some(blend.alpha),
        }
    }
}

/// Depth and stencil state of the PSO.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DepthStencilInfo {
    /// Optional depth test configuration
    pub depth: Option<state::Depth>,
    /// Optional stencil test on the front faces
    pub front: Option<state::StencilSide>,
    /// Optional stencil test on the back faces
    pub back: Option<state::StencilSide>,
}
impl From<state::Depth> for DepthStencilInfo {
    fn from(depth: state::Depth) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(depth),
            front: None,
            back: None,
        }
    }
}
impl From<state::Stencil> for DepthStencilInfo {
    fn from(stencil: state::Stencil) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: None,
            front: Some(stencil.front),
            back: Some(stencil.back),
        }
    }
}
impl From<(state::Depth, state::Stencil)> for DepthStencilInfo {
    fn from(ds: (state::Depth, state::Stencil)) -> DepthStencilInfo {
        DepthStencilInfo {
            depth: Some(ds.0),
            front: Some(ds.1.front),
            back: Some(ds.1.back),
        }
    }
}

/// Shader entry point.
pub type EntryPoint = &'static str;
/// PSO color target descriptor
pub type ColorTargetDesc = (format::Format, ColorInfo);
/// PSO depth-stencil target descriptor
pub type DepthStencilDesc = (format::Format, DepthStencilInfo);

/// A complete set of shaders to build a graphics pipeline.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GraphicsShaderSet {
    pub vertex_shader: EntryPoint,
    pub hull_shader: Option<EntryPoint>,
    pub domain_shader: Option<EntryPoint>,
    pub geometry_shader: Option<EntryPoint>,
    pub pixel_shader: Option<EntryPoint>,
}

/// Vertex buffer descriptor
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VertexBufferDesc {
    /// Total container size, in bytes
    pub stride: ElemStride,
    /// Rate of the input for the given buffer
    pub rate: InstanceRate,
}

/// Index of a vertex buffer.
pub type BufferIndex = u8;
/// Offset of an attribute from the start of the buffer, in bytes
pub type ElemOffset = u32;
/// Offset between attribute values, in bytes
pub type ElemStride = u8;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;

/// A struct element descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Element<F> {
    /// Element format
    pub format: F,
    /// Offset from the beginning of the container, in bytes
    pub offset: ElemOffset,
}

/// PSO vertex attribute descriptor
pub type AttributeDesc = (BufferIndex, Element<format::Format>);

pub struct GraphicsPipelineDesc {
    /// Type of the primitive
    pub primitive: Primitive,
    /// Rasterizer setup
    pub rasterizer: state::Rasterizer,
    /// Depth stencil
    pub depth_stencil: Option<DepthStencilDesc>,
    /// Shader entry points
    pub shader_entries: GraphicsShaderSet,
    /// Render target views (RTV)
    /// The entries are supposed to be contiguous, starting from 0
    pub color_targets: [Option<ColorTargetDesc>; MAX_COLOR_TARGETS],
    /// Vertex buffers
    pub vertex_buffers: Vec<VertexBufferDesc>,
    /// Vertex attributes
    pub attributes: Vec<AttributeDesc>,
}

impl GraphicsPipelineDesc {
    /// Create a new empty PSO descriptor.
    pub fn new(primitive: Primitive, rasterizer: state::Rasterizer, shader_entries: GraphicsShaderSet) -> GraphicsPipelineDesc {
        GraphicsPipelineDesc {
            primitive: primitive,
            rasterizer: rasterizer,
            depth_stencil: None,
            shader_entries: shader_entries,
            color_targets: [None; MAX_COLOR_TARGETS],
            vertex_buffers: Vec::new(),
            attributes: Vec::new(),
        }
    }
}

/// A complete set of vertex buffers to be used for vertex import in PSO.
#[derive(Clone, Debug)]
pub struct VertexBufferSet<'a, R: Resources>(
    /// Array of buffer handles with offsets in them
    pub Vec<(&'a R::Buffer, BufferOffset)>,
);

impl<'a, R: Resources> VertexBufferSet<'a, R> {
    /// Create an empty set
    pub fn new() -> VertexBufferSet<'a, R> {
        VertexBufferSet(Vec::new())
    }
}

bitflags!(
    /// Stages of the logical pipeline.
    ///
    /// The pipeline is structured as given the by the ordering of the flags.
    /// Some stages are queue type dependent.
    pub flags PipelineStage: u32 {
        /// Beginning of the command queue.
        const TOP_OF_PIPE = 0x1,
        /// Indirect data consumption.
        const DRAW_INDIRECT = 0x2,
        /// Vertex data consumption.
        const VERTEX_INPUT = 0x4,
        /// Vertex shader execution.
        const VERTEX_SHADER = 0x8,
        /// Hull shader execution.
        const HULL_SHADER = 0x10,
        /// Domain shader execution.
        const DOMAIN_SHADER = 0x20,
        /// Geometry shader execution.
        const GEOMETRY_SHADER = 0x40,
        /// Pixel shader execution.
        const PIXEL_SHADER = 0x80,
        const EARLY_FRAGMENT_TESTS = 0x100,
        const LATE_FRAGMENT_TESTS = 0x200,
        const COLOR_ATTACHMENT_OUTPUT = 0x400,
        const COMPUTE_SHADER = 0x800,
        const TRANSFER = 0x1000,
        const BOTTOM_OF_PIPE = 0x2000,
        const HOST = 0x4000,
    }
);
