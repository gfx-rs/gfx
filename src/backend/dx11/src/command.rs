#![allow(missing_docs)]

use std::ptr;
use winapi::{FLOAT, INT, UINT, UINT8, DXGI_FORMAT,
             DXGI_FORMAT_R16_UINT, DXGI_FORMAT_R32_UINT,
             D3D11_CLEAR_FLAG, D3D11_PRIMITIVE_TOPOLOGY, D3D11_VIEWPORT, D3D11_RECT,
             ID3D11RasterizerState, ID3D11DepthStencilState, ID3D11BlendState};
use core::{command, pso, shade, state, target, texture as tex};
use core::{IndexType, VertexCount};
use core::{MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS,
           MAX_RESOURCE_VIEWS, MAX_UNORDERED_VIEWS,
           MAX_SAMPLERS, MAX_COLOR_TARGETS};
use {native, Backend, CommandList, Resources, InputLayout, Buffer, Texture, Pipeline, Program};

#[derive(Clone)]
pub struct SubmitInfo<P> {
    pub parser: P,
}

/// The place of some data in the data buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DataPointer {
    offset: u32,
    size: u32,
}

#[derive(Clone)]
pub struct DataBuffer(Vec<u8>);
impl DataBuffer {
    /// Create a new empty data buffer.
    pub fn new() -> DataBuffer {
        DataBuffer(Vec::new())
    }
    /// Reset the contents.
    pub fn reset(&mut self) {
        self.0.clear();
    }
    /// Copy a given vector slice into the buffer.
    pub fn add(&mut self, data: &[u8]) -> DataPointer {
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
    BindInputLayout(InputLayout),
    BindIndex(Buffer, DXGI_FORMAT),
    BindVertexBuffers([native::Buffer; MAX_VERTEX_ATTRIBUTES], [UINT; MAX_VERTEX_ATTRIBUTES], [UINT; MAX_VERTEX_ATTRIBUTES]),
    BindConstantBuffers(shade::Stage, [native::Buffer; MAX_CONSTANT_BUFFERS]),
    BindShaderResources(shade::Stage, [native::Srv; MAX_RESOURCE_VIEWS]),
    BindSamplers(shade::Stage, [native::Sampler; MAX_SAMPLERS]),
    BindPixelTargets([native::Rtv; MAX_COLOR_TARGETS], native::Dsv),
    SetPrimitive(D3D11_PRIMITIVE_TOPOLOGY),
    SetViewport(D3D11_VIEWPORT),
    SetScissor(D3D11_RECT),
    SetRasterizer(*const ID3D11RasterizerState),
    SetDepthStencil(*const ID3D11DepthStencilState, UINT),
    SetBlend(*const ID3D11BlendState, [FLOAT; 4], UINT),
    CopyBuffer(Buffer, Buffer, UINT, UINT, UINT),
    // resource updates
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(Texture, tex::Kind, Option<tex::CubeFace>, DataPointer, tex::RawImageInfo),
    GenerateMips(native::Srv),
    // drawing
    ClearColor(native::Rtv, [f32; 4]),
    ClearDepthStencil(native::Dsv, D3D11_CLEAR_FLAG, FLOAT, UINT8),
    Draw(UINT, UINT),
    DrawInstanced(UINT, UINT, UINT, UINT),
    DrawIndexed(UINT, UINT, INT),
    DrawIndexedInstanced(UINT, UINT, UINT, INT, UINT),
}

unsafe impl Send for Command {}

struct Cache {
    attrib_strides: [Option<pso::ElemStride>; MAX_VERTEX_ATTRIBUTES],
    rasterizer: *const ID3D11RasterizerState,
    depth_stencil: *const ID3D11DepthStencilState,
    stencil_ref: UINT,
    blend: *const ID3D11BlendState,
    blend_ref: [FLOAT; 4],
}
unsafe impl Send for Cache {}

impl Cache {
    fn new() -> Cache {
        Cache {
            attrib_strides: [None; MAX_VERTEX_ATTRIBUTES],
            rasterizer: ptr::null(),
            depth_stencil: ptr::null(),
            stencil_ref: 0,
            blend: ptr::null(),
            blend_ref: [0.0; 4],
        }
    }
}

pub struct RawCommandBuffer<P> {
    pub parser: P,
    cache: Cache,
}

impl command::CommandBuffer<Backend> for RawCommandBuffer<CommandList> {
    unsafe fn end(&mut self) -> SubmitInfo<CommandList> {
        SubmitInfo {
            parser: self.parser.clone(), // TODO: slow
        }
    }
}
pub trait Parser: Sized + Send {
    fn reset(&mut self);
    fn parse(&mut self, cmd: Command);
    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize);
    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>, data: &[u8], image: tex::RawImageInfo);
}

