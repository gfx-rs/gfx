//! Graphics pipeline descriptor.

use {pass, Backend, Primitive};
use super::{BasePipeline, EntryPoint, PipelineCreationFlags};
use super::input_assembler::{AttributeDesc, InputAssemblerDesc, VertexBufferDesc};
use super::output_merger::{ColorBlendDesc, DepthStencilDesc};

/// A complete set of shaders to build a graphics pipeline.
///
/// All except the vertex shader are optional; omitting them
/// passes through the inputs without change.
/// 
/// If a fragment shader is omitted, the results of fragment 
/// processing are undefined. Specifically, any fragment color 
/// outputs are considered to have undefined values, and the 
/// fragment depth is considered to be unmodified. This can 
/// be useful for depth-only rendering.
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
    /// A shader that takes given input vertexes and outputs zero
    /// or more output vertexes.
    pub geometry: Option<EntryPoint<'a, B>>,
    /// A shader that outputs a value for a fragment.
    /// Usually this value is a color that is then displayed as a
    /// pixel on a screen.
    pub fragment: Option<EntryPoint<'a, B>>,
}

/// A description of all the settings that can be altered
/// when creating a graphics pipeline.
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
    /// Input assembler attributes, describes how
    /// vertices are assembled into primitives (such as triangles).
    pub input_assembler: InputAssemblerDesc,
    /// Description of how blend operations should be performed.
    pub blender: BlendDesc,
    /// Depth stencil (DSV)
    pub depth_stencil: Option<DepthStencilDesc>,
    /// Pipeline layout.
    pub layout: &'a B::PipelineLayout,
    /// Subpass in which the pipeline can be executed.
    pub subpass: pass::Subpass<'a, B>,
    /// Options that may be set to alter pipeline properties.
    pub flags: PipelineCreationFlags,
    /// The parent pipeline, which may be
    /// `BasePipeline::None`.
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

/// Which faces of the polygon, if any, to cull. Face culling
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

/// The front face winding order of a set of vertices. This is
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

/// A depth bias allows changing the produced depth values 
/// for fragments slightly but consistently. This permits 
/// drawing of multiple polygons in the same plane without 
/// Z-fighting, such as when trying to draw shadows on a wall.
///
/// For details of the algorithm and equations, see
/// [the Vulkan spec](https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#primsrast-depthbias).
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DepthBias {
    /// A constant depth value added to each fragment.
    pub const_factor: f32,
    /// The minimum or maximum depth bias of a fragment.
    pub clamp: f32,
    /// A constant bias applied to the fragment's slope.
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
    /// Whether or not to enable depth clamping; when enabled, instead of
    /// fragments being omitted when they are outside the bounds of the z-plane,
    /// they will be clamped to the min or max z value.
    pub depth_clamping: bool,
    /// What depth bias, if any, to use for the drawn primitives.
    pub depth_bias: Option<DepthBias>,
    /// Controls how triangles will be rasterized depending on their overlap with pixels.
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

/// A description of an equation for how to blend transparent, overlapping fragments.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BlendDesc {
    /// Toggles alpha-to-coverage multisampling, which can produce nicer edges
    /// when many partially-transparent polygons are overlapping.
    /// See [here]( https://msdn.microsoft.com/en-us/library/windows/desktop/bb205072(v=vs.85).aspx#Alpha_To_Coverage) for a full description.
    pub alpha_coverage: bool,
    /// The logic operation to apply to the blending equation, if any.
    pub logic_op: Option<LogicOp>,
    /// Which color targets to apply the blending operation to.
    pub targets: Vec<ColorBlendDesc>,
}

/// Logic operations used for specifying blend equations.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(missing_docs)]
pub enum LogicOp {
    Clear,
    And,
    AndReverse,
    AndInverted,
    Copy,
    CopyInverted,
    NoOp,
    Xor,
    Nor,
    Or,
    OrReverse,
    OrInverted,
    Equivalent,
    Invert,
    Nand,
    Set,
}
