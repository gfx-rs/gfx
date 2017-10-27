//! Graphics pipeline descriptor.

use {Backend, Primitive};
use super::EntryPoint;
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorBlendDesc, DepthStencilDesc};

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
    pub fragment: Option<EntryPoint<'a, B>>,
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
            blender: BlendDesc::default(),
            depth_stencil: None,
        }
    }
}

/// Way to rasterize polygons.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub enum PolygonMode {
    /// Rasterize as a point.
    Point,
    /// Rasterize as a line with the given width.
    Line(f32),
    /// Rasterize as a face.
    Fill,
}

/// Which face, if any, to cull.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub enum CullFace {
    ///
    Front,
    ///
    Back,
}

/// The front face winding order of a set of vertices.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub enum FrontFace {
    /// Clockwise winding order.
    Clockwise,
    /// Counter-clockwise winding order.
    CounterClockwise,
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
    pub polygon_mode: PolygonMode,
    /// Which face should be culled.
    pub cull_face: Option<CullFace>,
    /// Which vertex winding is considered to be the front face for culling.
    pub front_face: FrontFace,
    ///
    pub depth_clamping: bool,
    ///
    pub depth_bias: Option<DepthBias>,
    ///
    pub conservative: bool,
}

impl Rasterizer {
    /// Simple polygon-filling rasterizer state
    pub const FILL: Self = Rasterizer {
        polygon_mode: PolygonMode::Fill,
        cull_face: None,
        front_face: FrontFace::CounterClockwise,
        depth_clamping: true,
        depth_bias: None,
        conservative: false,
    };
}

///
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct BlendDesc {
    ///
    pub alpha_coverage: bool,
    ///
    pub logic_op: Option<LogicOp>,
    ///
    pub targets: Vec<ColorBlendDesc>,
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
