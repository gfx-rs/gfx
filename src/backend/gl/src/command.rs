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
use core::{self as c, command, state as s};
use core::target::{ColorValue, Depth, Mirror, Rect, Stencil};
use {Buffer, BufferElement, Program, FrameBuffer, Texture, NewTexture, Resources, PipelineState,
     ResourceView, TargetView};


fn primitive_to_gl(primitive: c::Primitive) -> gl::types::GLenum {
    use core::Primitive::*;
    match primitive {
        PointList => gl::POINTS,
        LineList => gl::LINES,
        LineStrip => gl::LINE_STRIP,
        TriangleList => gl::TRIANGLES,
        TriangleStrip => gl::TRIANGLE_STRIP,
        LineListAdjacency => gl::LINES_ADJACENCY,
        LineStripAdjacency => gl::LINE_STRIP_ADJACENCY,
        TriangleListAdjacency => gl::TRIANGLES_ADJACENCY,
        TriangleStripAdjacency => gl::TRIANGLE_STRIP_ADJACENCY,
        // TriangleFan => gl::TRIANGLE_FAN,
        PatchList(_) => gl::PATCHES,
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
        &self.0[ptr.offset as usize..(ptr.offset + ptr.size) as usize]
    }
}


/// Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    // states
    BindProgram(Program),
    BindConstantBuffer(c::pso::ConstantBufferParam<Resources>),
    BindResourceView(c::pso::ResourceViewParam<Resources>),
    BindUnorderedView(c::pso::UnorderedViewParam<Resources>),
    BindSampler(c::pso::SamplerParam<Resources>, Option<gl::types::GLenum>),
    BindPixelTargets(c::pso::PixelTargetSet<Resources>),
    BindVao,
    BindAttribute(c::AttributeSlot, Buffer, BufferElement),
    UnbindAttribute(c::AttributeSlot),
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
    SetPatches(c::PatchSize),
    CopyBuffer(Buffer, Buffer, gl::types::GLintptr, gl::types::GLintptr, gl::types::GLsizeiptr),
    // resource updates
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture,
                  c::texture::Kind,
                  Option<c::texture::CubeFace>,
                  DataPointer,
                  c::texture::RawImageInfo),
    GenerateMipmap(ResourceView),
    // drawing
    Clear(Option<command::ClearColor>, Option<Depth>, Option<Stencil>),
    Draw(gl::types::GLenum, c::VertexCount, c::VertexCount, Option<command::InstanceParams>),
    DrawIndexed(gl::types::GLenum,
                gl::types::GLenum,
                RawOffset,
                c::VertexCount,
                c::VertexCount,
                Option<command::InstanceParams>),
    _Blit(Rect, Rect, Mirror, usize),
}

pub const COLOR_DEFAULT: s::Color = s::Color {
    mask: s::MASK_ALL,
    blend: None,
};

pub const RESET: [Command; 14] = [Command::BindProgram(0),
                                  Command::BindVao,
                                  // Command::UnbindAttribute, //not needed, handled by the cache
                                  Command::BindIndex(0),
                                  Command::BindFrameBuffer(gl::FRAMEBUFFER, 0),
                                  Command::SetRasterizer(s::Rasterizer {
                                      front_face: s::FrontFace::CounterClockwise,
                                      cull_face: s::CullFace::Back,
                                      method: s::RasterMethod::Fill,
                                      offset: None,
                                      samples: None,
                                  }),
                                  Command::SetViewport(Rect {
                                      x: 0,
                                      y: 0,
                                      w: 0,
                                      h: 0,
                                  }),
                                  Command::SetScissor(None),
                                  Command::SetDepthState(None),
                                  Command::SetStencilState(None, (0, 0), s::CullFace::Nothing),
                                  Command::SetBlendState(0, COLOR_DEFAULT),
                                  Command::SetBlendState(1, COLOR_DEFAULT),
                                  Command::SetBlendState(2, COLOR_DEFAULT),
                                  Command::SetBlendState(3, COLOR_DEFAULT),
                                  Command::SetBlendColor([0f32; 4])];

