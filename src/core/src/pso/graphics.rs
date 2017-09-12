//! Graphics pipeline descriptor.

use {state as s, Backend, Primitive};
use super::EntryPoint;
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorInfo, DepthStencilDesc};

// Vulkan:
//  - SpecializationInfo not provided per shader
//
// D3D12:
//  - rootSignature specified outside
//  - logicOp can be set for each RTV
//  - streamOutput not included
//  - IA: semantic name and index extracted from shader reflection

/// A complete set of shaders to build a graphics pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsShaderSet<'a, B: Backend> {
    ///
    pub vertex: EntryPoint<'a, B>,
    ///
    pub hull: Option<EntryPoint<'a, B>>,
    ///
    pub domain: Option<EntryPoint<'a, B>>,
    ///
    pub geometry: Option<EntryPoint<'a, B>>,
    ///
    pub pixel: Option<EntryPoint<'a, B>>,
}

///
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct GraphicsPipelineDesc {
    /// Rasterizer setup
    pub rasterizer: Rasterizer,

    /// Vertex buffers (IA)
    pub vertex_buffers: Vec<VertexBufferDesc>,
    /// Vertex attributes (IA)
    pub attributes: Vec<AttributeDesc>,
    ///
    pub input_assembler: InputAssemblerDesc,

    ///
    pub blender: BlendDesc,
    /// Depth stencil (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
}

impl GraphicsPipelineDesc {
    /// Create a new empty PSO descriptor.
    pub fn new(primitive: Primitive, rasterizer: Rasterizer) -> Self {
        GraphicsPipelineDesc {
            rasterizer,
            vertex_buffers: Vec::new(),
            attributes: Vec::new(),
            input_assembler: InputAssemblerDesc::new(primitive),
            blender: BlendDesc::new(),
            depth_stencil: None,
        }
    }
}

///
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
    /// Discard primitives before the rasterizer.
    pub rasterizer_discard: bool,
}

impl Rasterizer {
    /// Create a new polygon-filling rasterizer state
    pub fn new_fill() -> Self {
        Rasterizer {
            polgyon_mode: s::RasterMethod::Fill,
            cull_mode: s::CullFace::Nothing,
            front_face: s::FrontFace::CounterClockwise,
            depth_clamping: true,
            depth_bias: None,
            conservative_rasterization: false,
            rasterizer_discard: false,
        }
    }
}

///
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct BlendDesc {
    ///
    pub alpha_coverage: bool,
    ///
    pub logic_op: Option<LogicOp>,
    ///
    pub targets: Vec<ColorInfo>,
}

impl BlendDesc {
    /// Create a new empty blend descriptor
    pub fn new() -> Self {
        BlendDesc {
            alpha_coverage: false,
            logic_op: None,
            targets: Vec::new(),
        }
    }
}

///
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
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
