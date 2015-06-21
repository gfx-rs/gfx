#![allow(missing_docs)]
use device as d;
use device::{Device, Resources, Capabilities, SubmitInfo};
use device::draw::{CommandBuffer, Access, Gamma, Target};
use device::shade;
use super::{tex};
use draw_state::target::{Rect, Mirror, Mask, ClearData, Layer, Level};

pub struct DummyDevice {
    capabilities: Capabilities
}
pub struct DummyCommandBuffer {
    buf: Vec<String>
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
        self.buf.push("bind_program".to_string());
    }

    fn bind_array_buffer(&mut self, vao: ArrayBuffer) {
        self.buf.push("bind_array_buffer".to_string());
    }

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buf: Buffer,
                      format: d::attrib::Format) {
        self.buf.push("bind_attribute".to_string());
    }

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push("bind_index".to_string());
    }

    fn bind_frame_buffer(&mut self, access: Access, fbo: FrameBuffer,
                         gamma: Gamma) {
        self.buf.push("bind_frame_buffer".to_string());
    }

    fn unbind_target(&mut self, access: Access, tar: Target) {
        self.buf.push("unbind_target".to_string());
    }

    fn bind_target_surface(&mut self, access: Access, tar: Target,
                           suf: Surface) {
        self.buf.push("bind_target_surface".to_string());
    }

    fn bind_target_texture(&mut self, access: Access, tar: Target,
                           tex: Texture, level: Level, layer: Option<Layer>) {
        self.buf.push("bind_target_texture".to_string());
    }

    fn bind_uniform_block(&mut self, prog: Program, slot: d::UniformBufferSlot,
                          index: d::UniformBlockIndex, buf: Buffer) {
        self.buf.push("bind_uniform_block".to_string());
    }

    fn bind_uniform(&mut self, loc: d::shade::Location,
                    value: d::shade::UniformValue) {
        self.buf.push("bind_uniform".to_string());
    }
    fn bind_texture(&mut self, slot: d::TextureSlot, kind: d::tex::Kind,
                    tex: Texture,
                    sampler: Option<(Sampler, d::tex::SamplerInfo)>) {
        self.buf.push("set_draw_color_buffers".to_string());
    }

    fn set_draw_color_buffers(&mut self, num: usize) {
        self.buf.push("set_draw_color_buffers".to_string());
    }

    fn set_primitive(&mut self, prim: d::state::Primitive) {
        self.buf.push("set_primitive".to_string());
    }

    fn set_viewport(&mut self, view: Rect) {
        self.buf.push("set_viewport".to_string());
    }

    fn set_multi_sample(&mut self, ms: Option<d::state::MultiSample>) {
        self.buf.push("set_multi_sample".to_string());
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push("set_scissor".to_string());
    }

    fn set_depth_stencil(&mut self, depth: Option<d::state::Depth>,
                         stencil: Option<d::state::Stencil>,
                         cull: d::state::CullFace) {
        self.buf.push("set_depth_stencil".to_string());
    }

    fn set_blend(&mut self, blend: Option<d::state::Blend>) {
        self.buf.push("set_blend".to_string());
    }

    fn set_color_mask(&mut self, mask: d::state::ColorMask) {
        self.buf.push("set_color_mask".to_string());
    }

    fn update_buffer(&mut self, buf: Buffer, data: d::draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push("update_buffer".to_string());
    }

    fn update_texture(&mut self, kind: d::tex::Kind, tex: Texture,
                      info: d::tex::ImageInfo, data: d::draw::DataPointer) {
        self.buf.push("update_texture".to_string());
    }

    fn call_clear(&mut self, data: ClearData, mask: Mask) {
        self.buf.push("call_clear".to_string());
    }

    fn call_draw(&mut self, ptype: d::PrimitiveType, start: d::VertexCount,
                 count: d::VertexCount, instances: d::draw::InstanceOption) {
        self.buf.push("call_draw".to_string());
    }

    fn call_draw_indexed(&mut self, ptype: d::PrimitiveType,
                         itype: d::IndexType, start: d::VertexCount,
                         count: d::VertexCount, base: d::VertexCount,
                         instances: d::draw::InstanceOption) {
        self.buf.push("call_draw_indexed".to_string());
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror,
                 mask: Mask) {
        self.buf.push("call_blit".to_string());
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
