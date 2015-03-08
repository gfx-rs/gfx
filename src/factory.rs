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

use libc;
use log::LogLevel;
use std::slice;

use {gl, tex};
use gfx::{Factory, BufferUsage};
use gfx::device as d;
use gfx::device::handle;
use gfx::device::handle::{Bare, Producer};
use gfx::device::mapping::Builder;

use GlDevice;
use GlResources as R;

/// Create a default FBO for the device.
/// Implemented here to limit `Producer` import to this module.
pub fn make_default_frame_buffer(handles: &mut handle::Manager<R>)
							     -> handle::FrameBuffer<R> {
	handles.make_frame_buffer(0)
}


#[allow(raw_pointer_derive)]
#[derive(Copy, Clone)]
pub struct RawMapping {
    pub pointer: *mut libc::c_void,
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


impl Factory<R> for GlDevice {
    type Mapper = RawMapping;

    fn create_buffer_raw(&mut self, size: usize, usage: BufferUsage)
                         -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();
        let info = d::BufferInfo {
            usage: usage,
            size: size,
        };
        self.init_buffer(name, &info);
        self.handles.make_buffer(name, info)
    }

    fn create_buffer_static_raw(&mut self, data: &[u8]) -> handle::RawBuffer<R> {
        let name = self.create_buffer_internal();

        let info = d::BufferInfo {
            usage: BufferUsage::Static,
            size: data.len(),
        };
        self.init_buffer(name, &info);
        self.update_sub_buffer(name, data.as_ptr(), data.len(), 0);
        self.handles.make_buffer(name, info)
    }

    fn create_array_buffer(&mut self) -> Result<handle::ArrayBuffer<R>, ()> {
        if self.caps.array_buffer_supported {
            let mut name = 0 as ::ArrayBuffer;
            unsafe {
                self.gl.GenVertexArrays(1, &mut name);
            }
            info!("\tCreated array buffer {}", name);
            Ok(self.handles.make_array_buffer(name))
        } else {
            error!("\tarray buffer creation unsupported, ignored");
            Err(())
        }
    }

    fn create_shader(&mut self, stage: d::shade::Stage, code: &[u8])
                     -> Result<handle::Shader<R>, d::shade::CreateShaderError> {
        let (name, info) = ::shade::create_shader(&self.gl, stage, code);
        info.map(|info| {
            let level = if name.is_err() { LogLevel::Error } else { LogLevel::Warn };
            log!(level, "\tShader compile log: {}", info);
        });
        name.map(|sh| self.handles.make_shader(sh, stage))
    }

    fn create_program(&mut self, shaders: &[handle::Shader<R>],
                      targets: Option<&[&str]>)
                      -> Result<handle::Program<R>, ()> {
        let (prog, log) = ::shade::create_program(&self.gl, &self.caps, shaders, targets);
        log.map(|log| {
            let level = if prog.is_err() { LogLevel::Error } else { LogLevel::Warn };
            log!(level, "\tProgram link log: {}", log);
        });
        prog.map(|(name, info)| self.handles.make_program(name, info))
    }

    fn create_frame_buffer(&mut self) -> handle::FrameBuffer<R> {
        if !self.caps.render_targets_supported {
            panic!("No framebuffer objects, can't make a new one!");
        }

        let mut name = 0 as ::FrameBuffer;
        unsafe {
            self.gl.GenFramebuffers(1, &mut name);
        }
        info!("\tCreated frame buffer {}", name);
        self.handles.make_frame_buffer(name)
    }

    fn create_surface(&mut self, info: d::tex::SurfaceInfo) ->
                      Result<handle::Surface<R>, d::tex::SurfaceError> {
        tex::make_surface(&self.gl, &info)
            .map(|suf| self.handles.make_surface(suf, info))
    }

