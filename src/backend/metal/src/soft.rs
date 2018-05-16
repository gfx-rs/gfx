use command::IndexBuffer;

use hal;
use metal;

use std::ops::Range;

#[derive(Clone, Debug)]
pub enum RenderCommand {
    SetViewport(metal::MTLViewport),
    SetScissor(metal::MTLScissorRect),
    SetBlendColor(hal::pso::ColorValue),
    BindBuffer {
        stage: hal::pso::Stage,
        index: usize,
        buffer: Option<metal::Buffer>,
        offset: hal::buffer::Offset,
    },
    BindBufferData {
        stage: hal::pso::Stage,
        bytes: Vec<u8>,
        index: usize,
    },
    BindTexture {
        stage: hal::pso::Stage,
        index: usize,
        texture: Option<metal::Texture>,
    },
    BindSampler {
        stage: hal::pso::Stage,
        index: usize,
        sampler: Option<metal::SamplerState>,
    },
    BindPipeline(metal::RenderPipelineState, Option<metal::DepthStencilState>),
    Draw {
        primitive_type: metal::MTLPrimitiveType,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>
    },
    DrawIndexed {
        index: IndexBuffer,
        primitive_type: metal::MTLPrimitiveType,
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    }
}

#[derive(Debug)]
pub enum BlitCommand {
    CopyBuffer {
        src: metal::Buffer,
        dst: metal::Buffer,
        region: hal::command::BufferCopy,
    },
    CopyImage {
        src: metal::Texture,
        dst: metal::Texture,
        region: hal::command::ImageCopy,
    },
    CopyBufferToImage {
        src: metal::Buffer,
        dst: metal::Texture,
        dst_desc: hal::format::FormatDesc,
        region: hal::command::BufferImageCopy,
    },
    CopyImageToBuffer {
        src: metal::Texture,
        src_desc: hal::format::FormatDesc,
        dst: metal::Buffer,
        region: hal::command::BufferImageCopy,
    },
}

#[derive(Debug)]
pub enum ComputeCommand {
    BindBuffer {
        index: usize,
        buffer: Option<metal::Buffer>,
        offset: hal::buffer::Offset,
    },
    BindBufferData {
        bytes: Vec<u8>,
        index: usize,
    },
    BindTexture {
        index: usize,
        texture: Option<metal::Texture>,
    },
    BindSampler {
        index: usize,
        sampler: Option<metal::SamplerState>,
    },
    BindPipeline(metal::ComputePipelineState),
    Dispatch {
        wg_size: metal::MTLSize,
        wg_count: metal::MTLSize,
    },
    DispatchIndirect {
        wg_size: metal::MTLSize,
        buffer: metal::Buffer,
        offset: hal::buffer::Offset,
    },
}

pub enum Pass {
    Render(metal::RenderPassDescriptor, Vec<RenderCommand>),
    Blit(Vec<BlitCommand>),
    Compute(Vec<ComputeCommand>),
}
