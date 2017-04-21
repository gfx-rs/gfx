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
use {Buffer, BufferElement, Program, FrameBuffer, Texture,
     NewTexture, Resources, PipelineState, ResourceView, TargetView};


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
        //TriangleFan => gl::TRIANGLE_FAN,
        PatchList(_) => gl::PATCHES
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
    CopyBuffer(Buffer, Buffer,
               gl::types::GLintptr, gl::types::GLintptr,
               gl::types::GLsizeiptr),
    CopyBufferToTexture(Buffer, gl::types::GLintptr,
                        Texture,
                        c::texture::Kind,
                        Option<c::texture::CubeFace>,
                        c::texture::RawImageInfo),
    CopyTextureToBuffer(NewTexture,
                        c::texture::Kind,
                        Option<c::texture::CubeFace>,
                        c::texture::RawImageInfo,
                        Buffer, gl::types::GLintptr),
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

pub const RESET: [Command; 14] = [
    Command::BindProgram(0),
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
    Command::SetBlendColor([0f32; 4]),
];

struct Cache {
    primitive: gl::types::GLenum,
    index_type: c::IndexType,
    current_vbs: Option<c::pso::VertexBufferSet<Resources>>,
    attributes: [Option<BufferElement>; c::MAX_VERTEX_ATTRIBUTES],
    resource_binds: [Option<gl::types::GLenum>; c::MAX_RESOURCE_VIEWS],
    scissor: bool,
    target_dim: (u16, u16, u16),
    stencil: Option<s::Stencil>,
    // blend: Option<s::Blend>,
    cull_face: s::CullFace,
    draw_mask: u32,

    program: Program,
    constant_buffer: Option<c::pso::ConstantBufferParam<Resources>>,
    resource_view: Option<c::pso::ResourceViewParam<Resources>>,
    scissor_test: Option<Rect>,
    depth_state: Option<s::Depth>,
    blend_state: Option<(c::ColorSlot, s::Color)>,
    blend_color: Option<ColorValue>,
    viewport: Option<Rect>,
    rasterizer: Option<s::Rasterizer>,
    framebuffer: Option<(Access, FrameBuffer)>,
    index: Buffer,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            primitive: 0,
            index_type: c::IndexType::U16,
            current_vbs: None,
            attributes: [None; c::MAX_VERTEX_ATTRIBUTES],
            resource_binds: [None; c::MAX_RESOURCE_VIEWS],
            scissor: false,
            target_dim: (0, 0, 0),
            stencil: None,
            cull_face: s::CullFace::Nothing,
            // blend: None,
            draw_mask: 0,

            program: 0,
            constant_buffer: None,
            resource_view: None,
            scissor_test: None,
            depth_state: None,
            blend_state: None,
            blend_color: None,
            viewport: None,
            rasterizer: None,
            framebuffer: None,
            index: 0,
        }
    }

    fn bind_program(&mut self, program: Program) -> Option<Command> {
        if program == self.program {
            return None;
        }
        self.program = program;
        Some(Command::BindProgram(program))
    }

    fn bind_constant_buffer(&mut self, constant_buffer: c::pso::ConstantBufferParam<Resources>) -> Option<Command> {
        if self.constant_buffer == Some(constant_buffer) {
            return None;
        }
        self.constant_buffer = Some(constant_buffer);
        Some(Command::BindConstantBuffer(constant_buffer))
    }

    fn bind_resource_view(&mut self, resource_view: c::pso::ResourceViewParam<Resources>) -> Option<Command> {
        if self.resource_view == Some(resource_view) {
            return None;
        }
        self.resource_view = Some(resource_view);
        Some(Command::BindResourceView(resource_view))
    }

    fn bind_index(&mut self, buffer: Buffer, itype: c::IndexType) -> Option<Command> {
        if self.index == buffer && itype == self.index_type {
            return None;
        }
        self.index_type = itype;
        self.index = buffer;
        Some(Command::BindIndex(buffer))
    }

    fn bind_framebuffer(&mut self, access: Access, fb: FrameBuffer) -> Option<Command> {
        if self.framebuffer == Some((access, fb)) {
            return None;
        }
        self.framebuffer = Some((access, fb));
        Some(Command::BindFrameBuffer(access, fb))
    }

    fn set_rasterizer(&mut self, rasterizer: s::Rasterizer) -> Option<Command> {
        if self.rasterizer == Some(rasterizer) {
            return None;
        }
        self.rasterizer = Some(rasterizer);
        Some(Command::SetRasterizer(rasterizer))
    }

    fn set_viewport(&mut self, rect: Rect) -> Option<Command> {
        if self.viewport == Some(rect) {
            return None;
        }
        self.viewport = Some(rect);
        Some(Command::SetViewport(rect))
    }

    fn set_scissor(&mut self, rect: Option<Rect>) -> Option<Command> {
        if self.scissor_test == rect {
            return None;
        }
        self.scissor_test = rect;
        Some(Command::SetScissor(rect))
    }
    fn set_depth_state(&mut self, depth: Option<s::Depth>) -> Option<Command> {
        if self.depth_state == depth {
            return None;
        }
        self.depth_state = depth;
        Some(Command::SetDepthState(depth))
    }
    fn set_stencil_state(&mut self, option_stencil: Option<s::Stencil>, stencils: (Stencil, Stencil), cullface: s:: CullFace) -> Option<Command> {
        // This is a little more complex 'cause if option_stencil
        // is None the stencil state is disabled, it it's Some
        // then it's enabled and parameters are set.
        // That's actually bad because it makes it impossible
        // to completely remove all redundant calls if the
        // stencil is enabled;
        // we'll be re-enabling it over and over.
        // For now though, we just don't handle it.
        Some(Command::SetStencilState(option_stencil, stencils, cullface))
    }
    fn set_blend_state(&mut self, color_slot: c::ColorSlot, color: s::Color) -> Option<Command> {
        if self.blend_state == Some((color_slot, color)) {
            return None;
        }
        self.blend_state = Some((color_slot, color));
        Some(Command::SetBlendState(color_slot, color))
    }
    fn set_blend_color(&mut self, color_value: ColorValue) -> Option<Command> {
        if self.blend_color == Some(color_value) {
            return None;
        }
        self.blend_color = Some(color_value);
        Some(Command::SetBlendColor(color_value))
    }

}

