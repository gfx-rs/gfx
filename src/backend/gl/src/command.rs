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
use gfx_core::draw::{ClearSet, DataPointer, InstanceOption};
use gfx_core::state as s;
use gfx_core::target::{ColorValue, Mirror, Rect, Stencil};
use {Buffer, ArrayBuffer, Program, FrameBuffer,
     Resources, PipelineState, TargetView};


fn primitive_to_gl(primitive: c::Primitive) -> gl::types::GLenum {
    use gfx_core::Primitive as P;
    match primitive {
        P::Point => gl::POINTS,
        P::Line => gl::LINES,
        P::LineStrip => gl::LINE_STRIP,
        P::TriangleList => gl::TRIANGLES,
        P::TriangleStrip => gl::TRIANGLE_STRIP,
        P::TriangleFan => gl::TRIANGLE_FAN,
    }
}

pub type Access = gl::types::GLenum;

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    BindProgram(Program),
    BindConstantBuffers(c::pso::ConstantBufferSet<Resources>),
    BindResourceViews(c::pso::ResourceViewSet<Resources>),
    BindUnorderedViews(c::pso::UnorderedViewSet<Resources>),
    BindSamplers(c::pso::SamplerSet<Resources>),
    BindPixelTargets(c::pso::PixelTargetSet<Resources>),
    BindArrayBuffer(ArrayBuffer),
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
    // drawing
    Clear(ClearSet),
    Draw(gl::types::GLenum, c::VertexCount, c::VertexCount, InstanceOption),
    DrawIndexed(gl::types::GLenum, c::IndexType, c::VertexCount, c::VertexCount,
                c::VertexCount, InstanceOption),
    Blit(Rect, Rect, Mirror, usize),
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
    attributes: [Option<c::pso::AttributeDesc>;c::MAX_VERTEX_ATTRIBUTES],
    //resource_views: [Option<(Texture, BindAnchor)>; c::MAX_RESOURCE_VIEWS],
    stencil: Option<s::Stencil>,
    //blend: Option<s::Blend>,
    cull_face: s::CullFace,
    draw_mask: u32,
}

impl Cache {
    fn new() -> Cache {
        Cache {
            primitive: 0,
            attributes: [None; c::MAX_VERTEX_ATTRIBUTES],
            //resource_views: [None; c::MAX_RESOURCE_VIEWS],
            stencil: None,
            cull_face: s::CullFace::Nothing,
            //blend: None,
            draw_mask: 0,
        }
    }
}

pub struct CommandBuffer {
    pub buf: Vec<Command>,
    fbo: FrameBuffer,
    cache: Cache,
}

impl CommandBuffer {
    pub fn new(fbo: FrameBuffer) -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
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
        CommandBuffer {
            buf: Vec::new(),
            fbo: self.fbo,
            cache: Cache::new(),
        }
    }

    fn reset(&mut self) {
        self.buf.clear();
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

    fn bind_constant_buffers(&mut self, cbs: c::pso::ConstantBufferSet<Resources>) {
        self.buf.push(Command::BindConstantBuffers(cbs));
    }

    fn bind_global_constant(&mut self, loc: c::shade::Location,
                    value: c::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_resource_views(&mut self, srvs: c::pso::ResourceViewSet<Resources>) {
        self.buf.push(Command::BindResourceViews(srvs));
    }

    fn bind_unordered_views(&mut self, uavs: c::pso::UnorderedViewSet<Resources>) {
        self.buf.push(Command::BindUnorderedViews(uavs));
    }

    fn bind_samplers(&mut self, ss: c::pso::SamplerSet<Resources>) {
        self.buf.push(Command::BindSamplers(ss));
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

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        self.buf.push(Command::SetStencilState(self.cache.stencil, rv.stencil, self.cache.cull_face));
        self.buf.push(Command::SetBlendColor(rv.blend));
    }

    fn update_buffer(&mut self, buf: Buffer, data: DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn clear(&mut self, set: ClearSet) {
        self.buf.push(Command::Clear(set));
    }

    fn call_draw(&mut self, start: c::VertexCount,
                 count: c::VertexCount, instances: InstanceOption) {
        self.buf.push(Command::Draw(self.cache.primitive, start, count, instances));
    }

    fn call_draw_indexed(&mut self,
                         itype: c::IndexType, start: c::VertexCount,
                         count: c::VertexCount, base: c::VertexCount,
                         instances: InstanceOption) {
        self.buf.push(Command::DrawIndexed(self.cache.primitive,
            itype, start, count, base, instances));
    }
}
