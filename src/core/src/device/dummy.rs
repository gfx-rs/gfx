#![allow(missing_docs)]
use std::slice;

use device as d;
use device::{Device, Resources, Capabilities, SubmitInfo};
use device::draw::{CommandBuffer, Access, Gamma, Target};
use draw_state::target::{Rect, Mirror, Mask, ClearData, Layer, Level};

pub struct DummyDevice {
    capabilities: Capabilities
}
pub struct DummyCommandBuffer {
    buf: Vec<Command>
}

impl DummyCommandBuffer {
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Command> {
        self.buf.iter()
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DummyResources{}

pub type Buffer         = u32;
pub type ArrayBuffer    = u32;
pub type Shader         = u32;
pub type Program        = u32;
pub type FrameBuffer    = u32;
pub type Surface        = u32;
pub type Sampler        = u32;
pub type Texture        = u32;

impl Resources for DummyResources {
    type Buffer         = Buffer;
    type ArrayBuffer    = ArrayBuffer;
    type Shader         = Shader;
    type Program        = Program;
    type FrameBuffer    = FrameBuffer;
    type Surface        = Surface;
    type Texture        = Texture;
    type Sampler        = Sampler;
}

#[derive(Copy, Clone, Debug)]
pub enum Command {
    BindProgram(Program),
    BindArrayBuffer(ArrayBuffer),
    BindAttribute(d::AttributeSlot, Buffer, d::attrib::Format),
    BindIndex(Buffer),
    BindFrameBuffer(Access, FrameBuffer, Gamma),
    UnbindTarget(Access, Target),
    BindTargetSurface(Access, Target, Surface),
    BindTargetTexture(Access, Target, Texture, Level, Option<Layer>),
    BindUniformBlock(Program, d::UniformBufferSlot, d::UniformBlockIndex,
                     Buffer),
    BindUniform(d::shade::Location, d::shade::UniformValue),
    BindTexture(d::TextureSlot, d::tex::Kind, Texture,
                Option<(Sampler, d::tex::SamplerInfo)>),
    SetDrawColorBuffers(usize),
    SetPrimitiveState(d::state::Primitive),
    SetViewport(Rect),
    SetMultiSampleState(Option<d::state::MultiSample>),
    SetScissor(Option<Rect>),
    SetDepthStencilState(Option<d::state::Depth>, Option<d::state::Stencil>,
                         d::state::CullFace),
    SetBlendState(Option<d::state::Blend>),
    SetColorMask(d::state::ColorMask),
    UpdateBuffer(Buffer, d::draw::DataPointer, usize),
    UpdateTexture(d::tex::Kind, Texture, d::tex::ImageInfo,
                  d::draw::DataPointer),
    // drawing
    Clear(ClearData, Mask),
    Draw(d::PrimitiveType, d::VertexCount, d::VertexCount,
         d::draw::InstanceOption),
    DrawIndexed(d::PrimitiveType, d::IndexType, d::VertexCount, d::VertexCount,
                d::VertexCount, d::draw::InstanceOption),
    Blit(Rect, Rect, Mirror, Mask),
}


impl CommandBuffer<DummyResources> for DummyCommandBuffer {
    fn new() -> DummyCommandBuffer {
        DummyCommandBuffer {
            buf: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
    }

    fn bind_program(&mut self, prog: Program) {
        self.buf.push(Command::BindProgram(prog));
    }

    fn bind_array_buffer(&mut self, vao: ArrayBuffer) {
        self.buf.push(Command::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buf: Buffer,
                      format: d::attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: Access, fbo: FrameBuffer,
                         gamma: Gamma) {
        self.buf.push(Command::BindFrameBuffer(access, fbo, gamma));
    }

    fn unbind_target(&mut self, access: Access, tar: Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: Access, tar: Target,
                           suf: Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: Access, tar: Target,
                           tex: Texture, level: Level, layer: Option<Layer>) {
        self.buf.push(Command::BindTargetTexture(
            access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: Program, slot: d::UniformBufferSlot,
                          index: d::UniformBlockIndex, buf: Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: d::shade::Location,
                    value: d::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }
    fn bind_texture(&mut self, slot: d::TextureSlot, kind: d::tex::Kind,
                    tex: Texture,
                    sampler: Option<(Sampler, d::tex::SamplerInfo)>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: usize) {
        self.buf.push(Command::SetDrawColorBuffers(num));
    }

    fn set_primitive(&mut self, prim: d::state::Primitive) {
        self.buf.push(Command::SetPrimitiveState(prim));
    }

    fn set_viewport(&mut self, view: Rect) {
        self.buf.push(Command::SetViewport(view));
    }

    fn set_multi_sample(&mut self, ms: Option<d::state::MultiSample>) {
        self.buf.push(Command::SetMultiSampleState(ms));
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<d::state::Depth>,
                         stencil: Option<d::state::Stencil>,
                         cull: d::state::CullFace) {
        self.buf.push(Command::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, blend: Option<d::state::Blend>) {
        self.buf.push(Command::SetBlendState(blend));
    }

    fn set_color_mask(&mut self, mask: d::state::ColorMask) {
        self.buf.push(Command::SetColorMask(mask));
    }

    fn update_buffer(&mut self, buf: Buffer, data: d::draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: d::tex::Kind, tex: Texture,
                      info: d::tex::ImageInfo, data: d::draw::DataPointer) {
        self.buf.push(Command::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: ClearData, mask: Mask) {
        self.buf.push(Command::Clear(data, mask));
    }

    fn call_draw(&mut self, ptype: d::PrimitiveType, start: d::VertexCount,
                 count: d::VertexCount, instances: d::draw::InstanceOption) {
        self.buf.push(Command::Draw(ptype, start, count, instances));
    }

    fn call_draw_indexed(&mut self, ptype: d::PrimitiveType,
                         itype: d::IndexType, start: d::VertexCount,
                         count: d::VertexCount, base: d::VertexCount,
                         instances: d::draw::InstanceOption) {
        self.buf.push(Command::DrawIndexed(
            ptype, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror,
                 mask: Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}

impl DummyDevice {
    fn new(capabilities: Capabilities) -> DummyDevice {
        DummyDevice {
            capabilities: capabilities
        }
    }
}

impl Device for DummyDevice {
    type Resources = DummyResources;
    type CommandBuffer = DummyCommandBuffer;

    fn get_capabilities<'a>(&'a self) -> &'a Capabilities {
        &self.capabilities
    }
    fn reset_state(&mut self) {}
    fn submit(&mut self, (cb, db, handles): SubmitInfo<Self>) {}
    fn cleanup(&mut self) {}
}
