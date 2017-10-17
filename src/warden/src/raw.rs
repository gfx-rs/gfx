use std::collections::HashMap;
use std::ops::Range;

use hal;


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
}

#[derive(Debug, Deserialize)]
pub struct SubpassDependency {
    pub passes: Range<String>,
    pub stages: Range<hal::pso::PipelineStage>,
    pub accesses: Range<hal::image::Access>,
}

#[derive(Debug, Deserialize)]
pub enum Resource {
    Shader,
    Buffer,
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
    DescriptorSet,
    PipelineLayout,
    GraphicsPipeline,
    Framebuffer {
        pass: String,
        views: HashMap<String, String>,
        extent: hal::device::Extent,
    },
}

#[derive(Debug, Deserialize)]
pub enum TransferCommand {
    CopyBufferToImage,
    //CopyImageToBuffer,
}

#[derive(Debug, Deserialize)]
pub struct DescriptorSetData {
    //TODO: update_descriptor_sets
}

#[derive(Debug, Deserialize)]
pub enum DrawCommand {
    BindVertexBuffers(Vec<(String, hal::pso::BufferOffset)>),
    BindPipeline(String),
    BindDescriptorSets {
        layout: String,
        first: usize,
        sets: Vec<String>,
    },
    Draw {
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>,
    },
    DrawIndexed {
        buffer: String,
        indices: Range<hal::IndexCount>,
        instances: Range<hal::InstanceCount>,
    },
}

#[derive(Debug, Deserialize)]
pub struct DrawPass {
    pub commands: Vec<DrawCommand>,
}

#[derive(Debug, Deserialize)]
pub enum Job {
    Transfer {
        commands: Vec<TransferCommand>,
    },
    Graphics {
        descriptors: HashMap<String, DescriptorSetData>,
        framebuffer: String,
        pass: (String, HashMap<String, DrawPass>),
    },
}

#[derive(Debug, Deserialize)]
pub struct Scene {
    pub resources: HashMap<String, Resource>,
    pub jobs: HashMap<String, Job>,
}