/// A command buffer abstraction for OpenGL.
///
/// Manages a list of commands that will be executed when submitted to a `Device`. Usually it is
/// best to use a `Encoder` to manage the command buffer which implements `From<CommandBuffer>`.
///
/// If you want to display your rendered results to a framebuffer created externally, see the
/// `display_fb` field.
pub struct CommandBuffer {
    pub buf: Vec<Command>,
    pub data: DataBuffer,
    fbo: FrameBuffer,
    /// The framebuffer to use for rendering to the main targets (0 by default).
    ///
    /// Use this to set the framebuffer that will be used for the screen display targets created
    /// with `create_main_targets_raw`. Usually you don't need to set this field directly unless
    /// your OS doesn't provide a default framebuffer with name 0 and you have to render to a
    /// different framebuffer object that can be made visible on the screen (iOS/tvOS need this).
    ///
    /// This framebuffer must exist and be configured correctly (with renderbuffer attachments,
    /// etc.) so that rendering to it can occur immediately.
    pub display_fb: FrameBuffer,
    cache: Cache,
    active_attribs: usize,
}

impl CommandBuffer {
    pub fn new(fbo: FrameBuffer) -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
            data: DataBuffer::new(),
            fbo: fbo,
            display_fb: 0 as FrameBuffer,
            cache: Cache::new(),
            active_attribs: 0,
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
        self.buf.extend(self.cache.bind_program(pso.program));
        self.cache.scissor = pso.scissor;
        self.buf.extend(self.cache.set_rasterizer(pso.rasterizer));
        self.buf.extend(self.cache.set_depth_state(pso.output.depth));
        self.buf.extend(self.cache.set_stencil_state(pso.output.stencil, (0, 0), cull));
        for i in 0..c::MAX_COLOR_TARGETS {
            if pso.output.draw_mask & (1 << i) != 0 {
                self.buf.extend(self.cache.set_blend_state(i as c::ColorSlot,
                                           pso.output.colors[i]));
            }
        }
        if let c::Primitive::PatchList(num) = pso.primitive {
            self.buf.push(Command::SetPatches(num));
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: c::pso::VertexBufferSet<Resources>) {
        if self.cache.current_vbs == Some(vbs) {
            return;
        }
        self.cache.current_vbs = Some(vbs);
        for i in 0..c::MAX_VERTEX_ATTRIBUTES {
            match (vbs.0[i], self.cache.attributes[i]) {
                (None, Some(fm)) => {
                    error!("No vertex input provided for slot {} of format {:?}", i, fm)
                }
                (Some((buffer, offset)), Some(mut bel)) => {
                    bel.elem.offset += offset as gl::types::GLuint;
                    self.buf.push(Command::BindAttribute(
                        i as c::AttributeSlot,
                        buffer,
                        bel));
                    self.active_attribs |= 1 << i;
                }
                (_, None) if self.active_attribs & (1 << i) != 0 => {
                    self.buf.push(Command::UnbindAttribute(i as c::AttributeSlot));
                    self.active_attribs ^= 1 << i;
                }
                (_, None) => (),
            }
        }
    }

    fn bind_constant_buffers(&mut self, cbs: &[c::pso::ConstantBufferParam<Resources>]) {
        for param in cbs.iter() {
            self.buf.extend(self.cache.bind_constant_buffer(param.clone()));
        }
    }

