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

use std::rc::Rc;
use std::slice;

use {gl, tex};
use gfx_core as d;
use gfx_core::factory as f;
use gfx_core::factory::Phantom;
use gfx_core::format::ChannelType;
use gfx_core::handle;
use gfx_core::handle::Producer;
use gfx_core::mapping::Builder;
use gfx_core::target::{Layer, Level};
use gfx_core::tex as t;

use command::CommandBuffer;
use {Resources as R, Share, OutputMerger};
use {Buffer, FatSampler, NewTexture, PipelineState, ResourceView, TargetView};


fn role_to_target(role: f::BufferRole) -> gl::types::GLenum {
    match role {
        f::BufferRole::Vertex  => gl::ARRAY_BUFFER,
        f::BufferRole::Index   => gl::ELEMENT_ARRAY_BUFFER,
        f::BufferRole::Uniform => gl::UNIFORM_BUFFER,
    }
}

pub fn update_sub_buffer(gl: &gl::Gl, buffer: Buffer, address: *const u8,
                         size: usize, offset: usize, role: f::BufferRole) {
    let target = role_to_target(role);
    unsafe {
        gl.BindBuffer(target, buffer);
        gl.BufferSubData(target,
            offset as gl::types::GLintptr,
            size as gl::types::GLsizeiptr,
            address as *const gl::types::GLvoid
        );
    }
}

/// A placeholder for a real `Output` implemented by your window.
pub struct Output {
    /// render frame width.
    pub width: t::Size,
    /// render frame height.
    pub height: t::Size,
    /// main FBO handle
    handle: handle::FrameBuffer<R>,
}

impl d::output::Output<R> for Output {
    fn get_handle(&self) -> Option<&handle::FrameBuffer<R>> {
        Some(&self.handle)
    }

    fn get_size(&self) -> (t::Size, t::Size) {
        (self.width, self.height)
    }

    fn get_mask(&self) -> d::target::Mask {
        d::target::COLOR | d::target::DEPTH | d::target::STENCIL
    }
}

/// GL resource factory.
pub struct Factory {
    share: Rc<Share>,
    frame_handles: handle::Manager<R>,
}

impl Clone for Factory {
    fn clone(&self) -> Factory {
        Factory {
            share: self.share.clone(),
            frame_handles: handle::Manager::new(),
        }
    }
}

impl Factory {
    /// Create a new `Factory`.
    pub fn new(share: Rc<Share>) -> Factory {
        Factory {
            share: share,
            frame_handles: handle::Manager::new(),
        }
    }

    fn create_fbo_internal(&mut self) -> gl::types::GLuint {
        let gl = &self.share.context;
        let mut name = 0 as ::FrameBuffer;
        unsafe {
            gl.GenFramebuffers(1, &mut name);
        }
        info!("\tCreated frame buffer {}", name);
        name
    }

    fn create_buffer_internal(&mut self) -> Buffer {
        let gl = &self.share.context;
        let mut name = 0 as Buffer;
        unsafe {
            gl.GenBuffers(1, &mut name);
        }
        info!("\tCreated buffer {}", name);
        name
    }

    fn init_buffer(&mut self, buffer: Buffer, info: &f::BufferInfo) {
        let gl = &self.share.context;
        let target = role_to_target(info.role);
        let usage = match info.usage {
            f::BufferUsage::Const   => gl::STATIC_DRAW,
            f::BufferUsage::Dynamic => gl::DYNAMIC_DRAW,
            f::BufferUsage::Stream  => gl::STREAM_DRAW,
        };
        unsafe {
            gl.BindBuffer(target, buffer);
            gl.BufferData(target,
                info.size as gl::types::GLsizeiptr,
                0 as *const gl::types::GLvoid,
                usage
            );
        }
    }

    pub fn create_program_raw(&mut self, shader_set: &d::ShaderSet<R>)
                              -> Result<(gl::types::GLuint, d::shade::ProgramInfo), d::shade::CreateProgramError> {
        let frame_handles = &mut self.frame_handles;
        let mut shaders = [0; 3];
        let shader_slice = match shader_set {
            &d::ShaderSet::Simple(ref vs, ref ps) => {
                shaders[0] = *vs.reference(frame_handles);
                shaders[1] = *ps.reference(frame_handles);
                &shaders[..2]
            },
            &d::ShaderSet::Geometry(ref vs, ref gs, ref ps) => {
                shaders[0] = *vs.reference(frame_handles);
                shaders[1] = *gs.reference(frame_handles);
                shaders[2] = *ps.reference(frame_handles);
                &shaders[..3]
            },
        };
        ::shade::create_program(&self.share.context,
                                &self.share.capabilities,
                                shader_slice)
    }