struct Cache {
    primitive: gl::types::GLenum,
    index_type: c::IndexType,
    attributes: [Option<BufferElement>; c::MAX_VERTEX_ATTRIBUTES],
    resource_binds: [Option<gl::types::GLenum>; c::MAX_RESOURCE_VIEWS],
    scissor: bool,
    target_dim: (u16, u16, u16),
    stencil: Option<s::Stencil>,
    // blend: Option<s::Blend>,
    cull_face: s::CullFace,
    draw_mask: u32,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            primitive: 0,
            index_type: c::IndexType::U16,
            attributes: [None; c::MAX_VERTEX_ATTRIBUTES],
            resource_binds: [None; c::MAX_RESOURCE_VIEWS],
            scissor: false,
            target_dim: (0, 0, 0),
            stencil: None,
            cull_face: s::CullFace::Nothing,
            // blend: None,
            draw_mask: 0,
        }
    }
}


struct GlState {
    program: Program,
    constant_buffer: Option<c::pso::ConstantBufferParam<Resources>>,
    resource_view: Option<c::pso::ResourceViewParam<Resources>>,
    vao_bound: bool,
    scissor_test: Option<Rect>,
    depth_state: Option<s::Depth>,
    blend_state: Option<(c::ColorSlot, s::Color)>,
    blend_color: Option<ColorValue>,
    viewport: Option<Rect>,
    rasterizer: Option<s::Rasterizer>,
    stencil_enabled: Option<s::Stencil>,
    stencil_state: Option<((Stencil, Stencil), s::CullFace)>,
    framebuffer: Option<(Access, FrameBuffer)>,
    index: Buffer,
    attribute: Option<(c::AttributeSlot, Buffer, BufferElement)>,
}

impl GlState {
    fn new() -> Self {
        GlState {
            program: 0,
            constant_buffer: None,
            resource_view: None,
            vao_bound: false,
            scissor_test: None,
            depth_state: None,
            blend_state: None,
            blend_color: None,
            viewport: None,
            rasterizer: None,
            stencil_enabled: None,
            stencil_state: None,
            framebuffer: None,
            index: 0,
            attribute: None,
        }
    }

    /// Pushes a command to the command buffer, but
    /// attempts to keep track of the current state
    /// and not push redundant commands.
    fn filter_push(&mut self, buf: &mut Vec<Command>, cmd: Command) {
        buf.push(cmd);
    }

    fn bind_program(&mut self, buf: &mut Vec<Command>, program: Program) {
        if program == self.program {
            return;
        }
        self.program = program;
        buf.push(Command::BindProgram(program));
    }

    fn bind_constant_buffer(&mut self, buf: &mut Vec<Command>, constant_buffer: c::pso::ConstantBufferParam<Resources>) {
        if let Some(cb) = self.constant_buffer {
            if cb == constant_buffer {
                return;
            }
        }
        self.constant_buffer = Some(constant_buffer);
        buf.push(Command::BindConstantBuffer(constant_buffer));
    }

    fn bind_resource_view(&mut self, buf: &mut Vec<Command>, resource_view: c::pso::ResourceViewParam<Resources>) {
        if let Some(rv) = self.resource_view {
            if rv == resource_view {
                return;
            }
        }
        self.resource_view = Some(resource_view);
        buf.push(Command::BindResourceView(resource_view));
    }

    fn bind_vao(&mut self, buf: &mut Vec<Command>) {
        // TODO: Double-check this is where the
        // EnableVertexAttrib stuff happens.
        if self.vao_bound {
                    return;
                }
        self.vao_bound = true;
        buf.push(Command::BindVao);
    }

    // fn bind_attribute(&mut self, buf: &mut Vec<Command>, attribute_slot: c::AttributeSlot, buffer: Buffer, buffer_element: BufferElement) {
    //     // BUGGO: This can potentially bind many different
    //     // attributes but we only record the latest one,
    //     // we'll have to keep a map of all attributes
    //     // to do this right.
    //     if self.attribute == Some((attribute_slot, buffer, buffer_element)) {
    //         return;
    //     }
    //     self.attribute = Some((attribute_slot, buffer, buffer_element));
    //     buf.push(Command::BindAttribute(attribute_slot, buffer, buffer_element));
    // }

    fn bind_index(&mut self, buf: &mut Vec<Command>, buffer: Buffer) {
        if self.index == buffer {
            return;
        }
        self.index = buffer;
        buf.push(Command::BindIndex(buffer));
    }

