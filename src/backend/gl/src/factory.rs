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
use gfx_core::handle;
use gfx_core::handle::Producer;
use gfx_core::mapping::Builder;
use gfx_core::target::{Layer, Level};
use gfx_core::tex::Size;

use {Resources as R, Share, OutputMerger};
use {Buffer, FatSampler, NewTexture, PipelineState, Program, ResourceView, TargetView};


fn role_to_target(role: d::BufferRole) -> gl::types::GLenum {
    match role {
        d::BufferRole::Vertex  => gl::ARRAY_BUFFER,
        d::BufferRole::Index   => gl::ELEMENT_ARRAY_BUFFER,
        d::BufferRole::Uniform => gl::UNIFORM_BUFFER,
    }
}

pub fn update_sub_buffer(gl: &gl::Gl, buffer: Buffer, address: *const u8,
                         size: usize, offset: usize, role: d::BufferRole) {
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

fn surface_type_to_old_format(sf: d::format::SurfaceType) -> d::tex::Format {
    use gfx_core::format::SurfaceType;
    use gfx_core::tex::{Format, Components, FloatSize, IntSubType};
    match sf {
        SurfaceType::R8_G8_B8_A8 => Format::Unsigned(Components::RGBA, 8, IntSubType::Normalized),
        SurfaceType::R10_G10_B10_A2 => Format::RGB10_A2,
        SurfaceType::R16_G16_B16_A16 => Format::Float(Components::RGBA, FloatSize::F16),
        SurfaceType::R32_G32_B32_A32 => Format::Float(Components::RGBA, FloatSize::F32),
        SurfaceType::D24_S8 => Format::DEPTH24_STENCIL8,
    }
}

fn descriptor_to_texture_info(d: &d::tex::Descriptor) -> d::tex::TextureInfo {
    d::tex::TextureInfo {
        width: d.width,
        height: d.height,
        depth: d.depth,
        levels: d.levels,
        kind: d.kind,
        format: surface_type_to_old_format(d.format),
    }
}

/// A placeholder for a real `Output` implemented by your window.
pub struct Output {
    /// render frame width.
    pub width: Size,
    /// render frame height.
    pub height: Size,
    /// main FBO handle
    handle: handle::FrameBuffer<R>,
}

impl d::output::Output<R> for Output {
    fn get_handle(&self) -> Option<&handle::FrameBuffer<R>> {
        Some(&self.handle)
    }

    fn get_size(&self) -> (Size, Size) {
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

    fn create_buffer_internal(&mut self) -> Buffer {
        let gl = &self.share.context;
        let mut name = 0 as Buffer;
        unsafe {
            gl.GenBuffers(1, &mut name);
        }
        info!("\tCreated buffer {}", name);
        name
    }

    fn init_buffer(&mut self, buffer: Buffer, info: &d::BufferInfo) {
        let gl = &self.share.context;
        let target = role_to_target(info.role);
        let usage = match info.usage {
            d::BufferUsage::Static  => gl::STATIC_DRAW,
            d::BufferUsage::Dynamic => gl::DYNAMIC_DRAW,
            d::BufferUsage::Stream  => gl::STREAM_DRAW,
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
                              -> Result<(Program, d::shade::ProgramInfo), d::shade::CreateProgramError> {
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
                    -> Result<TargetView, d::TargetViewError> {
        match (self.frame_handles.ref_new_texture(htex), layer) {
            (&NewTexture::Surface(_), Some(_)) => Err(d::TargetViewError::Unsupported),
            (&NewTexture::Surface(_), None) if level != 0 => Err(d::TargetViewError::Unsupported),
            (&NewTexture::Surface(s), None) => Ok(TargetView::Surface(s)),
            (&NewTexture::Texture(t), Some(l)) => Ok(TargetView::TextureLayer(t, level, l)),
            (&NewTexture::Texture(t), None) => Ok(TargetView::Texture(t, level)),
        }
    }

    pub fn get_main_frame_buffer(&self) -> handle::FrameBuffer<R> {
        self.share.main_fbo.clone()
    }

    pub fn make_fake_output(&self, w: Size, h: Size) -> Output {
        Output {
            width: w,
            height: h,
            handle: self.get_main_frame_buffer(),
        }
    }

    pub fn get_main_color<T: d::format::Formatted>(&self) -> handle::RenderTargetView<R, T> {
        self.share.main_color.clone().into()
    }

    pub fn get_main_depth_stencil<T: d::format::Formatted>(&self) -> handle::DepthStencilView<R, T> {
        self.share.main_depth_stencil.clone().into()
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
    type Mapper = RawMapping;

    fn get_capabilities(&self) -> &d::Capabilities {
        &self.share.capabilities
    }

    fn create_buffer_raw(&mut self, size: usize, role: d::BufferRole, usage: d::BufferUsage)
                         -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();
        let info = d::BufferInfo {
            role: role,
            usage: usage,
            size: size,
        };
        self.init_buffer(name, &info);
        self.share.handles.borrow_mut().make_buffer(name, info)
    }

    fn create_buffer_static_raw(&mut self, data: &[u8], role: d::BufferRole)
                                -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();

        let info = d::BufferInfo {
            role: role,
            usage: d::BufferUsage::Static,
            size: data.len(),
        };
        self.init_buffer(name, &info);
        update_sub_buffer(&self.share.context, name, data.as_ptr(), data.len(), 0, role);
        self.share.handles.borrow_mut().make_buffer(name, info)
    }

    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, d::NotSupported> {
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
            Err(d::NotSupported)
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

    fn create_frame_buffer(&mut self) -> Result<handle::FrameBuffer<R>, d::NotSupported> {
        if self.share.capabilities.render_targets_supported {
            let gl = &self.share.context;
            let mut name = 0 as ::FrameBuffer;
            unsafe {
                gl.GenFramebuffers(1, &mut name);
            }
            info!("\tCreated frame buffer {}", name);
            Ok(self.share.handles.borrow_mut().make_frame_buffer(name))
        } else {
            error!("No framebuffer objects, can't make a new one!");
            Err(d::NotSupported)
        }
    }

    fn create_surface(&mut self, info: d::tex::SurfaceInfo) ->
                      Result<handle::Surface<R>, d::tex::SurfaceError> {
        if info.format.does_convert_gamma() && !self.share.capabilities.srgb_color_supported {
            return Err(d::tex::SurfaceError::UnsupportedGamma)
        }
        tex::make_surface(&self.share.context, &info)
            .map(|suf| self.share.handles.borrow_mut().make_surface(suf, info))
    }

    fn create_texture(&mut self, info: d::tex::TextureInfo) ->
                      Result<handle::Texture<R>, d::tex::TextureError> {
        let caps = &self.share.capabilities;
        if info.width == 0 || info.height == 0 || info.levels == 0 {
            return Err(d::tex::TextureError::InvalidInfo(info))
        }
        if info.format.does_convert_gamma() && !caps.srgb_color_supported {
            return Err(d::tex::TextureError::UnsupportedGamma)
        }
        let gl = &self.share.context;
        let name = if caps.immutable_storage_supported {
            tex::make_with_storage(gl, &info)
        } else {
            tex::make_without_storage(gl, &info)
        };
        name.map(|tex| self.share.handles.borrow_mut().make_texture(tex, info))
    }

    fn create_new_texture_raw(&mut self, desc: d::tex::Descriptor)
                              -> Result<handle::RawTexture<R>, d::tex::Error> {
        use gfx_core::tex::Error;
        let caps = &self.share.capabilities;
        if desc.width == 0 || desc.height == 0 || desc.levels == 0 {
            return Err(Error::Size(0))
        }
        let info = descriptor_to_texture_info(&desc);
        let gl = &self.share.context;
        let object = if desc.bind.intersects(d::SHADER_RESOURCE | d::UNORDERED_ACCESS) {
            use gfx_core::tex::TextureError;
            let result = if caps.immutable_storage_supported {
                tex::make_with_storage(gl, &info)
            } else {
                tex::make_without_storage(gl, &info)
            };
            match result {
                Ok(name) => NewTexture::Texture(name),
                Err(TextureError::UnsupportedGamma) => return Err(Error::Gamma),
                Err(TextureError::UnsupportedSamples) => {
                    let aa = desc.kind.get_aa_mode().unwrap_or(d::tex::AaMode::Msaa(0));
                    return Err(Error::Samples(aa));
                },
                Err(_) => return Err(Error::Format(desc.format)),
            }
        }else {
            use gfx_core::tex::SurfaceError;
            let result = tex::make_surface(gl, &info.into());
            match result {
                Ok(name) => NewTexture::Surface(name),
                Err(SurfaceError::UnsupportedFormat) => return Err(Error::Format(desc.format)),
                Err(SurfaceError::UnsupportedGamma) => return Err(Error::Gamma),
            }
        };
        Ok(self.share.handles.borrow_mut().make_new_texture(object, desc))
    }

    fn create_new_texture_with_data(&mut self, desc: d::tex::Descriptor, data: &[u8])
                                    -> Result<handle::RawTexture<R>, d::tex::Error> {
        let kind = desc.kind;
        let img = descriptor_to_texture_info(&desc).into();
        let tex = try!(self.create_new_texture_raw(desc));
        match self.frame_handles.ref_new_texture(&tex) {
            &NewTexture::Surface(_) => Err(d::tex::Error::Data(0)),
            &NewTexture::Texture(t) => match tex::update_texture(&self.share.context,
                kind, t, &img, data.as_ptr(), data.len()) {
                Ok(_) => Ok(tex),
                Err(_) => Err(d::tex::Error::Data(0)),
            }
        }
    }

    fn view_buffer_as_shader_resource(&mut self, hbuf: &handle::RawBuffer<R>)
                                      -> Result<handle::RawShaderResourceView<R>, d::ResourceViewError> {
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

    fn view_buffer_as_unordered_access(&mut self, _hbuf: &handle::RawBuffer<R>)
                                       -> Result<handle::RawUnorderedAccessView<R>, d::ResourceViewError> {
        Err(d::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource(&mut self, htex: &handle::RawTexture<R>, _desc: d::tex::ViewDesc)
                                       -> Result<handle::RawShaderResourceView<R>, d::ResourceViewError> {
        match self.frame_handles.ref_new_texture(htex) {
            &NewTexture::Surface(_) => Err(d::ResourceViewError::NoBindFlag),
            &NewTexture::Texture(t) => {
                //TODO: use the view descriptor
                let view = ResourceView::new_texture(t, htex.get_info().kind);
                Ok(self.share.handles.borrow_mut().make_texture_srv(view, htex))
            },
        }
    }

    fn view_texture_as_unordered_access(&mut self, _htex: &handle::RawTexture<R>)
                                        -> Result<handle::RawUnorderedAccessView<R>, d::ResourceViewError> {
        Err(d::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &handle::RawTexture<R>, level: Level, layer: Option<Layer>)
                                         -> Result<handle::RawRenderTargetView<R>, d::TargetViewError> {
        self.view_texture_as_target(htex, level, layer)
            .map(|view| self.share.handles.borrow_mut().make_rtv(view, htex))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &handle::RawTexture<R>, layer: Option<Layer>)
                                         -> Result<handle::RawDepthStencilView<R>, d::TargetViewError> {
        self.view_texture_as_target(htex, 0, layer)
            .map(|view| self.share.handles.borrow_mut().make_dsv(view, htex))
    }

    fn create_sampler(&mut self, info: d::tex::SamplerInfo) -> handle::Sampler<R> {
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
                         offset_bytes: usize) -> Result<(), d::BufferUpdateError> {
        if offset_bytes + data.len() > buffer.get_info().size {
            Err(d::BufferUpdateError::OutOfBounds)
        } else {
            let raw_handle = *self.frame_handles.ref_buffer(buffer);
            update_sub_buffer(&self.share.context, raw_handle, data.as_ptr(), data.len(),
                              offset_bytes, buffer.get_info().role);
            Ok(())
        }
    }

    fn update_texture_raw(&mut self, texture: &handle::Texture<R>,
                          img: &d::tex::ImageInfo, data: &[u8],
                          kind_override: Option<d::tex::Kind>)
                          -> Result<(), d::tex::TextureError> {

        // use the specified texture kind if set for this update, otherwise
        // fall back on the kind that was set when the texture was created.
        let kind = kind_override.unwrap_or(texture.get_info().kind);

        tex::update_texture(&self.share.context, kind,
                            *self.frame_handles.ref_texture(texture),
                            img, data.as_ptr(), data.len())
    }

    fn generate_mipmap(&mut self, texture: &handle::Texture<R>) {
        tex::generate_mipmap(&self.share.context, texture.get_info().kind,
                             *self.frame_handles.ref_texture(texture));
    }

    fn generate_mipmap_new(&mut self, texture: &handle::RawTexture<R>) {
        match self.frame_handles.ref_new_texture(texture) {
            &NewTexture::Surface(_) => (), // no mip chain
            &NewTexture::Texture(t) =>
                tex::generate_mipmap(&self.share.context, texture.get_info().kind, t),
        }
    }

    fn map_buffer_raw(&mut self, buf: &handle::RawBuffer<R>,
                      access: d::MapAccess) -> RawMapping {
        let gl = &self.share.context;
        let raw_handle = *self.frame_handles.ref_buffer(buf);
        unsafe { gl.BindBuffer(gl::ARRAY_BUFFER, raw_handle) };
        let ptr = unsafe { gl.MapBuffer(gl::ARRAY_BUFFER, match access {
            d::MapAccess::Readable => gl::READ_ONLY,
            d::MapAccess::Writable => gl::WRITE_ONLY,
            d::MapAccess::RW => gl::READ_WRITE
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
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::Readable);
        self.map_readable(map, buf.len())
    }

    fn map_buffer_writable<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                                    -> d::mapping::Writable<T, R, Factory> {
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::Writable);
        self.map_writable(map, buf.len())
    }

    fn map_buffer_rw<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                              -> d::mapping::RW<T, R, Factory> {
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::RW);
        self.map_read_write(map, buf.len())
    }
}
