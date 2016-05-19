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

use std::os::raw::c_void;
use std::sync::Arc;
use std::slice;
use std::mem;
use std::str;

use cocoa::base::{selector, class};
use cocoa::foundation::{NSUInteger};

use gfx_core as core;
use gfx_core::{factory, handle};
use gfx_core::handle::Producer;
use gfx_core::handle::Manager;

use metal::*;

use command::CommandBuffer;

use {Resources, Share, Texture, Buffer, Shader, Program, Pipeline};
use native;
use mirror;

#[derive(Copy, Clone)]
pub struct RawMapping {
    pointer: *mut c_void,
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
    device: MTLDevice,
    queue: MTLCommandQueue,
    share: Arc<Share>,
    frame_handles: handle::Manager<Resources>,
}

impl Factory {
    pub fn new(device: MTLDevice, share: Arc<Share>) -> Factory {
        Factory {
            device: device,
            queue: device.new_command_queue(),
            share: share,
            frame_handles: handle::Manager::new(),
        }
    }

    pub fn create_command_buffer(&self) -> CommandBuffer {
        CommandBuffer::new(self.queue)
    }

    fn create_buffer_internal(&self, info: factory::BufferInfo, raw_data: Option<*const c_void>)
            -> Result<handle::RawBuffer<Resources>, factory::BufferError> {
        use map::{map_buffer_usage};

        let usage = map_buffer_usage(info.usage);

        if info.bind.contains(factory::RENDER_TARGET) | info.bind.contains(factory::DEPTH_STENCIL) {
            return Err(factory::BufferError::UnsupportedBind(info.bind))
        }

        let mut raw_buf = native::Buffer(self.device.new_buffer(info.size as u64, usage));

        let buf = Buffer(raw_buf, info.usage);
        Ok(self.share.handles.borrow_mut().make_buffer(buf, info))
    }

}