    fn bind_framebuffer(&mut self, buf: &mut Vec<Command>, access: Access, fb: FrameBuffer) {

        if self.framebuffer == Some((access, fb)) {
            return;
        }
        self.framebuffer = Some((access, fb));
        buf.push(Command::BindFrameBuffer(access, fb));
    }

    fn set_rasterizer(&mut self, buf: &mut Vec<Command>, rasterizer: s::Rasterizer) {
        if self.rasterizer == Some(rasterizer) {
            return;
        }
        self.rasterizer = Some(rasterizer);
        buf.push(Command::SetRasterizer(rasterizer));
    }

    fn set_viewport(&mut self, buf: &mut Vec<Command>, rect: Rect) {
        if self.viewport == Some(rect) {
            return;
        }
        self.viewport = Some(rect);
        buf.push(Command::SetViewport(rect));
    }

    fn set_scissor(&mut self, buf: &mut Vec<Command>, rect: Option<Rect>) {
        if self.scissor_test == rect {
            return;
        }
        self.scissor_test = rect;
        buf.push(Command::SetScissor(rect));
    }
    fn set_depth_state(&mut self, buf: &mut Vec<Command>, depth: Option<s::Depth>) {
        if self.depth_state == depth {
            return;
        }
        self.depth_state = depth;
        buf.push(Command::SetDepthState(depth));
    }
    fn set_stencil_state(&mut self, buf: &mut Vec<Command>, option_stencil: Option<s::Stencil>, stencils: (Stencil, Stencil), cullface: s:: CullFace) {
        // This is a little more complex 'cause if option_stencil
        // is None the stencil state is disabled, it it's Some
        // then it's enabled and parameters are set.
        // That's actually bad because it makes it impossible
        // to completely remove all redundant calls if the
        // stencil is enabled;
        // we'll be re-enabling it over and over.
        // BUGGO: This isn't actually removing all the
        // bogus glDisable(cap = GL_STENCIL_TEST) calls,
        // look into it more.
        if let Some(stencil) = option_stencil {
            // Enable stenciling
            // BUGGO:
            // We don't bother optimizing this case yet
            // because it's a PITA, so we just continue.
        } else {
            // Disable stenciling
            if self.stencil_enabled == option_stencil {
                // Already disabled, all good.
                return;
            } else {
                self.stencil_enabled = option_stencil;
            }
                }
        buf.push(Command::SetStencilState(option_stencil, stencils, cullface));
    }
    fn set_blend_state(&mut self, buf: &mut Vec<Command>, color_slot: c::ColorSlot, color: s::Color) {
        if let Some(bs) = self.blend_state {
            if bs == (color_slot, color) {
                return;
            }
        }
        self.blend_state = Some((color_slot, color));
        buf.push(Command::SetBlendState(color_slot, color));
    }
    fn set_blend_color(&mut self, buf: &mut Vec<Command>, color_value: ColorValue) {
        if self.blend_color == Some(color_value) {
            return;
        }
        self.blend_color = Some(color_value);
        buf.push(Command::SetBlendColor(color_value));
    }
    
}


pub struct CommandBuffer {
    pub buf: Vec<Command>,
    pub data: DataBuffer,
    fbo: FrameBuffer,
    cache: Cache,
    active_attribs: usize,
    saved_state: GlState,
}

impl CommandBuffer {
    pub fn new(fbo: FrameBuffer) -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
            data: DataBuffer::new(),
            fbo: fbo,
            cache: Cache::new(),
            active_attribs: 0,
            saved_state: GlState::new(),
        }
    }
    fn is_main_target(&self, tv: Option<TargetView>) -> bool {
        match tv {
            Some(TargetView::Surface(0)) |
            None => true,
            Some(_) => false,
        }
    }
}

impl command::Buffer<Resources> for CommandBuffer {
    fn reset(&mut self) {
        self.buf.clear();
        self.data.0.clear();
        self.cache = Cache::new();
        self.active_attribs = (1 << c::MAX_VERTEX_ATTRIBUTES) - 1;
    }

