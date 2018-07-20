use {BufferPtr, SamplerPtr, TexturePtr};
use command::IndexBuffer;
use native::RasterizerState;

use hal;
use metal;

use std::ops::Range;


pub trait Resources {
    type Data;
    type Buffer;
    type Texture;
    type Sampler;
    type DepthStencil;
    type RenderPipeline;
    type ComputePipeline;
}

#[derive(Debug)]
pub enum Own {}
impl Resources for Own {
    type Data = Vec<u32>;
    type Buffer = BufferPtr;
    type Texture = TexturePtr;
    type Sampler = SamplerPtr;
    type DepthStencil = metal::DepthStencilState;
    type RenderPipeline = metal::RenderPipelineState;
    type ComputePipeline = metal::ComputePipelineState;
}

impl<'a> Resources for &'a Own {
    type Data = &'a [u32];
    type Buffer = BufferPtr;
    type Texture = TexturePtr;
    type Sampler = SamplerPtr;
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
        buffer: Option<R::Buffer>,
        offset: hal::buffer::Offset,
    },
    BindBufferData {
        stage: hal::pso::Stage,
        index: usize,
        words: R::Data,
    },
    BindTexture {
        stage: hal::pso::Stage,
        index: usize,
        texture: Option<R::Texture>,
    },
    BindSampler {
        stage: hal::pso::Stage,
        index: usize,
        sampler: Option<R::Sampler>,
    },
    BindPipeline(R::RenderPipeline),
    Draw {
        primitive_type: metal::MTLPrimitiveType,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>
    },
    DrawIndexed {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<R::Buffer>,
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    },
    DrawIndirect {
        primitive_type: metal::MTLPrimitiveType,
        buffer: R::Buffer,
        offset: hal::buffer::Offset,
    },
    DrawIndexedIndirect {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<R::Buffer>,
        buffer: R::Buffer,
        offset: hal::buffer::Offset,
    },
}

impl RenderCommand<Own> {
    pub fn as_ref<'a>(&'a self) -> RenderCommand<&'a Own> {
        use self::RenderCommand::*;
        match *self {
            SetViewport(vp) => SetViewport(vp),
            SetScissor(rect) => SetScissor(rect),
            SetBlendColor(color) => SetBlendColor(color),
            SetDepthBias(bias) => SetDepthBias(bias),
            SetDepthStencilState(ref state) => SetDepthStencilState(&**state),
            SetStencilReferenceValues(front, back) => SetStencilReferenceValues(front, back),
            SetRasterizerState(ref state) => SetRasterizerState(state.clone()),
            BindBuffer { stage, index, buffer, offset } => BindBuffer {
                stage,
                index,
                buffer,
                offset,
            },
            BindBufferData { stage, index, ref words } => BindBufferData {
                stage,
                index,
                words: words.as_slice(),
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
            BindPipeline(ref pso) => BindPipeline(&**pso),
            Draw { primitive_type, ref vertices, ref instances } => Draw {
                primitive_type,
                vertices: vertices.clone(),
                instances: instances.clone(),
            },
            DrawIndexed { primitive_type, index, ref indices, base_vertex, ref instances } => DrawIndexed {
                primitive_type,
                index,
                indices: indices.clone(),
                base_vertex,
                instances: instances.clone(),
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
            BindBuffer { stage, index, buffer, offset } => BindBuffer {
                stage,
                index,
                buffer,
                offset,
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
pub enum BlitCommand<R: Resources> {
    CopyBuffer {
        src: R::Buffer,
        dst: R::Buffer,
        region: hal::command::BufferCopy,
    },
    CopyImage {
        src: R::Texture,
        dst: R::Texture,
        region: hal::command::ImageCopy,
    },
    CopyBufferToImage {
        src: R::Buffer,
        dst: R::Texture,
        dst_desc: hal::format::FormatDesc,
        region: hal::command::BufferImageCopy,
    },
    CopyImageToBuffer {
        src: R::Texture,
        src_desc: hal::format::FormatDesc,
        dst: R::Buffer,
        region: hal::command::BufferImageCopy,
    },
}

impl BlitCommand<Own> {
    pub fn as_ref<'a>(&'a self) -> BlitCommand<&'a Own> {
        use self::BlitCommand::*;
        match *self {
            CopyBuffer { src, dst, region } => CopyBuffer {
                src,
                dst,
                region,
            },
            CopyImage { src, dst, ref region } => CopyImage {
                src,
                dst,
                region: region.clone(),
            },
            CopyBufferToImage { src, dst, dst_desc, ref region } => CopyBufferToImage {
                src,
                dst,
                dst_desc,
                region: region.clone(),
            },
            CopyImageToBuffer { src, src_desc, dst, ref region } => CopyImageToBuffer {
                src,
                src_desc,
                dst,
                region: region.clone(),
            },
        }
    }
}

impl<'a> BlitCommand<&'a Own> {
    pub fn own(self) -> BlitCommand<Own> {
        use self::BlitCommand::*;
        match self {
            CopyBuffer { src, dst, region } => CopyBuffer {
                src,
                dst,
                region,
            },
            CopyImage { src, dst, region } => CopyImage {
                src: src.to_owned(),
                dst: dst.to_owned(),
                region,
            },
            CopyBufferToImage { src, dst, dst_desc, region } => CopyBufferToImage {
                src,
                dst: dst.to_owned(),
                dst_desc,
                region,
            },
            CopyImageToBuffer { src, src_desc, dst, region } => CopyImageToBuffer {
                src: src.to_owned(),
                src_desc,
                dst,
                region,
            },
        }
    }
}


#[derive(Clone, Debug)]
pub enum ComputeCommand<R: Resources> {
    BindBuffer {
        index: usize,
        buffer: Option<R::Buffer>,
        offset: hal::buffer::Offset,
    },
    BindBufferData {
        index: usize,
        words: R::Data,
    },
    BindTexture {
        index: usize,
        texture: Option<R::Texture>,
    },
    BindSampler {
        index: usize,
        sampler: Option<R::Sampler>,
    },
    BindPipeline(R::ComputePipeline),
    Dispatch {
        wg_size: metal::MTLSize,
        wg_count: metal::MTLSize,
    },
    DispatchIndirect {
        wg_size: metal::MTLSize,
        buffer: R::Buffer,
        offset: hal::buffer::Offset,
    },
}

impl ComputeCommand<Own> {
    pub fn as_ref<'a>(&'a self) -> ComputeCommand<&'a Own> {
        use self::ComputeCommand::*;
        match *self {
            BindBuffer { index, buffer, offset } => BindBuffer {
                index,
                buffer,
                offset,
            },
            BindBufferData { index, ref words } => BindBufferData {
                index,
                words: words.as_slice(),
            },
            BindTexture { index, texture } => BindTexture {
                index,
                texture,
            },
            BindSampler { index, sampler } => BindSampler {
                index,
                sampler,
            },
            BindPipeline(ref pso) => BindPipeline(&**pso),
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

impl<'a> ComputeCommand<&'a Own> {
    pub fn own(self) -> ComputeCommand<Own> {
        use self::ComputeCommand::*;
        match self {
            BindBuffer { index, buffer, offset } => BindBuffer {
                index,
                buffer,
                offset,
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
