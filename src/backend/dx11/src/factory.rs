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
use std::collections::BTreeMap as Map;
use std::os::raw::c_void;
use std::sync::Arc;
use winapi;
use gfx_core as core;
use gfx_core::factory as f;
use gfx_core::handle as h;
use gfx_core::handle::Producer;
use {Resources as R, Share, Texture, Pipeline, Program, Shader};
use command::CommandBuffer;
use native;


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

struct TextureParam {
    levels: winapi::UINT,
    format: winapi::DXGI_FORMAT,
    bytes_per_texel: winapi::UINT,
    bind: winapi::D3D11_BIND_FLAG,
    usage: winapi::D3D11_USAGE,
    cpu_access: winapi::D3D11_CPU_ACCESS_FLAG,
    init: *const c_void,
}

impl TextureParam {
    fn to_sub_data(&self, w: core::tex::Size, h: core::tex::Size) -> winapi::D3D11_SUBRESOURCE_DATA {
        use winapi::UINT;
        winapi::D3D11_SUBRESOURCE_DATA {
            pSysMem: self.init,
            SysMemPitch: w as UINT * self.bytes_per_texel,
            SysMemSlicePitch: (w as UINT) * (h as UINT) * self.bytes_per_texel,
        }
    }
}

pub struct Factory {
    share: Arc<Share>,
    frame_handles: h::Manager<R>,
    vs_cache: Map<u64, Vec<u8>>,
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
            vs_cache: Map::new(),
        }
    }

    fn create_buffer_internal(&self, info: f::BufferInfo, raw_data: Option<*const c_void>)
                              -> Result<h::RawBuffer<R>, f::BufferError> {
        use winapi::d3d11::*;
        use data::{map_bind, map_usage};

        let (subind, size) = match info.role {
            f::BufferRole::Vertex   =>
                (D3D11_BIND_VERTEX_BUFFER, info.size),
            f::BufferRole::Index    => {
                if info.stride != 2 && info.stride != 4 {
                    error!("Only U16 and U32 index buffers are allowed");
                    return Err(f::BufferError::Other);
                }
                (D3D11_BIND_INDEX_BUFFER, info.size)
            },
            f::BufferRole::Uniform  => // 16 bit alignment
                (D3D11_BIND_CONSTANT_BUFFER, (info.size + 0xF) & !0xF),
        };

        let (usage, cpu) = map_usage(info.usage);
        let bind = map_bind(info.bind) | subind;
        if info.bind.contains(f::RENDER_TARGET) | info.bind.contains(f::DEPTH_STENCIL) {
            return Err(f::BufferError::UnsupportedBind(info.bind))
        }
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: size as winapi::UINT,
            Usage: usage,
            BindFlags: bind.0,
            CPUAccessFlags: cpu.0,
            MiscFlags: 0,
            StructureByteStride: 0, //TODO
        };
        let mut sub = D3D11_SUBRESOURCE_DATA {
            pSysMem: ptr::null(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let sub_raw = match raw_data {
            Some(data) => {
                sub.pSysMem = data;
                &sub as *const _
            },
            None => ptr::null(),
        };

        debug!("Creating Buffer with desc {:#?} and sub-data {:?}", desc, sub);
        let mut buf = native::Buffer(ptr::null_mut());
        let hr = unsafe {
            (*self.share.device).CreateBuffer(&desc, sub_raw, &mut buf.0)
        };
        if winapi::SUCCEEDED(hr) {
            Ok(self.share.handles.borrow_mut().make_buffer(buf, info))
        }else {
            error!("Failed to create a buffer with code {:x}", hr);
            Err(f::BufferError::Other)
        }
    }

    fn create_texture_1d(&mut self, size: core::tex::Size, array: core::tex::Layer,
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG) -> (winapi::HRESULT, Texture)
    {
        use winapi::UINT;
        let native_desc = winapi::D3D11_TEXTURE1D_DESC {
            Width: size as UINT,
            MipLevels: tp.levels,
            ArraySize: array as UINT,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = tp.to_sub_data(size, 0);
        debug!("Creating Texture1D with desc {:#?} and sub-data {:?}", native_desc, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.share.device).CreateTexture1D(&native_desc,
                if tp.init != ptr::null() {&sub_data} else {ptr::null()}, &mut raw)
        };
        (hr, Texture::D1(raw))
    }

    fn create_texture_2d(&mut self, size: [core::tex::Size; 2], array: core::tex::Layer, aa: core::tex::AaMode,
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG) -> (winapi::HRESULT, Texture)
    {
        use winapi::UINT;
        use data::map_anti_alias;

        let native_desc = winapi::D3D11_TEXTURE2D_DESC {
            Width: size[0] as UINT,
            Height: size[1] as UINT,
            MipLevels: tp.levels,
            ArraySize: array as UINT,
            Format: tp.format,
            SampleDesc: map_anti_alias(aa),
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = tp.to_sub_data(size[0], size[1]);
        debug!("Creating Texture2D with desc {:#?} and sub-data {:?}", native_desc, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.share.device).CreateTexture2D(&native_desc,
                if tp.init != ptr::null() {&sub_data} else {ptr::null()}, &mut raw)
        };
        (hr, Texture::D2(raw))
    }

    fn create_texture_3d(&mut self, size: [core::tex::Size; 3],
                         tp: TextureParam, misc: winapi::D3D11_RESOURCE_MISC_FLAG) -> (winapi::HRESULT, Texture)
    {
        use winapi::UINT;
        let native_desc = winapi::D3D11_TEXTURE3D_DESC {
            Width: size[0] as UINT,
            Height: size[1] as UINT,
            Depth: size[2] as UINT,
            MipLevels: tp.levels,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind.0,
            CPUAccessFlags: tp.cpu_access.0,
            MiscFlags: misc.0,
        };
        let sub_data = tp.to_sub_data(size[0], size[1]);
        debug!("Creating Texture3D with desc {:#?} and sub-data {:?}", native_desc, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.share.device).CreateTexture3D(&native_desc,
                if tp.init != ptr::null() {&sub_data} else {ptr::null()}, &mut raw)
        };
        (hr, Texture::D3(raw))
    }

    fn create_texture_internal(&mut self, desc: core::tex::Descriptor,
                               init_opt: Option<(&[u8], core::format::ChannelType, bool)>)
                               -> Result<h::RawTexture<R>, core::tex::Error>
    {
        use gfx_core::tex::{AaMode, Error, Kind};
        use data::{map_bind, map_usage, map_surface};

        let (usage, cpu_access) = map_usage(desc.usage);
        let tparam = TextureParam {
            levels: desc.levels as winapi::UINT,
            format: match map_surface(desc.format) {
                Some(f) => f,
                None => return Err(Error::Format(desc.format, None))
            },
            bytes_per_texel: (desc.format.get_total_bits() >> 3) as winapi::UINT,
            bind: map_bind(desc.bind),
            usage: usage,
            cpu_access: cpu_access,
            init: match init_opt {
                Some((data, _, _)) => data.as_ptr() as *const c_void,
                None => ptr::null(),
            },
        };
        let misc = match init_opt {
            Some((_, _, true)) => winapi::D3D11_RESOURCE_MISC_GENERATE_MIPS,
            _ => winapi::D3D11_RESOURCE_MISC_FLAG(0),
        };
        let (hr, texture) = match desc.kind {
            Kind::D1(w) =>
                self.create_texture_1d(w, 1, tparam, misc),
            Kind::D1Array(w, d) =>
                self.create_texture_1d(w, d, tparam, misc),
            Kind::D2(w, h, aa) =>
                self.create_texture_2d([w,h], 1, aa, tparam, misc),
            Kind::D2Array(w, h, d, aa) =>
                self.create_texture_2d([w,h], d, aa, tparam, misc),
            Kind::D3(w, h, d) =>
                self.create_texture_3d([w,h,d], tparam, misc),
            Kind::Cube(w) =>
                self.create_texture_2d([w,w], 6*1, AaMode::Single, tparam, misc | winapi::D3D11_RESOURCE_MISC_TEXTURECUBE),
            Kind::CubeArray(w, d) =>
                self.create_texture_2d([w,w], 6*d, AaMode::Single, tparam, misc | winapi::D3D11_RESOURCE_MISC_TEXTURECUBE),
        };

        if winapi::SUCCEEDED(hr) {
            Ok(self.share.handles.borrow_mut().make_texture(texture, desc))
        }else {
            error!("Failed to create a texture with code {:x}", hr);
            Err(Error::Kind) //we should check for the error code here
        }
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

    fn create_buffer_raw(&mut self, info: f::BufferInfo) -> Result<h::RawBuffer<R>, f::BufferError> {
        self.create_buffer_internal(info, None)
    }

    fn create_buffer_const_raw(&mut self, data: &[u8], stride: usize, role: f::BufferRole, bind: f::Bind)
                                -> Result<h::RawBuffer<R>, f::BufferError> {
        let info = f::BufferInfo {
            role: role,
            usage: f::Usage::Const,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        self.create_buffer_internal(info, Some(data.as_ptr() as *const c_void))
    }

    fn create_shader(&mut self, stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use winapi::ID3D11DeviceChild;
        use gfx_core::shade::{CreateShaderError, Stage};
        use mirror::reflect_shader;

        let dev = self.share.device;
        let len = code.len() as winapi::SIZE_T;
        let (hr, object) = match stage {
            Stage::Vertex => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateVertexShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Geometry => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateGeometryShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            Stage::Pixel => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreatePixelShader(code.as_ptr() as *const c_void, len, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut ID3D11DeviceChild)
            },
            //_ => return Err(CreateShaderError::StageNotSupported(stage))
        };

        if winapi::SUCCEEDED(hr) {
            let reflection = reflect_shader(code);
            let hash = {
                use std::hash::{Hash, Hasher, SipHasher};
                let mut hasher = SipHasher::new();
                code.hash(&mut hasher);
                hasher.finish()
            };
            if stage == Stage::Vertex {
                self.vs_cache.insert(hash, code.to_owned());
            }
            let shader = Shader {
                object: object,
                reflection: reflection,
                code_hash: hash,
            };
            Ok(self.share.handles.borrow_mut().make_shader(shader))
        }else {
            Err(CreateShaderError::CompilationFailed(format!("code {}", hr)))
        }
    }

    fn create_program(&mut self, shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        use winapi::{ID3D11VertexShader, ID3D11GeometryShader, ID3D11PixelShader};
        use gfx_core::shade::{ProgramInfo, Stage};
        use mirror::populate_info;

        let mut info = ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            knows_outputs: true,
        };
        let fh = &mut self.frame_handles;
        let prog = match shader_set {
            &core::ShaderSet::Simple(ref vs, ref ps) => {
                let (vs, ps) = (vs.reference(fh), ps.reference(fh));
                populate_info(&mut info, Stage::Vertex, vs.reflection);
                populate_info(&mut info, Stage::Pixel,  ps.reflection);
                unsafe { (*vs.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut ID3D11VertexShader,
                    gs: ptr::null_mut(),
                    ps: ps.object as *mut ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
            &core::ShaderSet::Geometry(ref vs, ref gs, ref ps) => {
                let (vs, gs, ps) = (vs.reference(fh), gs.reference(fh), ps.reference(fh));
                populate_info(&mut info, Stage::Vertex,   vs.reflection);
                populate_info(&mut info, Stage::Geometry, gs.reflection);
                populate_info(&mut info, Stage::Pixel,    ps.reflection);
                unsafe { (*vs.object).AddRef(); (*gs.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut ID3D11VertexShader,
                    gs: vs.object as *mut ID3D11GeometryShader,
                    ps: ps.object as *mut ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
        };
        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        use winapi::d3dcommon::*;
        use gfx_core::Primitive::*;
        use data::map_format;
        use state;

        let mut layouts = Vec::new();
        let mut charbuf = [0; 256];
        let mut charpos = 0;
        for (attrib, at_desc) in program.get_info().vertex_attributes.iter().zip(desc.attributes.iter()) {
            use winapi::UINT;
            let (elem, irate) = match at_desc {
                &Some((ref el, ir)) => (el, ir),
                &None => continue,
            };
            layouts.push(winapi::D3D11_INPUT_ELEMENT_DESC {
                SemanticName: &charbuf[charpos],
                SemanticIndex: 0,
                Format: match map_format(elem.format, false) {
                    Some(fm) => fm,
                    None => {
                        error!("Unable to find DXGI format for {:?}", elem.format);
                        return Err(core::pso::CreationError);
                    }
                },
                InputSlot: attrib.slot as UINT,
                AlignedByteOffset: elem.offset as UINT,
                InputSlotClass: if irate == 0 {
                    winapi::D3D11_INPUT_PER_VERTEX_DATA
                }else {
                    winapi::D3D11_INPUT_PER_INSTANCE_DATA
                },
                InstanceDataStepRate: irate as UINT,
            });
            for (out, inp) in charbuf[charpos..].iter_mut().zip(attrib.name.as_bytes().iter()) {
                *out = *inp as i8;
            }
            charpos += attrib.name.as_bytes().len() + 1;
        }

        let prog = *self.frame_handles.ref_program(program);
        let vs_bin = match self.vs_cache.get(&prog.vs_hash) {
            Some(ref code) => &code[..],
            None => {
                error!("VS hash {} is not found in the factory cache", prog.vs_hash);
                return Err(core::pso::CreationError);
            }
        };

        let dev = self.share.device;
        let mut vertex_layout = ptr::null_mut();
        let hr = unsafe {
            (*dev).CreateInputLayout(
                layouts.as_ptr(), layouts.len() as winapi::UINT,
                vs_bin.as_ptr() as *const c_void, vs_bin.len() as winapi::SIZE_T,
                &mut vertex_layout)
        };
        if !winapi::SUCCEEDED(hr) {
            error!("Failed to create input layout from {:?}, error {:x}", layouts, hr);
            return Err(core::pso::CreationError);
        }
        let dummy_dsi = core::pso::DepthStencilInfo { depth: None, front: None, back: None };
        //TODO: cache rasterizer, depth-stencil, and blend states

        let pso = Pipeline {
            topology: match desc.primitive {
                PointList       => D3D11_PRIMITIVE_TOPOLOGY_POINTLIST,
                LineList        => D3D11_PRIMITIVE_TOPOLOGY_LINELIST,
                LineStrip       => D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP,
                TriangleList    => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                TriangleStrip   => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            },
            layout: vertex_layout,
            attributes: desc.attributes,
            program: prog,
            rasterizer: state::make_rasterizer(dev, &desc.rasterizer, desc.scissor),
            depth_stencil: state::make_depth_stencil(dev, match desc.depth_stencil {
                Some((_, ref dsi)) => dsi,
                None => &dummy_dsi,
            }),
            blend: state::make_blend(dev, &desc.color_targets),
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: core::tex::Descriptor, _hint: Option<core::format::ChannelType>)
                          -> Result<h::RawTexture<R>, core::tex::Error> {
        self.create_texture_internal(desc, None)
    }

    fn create_texture_with_data(&mut self, desc: core::tex::Descriptor, channel: core::format::ChannelType,
                                data: &[u8], mipmap: bool) -> Result<core::handle::RawTexture<R>, core::tex::Error> {
        self.create_texture_internal(desc, Some((data, channel, mipmap)))
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                      -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, desc: core::tex::ResourceDesc)
                                       -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        use winapi::UINT;
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
        unsafe {
            (*self.share.device).CreateShaderResourceView(raw_tex, &native_desc, &mut raw_view);
        }
        Ok(self.share.handles.borrow_mut().make_texture_srv(native::Srv(raw_view), htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, desc: core::tex::RenderDesc)
                                         -> Result<h::RawRenderTargetView<R>, f::TargetViewError>
    {
        use winapi::UINT;
        use gfx_core::tex::{AaMode, Kind};
        use data::map_format;

        let level = desc.level as UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D1Array(_, nlayers), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as UINT]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as UINT, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [lid as UINT, 1+lid as UINT, 0]),
            (Kind::D3(_, _, depth), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE3D, [level, 0, depth as UINT]),
            (Kind::D3(_, _, depth), Some(lid)) if lid < depth =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE3D, [level, lid as UINT, 1+lid as UINT]),
            (Kind::Cube(..), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as UINT, 1+lid as UINT]),
            (Kind::CubeArray(_, nlayers), None) =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6 * nlayers as UINT]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (winapi::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 6 * lid as UINT, 6 * (1+lid) as UINT]),
            (_, None) => return Err(f::TargetViewError::BadLevel(desc.level)),
            (_, Some(lid)) => return Err(f::TargetViewError::BadLayer(lid)),
        };
        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = winapi::D3D11_RENDER_TARGET_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(f::TargetViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: extra,
        };
        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).to_resource();
        unsafe {
            (*self.share.device).CreateRenderTargetView(raw_tex, &native_desc, &mut raw_view);
        }
        let size = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_rtv(native::Rtv(raw_view), htex, size))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &h::RawTexture<R>, desc: core::tex::DepthStencilDesc)
                                         -> Result<h::RawDepthStencilView<R>, f::TargetViewError>
    {
        use winapi::UINT;
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
        unsafe {
            (*self.share.device).CreateDepthStencilView(raw_tex, &native_desc, &mut raw_view);
        }
        let dim = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_view), htex, dim))
    }

    fn create_sampler(&mut self, info: core::tex::SamplerInfo) -> h::Sampler<R> {
        use gfx_core::tex::FilterMethod;
        use data::{FilterOp, map_function, map_filter, map_wrap};

        let op = if info.comparison.is_some() {FilterOp::Comparison} else {FilterOp::Product};
        let native_desc = winapi::D3D11_SAMPLER_DESC {
            Filter: map_filter(info.filter, op),
            AddressU: map_wrap(info.wrap_mode.0),
            AddressV: map_wrap(info.wrap_mode.1),
            AddressW: map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.filter {
                FilterMethod::Anisotropic(max) => max as winapi::UINT,
                _ => 0,
            },
            ComparisonFunc: map_function(info.comparison.unwrap_or(core::state::Comparison::Always)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.0.into(),
            MaxLOD: info.lod_range.1.into(),
        };

        let mut raw_sampler = ptr::null_mut();
        let hr = unsafe {
            (*self.share.device).CreateSamplerState(&native_desc, &mut raw_sampler)
        };
        if winapi::SUCCEEDED(hr) {
            self.share.handles.borrow_mut().make_sampler(native::Sampler(raw_sampler), info)
        }else {
            error!("Unable to create a sampler with desc {:?}, error {:x}", info, hr);
            unimplemented!()
        }
    }

    fn update_buffer_raw(&mut self, _buffer: &h::RawBuffer<R>, _data: &[u8],
                         _offset_bytes: usize) -> Result<(), f::BufferUpdateError> {
        Ok(()) //TODO
    }

    fn update_texture_raw(&mut self, _texture: &h::RawTexture<R>, _image: &core::tex::RawImageInfo,
                          _data: &[u8], _face: Option<core::tex::CubeFace>) -> Result<(), core::tex::Error> {
        Ok(()) //TODO
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
