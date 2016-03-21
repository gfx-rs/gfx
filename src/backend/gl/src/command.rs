// Copyright 2015 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(missing_docs)]

use gl;
use gfx_core as c;
use gfx_core::draw;
use gfx_core::state as s;
use gfx_core::target::{ColorValue, Depth, Mirror, Rect, Stencil};
use {Buffer, Program, FrameBuffer, Texture,
     NewTexture, Resources, PipelineState, ResourceView, TargetView};


fn primitive_to_gl(primitive: c::Primitive) -> gl::types::GLenum {
    use gfx_core::Primitive::*;
    match primitive {
        PointList => gl::POINTS,
        LineList => gl::LINES,
        LineStrip => gl::LINE_STRIP,
        TriangleList => gl::TRIANGLES,
        TriangleStrip => gl::TRIANGLE_STRIP,
        //TriangleFan => gl::TRIANGLE_FAN,
    }
}

pub type Access = gl::types::GLenum;

#[derive(Clone, Copy, Debug)]
pub struct RawOffset(pub *const gl::types::GLvoid);
unsafe impl Send for RawOffset {}

/// The place of some data in the data buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DataPointer {
    offset: u32,
    size: u32,
}

pub struct DataBuffer(Vec<u8>);
impl DataBuffer {
    /// Create a new empty data buffer.
    pub fn new() -> DataBuffer {
        DataBuffer(Vec::new())
    }
    /// Copy a given vector slice into the buffer.
    fn add(&mut self, data: &[u8]) -> DataPointer {
        self.0.extend_from_slice(data);
        DataPointer {
            offset: (self.0.len() - data.len()) as u32,
            size: data.len() as u32,
        }
    }
    /// Return a reference to a stored data object.
    pub fn get(&self, ptr: DataPointer) -> &[u8] {
        &self.0[ptr.offset as usize .. (ptr.offset + ptr.size) as usize]
    }
}

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    BindProgram(Program),
    BindConstantBuffer(c::pso::ConstantBufferParam<Resources>),
    BindResourceView(c::pso::ResourceViewParam<Resources>),
    BindUnorderedView(c::pso::UnorderedViewParam<Resources>),
    BindSampler(c::pso::SamplerParam<Resources>, Option<gl::types::GLenum>),
    BindPixelTargets(c::pso::PixelTargetSet<Resources>),
    BindAttribute(c::AttributeSlot, Buffer, c::pso::AttributeDesc),
    BindIndex(Buffer),
    BindFrameBuffer(Access, FrameBuffer),
    BindUniform(c::shade::Location, c::shade::UniformValue),
    SetDrawColorBuffers(c::ColorSlot),
    SetRasterizer(s::Rasterizer),
    SetViewport(Rect),
    SetScissor(Option<Rect>),
    SetDepthState(Option<s::Depth>),
    SetStencilState(Option<s::Stencil>, (Stencil, Stencil), s::CullFace),
    SetBlendState(c::ColorSlot, s::Color),
    SetBlendColor(ColorValue),
    // resource updates
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture, c::tex::Kind, Option<c::tex::CubeFace>,
                  DataPointer, c::tex::RawImageInfo),
    GenerateMipmap(ResourceView),
    // drawing
    Clear(Option<draw::ClearColor>, Option<Depth>, Option<Stencil>),
    Draw(gl::types::GLenum, c::VertexCount, c::VertexCount, draw::InstanceOption),
    DrawIndexed(gl::types::GLenum, gl::types::GLenum, RawOffset,
                c::VertexCount, c::VertexCount, draw::InstanceOption),
    _Blit(Rect, Rect, Mirror, usize),
}

pub const COLOR_DEFAULT: s::Color = s::Color {
    mask: s::MASK_ALL,
    blend: None,
};

pub const RESET: [Command; 13] = [
    Command::BindProgram(0),
    // BindAttribute
    Command::BindIndex(0),
    Command::BindFrameBuffer(gl::FRAMEBUFFER, 0),
    Command::SetRasterizer(s::Rasterizer {
        front_face: s::FrontFace::CounterClockwise,
        method: s::RasterMethod::Fill(s::CullFace::Back),
        offset: None,
        samples: None,
    }),
    Command::SetViewport(Rect{x: 0, y: 0, w: 0, h: 0}),
    Command::SetScissor(None),
    Command::SetDepthState(None),
    Command::SetStencilState(None, (0, 0), s::CullFace::Nothing),
    Command::SetBlendState(0, COLOR_DEFAULT),
    Command::SetBlendState(1, COLOR_DEFAULT),
    Command::SetBlendState(2, COLOR_DEFAULT),
    Command::SetBlendState(3, COLOR_DEFAULT),
    Command::SetBlendColor([0f32; 4]),
];