    fn view_texture_as_target(&mut self, htex: &handle::RawTexture<R>, level: Level, layer: Option<Layer>)
                              -> Result<TargetView, f::TargetViewError> {
        match (self.frame_handles.ref_new_texture(htex), layer) {
            (&NewTexture::Surface(_), Some(_)) => Err(f::TargetViewError::Unsupported),
            (&NewTexture::Surface(_), None) if level != 0 => Err(f::TargetViewError::Unsupported),
            (&NewTexture::Surface(s), None) => Ok(TargetView::Surface(s)),
            (&NewTexture::Texture(t), Some(l)) => Ok(TargetView::TextureLayer(t, level, l)),
            (&NewTexture::Texture(t), None) => Ok(TargetView::Texture(t, level)),
        }
    }

    pub fn get_main_frame_buffer(&self) -> handle::FrameBuffer<R> {
        self.share.main_fbo.clone()
    }

    pub fn make_fake_output(&self, w: t::Size, h: t::Size) -> Output {
        Output {
            width: w,
            height: h,
            handle: self.get_main_frame_buffer(),
        }
    }
}


#[allow(raw_pointer_derive)]
#[derive(Copy, Clone)]
pub struct RawMapping {
    pub pointer: *mut ::std::os::raw::c_void,
    target: gl::types::GLenum,
}

impl d::mapping::Raw for RawMapping {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn to_slice<T>(&self, len: usize) -> &[T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn to_mut_slice<T>(&self, len: usize) -> &mut [T] {
        slice::from_raw_parts_mut(self.pointer as *mut T, len)
    }
}


impl d::Factory<R> for Factory {
    type CommandBuffer = CommandBuffer;
    type Mapper = RawMapping;

    fn get_capabilities(&self) -> &d::Capabilities {
        &self.share.capabilities
    }

    fn create_command_buffer(&mut self) -> CommandBuffer {
        CommandBuffer::new(self.create_fbo_internal())
    }

    fn create_buffer_raw(&mut self, size: usize, role: f::BufferRole, usage: f::BufferUsage)
                         -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();
        let info = f::BufferInfo {
            role: role,
            usage: usage,
            size: size,
        };
        self.init_buffer(name, &info);
        self.share.handles.borrow_mut().make_buffer(name, info)
    }

    fn create_buffer_static_raw(&mut self, data: &[u8], role: f::BufferRole)
                                -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();

