//! Graphics pipeline descriptor.

use {pass, Backend, Primitive};
use super::{BasePipeline, EntryPoint, PipelineCreationFlags};
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorBlendDesc, DepthStencilDesc};

/// A complete set of shaders to build a graphics pipeline.
///
/// All except the vertex shader are optional.
/// DOC TODO: What happens if, say, a fragment shader is not defined?
/// does it use a default?
#[derive(Clone, Debug)]
pub struct GraphicsShaderSet<'a, B: Backend> {
    /// A shader that outputs a vertex in a model.
    pub vertex: EntryPoint<'a, B>,
    /// A hull shader takes in an input patch (values representing
    /// a small portion of a shape, which may be actual geometry or may
    /// be parameters for creating geometry) and produces one or more
    /// output patches.
    pub hull: Option<EntryPoint<'a, B>>,
    /// A shader that takes in domains produced from a hull shader's output
    /// patches and computes actual vertex positions.
    pub domain: Option<EntryPoint<'a, B>>,
    /// A shader that takes given input vertexes and outputs one
    /// or more output vertexes.
    pub geometry: Option<EntryPoint<'a, B>>,
    /// A shader that outputs a value for a texel.
    /// Usually this value is a color that is then displayed as a
    /// pixel on a screen.
    pub fragment: Option<EntryPoint<'a, B>>,
}

/// DOC TODO
#[derive(Debug)]
pub struct GraphicsPipelineDesc<'a, B: Backend> {
    /// A set of graphics shaders to use for the pipeline.
    pub shaders: GraphicsShaderSet<'a, B>,
    /// Rasterizer setup
    pub rasterizer: Rasterizer,
    /// Vertex buffers (IA)
    pub vertex_buffers: Vec<VertexBufferDesc>,
    /// Vertex attributes (IA)
    pub attributes: Vec<AttributeDesc>,
    ///DOC TODO
    pub input_assembler: InputAssemblerDesc,
    /// DOC TODO
    pub blender: BlendDesc,
    /// Depth stencil (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
    /// Pipeline layout.
    pub layout: &'a B::PipelineLayout,
    /// Subpass in which the pipeline can be executed.
    pub subpass: pass::Subpass<'a, B>,
    /// DOC TODO
    pub flags: PipelineCreationFlags,
    /// DOC TODO
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

/// Methods for rasterizing polygons, ie, turning the mesh
/// into a raster image.
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

/// Which faces of the polygon, if any, to cull.  Face culling
/// is often used to reduce the amount of geometry that has to get
/// drawn, so the renderer, for instance, doesn't have to worry about
/// drawing the insides of closed objects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CullFace {
    /// Cull front faces
    Front,
    /// Cull back faces.
    Back,
}

/// The front face winding order of a set of vertices.  This is
/// the order of vertexes that define which side of a face is
/// the "front".
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FrontFace {
    /// Clockwise winding order.
    Clockwise,
    /// Counter-clockwise winding order.
    CounterClockwise,
}

/// A depth bias allows you to specify a hint for how to draw
/// polygons that are in the same plane, such as shadows on a wall.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DepthBias {
    /// DOC TODO
    pub const_factor: f32,
    /// DOC TODO
    pub clamp: f32,
    /// DOC TODO
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
    /// DOC TODO
    pub depth_clamping: bool,
    /// What depth bias, if any, to use for the drawn primitives.
    pub depth_bias: Option<DepthBias>,
    /// DOC TODO
    pub conservative: bool,
    //TODO: multisampling
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

/// A description of an equation for how to blend transparent, overlapping texels.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BlendDesc {
    /// DOC TODO
    pub alpha_coverage: bool,
    /// DOC TODO
    pub logic_op: Option<LogicOp>,
    /// DOC TODO
    pub targets: Vec<ColorBlendDesc>,
}

/// Logic operations used for specifying blend equations.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LogicOp {
    /// DOC TODO alllll of these, with examples.
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