struct Cache {
    primitive: gl::types::GLenum,
    index_type: c::IndexType,
    attributes: [Option<c::pso::AttributeDesc>; c::MAX_VERTEX_ATTRIBUTES],
    resource_binds: [Option<gl::types::GLenum>; c::MAX_RESOURCE_VIEWS],
    scissor: bool,
    stencil: Option<s::Stencil>,
    //blend: Option<s::Blend>,
    cull_face: s::CullFace,
    draw_mask: u32,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            primitive: 0,
            index_type: c::IndexType::U8,
            attributes: [None; c::MAX_VERTEX_ATTRIBUTES],
            resource_binds: [None; c::MAX_RESOURCE_VIEWS],
            scissor: false,
            stencil: None,
            cull_face: s::CullFace::Nothing,
            //blend: None,
            draw_mask: 0,
        }
    }
}

pub struct CommandBuffer {
    pub buf: Vec<Command>,
    pub data: DataBuffer,
    fbo: FrameBuffer,
    cache: Cache,
}

impl CommandBuffer {
    pub fn new(fbo: FrameBuffer) -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
            data: DataBuffer::new(),
            fbo: fbo,
            cache: Cache::new(),
        }
    }
    fn is_main_target(&self, tv: Option<TargetView>) -> bool {
        match tv {
            Some(TargetView::Surface(0)) | None => true,
            Some(_) => false,
        }
    }
}

impl c::draw::CommandBuffer<Resources> for CommandBuffer {
    fn clone_empty(&self) -> CommandBuffer {
        CommandBuffer::new(self.fbo)
    }

    fn reset(&mut self) {
        self.buf.clear();
        self.data.0.clear();
        self.cache = Cache::new();
    }

    fn bind_pipeline_state(&mut self, pso: PipelineState) {
        let cull = pso.rasterizer.method.get_cull_face();
        self.cache.primitive = primitive_to_gl(pso.primitive);
        self.cache.attributes = pso.input;
        self.cache.stencil = pso.output.stencil;
        self.cache.cull_face = cull;
        self.cache.draw_mask = pso.output.draw_mask;
        self.buf.push(Command::BindProgram(pso.program));
        self.cache.scissor = pso.scissor;
        self.buf.push(Command::SetRasterizer(pso.rasterizer));
        self.buf.push(Command::SetDepthState(pso.output.depth));
        self.buf.push(Command::SetStencilState(pso.output.stencil, (0, 0), cull));
        for i in 0 .. c::MAX_COLOR_TARGETS {
            if pso.output.draw_mask & (1<<i) != 0 {
                self.buf.push(Command::SetBlendState(i as c::ColorSlot, pso.output.colors[i]));
            }
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: c::pso::VertexBufferSet<Resources>) {
        for i in 0 .. c::MAX_VERTEX_ATTRIBUTES {
            match (vbs.0[i], self.cache.attributes[i]) {
                (None, Some(fm)) => {
                    error!("No vertex input provided for slot {} of format {:?}", i, fm)
                },
                (Some((buffer, offset)), Some(mut format)) => {
                    format.0.offset += offset as gl::types::GLuint;
                    self.buf.push(Command::BindAttribute(i as c::AttributeSlot, buffer, format));
                },
                (_, None) => (),
            }
        }
    }

    fn bind_constant_buffers(&mut self, cbs: &[c::pso::ConstantBufferParam<Resources>]) {
        for param in cbs.iter() {
            self.buf.push(Command::BindConstantBuffer(param.clone()));
        }
    }

    fn bind_global_constant(&mut self, loc: c::shade::Location,
                    value: c::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_resource_views(&mut self, srvs: &[c::pso::ResourceViewParam<Resources>]) {
        for i in 0 .. c::MAX_RESOURCE_VIEWS {
            self.cache.resource_binds[i] = None;
        }
        for param in srvs.iter() {
            self.cache.resource_binds[param.2 as usize] = Some(param.0.bind);
            self.buf.push(Command::BindResourceView(param.clone()));
        }
    }

    fn bind_unordered_views(&mut self, uavs: &[c::pso::UnorderedViewParam<Resources>]) {
        for param in uavs.iter() {
            self.buf.push(Command::BindUnorderedView(param.clone()));
        }
    }

    fn bind_samplers(&mut self, ss: &[c::pso::SamplerParam<Resources>]) {
        for param in ss.iter() {
            let bind = self.cache.resource_binds[param.2 as usize];
            self.buf.push(Command::BindSampler(param.clone(), bind));
        }
    }

    fn bind_pixel_targets(&mut self, pts: c::pso::PixelTargetSet<Resources>) {
        let is_main = pts.colors.iter().skip(1).find(|c| c.is_some()).is_none() &&
            self.is_main_target(pts.colors[0]) &&
            self.is_main_target(pts.depth) &&
            self.is_main_target(pts.stencil);
        if is_main {
            self.buf.push(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, 0));
        }else {
            let num = pts.colors.iter().position(|c| c.is_none())
                         .unwrap_or(pts.colors.len()) as c::ColorSlot;
            self.buf.push(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, self.fbo));
            self.buf.push(Command::BindPixelTargets(pts));
            self.buf.push(Command::SetDrawColorBuffers(num));
        }
        self.buf.push(Command::SetViewport(Rect {
            x: 0, y: 0, w: pts.size.0, h: pts.size.1}));
    }

