//! Graphics pipeline descriptor.

use {pass, Backend, Primitive};
use super::{BasePipeline, EntryPoint, PipelineCreationFlags};
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorBlendDesc, DepthStencilDesc};

/// A complete set of shaders to build a graphics pipeline.
#[derive(Clone, Debug)]
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
#[derive(Debug)]
pub struct GraphicsPipelineDesc<'a, B: Backend> {
    ///
    pub shaders: GraphicsShaderSet<'a, B>,
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
    /// Pipeline layout.
    pub layout: &'a B::PipelineLayout,
    /// Subpass in which the pipeline can be executed.
    pub subpass: pass::Subpass<'a, B>,
    ///
    pub flags: PipelineCreationFlags,
    ///
    pub parent: BasePipeline<'a, B::GraphicsPipeline>,
}

impl<'a, B: Backend> GraphicsPipelineDesc<'a, B> {
    /// Create a new empty PSO descriptor.
    pub fn new(
        shaders: GraphicsShaderSet<'a, B>,
        primitive: Primitive,
        rasterizer: Rasterizer,
        layout: &'a B::PipelineLayout,
        subpass: pass::Subpass<'a, B>,
    ) -> Self {
        GraphicsPipelineDesc {
            shaders,
            rasterizer,
            vertex_buffers: Vec::new(),
            attributes: Vec::new(),
            input_assembler: InputAssemblerDesc::new(primitive),
            blender: BlendDesc::default(),
            depth_stencil: None,
            layout,
            subpass,
            flags: PipelineCreationFlags::empty(),
            parent: BasePipeline::None,
        }
    }
}

/// Way to rasterize polygons.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CullFace {
    ///
    Front,
    ///
    Back,
}

/// The front face winding order of a set of vertices.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FrontFace {
    /// Clockwise winding order.
    Clockwise,
    /// Counter-clockwise winding order.
    CounterClockwise,
}

///
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    //TODO: multisampling
}

impl Rasterizer {
    /// Simple polygon-filling rasterizer state
    pub const FILL: Self = Rasterizer {
        polygon_mode: PolygonMode::Fill,
        cull_face: None,
        front_face: FrontFace::CounterClockwise,
        depth_clamping: false,
        depth_bias: None,
        conservative: false,
    };
}

///
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
