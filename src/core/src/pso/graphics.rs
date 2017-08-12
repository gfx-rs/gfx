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

//! Graphics pipeline descriptor.

use {state as s};
use Primitive;
use super::EntryPoint;
use super::input_assembler::{AttributeDesc, VertexBufferDesc};
use super::output_merger::{ColorTargetDesc, DepthStencilDesc};

/// A complete set of shaders to build a graphics pipeline.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GraphicsShaderSet {
    ///
    pub vertex_shader: EntryPoint,
    ///
    pub hull_shader: Option<EntryPoint>,
    ///
    pub domain_shader: Option<EntryPoint>,
    ///
    pub geometry_shader: Option<EntryPoint>,
    ///
    pub pixel_shader: Option<EntryPoint>,
}

///
pub struct GraphicsPipelineDesc {
    /// Type of the primitive
    pub primitive: Primitive,
    /// Rasterizer setup
    pub rasterizer: s::Rasterizer,
    /// Shader entry points
    pub shader_entries: GraphicsShaderSet,

    /// Vertex buffers (IA)
    pub vertex_buffers: Vec<VertexBufferDesc>,
    /// Vertex attributes (IA)
    pub attributes: Vec<AttributeDesc>,

    /// Render target views (RTV)
    pub color_targets: Vec<ColorTargetDesc>,
    /// Depth stencil (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
}

impl GraphicsPipelineDesc {
    /// Create a new empty PSO descriptor.
    pub fn new(primitive: Primitive, rasterizer: s::Rasterizer, shader_entries: GraphicsShaderSet) -> GraphicsPipelineDesc {
        GraphicsPipelineDesc {
            primitive: primitive,
            rasterizer: rasterizer,
            depth_stencil: None,
            shader_entries: shader_entries,
            color_targets: Vec::new(),
            vertex_buffers: Vec::new(),
            attributes: Vec::new(),
        }
    }
}