impl core::Factory<Resources> for Factory {
    type Mapper = RawMapping;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.share.capabilities
    }

    fn create_buffer_raw(&mut self, info: factory::BufferInfo) -> Result<handle::RawBuffer<Resources>, factory::BufferError> {
        self.create_buffer_internal(info, None)
    }

    fn create_buffer_const_raw(&mut self, data: &[u8], stride: usize, role: factory::BufferRole, bind: factory::Bind)
                                -> Result<handle::RawBuffer<Resources>, factory::BufferError> {
        let info = factory::BufferInfo {
            role: role,
            usage: factory::Usage::Const,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        self.create_buffer_internal(info, Some(data.as_ptr() as *const c_void))
    }

    fn create_shader(&mut self, stage: core::shade::Stage, code: &[u8])
                     -> Result<handle::Shader<Resources>, core::shade::CreateShaderError> {
        use gfx_core::shade::{CreateShaderError, Stage};

        let lib = match stage {
            Stage::Vertex | Stage::Pixel => {
                let src = str::from_utf8(code).unwrap();
                match self.device.new_library_with_source(src, MTLCompileOptions::nil()) {
                    Ok(lib) => lib,
                    Err(err) => return Err(CreateShaderError::CompilationFailed(err.into()))
                }
            },
            _ => return Err(CreateShaderError::StageNotSupported(stage))
        };

        let shader = Shader {
            func: lib.get_function(match stage {
                Stage::Vertex => "vert",
                Stage::Pixel => "frag",
                _ => return Err(CreateShaderError::StageNotSupported(stage))
            }).unwrap()
        };

        Ok(self.share.handles.borrow_mut().make_shader(shader))
    }

    fn create_program(&mut self, shader_set: &core::ShaderSet<Resources>)
                      -> Result<handle::Program<Resources>, core::shade::CreateProgramError> {
        use gfx_core::shade::{ProgramInfo, Stage};

        let (prog, info) = match shader_set {
            &core::ShaderSet::Simple(ref vs, ref ps) => {
                let mut info = ProgramInfo {
                    vertex_attributes: Vec::new(),
                    globals: Vec::new(),
                    constant_buffers: Vec::new(),
                    textures: Vec::new(),
                    unordereds: Vec::new(),
                    samplers: Vec::new(),
                    outputs: Vec::new(),
                    knows_outputs: false,
                };

                let fh = &mut self.frame_handles;
                let (vs, ps) = (vs.reference(fh).func, ps.reference(fh).func);

                let mut reflection = MTLRenderPipelineReflection::nil();

                // since Metal doesn't allow for fetching shader reflection
                // without creating a PSO, we're creating a "fake" PSO to get
                // the reflection, and destroying it afterwards.
                //
                // Tracking: https://forums.developer.apple.com/thread/46535
                let pso_descriptor = MTLRenderPipelineDescriptor::alloc().init();
                pso_descriptor.set_vertex_function(vs);
                pso_descriptor.set_fragment_function(ps);
                pso_descriptor.color_attachments().object_at(0).set_pixel_format(MTLPixelFormat::BGRA8Unorm);

                // when we use `[[ stage_in ]]` we have to have a vertex
                // descriptor attached to the PSO descriptor
                //
                // dummy values are used for the attributes but the buffer
                // slot `30` is occupied by the dummy buffer
                //
                // TODO: prevent collision between dummy buffers and real
                //       values
                let vertex_desc = MTLVertexDescriptor::new();

                let buf = vertex_desc.layouts().object_at(30);
                buf.set_stride(16);
                buf.set_step_function(MTLVertexStepFunction::Constant);
                buf.set_step_rate(0);

                for i in 0..16 {
                    let attribute = vertex_desc.attributes().object_at(i as usize);
                    attribute.set_format(MTLVertexFormat::Int4);
                    attribute.set_offset(0);
                    attribute.set_buffer_index(30);
                }

                pso_descriptor.set_vertex_descriptor(vertex_desc);

                let pso = self.device.new_render_pipeline_state_with_reflection(pso_descriptor, &mut reflection).unwrap();

                // fill the `ProgramInfo` struct with goodies
                mirror::populate_vertex_attributes(&mut info, vs.vertex_attributes());

                mirror::populate_info(&mut info, Stage::Vertex, reflection.vertex_arguments());
                mirror::populate_info(&mut info, Stage::Pixel,  reflection.fragment_arguments());

                println!("{:?},\n{:?},\n{:?},\n{:?},\n{:?},\n", info.vertex_attributes, info.constant_buffers, info.textures, info.samplers, info.outputs);

                // destroy PSO & reflection object after we're done with
                // parsing reflection
                unsafe {
                    pso.release();
                    reflection.release();
                }

                // FIXME: retain functions?
                let program = Program {
                    vs: vs,
                    ps: ps
                };

                (program, info)
            },

            // Metal only supports vertex + fragment and has some features from
            // geometry shaders in vertex (layered rendering)
            //
            // Tracking: https://forums.developer.apple.com/message/9495
            _ => { return Err("Metal only supports vertex + fragment shader programs".into()); }
        };

        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &handle::Program<Resources>, desc: &core::pso::Descriptor)
                                 -> Result<handle::RawPipelineState<Resources>, core::pso::CreationError> {
        use map::{map_vertex_format, map_topology};

        let vertex_desc = MTLVertexDescriptor::new();

        // TODO: implement instancing
        //
        // TODO: find a better way to set the buffer's stride, step func and
        //       step rate
        let buf = vertex_desc.layouts().object_at(0);
        buf.set_stride(desc.attributes[0].unwrap().0.stride as u64);
        buf.set_step_function(MTLVertexStepFunction::PerVertex);
        buf.set_step_rate(1);

        for (attr, attr_desc) in program.get_info().vertex_attributes.iter().zip(desc.attributes.iter()) {
            let (elem, irate) = match attr_desc {
                &Some((ref el, ir)) => (el, ir),
                &None => continue,
            };

            if elem.offset & 1 != 0 {
                error!("Vertex attribute {} must be aligned to 2 bytes, has offset {}",
                    attr.name, elem.offset);
                return Err(core::pso::CreationError);
            }

            // TODO: handle case when requested vertex format is invalid
            let attribute = vertex_desc.attributes().object_at(attr.slot as usize);
            attribute.set_format(map_vertex_format(elem.format).unwrap());
            attribute.set_offset(elem.offset as u64);
            attribute.set_buffer_index(0);
        }

        let prog = self.frame_handles.ref_program(program);

        let pso_descriptor = MTLRenderPipelineDescriptor::alloc().init();
        pso_descriptor.set_vertex_function(prog.vs);
        pso_descriptor.set_fragment_function(prog.ps);
        pso_descriptor.set_vertex_descriptor(vertex_desc);
        pso_descriptor.set_input_primitive_topology(map_topology(desc.primitive));
        pso_descriptor.color_attachments().object_at(0).set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        if let Some(depth_desc) = desc.depth_stencil {
            // TODO: depthstencil
        }

        let pso = self.device.new_render_pipeline_state(pso_descriptor).unwrap();

        let pso = Pipeline {
            pipeline: pso
        };

        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: core::tex::Descriptor, hint: Option<core::format::ChannelType>,
                          data_opt: Option<&[&[u8]]>) -> Result<handle::RawTexture<Resources>, core::tex::Error> {
        use gfx_core::tex::{AaMode, Error, Kind};
        use map::{map_texture_bind, map_texture_usage, map_format};

        let (resource, storage) = map_texture_usage(desc.usage);

        let descriptor = MTLTextureDescriptor::alloc().init();
        descriptor.set_mipmap_level_count(desc.levels as u64);
        descriptor.set_resource_options(resource);
        descriptor.set_storage_mode(storage);

        descriptor.set_usage(map_texture_bind(desc.bind));

        match desc.kind {
            Kind::D1(w) => {
                descriptor.set_width(w as u64);
                descriptor.set_texture_type(MTLTextureType::D1);
            },
            Kind::D1Array(w, d) => {
                descriptor.set_width(w as u64);
                descriptor.set_array_length(d as u64);
                descriptor.set_texture_type(MTLTextureType::D1Array);
            },
            Kind::D2(w, h, aa) => {
                descriptor.set_width(w as u64);
                descriptor.set_height(h as u64);
                match aa {
                    AaMode::Single => {
                        descriptor.set_texture_type(MTLTextureType::D2);
                    },
                    AaMode::Multi(samples) => {

                        descriptor.set_texture_type(MTLTextureType::D2Multisample);
                        descriptor.set_sample_count(samples as u64);
                    },
                    _ => unimplemented!()
                };
            },
            Kind::D2Array(w, h, d, aa) => {
                descriptor.set_width(w as u64);
                descriptor.set_height(h as u64);
                descriptor.set_array_length(d as u64);
                descriptor.set_texture_type(MTLTextureType::D2Array);
            },
            Kind::D3(w, h, d) => {
                descriptor.set_width(w as u64);
                descriptor.set_height(h as u64);
                descriptor.set_depth(d as u64);
                descriptor.set_texture_type(MTLTextureType::D3);
            },
            Kind::Cube(w) => {
                descriptor.set_width(w as u64);
                descriptor.set_texture_type(MTLTextureType::Cube);
            },
            Kind::CubeArray(w, d) => {
                descriptor.set_width(w as u64);
                descriptor.set_array_length(d as u64);
                descriptor.set_texture_type(MTLTextureType::CubeArray);
            },
        };


        let tex = Texture(native::Texture(&mut self.device.new_texture(descriptor)), desc.usage);
        Ok(self.share.handles.borrow_mut().make_texture(tex, desc))
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &handle::RawBuffer<Resources>)
                                      -> Result<handle::RawShaderResourceView<Resources>, factory::ResourceViewError> {
        unimplemented!()
        // Err(factory::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &handle::RawBuffer<Resources>)
                                       -> Result<handle::RawUnorderedAccessView<Resources>, factory::ResourceViewError> {
        unimplemented!()
        // Err(factory::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &handle::RawTexture<Resources>, desc: core::tex::ResourceDesc)
                                       -> Result<handle::RawShaderResourceView<Resources>, factory::ResourceViewError> {
        /*use winapi::UINT;
        use gfx_core::tex::{AaMode, Kind};
        use data::map_format;

        let (dim, layers, has_levels) = match htex.get_info().kind {
            Kind::D1(_) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE1D, 1, true),
            Kind::D1Array(_, d) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE1DARRAY, d, true),
            Kind::D2(_, _, AaMode::Single) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2D, 1, true),
            Kind::D2(_, _, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DMS, 1, false),
            Kind::D2Array(_, _, d, AaMode::Single) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DARRAY, d, true),
            Kind::D2Array(_, _, d, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE2DMSARRAY, d, false),
            Kind::D3(_, _, _) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURE3D, 1, true),
            Kind::Cube(_) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURECUBE, 1, true),
            Kind::CubeArray(_, d) =>
                (winapi::D3D11_SRV_DIMENSION_TEXTURECUBEARRAY, d, true),
        };

        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = winapi::D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: match map_format(format, false) {
                Some(fm) => fm,
                None => return Err(f::ResourceViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: if has_levels {
                assert!(desc.max >= desc.min);
                [desc.min as UINT, (desc.max + 1 - desc.min) as UINT, 0, layers as UINT]
            }else {
                [0, layers as UINT, 0, 0]
            },
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).to_resource();
        let hr = unsafe {
            (*self.device).CreateShaderResourceView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create SRV from {:#?}, error {:x}", native_desc, hr);
            return Err(f::ResourceViewError::Unsupported);
        }
        Ok(self.share.handles.borrow_mut().make_texture_srv(native::Srv(raw_view), htex))*/
        let raw_tex = self.frame_handles.ref_texture(htex).0;
        Ok(self.share.handles.borrow_mut().make_texture_srv(native::Srv(raw_tex.0), htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &handle::RawTexture<Resources>)
                                        -> Result<handle::RawUnorderedAccessView<Resources>, factory::ResourceViewError> {
        // Err(factory::ResourceViewError::Unsupported) //TODO
        unimplemented!()
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &handle::RawTexture<Resources>, desc: core::tex::RenderDesc)
                                         -> Result<handle::RawRenderTargetView<Resources>, factory::TargetViewError>
    {
        let raw_tex = self.frame_handles.ref_texture(htex).0;
        let size = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_rtv(native::Rtv(raw_tex.0), htex, size))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &handle::RawTexture<Resources>, desc: core::tex::DepthStencilDesc)
                                         -> Result<handle::RawDepthStencilView<Resources>, factory::TargetViewError>
    {
        /*use winapi::UINT;
        use gfx_core::tex::{AaMode, Kind};
        use data::{map_format, map_dsv_flags};

        let level = desc.level as UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D1Array(_, nlayers), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as UINT, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [lid as UINT, 1+lid as UINT, 0]),
            (Kind::D3(..), _) => return Err(f::TargetViewError::Unsupported),
            (Kind::Cube(..), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::CubeArray(_, nlayers), None) =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6 * nlayers as UINT]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 6 * lid as UINT, 6 * (1+lid) as UINT]),
            (_, None) => return Err(f::TargetViewError::BadLevel(desc.level)),
            (_, Some(lid)) => return Err(f::TargetViewError::BadLayer(lid)),
        };

        let channel = core::format::ChannelType::Uint; //doesn't matter
        let format = core::format::Format(htex.get_info().format, channel);
        let native_desc = winapi::D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(f::TargetViewError::Channel(channel)),
            },
            ViewDimension: dim,
            Flags: map_dsv_flags(desc.flags).0,
            u: extra,
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).to_resource();
        let hr = unsafe {
            (*self.device).CreateDepthStencilView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create DSV from {:#?}, error {:x}", native_desc, hr);
            return Err(f::TargetViewError::Unsupported);
        }
        let dim = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_view), htex, dim))*/
        let raw_tex = self.frame_handles.ref_texture(htex).0;
        let size = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_tex.0), htex, size))
    }

    fn create_sampler(&mut self, info: core::tex::SamplerInfo) -> handle::Sampler<Resources> {
        use gfx_core::tex::FilterMethod;
        use map::{map_function, map_filter, map_wrap};

        let desc = MTLSamplerDescriptor::new();

        let (filter, mip) = map_filter(info.filter);
        desc.set_min_filter(filter);
        desc.set_mag_filter(filter);
        desc.set_mip_filter(mip);

        if let FilterMethod::Anisotropic(anisotropy) = info.filter {
            desc.set_max_anisotropy(anisotropy as u64);
        }

        desc.set_lod_bias(info.lod_bias.into());
        desc.set_lod_min_clamp(info.lod_range.0.into());
        desc.set_lod_max_clamp(info.lod_range.1.into());
        desc.set_address_mode_s(map_wrap(info.wrap_mode.0));
        desc.set_address_mode_t(map_wrap(info.wrap_mode.1));
        desc.set_address_mode_r(map_wrap(info.wrap_mode.2));
        desc.set_compare_function(map_function(info.comparison.unwrap_or(core::state::Comparison::Always)));

        let sampler = self.device.new_sampler(desc);

        self.share.handles.borrow_mut().make_sampler(native::Sampler(sampler), info)
    }

    fn map_buffer_raw(&mut self, _buf: &handle::RawBuffer<Resources>, _access: factory::MapAccess) -> RawMapping {
        unimplemented!()
    }

    fn unmap_buffer_raw(&mut self, _map: RawMapping) {
        unimplemented!()
    }

    fn map_buffer_readable<T: Copy>(&mut self, _buf: &handle::Buffer<Resources, T>)
                           -> core::mapping::Readable<T, Resources, Factory> {
        unimplemented!()
    }

    fn map_buffer_writable<T: Copy>(&mut self, _buf: &handle::Buffer<Resources, T>)
                                    -> core::mapping::Writable<T, Resources, Factory> {
        unimplemented!()
    }

    fn map_buffer_rw<T: Copy>(&mut self, _buf: &handle::Buffer<Resources, T>)
                              -> core::mapping::RW<T, Resources, Factory> {
        unimplemented!()
    }
}