impl<P: Parser> From<P> for RawCommandBuffer<P> {
    fn from(parser: P) -> RawCommandBuffer<P> {
        RawCommandBuffer {
            parser: parser,
            cache: Cache::new(),
        }
    }
}

impl<P: Parser> RawCommandBuffer<P> {
    fn flush(&mut self) {
        let sample_mask = !0; //TODO
        self.parser.parse(Command::SetDepthStencil(self.cache.depth_stencil, self.cache.stencil_ref));
        self.parser.parse(Command::SetBlend(self.cache.blend, self.cache.blend_ref, sample_mask));
    }
}

impl<P: 'static + Parser> command::Buffer<Resources> for RawCommandBuffer<P> {
    fn reset(&mut self) {
        self.parser.reset();
        self.cache = Cache::new();
    }

    fn bind_pipeline_state(&mut self, pso: Pipeline) {
        self.parser.parse(Command::SetPrimitive(pso.topology));
        for (stride, ad_option) in self.cache.attrib_strides.iter_mut().zip(pso.attributes.iter()) {
            *stride = ad_option.map(|(buf_id, _)| match pso.vertex_buffers[buf_id as usize] {
                Some(ref bdesc) => bdesc.stride,
                None => {
                    error!("Unexpected use of buffer id {}", buf_id);
                    0
                },
            });
        }
        if self.cache.rasterizer != pso.rasterizer {
            self.cache.rasterizer = pso.rasterizer;
            self.parser.parse(Command::SetRasterizer(pso.rasterizer));
        }
        self.cache.depth_stencil = pso.depth_stencil;
        self.cache.blend = pso.blend;
        self.parser.parse(Command::BindInputLayout(pso.layout));
        self.parser.parse(Command::BindProgram(pso.program));
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        //Note: assumes `bind_pipeline_state` is called prior
        let mut buffers = [native::Buffer(ptr::null_mut()); MAX_VERTEX_ATTRIBUTES];
        let mut strides = [0; MAX_VERTEX_ATTRIBUTES];
        let mut offsets = [0; MAX_VERTEX_ATTRIBUTES];
        for i in 0 .. MAX_VERTEX_ATTRIBUTES {
            match (vbs.0[i], self.cache.attrib_strides[i]) {
                (None, Some(stride)) => {
                    error!("No vertex input provided for slot {} with stride {}", i, stride)
                },
                (Some((buffer, offset)), Some(stride)) => {
                    buffers[i] = buffer.0;
                    strides[i] = stride as UINT;
                    offsets[i] = offset as UINT;
                },
                (_, None) => (),
            }
        }
        self.parser.parse(Command::BindVertexBuffers(buffers, strides, offsets));
    }

    fn bind_constant_buffers(&mut self, cbs: &[pso::ConstantBufferParam<Resources>]) {
        for &stage in shade::STAGES.iter() {
            let mut buffers = [native::Buffer(ptr::null_mut()); MAX_CONSTANT_BUFFERS];
            let mask = stage.into();
            let mut count = 0;
            for cbuf in cbs.iter() {
                if cbuf.1.contains(mask) {
                    buffers[cbuf.2 as usize] = (cbuf.0).0;
                    count += 1;
                }
            }
            if count != 0 {
                self.parser.parse(Command::BindConstantBuffers(stage, buffers));
            }
        }
    }

    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {
        error!("Global constants are not supported");
    }

    fn bind_resource_views(&mut self, rvs: &[pso::ResourceViewParam<Resources>]) {
        for &stage in shade::STAGES.iter() {
            let mut views = [native::Srv(ptr::null_mut()); MAX_RESOURCE_VIEWS];
            let mask = stage.into();
            let mut count = 0;
            for view in rvs.iter() {
                if view.1.contains(mask) {
                    views[view.2 as usize] = view.0;
                    count += 1;
                }
            }
            if count != 0 {
                self.parser.parse(Command::BindShaderResources(stage, views));
            }
        }
    }

    fn bind_unordered_views(&mut self, uvs: &[pso::UnorderedViewParam<Resources>]) {
        let mut views = [(); MAX_UNORDERED_VIEWS];
        let mut count = 0;
        for view in uvs.iter() {
            views[view.2 as usize] = view.0;
            count += 1;
        }
        if count != 0 {
            unimplemented!()
            //self.parser.parse(Command::BindUnorderedAccess(stage, views));
        }
    }

    fn bind_samplers(&mut self, ss: &[pso::SamplerParam<Resources>]) {
        for &stage in shade::STAGES.iter() {
            let mut samplers = [native::Sampler(ptr::null_mut()); MAX_SAMPLERS];
            let mask = stage.into();
            let mut count = 0;
            for sm in ss.iter() {
                if sm.1.contains(mask) {
                    samplers[sm.2 as usize] = sm.0;
                    count += 1;
                }
            }
            if count != 0 {
                self.parser.parse(Command::BindSamplers(stage, samplers));
            }
        }
    }

    fn bind_pixel_targets(&mut self, pts: pso::PixelTargetSet<Resources>) {
        if let (Some(ref d), Some(ref s)) = (pts.depth, pts.stencil) {
            if d != s {
                error!("Depth and stencil views have to be the same");
            }
        }

        let view = pts.get_view();
        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: view.0 as f32,
            Height: view.1 as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        let mut colors = [native::Rtv(ptr::null_mut()); MAX_COLOR_TARGETS];
        for i in 0 .. MAX_COLOR_TARGETS {
            if let Some(c) = pts.colors[i] {
                colors[i] = c;
            }
        }
        let ds = pts.depth.unwrap_or(native::Dsv(ptr::null_mut()));
        self.parser.parse(Command::BindPixelTargets(colors, ds));
        self.parser.parse(Command::SetViewport(viewport));
    }

    fn bind_index(&mut self, buf: Buffer, itype: IndexType) {
        let format = match itype {
            IndexType::U16 => DXGI_FORMAT_R16_UINT,
            IndexType::U32 => DXGI_FORMAT_R32_UINT,
        };
        self.parser.parse(Command::BindIndex(buf, format));
    }

    fn set_scissor(&mut self, rect: target::Rect) {
        self.parser.parse(Command::SetScissor(D3D11_RECT {
            left: rect.x as INT,
            top: rect.y as INT,
            right: (rect.x + rect.w) as INT,
            bottom: (rect.y + rect.h) as INT,
        }));
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        if rv.stencil.0 != rv.stencil.1 {
            error!("Unable to set different stencil ref values for front ({}) and back ({})",
                rv.stencil.0, rv.stencil.1);
        }
        self.cache.stencil_ref = rv.stencil.0 as UINT;
        self.cache.blend_ref = rv.blend;
    }

    fn copy_buffer(&mut self, src: Buffer, dst: Buffer,
                   src_offset_bytes: usize, dst_offset_bytes: usize,
                   size_bytes: usize) {
        self.parser.parse(Command::CopyBuffer(src, dst,
                                              src_offset_bytes as UINT,
                                              dst_offset_bytes as UINT,
                                              size_bytes as UINT));
    }

    #[allow(unused_variables)]
    fn copy_buffer_to_texture(&mut self, src: Buffer, src_offset_bytes: usize,
                              dst: Texture,
                              kind: tex::Kind,
                              face: Option<tex::CubeFace>,
                              img: tex::RawImageInfo) {
        unimplemented!()
    }

    #[allow(unused_variables)]
    fn copy_texture_to_buffer(&mut self,
                              src: Texture,
                              kind: tex::Kind,
                              face: Option<tex::CubeFace>,
                              img: tex::RawImageInfo,
                              dst: Buffer, dst_offset_bytes: usize) {
        unimplemented!()
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        self.parser.update_buffer(buf, data, offset);
    }

    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>,
                      data: &[u8], image: tex::RawImageInfo) {
        self.parser.update_texture(tex, kind, face, data, image);
    }

    fn generate_mipmap(&mut self, srv: native::Srv) {
        self.parser.parse(Command::GenerateMips(srv));
    }

    fn clear_color(&mut self, target: native::Rtv, value: command::ClearColor) {
        match value {
            command::ClearColor::Float(data) => {
                self.parser.parse(Command::ClearColor(target, data));
            },
            _ => {
                error!("Unable to clear int/uint target");
            },
        }
    }

    fn clear_depth_stencil(&mut self, target: native::Dsv, depth: Option<target::Depth>,
                           stencil: Option<target::Stencil>) {
        let flags = //warning: magic constants ahead
            D3D11_CLEAR_FLAG(if depth.is_some() {1} else {0}) |
            D3D11_CLEAR_FLAG(if stencil.is_some() {2} else {0});
        self.parser.parse(Command::ClearDepthStencil(target, flags,
                      depth.unwrap_or_default() as FLOAT,
                      stencil.unwrap_or_default() as UINT8
        ));
    }

    fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<command::InstanceParams>) {
        self.flush();
        self.parser.parse(match instances {
            Some((ninst, offset)) => Command::DrawInstanced(
                count as UINT, ninst as UINT, start as UINT, offset as UINT),
            None => Command::Draw(count as UINT, start as UINT),
        });
    }

    fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                         base: VertexCount, instances: Option<command::InstanceParams>) {
        self.flush();
        self.parser.parse(match instances {
            Some((ninst, offset)) => Command::DrawIndexedInstanced(
                count as UINT, ninst as UINT, start as UINT, base as INT, offset as UINT),
            None => Command::DrawIndexed(count as UINT, start as UINT, base as INT),
        });
    }
}