    fn bind_global_constant(&mut self, loc: c::shade::Location, value: c::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_resource_views(&mut self, srvs: &[c::pso::ResourceViewParam<Resources>]) {
        for i in 0..c::MAX_RESOURCE_VIEWS {
            self.cache.resource_binds[i] = None;
        }
        for param in srvs.iter() {
            self.cache.resource_binds[param.2 as usize] = Some(param.0.bind);
            self.buf.extend(self.cache.bind_resource_view(param.clone()));
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
            self.buf.extend(self.cache.bind_framebuffer(gl::DRAW_FRAMEBUFFER, self.display_fb));
        } else {
            let num = pts.colors
                .iter()
                .position(|c| c.is_none())
                .unwrap_or(pts.colors.len()) as c::ColorSlot;
            self.buf.extend(self.cache.bind_framebuffer(gl::DRAW_FRAMEBUFFER, self.fbo));
            self.buf.push(Command::BindPixelTargets(pts));
            self.buf.push(Command::SetDrawColorBuffers(num));
        }
        let view = pts.get_view();
        self.cache.target_dim = view;
        self.buf.extend(
            self.cache.set_viewport(Rect {
                                    x: 0,
                                    y: 0,
                                    w: view.0,
                                    h: view.1,
                                }));
    }

    fn bind_index(&mut self, buf: Buffer, itype: c::IndexType) {
        self.buf.extend(self.cache.bind_index(buf, itype));
    }

    fn set_scissor(&mut self, rect: Rect) {
        use std::cmp;
        let scissor = self.cache.scissor;
        let target_dim = self.cache.target_dim;
        let scissor_rect = if scissor {
           Some(Rect {
               // inverting the Y axis in order to match D3D11
               y: cmp::max(target_dim.1, rect.y + rect.h) -
                   rect.y -
                   rect.h,
               ..rect
           })
       } else {
            None //TODO: assert?
       };
        self.buf.extend(self.cache.set_scissor(scissor_rect));
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        let stencil = self.cache.stencil;
        let cull_face = self.cache.cull_face;
        self.buf.extend(self.cache.set_stencil_state(stencil,
                                                     rv.stencil,
                                                     cull_face));
        self.buf.extend(self.cache.set_blend_color(rv.blend));
    }

    fn copy_buffer(&mut self,
                   src: Buffer,
                   dst: Buffer,
                   src_offset_bytes: usize,
                   dst_offset_bytes: usize,
                   size_bytes: usize) {
        self.buf.push(Command::CopyBuffer(src, dst,
                                          src_offset_bytes as gl::types::GLintptr,
                                          dst_offset_bytes as gl::types::GLintptr,
                                          size_bytes as gl::types::GLsizeiptr));
    }

    fn copy_buffer_to_texture(&mut self,
                              src: Buffer, src_offset_bytes: usize,
                              dst: NewTexture,
                              kind: c::texture::Kind,
                              face: Option<c::texture::CubeFace>,
                              img: c::texture::RawImageInfo) {
        match dst {
            NewTexture::Texture(t) =>
                self.buf.push(Command::CopyBufferToTexture(
                    src, src_offset_bytes as gl::types::GLintptr,
                    t, kind, face, img
                )),
            NewTexture::Surface(s) =>
                error!("GL: Cannot copy to a Surface({})", s)
        }
    }

    fn copy_texture_to_buffer(&mut self,
                              src: NewTexture,
                              kind: c::texture::Kind,
                              face: Option<c::texture::CubeFace>,
                              img: c::texture::RawImageInfo,
                              dst: Buffer, dst_offset_bytes: usize) {
        self.buf.push(Command::CopyTextureToBuffer(
            src, kind, face, img,
            dst, dst_offset_bytes as gl::types::GLintptr
        ));
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset_bytes: usize) {
        let ptr = self.data.add(data);
        self.buf.push(Command::UpdateBuffer(buf, ptr, offset_bytes));
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
                self.buf.push(Command::UpdateTexture(t, kind, face, ptr, img))
            }
            NewTexture::Surface(s) => {
                error!("GL: unable to update the contents of a Surface({})", s)
            }
        }
    }

    fn generate_mipmap(&mut self, srv: ResourceView) {
        self.buf.push(Command::GenerateMipmap(srv));
    }

    fn clear_color(&mut self, target: TargetView, value: command::ClearColor) {
        // this could be optimized by deferring the actual clear call
        let mut pts = c::pso::PixelTargetSet::new();
        pts.colors[0] = Some(target);
        self.bind_pixel_targets(pts);
        self.buf.push(Command::Clear(Some(value), None, None));
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
        self.buf.push(Command::Clear(None, depth, stencil));
    }

    fn call_draw(&mut self,
                 start: c::VertexCount,
                 count: c::VertexCount,
                 instances: Option<command::InstanceParams>) {
        self.buf.push(Command::Draw(self.cache.primitive, start, count, instances));
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
        self.buf.push(
                  Command::DrawIndexed(
                      self.cache.primitive,
                      gl_index,
                      RawOffset(offset as *const gl::types::GLvoid),
                      count,
                      base,
                      instances));
    }
}
