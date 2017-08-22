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
use super::EntryPoint;
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorInfo, DepthStencilDesc};

// Vulkan:
//  - SpecializationInfo not provided per shader
//  - TODO: infer rasterization discard from shaders?
//
// D3D12:
//  - rootSignature specified outside
//  - logicOp can be set for each RTV
//  - streamOutput not included
//  - IA: semantic name and index extracted from shader reflection

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
    /// Rasterizer setup
    pub rasterizer: Rasterizer,
    /// Shader entry points
    pub shader_entries: GraphicsShaderSet,

    /// Vertex buffers (IA)
    pub vertex_buffers: Vec<VertexBufferDesc>,
    /// Vertex attributes (IA)
    pub attributes: Vec<AttributeDesc>,
    ///
    pub input_assembler: InputAssemblerDesc,

    ///
    pub blending: Vec<BlendDesc>,
    /// Depth stencil (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
}

///
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct DepthBias {
    ///
    pub const_factor: f32,
    ///
    pub clamp: f32,
    ///
    pub slope_factor: f32,
}

/// Rasterization state.
#[derive(Clone, Debug)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct Rasterizer {
    /// How to rasterize this primitive.
    pub polgyon_mode: s::RasterMethod,
    /// Which face should be culled.
    pub cull_mode: s::CullFace,
    /// Which vertex winding is considered to be the front face for culling.
    pub front_face: s::FrontFace,
    ///
    pub depth_clamping: bool,
    ///
    pub depth_bias: Option<DepthBias>,
    ///
    pub conservative_rasterization: bool,
}

///
pub enum BlendTargets {
    ///
    Single(ColorInfo),
    ///
    Independent(Vec<ColorInfo>),
}

///
pub struct BlendDesc {
    ///
    pub alpha_coverage: bool,
    ///
    pub logic_op: Option<LogicOp>,
    ///
    pub blend_targets: BlendTargets,
}

///
pub enum LogicOp {
    ///
    Clear,
    ///
    And,
    ///
    AndReverse,
    ///
    AndInverted,
    ///
    Copy,
    ///
    CopyInverted,
    ///
    NoOp,
    ///
    Xor,
    ///
    Nor,
    ///
    Or,
    ///
    OrReverse,
    ///
    OrInverted,
    ///
    Equivalent,
    ///
    Invert,
    ///
    Nand,
    ///
    Set,
}
