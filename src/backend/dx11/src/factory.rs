// Copyright 2016 The Gfx-rs Developers.
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

use std::{ptr, slice};
use std::sync::Arc;
use winapi;
use gfx_core as core;
use gfx_core::factory as f;
use gfx_core::handle as h;
use gfx_core::handle::Producer;
use {Resources as R, Share};
use command::CommandBuffer;
use native;


#[derive(Copy, Clone)]
pub struct RawMapping {
    pointer: *mut ::std::os::raw::c_void,
}

impl core::mapping::Raw for RawMapping {
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

pub struct Factory {
    share: Arc<Share>,
    frame_handles: h::Manager<R>,
}

impl Clone for Factory {
    fn clone(&self) -> Factory {
        Factory::new(self.share.clone())
    }
}

impl Factory {
    /// Create a new `Factory`.
    pub fn new(share: Arc<Share>) -> Factory {
        Factory {
            share: share,
            frame_handles: h::Manager::new(),
        }
    }

    fn create_buffer_internal(&self) -> () {
        () //TODO
    }
    fn init_buffer(&self, _: (), _: &f::BufferInfo) {
        //TODO
    }
}

impl core::Factory<R> for Factory {
    type CommandBuffer = CommandBuffer;
    type Mapper = RawMapping;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.share.capabilities
    }

    fn create_command_buffer(&mut self) -> CommandBuffer {
        CommandBuffer::new()
    }

    fn create_buffer_raw(&mut self, size: usize, role: f::BufferRole, usage: f::BufferUsage)
                         -> h::RawBuffer<R> {
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
                                -> h::RawBuffer<R> {
        let name = self.create_buffer_internal();

        let info = f::BufferInfo {
            role: role,
            usage: f::BufferUsage::Const,
            size: data.len(),
        };
        self.init_buffer(name, &info);
        //update_sub_buffer(&self.share.context, name, data.as_ptr(), data.len(), 0, role); //TODO
        self.share.handles.borrow_mut().make_buffer(name, info)
    }

    fn create_shader(&mut self, _stage: core::shade::Stage, _code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        Ok(self.share.handles.borrow_mut().make_shader(())) //TODO
    }

    fn create_program(&mut self, _shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        let info = core::shade::ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            knows_outputs: true,
        };
        Ok(self.share.handles.borrow_mut().make_program((), info)) //TODO
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, _desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        Ok(self.share.handles.borrow_mut().make_pso((), program)) //TODO
    }

    fn create_texture_raw(&mut self, desc: core::tex::Descriptor, _hint: Option<core::format::ChannelType>)
                          -> Result<h::RawTexture<R>, core::tex::Error> {
        Ok(self.share.handles.borrow_mut().make_texture(native::Texture(ptr::null_mut()), desc)) //TODO
    }

    fn view_buffer_as_shader_resource_raw(&mut self, hbuf: &h::RawBuffer<R>)
                                      -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Ok(self.share.handles.borrow_mut().make_buffer_srv((), hbuf)) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, _desc: core::tex::ViewDesc)
                                       -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Ok(self.share.handles.borrow_mut().make_texture_srv((), htex)) //TODO
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, level: core::target::Level, _layer: Option<core::target::Layer>)
                                         -> Result<h::RawRenderTargetView<R>, f::TargetViewError> {
        let mut raw_view: *mut winapi::ID3D11RenderTargetView = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).0;
        //TODO: pass in the descriptor
        unsafe {
            (*self.share.device).CreateRenderTargetView(raw_tex as *mut winapi::ID3D11Resource,
                ptr::null_mut(), &mut raw_view);
        }
        let dim = htex.get_info().kind.get_level_dimensions(level);
        Ok(self.share.handles.borrow_mut().make_rtv(native::Rtv(raw_view), htex, dim))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &h::RawTexture<R>, _layer: Option<core::target::Layer>)
                                         -> Result<h::RawDepthStencilView<R>, f::TargetViewError> {
        let mut raw_view: *mut winapi::ID3D11DepthStencilView = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).0;
        //TODO: pass in the descriptor
        unsafe {
            (*self.share.device).CreateDepthStencilView(raw_tex as *mut winapi::ID3D11Resource,
                ptr::null_mut(), &mut raw_view);
        }
        let dim = htex.get_info().kind.get_level_dimensions(0);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_view), htex, dim))
    }

    fn create_sampler(&mut self, info: core::tex::SamplerInfo) -> h::Sampler<R> {
        self.share.handles.borrow_mut().make_sampler((), info)
    }

    fn update_buffer_raw(&mut self, _buffer: &h::RawBuffer<R>, _data: &[u8],
                         _offset_bytes: usize) -> Result<(), f::BufferUpdateError> {
        Ok(()) //TODO
    }

    fn update_texture_raw(&mut self, _texture: &h::RawTexture<R>, _image: &core::tex::RawImageInfo,
                          _data: &[u8], _face: Option<core::tex::CubeFace>) -> Result<(), core::tex::Error> {
        Ok(()) //TODO
    }

    fn generate_mipmap_raw(&mut self, _texture: &h::RawTexture<R>) {
        //TODO
    }

    fn map_buffer_raw(&mut self, _buf: &h::RawBuffer<R>, _access: f::MapAccess) -> RawMapping {
        unimplemented!()
    }

    fn unmap_buffer_raw(&mut self, _map: RawMapping) {
        unimplemented!()
    }

    fn map_buffer_readable<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                           -> core::mapping::Readable<T, R, Factory> {
        unimplemented!()
    }

    fn map_buffer_writable<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                                    -> core::mapping::Writable<T, R, Factory> {
        unimplemented!()
    }

    fn map_buffer_rw<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                              -> core::mapping::RW<T, R, Factory> {
        unimplemented!()
    }
}