    fn bind_pipeline_state(&mut self, pso: PipelineState) {
        let cull = pso.rasterizer.cull_face;
        self.cache.primitive = primitive_to_gl(pso.primitive);
        self.cache.attributes = pso.input;
        self.cache.stencil = pso.output.stencil;
        self.cache.cull_face = cull;
        self.cache.draw_mask = pso.output.draw_mask;
        self.saved_state.bind_program(&mut self.buf, pso.program);
        self.cache.scissor = pso.scissor;
        self.saved_state.set_rasterizer(&mut self.buf, pso.rasterizer);
        self.saved_state.set_depth_state(&mut self.buf, pso.output.depth);
        self.saved_state.set_stencil_state(&mut self.buf,
                                     pso.output.stencil, (0, 0), cull);
        for i in 0..c::MAX_COLOR_TARGETS {
            if pso.output.draw_mask & (1 << i) != 0 {
                self.saved_state.set_blend_state(&mut self.buf,
                                            i as c::ColorSlot,
                                            pso.output.colors[i]);
            }
        }
        if let c::Primitive::PatchList(num) = pso.primitive {
            self.saved_state.filter_push(&mut self.buf, Command::SetPatches(num));
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: c::pso::VertexBufferSet<Resources>) {
        for i in 0..c::MAX_VERTEX_ATTRIBUTES {
            match (vbs.0[i], self.cache.attributes[i]) {
                (None, Some(fm)) => {
                    error!("No vertex input provided for slot {} of format {:?}", i, fm)
                }
                (Some((buffer, offset)), Some(mut bel)) => {
                    bel.elem.offset += offset as gl::types::GLuint;
                    self.saved_state.filter_push(&mut self.buf,
                                                 Command::BindAttribute(i as c::AttributeSlot,
                                                                        buffer,
                                                                        bel));
                    self.active_attribs |= 1 << i;
                }
                (_, None) if self.active_attribs & (1 << i) != 0 => {
                    self.saved_state.filter_push(&mut self.buf,
                                                 Command::UnbindAttribute(i as c::AttributeSlot));
                    self.active_attribs ^= 1 << i;
                }
                (_, None) => (),
            }
        }
    }

    fn bind_constant_buffers(&mut self, cbs: &[c::pso::ConstantBufferParam<Resources>]) {
        for param in cbs.iter() {
            self.saved_state.bind_constant_buffer(&mut self.buf, param.clone());
        }
    }

    fn bind_global_constant(&mut self, loc: c::shade::Location, value: c::shade::UniformValue) {
        self.saved_state.filter_push(&mut self.buf, Command::BindUniform(loc, value));
    }

    fn bind_resource_views(&mut self, srvs: &[c::pso::ResourceViewParam<Resources>]) {
        for i in 0..c::MAX_RESOURCE_VIEWS {
            self.cache.resource_binds[i] = None;
        }
        for param in srvs.iter() {
            self.cache.resource_binds[param.2 as usize] = Some(param.0.bind);
            self.saved_state.bind_resource_view(&mut self.buf, param.clone());
        }
    }

    fn bind_unordered_views(&mut self, uavs: &[c::pso::UnorderedViewParam<Resources>]) {
        for param in uavs.iter() {
            self.saved_state.filter_push(&mut self.buf, Command::BindUnorderedView(param.clone()));
        }
    }

    fn bind_samplers(&mut self, ss: &[c::pso::SamplerParam<Resources>]) {
        for param in ss.iter() {
            let bind = self.cache.resource_binds[param.2 as usize];
            self.saved_state.filter_push(&mut self.buf, Command::BindSampler(param.clone(), bind));
        }
    }

    fn bind_pixel_targets(&mut self, pts: c::pso::PixelTargetSet<Resources>) {
        let is_main = pts.colors.iter().skip(1).find(|c| c.is_some()).is_none() &&
                      self.is_main_target(pts.colors[0]) &&
                      self.is_main_target(pts.depth) &&
                      self.is_main_target(pts.stencil);
        if is_main {
            self.saved_state.filter_push(&mut self.buf,
                                         Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, 0));
        } else {
            let num = pts.colors
                .iter()
                .position(|c| c.is_none())
                .unwrap_or(pts.colors.len()) as c::ColorSlot;
            self.saved_state.bind_framebuffer(&mut self.buf,
                                         gl::DRAW_FRAMEBUFFER, self.fbo);
            self.saved_state.filter_push(&mut self.buf, Command::BindPixelTargets(pts));
            self.saved_state.filter_push(&mut self.buf, Command::SetDrawColorBuffers(num));
        }
        let view = pts.get_view();
        self.cache.target_dim = view;
        self.saved_state.set_viewport(&mut self.buf,
                                     Rect {
                                         x: 0,
                                         y: 0,
                                         w: view.0,
                                         h: view.1,
                                     });
    }