    fn create_texture(&mut self, info: d::tex::TextureInfo) ->
                      Result<handle::Texture<R>, d::tex::TextureError> {
        if info.width == 0 || info.height == 0 || info.levels == 0 {
            return Err(d::tex::TextureError::InvalidTextureInfo(info))
        }

        let name = if self.caps.immutable_storage_supported {
            tex::make_with_storage(&self.gl, &info)
        } else {
            tex::make_without_storage(&self.gl, &info)
        };
        name.map(|tex| self.handles.make_texture(tex, info))
    }

    fn create_sampler(&mut self, info: d::tex::SamplerInfo)
                      -> handle::Sampler<R> {
        let sam = if self.caps.sampler_objects_supported {
            tex::make_sampler(&self.gl, &info)
        } else {
            0
        };
        self.handles.make_sampler(sam, info)
    }

    fn get_main_frame_buffer(&self) -> handle::FrameBuffer<R> {
        self.main_fbo.clone()
    }

    fn update_buffer_raw(&mut self, buffer: &handle::RawBuffer<R>,
                         data: &[u8], offset_bytes: usize) {
        debug_assert!(offset_bytes + data.len() <= buffer.get_info().size);
        self.update_sub_buffer(unsafe{ buffer.bare() }, data.as_ptr(), data.len(),
                               offset_bytes)
    }

    fn update_texture_raw(&mut self, texture: &handle::Texture<R>,
                          img: &d::tex::ImageInfo, data: &[u8])
                          -> Result<(), d::tex::TextureError> {
        tex::update_texture(&self.gl, texture.get_info().kind,
                            unsafe{ texture.bare() }, img, data.as_ptr(),
                            data.len())
    }

    fn generate_mipmap(&mut self, texture: &handle::Texture<R>) {
        tex::generate_mipmap(&self.gl, texture.get_info().kind,
                             unsafe{ texture.bare() });
    }

    fn map_buffer_raw(&mut self, buf: &handle::RawBuffer<R>,
                      access: d::MapAccess) -> RawMapping {
        let ptr;
        unsafe { self.gl.BindBuffer(gl::ARRAY_BUFFER, buf.bare()) };
        ptr = unsafe { self.gl.MapBuffer(gl::ARRAY_BUFFER, match access {
            d::MapAccess::Readable => gl::READ_ONLY,
            d::MapAccess::Writable => gl::WRITE_ONLY,
            d::MapAccess::RW => gl::READ_WRITE
        }) } as *mut libc::c_void;
        RawMapping {
            pointer: ptr,
            target: gl::ARRAY_BUFFER
        }
    }

    fn unmap_buffer_raw(&mut self, map: RawMapping) {
        unsafe { self.gl.UnmapBuffer(map.target) };
    }

    fn map_buffer_readable<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                           -> d::mapping::Readable<T, R, GlDevice> {
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::Readable);
        self.map_readable(map, buf.len())
    }

    fn map_buffer_writable<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                                    -> d::mapping::Writable<T, R, GlDevice> {
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::Writable);
        self.map_writable(map, buf.len())
    }

    fn map_buffer_rw<T: Copy>(&mut self, buf: &handle::Buffer<R, T>)
                              -> d::mapping::RW<T, R, GlDevice> {
        let map = self.map_buffer_raw(buf.raw(), d::MapAccess::RW);
        self.map_read_write(map, buf.len())
    }

    fn cleanup(&mut self) {
        self.handles.clean_with(&mut self.gl,
            |gl, v| unsafe { gl.DeleteBuffers(1, v) },
            |gl, v| unsafe { gl.DeleteVertexArrays(1, v) },
            |gl, v| unsafe { gl.DeleteShader(*v) },
            |gl, v| unsafe { gl.DeleteProgram(*v) },
            |gl, v| unsafe { gl.DeleteFramebuffers(1, v) },
            |gl, v| unsafe { gl.DeleteRenderbuffers(1, v) },
            |gl, v| unsafe { gl.DeleteTextures(1, v) },
            |gl, v| unsafe { gl.DeleteSamplers(1, v) })
    }
}