        let info = f::BufferInfo {
            role: role,
            usage: f::BufferUsage::Const,
            size: data.len(),
        };
        self.init_buffer(name, &info);
        update_sub_buffer(&self.share.context, name, data.as_ptr(), data.len(), 0, role);
        self.share.handles.borrow_mut().make_buffer(name, info)
    }

    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, f::NotSupported> {
        if self.share.capabilities.array_buffer_supported {
            let gl = &self.share.context;
            let mut name = 0 as ::ArrayBuffer;
            unsafe {
                gl.GenVertexArrays(1, &mut name);
            }
            info!("\tCreated array buffer {}", name);
            Ok(self.share.handles.borrow_mut().make_array_buffer(name))
        } else {
            error!("\tArray buffer creation unsupported, ignored");
            Err(f::NotSupported)
        }
    }

    fn create_shader(&mut self, stage: d::shade::Stage, code: &[u8])
                     -> Result<handle::Shader<R>, d::shade::CreateShaderError> {
        ::shade::create_shader(&self.share.context, stage, code)
                .map(|sh| self.share.handles.borrow_mut().make_shader(sh))
    }

    fn create_program(&mut self, shader_set: &d::ShaderSet<R>)
                      -> Result<handle::Program<R>, d::shade::CreateProgramError> {
        self.create_program_raw(shader_set)
            .map(|(name, info)| self.share.handles.borrow_mut().make_program(name, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &handle::Program<R>, desc: &d::pso::Descriptor)
                                 -> Result<handle::RawPipelineState<R>, d::pso::CreationError> {
        use gfx_core::state as s;
        let mut output = OutputMerger {
            draw_mask: 0,
            stencil: desc.depth_stencil.map(|(_, t)| s::Stencil {
                front: t.front.unwrap_or_default(),
                back: t.back.unwrap_or_default(),
            }),
            depth: desc.depth_stencil.and_then(|(_, t)| t.depth),
            blend: [None; d::MAX_COLOR_TARGETS],
        };
        for i in 0 .. d::MAX_COLOR_TARGETS {
            if let Some((_, ref bi)) = desc.color_targets[i] {
                output.draw_mask |= 1<<i;
                if bi.mask != s::MASK_ALL || bi.color.is_some() || bi.alpha.is_some() {
                    output.blend[i] = Some(s::Blend {
                        color: bi.color.unwrap_or_default(),
                        alpha: bi.alpha.unwrap_or_default(),
                        mask: bi.mask,
                    });
                }
            }
        }
        let pso = PipelineState {
            program: *self.frame_handles.ref_program(program),
            primitive: desc.primitive,
            input: desc.attributes,
            rasterizer: desc.rasterizer,
            output: output,
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_frame_buffer(&mut self) -> Result<handle::FrameBuffer<R>, f::NotSupported> {
        if self.share.capabilities.render_targets_supported {
            let name = self.create_fbo_internal();
            Ok(self.share.handles.borrow_mut().make_frame_buffer(name))
        } else {
            error!("No framebuffer objects, can't make a new one!");
            Err(f::NotSupported)
        }
    }

    fn create_surface(&mut self, info: t::SurfaceInfo) ->
                      Result<handle::Surface<R>, t::SurfaceError> {
        if info.format.does_convert_gamma() && !self.share.capabilities.srgb_color_supported {
            return Err(t::SurfaceError::UnsupportedGamma)
        }
        tex::make_surface_old(&self.share.context, &info)
            .map(|suf| self.share.handles.borrow_mut().make_surface(suf, info))
    }

    fn create_texture(&mut self, info: t::TextureInfo) ->
                      Result<handle::Texture<R>, t::TextureError> {
        let caps = &self.share.capabilities;
        if info.levels == 0 {
            return Err(t::TextureError::InvalidInfo(info))
        }
        if info.format.does_convert_gamma() && !caps.srgb_color_supported {
            return Err(t::TextureError::UnsupportedGamma)
        }
        let gl = &self.share.context;
        let name = if caps.immutable_storage_supported {
            tex::make_with_storage_old(gl, &info)
        } else {
            tex::make_without_storage_old(gl, &info)
        };
        name.map(|tex| self.share.handles.borrow_mut().make_texture(tex, info))
    }

    fn create_new_texture_raw(&mut self, desc: t::Descriptor)
                              -> Result<handle::RawTexture<R>, t::Error> {
        use gfx_core::tex::Error;
        let caps = &self.share.capabilities;
        if desc.levels == 0 {
            return Err(Error::Size(0))
        }
        let cty = ChannelType::UintNormalized; //TODO
        let gl = &self.share.context;
        let object = if desc.bind.intersects(f::SHADER_RESOURCE | f::UNORDERED_ACCESS) {
            use gfx_core::tex::TextureError;
            let result = if caps.immutable_storage_supported {
                tex::make_with_storage(gl, &desc, cty)
            } else {
                tex::make_without_storage(gl, &desc, cty)
            };
            match result {
                Ok(name) => NewTexture::Texture(name),
                Err(TextureError::UnsupportedGamma) => return Err(Error::Gamma),
                Err(TextureError::UnsupportedSamples) => {
                    let (_, _, _, aa) = desc.kind.get_dimensions();
                    return Err(Error::Samples(aa));
                },
                Err(_) => return Err(Error::Format(desc.format)),
            }
        }else {
            use gfx_core::tex::SurfaceError;
            let result = tex::make_surface(gl, &desc, cty);
            match result {
                Ok(name) => NewTexture::Surface(name),
                Err(SurfaceError::UnsupportedFormat) => return Err(Error::Format(desc.format)),
                Err(SurfaceError::UnsupportedGamma) => return Err(Error::Gamma),
            }
        };
        Ok(self.share.handles.borrow_mut().make_new_texture(object, desc))
    }

    fn create_new_texture_with_data(&mut self, desc: t::Descriptor, cty: ChannelType, data: &[u8])
                                    -> Result<handle::RawTexture<R>, t::Error> {
        let kind = desc.kind;
        let face = None; //TODO: cubemap slice
        let img = desc.to_image_info(cty, 0);
        let tex = try!(self.create_new_texture_raw(desc));
        match self.frame_handles.ref_new_texture(&tex) {
            &NewTexture::Surface(_) => Err(t::Error::Data(0)),
            &NewTexture::Texture(t) => match tex::update_texture_new(&self.share.context, t, kind, face, &img, data) {
                Ok(_) => Ok(tex),
                Err(_) => Err(t::Error::Data(0)),
            }
        }
    }

    fn view_buffer_as_shader_resource_raw(&mut self, hbuf: &handle::RawBuffer<R>)
                                      -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> {
        let gl = &self.share.context;
        let mut name = 0 as gl::types::GLuint;
        let buf_name = *self.frame_handles.ref_buffer(hbuf);
        let format = gl::R8; //TODO: get from the buffer handle
        unsafe {
            gl.GenTextures(1, &mut name);
            gl.BindTexture(gl::TEXTURE_BUFFER, name);
            gl.TexBuffer(gl::TEXTURE_BUFFER, format, buf_name);
        }
        let view = ResourceView::new_buffer(name);
        Ok(self.share.handles.borrow_mut().make_buffer_srv(view, hbuf))
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &handle::RawBuffer<R>)
                                       -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &handle::RawTexture<R>, _desc: t::ViewDesc)
                                       -> Result<handle::RawShaderResourceView<R>, f::ResourceViewError> {
        match self.frame_handles.ref_new_texture(htex) {
            &NewTexture::Surface(_) => Err(f::ResourceViewError::NoBindFlag),
            &NewTexture::Texture(t) => {
                //TODO: use the view descriptor
                let view = ResourceView::new_texture(t, htex.get_info().kind);
                Ok(self.share.handles.borrow_mut().make_texture_srv(view, htex))
            },
        }
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &handle::RawTexture<R>)
                                        -> Result<handle::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &handle::RawTexture<R>, level: Level, layer: Option<Layer>)
                                         -> Result<handle::RawRenderTargetView<R>, f::TargetViewError> {
        self.view_texture_as_target(htex, level, layer)
            .map(|view| {
                let dim = htex.get_info().kind.get_level_dimensions(level);
                self.share.handles.borrow_mut().make_rtv(view, htex, dim)
            })
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &handle::RawTexture<R>, layer: Option<Layer>)
                                         -> Result<handle::RawDepthStencilView<R>, f::TargetViewError> {
        self.view_texture_as_target(htex, 0, layer)
            .map(|view| {
                let dim = htex.get_info().kind.get_level_dimensions(0);
                self.share.handles.borrow_mut().make_dsv(view, htex, dim)
            })
    }

    fn create_sampler(&mut self, info: t::SamplerInfo) -> handle::Sampler<R> {
        let name = if self.share.capabilities.sampler_objects_supported {
            tex::make_sampler(&self.share.context, &info)
        } else {
            0
        };
        let sam = FatSampler {
            object: name,
            info: info.clone(),
        };
        self.share.handles.borrow_mut().make_sampler(sam, info)
    }

    fn update_buffer_raw(&mut self, buffer: &handle::RawBuffer<R>, data: &[u8],
                         offset_bytes: usize) -> Result<(), f::BufferUpdateError> {
        if offset_bytes + data.len() > buffer.get_info().size {
            Err(f::BufferUpdateError::OutOfBounds)
        } else {
            let raw_handle = *self.frame_handles.ref_buffer(buffer);
            update_sub_buffer(&self.share.context, raw_handle, data.as_ptr(), data.len(),
                              offset_bytes, buffer.get_info().role);
            Ok(())
        }
    }

    fn update_texture_raw(&mut self, texture: &handle::Texture<R>,
                          img: &t::ImageInfo, data: &[u8],
                          face: Option<t::CubeFace>)
                          -> Result<(), t::TextureError> {

        tex::update_texture(&self.share.context, texture.get_info().kind, face,
                            *self.frame_handles.ref_texture(texture), img, data)
    }

    fn generate_mipmap(&mut self, texture: &handle::Texture<R>) {
        tex::generate_mipmap(&self.share.context, texture.get_info().kind,
                             *self.frame_handles.ref_texture(texture));
    }

    fn generate_mipmap_raw(&mut self, texture: &handle::RawTexture<R>) {
        match self.frame_handles.ref_new_texture(texture) {
            &NewTexture::Surface(_) => (), // no mip chain
            &NewTexture::Texture(t) =>
                tex::generate_mipmap(&self.share.context, texture.get_info().kind, t),
        }
    }

    fn map_buffer_raw(&mut self, buf: &handle::RawBuffer<R>,
                      access: f::MapAccess) -> RawMapping {
        let gl = &self.share.context;
        let raw_handle = *self.frame_handles.ref_buffer(buf);
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, raw_handle) };
        let ptr = unsafe { gl.MapBuffer(gl::ARRAY_BUFFER, match access {
            f::MapAccess::Readable => gl::READ_ONLY,
            f::MapAccess::Writable => gl::WRITE_ONLY,
            f::MapAccess::RW => gl::READ_WRITE
        }) } as *mut ::std::os::raw::c_void;
        RawMapping {
            pointer: ptr,
            target: gl::ARRAY_BUFFER
        }
    }

    fn unmap_buffer_raw(&mut self, map: RawMapping) {
        let gl = &self.share.context;
        unsafe { gl.UnmapBuffer(map.target) };
    }

    fn map_buffer_readable<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                           -> d::mapping::Readable<T, R, Factory> {
        let map = self.map_buffer_raw(buf.raw(), f::MapAccess::Readable);
        self.map_readable(map, buf.len())
    }

    fn map_buffer_writable<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                                    -> d::mapping::Writable<T, R, Factory> {
        let map = self.map_buffer_raw(buf.raw(), f::MapAccess::Writable);
        self.map_writable(map, buf.len())
    }

    fn map_buffer_rw<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                              -> d::mapping::RW<T, R, Factory> {
        let map = self.map_buffer_raw(buf.raw(), f::MapAccess::RW);
        self.map_read_write(map, buf.len())
    }
}
