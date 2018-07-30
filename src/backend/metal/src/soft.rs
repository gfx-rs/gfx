use {BufferPtr, SamplerPtr, TexturePtr};
use command::IndexBuffer;
use native::RasterizerState;

use hal;
use metal;

use std::ops::Range;


pub trait Resources {
    type Data;
    type DepthStencil;
    type RenderPipeline;
    type ComputePipeline;
}

#[derive(Debug)]
pub enum Own {}
impl Resources for Own {
    type Data = Vec<u32>;
    type DepthStencil = metal::DepthStencilState;
    type RenderPipeline = metal::RenderPipelineState;
    type ComputePipeline = metal::ComputePipelineState;
}

impl<'a> Resources for &'a Own {
    type Data = &'a [u32];
    type DepthStencil = &'a metal::DepthStencilStateRef;
    type RenderPipeline = &'a metal::RenderPipelineStateRef;
    type ComputePipeline = &'a metal::ComputePipelineStateRef;
}

#[derive(Clone, Debug)]
pub enum RenderCommand<R: Resources> {
    SetViewport(metal::MTLViewport),
    SetScissor(metal::MTLScissorRect),
    SetBlendColor(hal::pso::ColorValue),
    SetDepthBias(hal::pso::DepthBias),
    SetDepthStencilState(R::DepthStencil),
    SetStencilReferenceValues(hal::pso::StencilValue, hal::pso::StencilValue),
    SetRasterizerState(RasterizerState),
    BindBuffer {
        stage: hal::pso::Stage,
        index: usize,
        buffer: Option<(BufferPtr, hal::buffer::Offset)>,
    },
    BindBufferData {
        stage: hal::pso::Stage,
        index: usize,
        words: R::Data,
    },
    BindTexture {
        stage: hal::pso::Stage,
        index: usize,
        texture: Option<TexturePtr>,
    },
    BindSampler {
        stage: hal::pso::Stage,
        index: usize,
        sampler: Option<SamplerPtr>,
    },
    BindPipeline(R::RenderPipeline),
    Draw {
        primitive_type: metal::MTLPrimitiveType,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>
    },
    DrawIndexed {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<BufferPtr>,
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    },
    DrawIndirect {
        primitive_type: metal::MTLPrimitiveType,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
    DrawIndexedIndirect {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<BufferPtr>,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
}

impl<'a> RenderCommand<&'a Own> {
    pub fn own(self) -> RenderCommand<Own> {
        use self::RenderCommand::*;
        match self {
            SetViewport(vp) => SetViewport(vp),
            SetScissor(rect) => SetScissor(rect),
            SetBlendColor(color) => SetBlendColor(color),
            SetDepthBias(bias) => SetDepthBias(bias),
            SetDepthStencilState(state) => SetDepthStencilState(state.to_owned()),
            SetStencilReferenceValues(front, back) => SetStencilReferenceValues(front, back),
            SetRasterizerState(ref state) => SetRasterizerState(state.clone()),
            BindBuffer { stage, index, buffer } => BindBuffer {
                stage,
                index,
                buffer,
            },
            BindBufferData { stage, index, words } => BindBufferData {
                stage,
                index,
                words: words.to_vec(),
            },
            BindTexture { stage, index, texture } => BindTexture {
                stage,
                index,
                texture,
            },
            BindSampler { stage, index, sampler } => BindSampler {
                stage,
                index,
                sampler,
            },
            BindPipeline(pso) => BindPipeline(pso.to_owned()),
            Draw { primitive_type, vertices, instances } => Draw {
                primitive_type,
                vertices,
                instances,
            },
            DrawIndexed { primitive_type, index, indices, base_vertex, instances } => DrawIndexed {
                primitive_type,
                index,
                indices,
                base_vertex,
                instances,
            },
            DrawIndirect { primitive_type, buffer, offset } => DrawIndirect {
                primitive_type,
                buffer,
                offset,
            },
            DrawIndexedIndirect { primitive_type, index, buffer, offset } => DrawIndexedIndirect {
                primitive_type,
                index,
                buffer,
                offset,
            },
        }
    }
}


#[derive(Clone, Debug)]
pub enum BlitCommand {
    CopyBuffer {
        src: BufferPtr,
        dst: BufferPtr,
        region: hal::command::BufferCopy,
    },
    CopyImage {
        src: TexturePtr,
        dst: TexturePtr,
        region: hal::command::ImageCopy,
    },
    CopyBufferToImage {
        src: BufferPtr,
        dst: TexturePtr,
        dst_desc: hal::format::FormatDesc,
        region: hal::command::BufferImageCopy,
    },
    CopyImageToBuffer {
        src: TexturePtr,
        src_desc: hal::format::FormatDesc,
        dst: BufferPtr,
        region: hal::command::BufferImageCopy,
    },
}

#[derive(Clone, Debug)]
pub enum ComputeCommand<R: Resources> {
    BindBuffer {
        index: usize,
        buffer: Option<(BufferPtr, hal::buffer::Offset)>,
    },
    BindBufferData {
        index: usize,
        words: R::Data,
    },
    BindTexture {
        index: usize,
        texture: Option<TexturePtr>,
    },
    BindSampler {
        index: usize,
        sampler: Option<SamplerPtr>,
    },
    BindPipeline(R::ComputePipeline),
    Dispatch {
        wg_size: metal::MTLSize,
        wg_count: metal::MTLSize,
    },
    DispatchIndirect {
        wg_size: metal::MTLSize,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
}

impl<'a> ComputeCommand<&'a Own> {
    pub fn own(self) -> ComputeCommand<Own> {
        use self::ComputeCommand::*;
        match self {
            BindBuffer { index, buffer } => BindBuffer {
                index,
                buffer,
            },
            BindBufferData { index, words } => BindBufferData {
                index,
                words: words.to_vec(),
            },
            BindTexture { index, texture } => BindTexture {
                index,
                texture,
            },
            BindSampler { index, sampler } => BindSampler {
                index,
                sampler,
            },
            BindPipeline(pso) => BindPipeline(pso.to_owned()),
            Dispatch { wg_size, wg_count } => Dispatch {
                wg_size,
                wg_count,
            },
            DispatchIndirect { wg_size, buffer, offset } => DispatchIndirect {
                wg_size,
                buffer,
                offset,
            },
        }
    }
}


#[derive(Debug)]
pub enum Pass {
    Render(metal::RenderPassDescriptor),
    Blit,
    Compute,
}
