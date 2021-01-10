use std::collections::HashMap;
use std::ops::Range;

use hal;

#[derive(Debug, Deserialize)]
pub enum ClearColor {
    Float([f32; 4]),
    Uint([u32; 4]),
    Sint([i32; 4]),
}

impl ClearColor {
    pub fn to_raw(&self) -> hal::command::ClearColor {
        match *self {
            ClearColor::Float(array) => hal::command::ClearColor { float32: array },
            ClearColor::Uint(array) => hal::command::ClearColor { uint32: array },
            ClearColor::Sint(array) => hal::command::ClearColor { sint32: array },
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum ClearValue {
    Color(ClearColor),
    DepthStencil(hal::command::ClearDepthStencil),
}

impl ClearValue {
    pub fn to_raw(&self) -> hal::command::ClearValue {
        match *self {
            ClearValue::Color(ref color) => hal::command::ClearValue {
                color: color.to_raw(),
            },
            ClearValue::DepthStencil(ds) => hal::command::ClearValue { depth_stencil: ds },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AttachmentRef(pub String, pub hal::pass::AttachmentLayout);

#[derive(Debug, Deserialize)]
pub struct Subpass {
    pub colors: Vec<AttachmentRef>,
    pub depth_stencil: Option<AttachmentRef>,
    #[serde(default)]
    pub inputs: Vec<AttachmentRef>,
    #[serde(default)]
    pub preserves: Vec<String>,
    #[serde(default)]
    pub resolves: Vec<AttachmentRef>,
}

#[derive(Debug, Deserialize)]
pub struct SubpassDependency {
    pub passes: Range<String>,
    pub stages: Range<hal::pso::PipelineStage>,
    pub accesses: Range<hal::image::Access>,
}

#[derive(Debug, Deserialize)]
pub struct GraphicsShaderSet {
    pub vertex: String,
    #[serde(default)]
    pub hull: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub geometry: String,
    #[serde(default)]
    pub fragment: String,
}

#[derive(Debug, Deserialize)]
pub struct SubpassRef {
    pub parent: String,
    pub index: hal::pass::SubpassId,
}

#[derive(Debug, Deserialize)]
pub enum Resource {
    Buffer {
        size: usize,
        usage: hal::buffer::Usage,
        #[serde(default)]
        data: String,
    },
    Image {
        kind: hal::image::Kind,
        num_levels: hal::image::Level,
        format: hal::format::Format,
        usage: hal::image::Usage,
        #[serde(default)]
        view_caps: hal::image::ViewCapabilities,
        #[serde(default)]
        data: String,
    },
    ImageView {
        image: String,
        kind: hal::image::ViewKind,
        format: hal::format::Format,
        #[serde(default)]
        swizzle: hal::format::Swizzle,
        range: hal::image::SubresourceRange,
    },
    Sampler {
        info: hal::image::SamplerDesc,
    },
    RenderPass {
        attachments: HashMap<String, hal::pass::Attachment>,
        subpasses: HashMap<String, Subpass>,
        dependencies: Vec<SubpassDependency>,
    },
    Shader(String),
    DescriptorSetLayout {
        bindings: Vec<hal::pso::DescriptorSetLayoutBinding>,
        #[serde(default)]
        immutable_samplers: Vec<String>,
    },
    DescriptorPool {
        capacity: usize,
        ranges: Vec<hal::pso::DescriptorRangeDesc>,
    },
    DescriptorSet {
        pool: String,
        layout: String,
        data: Vec<DescriptorRange>,
    },
    PipelineLayout {
        set_layouts: Vec<String>,
        push_constant_ranges: Vec<(hal::pso::ShaderStageFlags, Range<u32>)>,
    },
    GraphicsPipeline {
        shaders: GraphicsShaderSet,
        rasterizer: hal::pso::Rasterizer,
        #[serde(default)]
        vertex_buffers: Vec<hal::pso::VertexBufferDesc>,
        #[serde(default)]
        attributes: Vec<hal::pso::AttributeDesc>,
        input_assembler: hal::pso::InputAssemblerDesc,
        blender: hal::pso::BlendDesc,
        #[serde(default)]
        depth_stencil: hal::pso::DepthStencilDesc,
        layout: String,
        subpass: SubpassRef,
    },
    ComputePipeline {
        shader: String,
        layout: String,
    },
    Framebuffer {
        pass: String,
        attachments: HashMap<String, hal::image::FramebufferAttachment>,
        extent: hal::image::Extent,
    },
}

#[derive(Debug, Deserialize)]
pub enum TransferCommand {
    CopyBuffer {
        src: String,
        dst: String,
        regions: Vec<hal::command::BufferCopy>,
    },
    CopyImage {
        src: String,
        dst: String,
        regions: Vec<hal::command::ImageCopy>,
    },
    CopyBufferToImage {
        src: String,
        dst: String,
        regions: Vec<hal::command::BufferImageCopy>,
    },
    CopyImageToBuffer {
        src: String,
        dst: String,
        regions: Vec<hal::command::BufferImageCopy>,
    },
    ClearImage {
        image: String,
        value: ClearValue,
        ranges: Vec<hal::image::SubresourceRange>,
    },
    BlitImage {
        src: String,
        dst: String,
        filter: hal::image::Filter,
        regions: Vec<hal::command::ImageBlit>,
    },
    FillBuffer {
        buffer: String,
        offset: hal::buffer::Offset,
        size: Option<hal::buffer::Offset>,
        data: u32,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub enum DescriptorRange {
    Buffers(Vec<String>),
    Images(Vec<(String, hal::image::Layout)>),
    Samplers(Vec<String>),
}

fn default_instance_range() -> Range<hal::InstanceCount> {
    0..1
}

#[derive(Debug, Deserialize)]
pub enum DrawCommand {
    BindIndexBuffer {
        buffer: String,
        range: hal::buffer::SubRange,
        index_type: hal::IndexType,
    },
    BindVertexBuffers(Vec<(String, hal::buffer::SubRange)>),
    BindPipeline(String),
    BindDescriptorSets {
        layout: String,
        first: usize,
        sets: Vec<String>,
    },
    Draw {
        vertices: Range<hal::VertexCount>,
        #[serde(default = "default_instance_range")]
        instances: Range<hal::InstanceCount>,
    },
    DrawIndexed {
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    },
    SetViewports(Vec<hal::pso::Viewport>),
    SetScissors(Vec<hal::pso::Rect>),
}

#[derive(Debug, Deserialize)]
pub struct DrawPass {
    pub commands: Vec<DrawCommand>,
}

#[derive(Debug, Deserialize)]
pub struct RenderAttachmentInfo {
    pub image_view: String,
    pub clear_value: ClearValue,
}

#[derive(Debug, Deserialize)]
pub enum Job {
    Transfer {
        commands: Vec<TransferCommand>,
    },
    Graphics {
        framebuffer: String,
        attachments: HashMap<String, RenderAttachmentInfo>,
        pass: (String, HashMap<String, DrawPass>),
    },
    Compute {
        pipeline: String,
        descriptor_sets: Vec<String>,
        dispatch: hal::WorkGroupCount,
    },
}

#[derive(Debug, Deserialize)]
pub struct Scene {
    pub resources: HashMap<String, Resource>,
    pub jobs: HashMap<String, Job>,
}