    fn bind_index(&mut self, buf: Buffer, itype: c::IndexType) {
        self.cache.index_type = itype;
        self.buf.push(Command::BindIndex(buf));
    }

    fn set_scissor(&mut self, rect: Rect) {
        self.buf.push(Command::SetScissor(
            if self.cache.scissor {Some(rect)} else {None}
        ));
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        self.buf.push(Command::SetStencilState(self.cache.stencil, rv.stencil, self.cache.cull_face));
        self.buf.push(Command::SetBlendColor(rv.blend));
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset_bytes: usize) {
        let ptr = self.data.add(data);
        self.buf.push(Command::UpdateBuffer(buf, ptr, offset_bytes));
    }

    fn update_texture(&mut self, ntex: NewTexture, kind: c::tex::Kind,
                      face: Option<c::tex::CubeFace>, data: &[u8],
                      img: c::tex::RawImageInfo) {
        let ptr = self.data.add(data);
        match ntex {
            NewTexture::Texture(t) =>
                self.buf.push(Command::UpdateTexture(t, kind, face, ptr, img)),
            NewTexture::Surface(s) =>
                error!("GL: unable to update the contents of a Surface({})", s),
        }
    }

    fn generate_mipmap(&mut self, srv: ResourceView) {
        self.buf.push(Command::GenerateMipmap(srv));
    }

    fn clear_color(&mut self, target: TargetView, value: draw::ClearColor) {
        // this could be optimized by deferring the actual clear call
        let mut pts = c::pso::PixelTargetSet::new();
        pts.colors[0] = Some(target);
        self.bind_pixel_targets(pts);
        self.buf.push(Command::Clear(Some(value), None, None));
    }

    fn clear_depth_stencil(&mut self, target: TargetView, depth: Option<Depth>, stencil: Option<Stencil>) {
        let mut pts = c::pso::PixelTargetSet::new();
        if depth.is_some() {
            pts.depth = Some(target);
        }
        if stencil.is_some() {
            pts.stencil = Some(target);
        }
        self.bind_pixel_targets(pts);
        self.buf.push(Command::Clear(None, depth, stencil));
    }

    fn call_draw(&mut self, start: c::VertexCount,
                 count: c::VertexCount, instances: draw::InstanceOption) {
        self.buf.push(Command::Draw(self.cache.primitive, start, count, instances));
    }

    fn call_draw_indexed(&mut self, start: c::VertexCount,
                         count: c::VertexCount, base: c::VertexCount,
                         instances: draw::InstanceOption) {
        let (offset, gl_index) = match self.cache.index_type {
            c::IndexType::U8  => (start * 1u32, gl::UNSIGNED_BYTE),
            c::IndexType::U16 => (start * 2u32, gl::UNSIGNED_SHORT),
            c::IndexType::U32 => (start * 4u32, gl::UNSIGNED_INT),
        };
        self.buf.push(Command::DrawIndexed(self.cache.primitive,
            gl_index, RawOffset(offset as *const gl::types::GLvoid), count, base, instances));
    }
}
