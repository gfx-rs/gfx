use std::collections::HashMap;
use std::ops::Range;

use crate::hal;

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
    pub src_subpass: String,
    pub dst_subpass: String,
    pub src_stage: hal::pso::PipelineStage,
    pub dst_stage: hal::pso::PipelineStage,
    pub src_access: hal::image::Access,
    pub dst_access: hal::image::Access,
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
    pub index: usize,
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
        views: HashMap<String, String>,
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
        color: hal::command::ClearColor,
        depth_stencil: hal::command::ClearDepthStencil,
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
        start: Option<hal::buffer::Offset>,
        end: Option<hal::buffer::Offset>,
        data: u32,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub enum DescriptorRange {
    Buffers(Vec<String>),
    Images(Vec<String>),
}

fn default_instance_range() -> Range<hal::InstanceCount> {
    0 .. 1
}

#[derive(Debug, Deserialize)]
pub enum DrawCommand {
    BindIndexBuffer {
        buffer: String,
        offset: hal::buffer::Offset,
        index_type: hal::IndexType,
    },
    BindVertexBuffers(Vec<(String, hal::buffer::Offset)>),
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
pub enum Job {
    Transfer(TransferCommand),
    Graphics {
        framebuffer: String,
        clear_values: Vec<hal::command::ClearValue>,
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