    fn bind_index(&mut self, buf: Buffer, itype: c::IndexType) {
        self.cache.index_type = itype;
        self.saved_state.filter_push(&mut self.buf, Command::BindIndex(buf));
    }

    fn set_scissor(&mut self, rect: Rect) {
        use std::cmp;
        self.saved_state.set_scissor(&mut self.buf,
                                     if self.cache.scissor {
                                         Some(Rect {
                                             // inverting the Y axis in order to match D3D11
                                             y: cmp::max(self.cache.target_dim.1, rect.y + rect.h) -
                                                rect.y -
                                                rect.h,
                                             ..rect
                                         })
                                     } else {
                                         None //TODO: assert?
                                     });
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        self.saved_state.set_stencil_state(&mut self.buf,
                                     self.cache.stencil,
                                                              rv.stencil,
                                                              self.cache.cull_face);
        self.saved_state.set_blend_color(&mut self.buf, rv.blend);
    }

    fn copy_buffer(&mut self,
                   src: Buffer,
                   dst: Buffer,
                   src_offset_bytes: usize,
                   dst_offset_bytes: usize,
                   size_bytes: usize) {
        self.saved_state.filter_push(&mut self.buf,
                                     Command::CopyBuffer(src,
                                                         dst,
                                                         src_offset_bytes as gl::types::GLintptr,
                                                         dst_offset_bytes as gl::types::GLintptr,
                                                         size_bytes as gl::types::GLsizeiptr));
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset_bytes: usize) {
        let ptr = self.data.add(data);
        self.saved_state.filter_push(&mut self.buf, Command::UpdateBuffer(buf, ptr, offset_bytes));
    }

    fn update_texture(&mut self,
                      ntex: NewTexture,
                      kind: c::texture::Kind,
                      face: Option<c::texture::CubeFace>,
                      data: &[u8],
                      img: c::texture::RawImageInfo) {
        let ptr = self.data.add(data);
        match ntex {
            NewTexture::Texture(t) => {
                self.saved_state.filter_push(&mut self.buf,
                                             Command::UpdateTexture(t, kind, face, ptr, img))
            }
            NewTexture::Surface(s) => {
                error!("GL: unable to update the contents of a Surface({})", s)
            }
        }
    }

    fn generate_mipmap(&mut self, srv: ResourceView) {
        self.saved_state.filter_push(&mut self.buf, Command::GenerateMipmap(srv));
    }

    fn clear_color(&mut self, target: TargetView, value: command::ClearColor) {
        // this could be optimized by deferring the actual clear call
        let mut pts = c::pso::PixelTargetSet::new();
        pts.colors[0] = Some(target);
        self.bind_pixel_targets(pts);
        self.saved_state.filter_push(&mut self.buf, Command::Clear(Some(value), None, None));
    }

    fn clear_depth_stencil(&mut self,
                           target: TargetView,
                           depth: Option<Depth>,
                           stencil: Option<Stencil>) {
        let mut pts = c::pso::PixelTargetSet::new();
        if depth.is_some() {
            pts.depth = Some(target);
        }
        if stencil.is_some() {
            pts.stencil = Some(target);
        }
        self.bind_pixel_targets(pts);
        self.saved_state.filter_push(&mut self.buf, Command::Clear(None, depth, stencil));
    }

    fn call_draw(&mut self,
                 start: c::VertexCount,
                 count: c::VertexCount,
                 instances: Option<command::InstanceParams>) {
        self.saved_state.filter_push(&mut self.buf,
                                     Command::Draw(self.cache.primitive, start, count, instances));
    }

    fn call_draw_indexed(&mut self,
                         start: c::VertexCount,
                         count: c::VertexCount,
                         base: c::VertexCount,
                         instances: Option<command::InstanceParams>) {
        let (offset, gl_index) = match self.cache.index_type {
            c::IndexType::U16 => (start * 2u32, gl::UNSIGNED_SHORT),
            c::IndexType::U32 => (start * 4u32, gl::UNSIGNED_INT),
        };
        self.saved_state
            .filter_push(&mut self.buf,
                         Command::DrawIndexed(self.cache.primitive,
                                              gl_index,
                                              RawOffset(offset as *const gl::types::GLvoid),
                                              count,
                                              base,
                                              instances));
    }
}
