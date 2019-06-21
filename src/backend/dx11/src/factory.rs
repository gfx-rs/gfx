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

use std::{cmp, mem, ptr, slice};
use std::collections::BTreeMap as Map;
use std::os::raw::c_void;
use std::sync::Arc;

use winapi::um::{d3d11, d3dcommon};
use winapi::shared::{dxgiformat, minwindef, winerror};

use core::{self, factory as f, buffer, texture, mapping};
use core::format::SurfaceTyped;
use core::memory::{Bind, Typed, Usage};
use core::handle::{self as h, Producer};
use {Resources as R, Share, Buffer, Texture, Pipeline, Program, Shader};
use command::CommandBuffer;
use {CommandList, DeferredContext};
use native;


#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MappingGate {
    pointer: *mut c_void,
}

unsafe impl Send for MappingGate {}
unsafe impl Sync for MappingGate {}

impl core::mapping::Gate<R> for MappingGate {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        slice::from_raw_parts_mut(self.pointer as *mut T, len)
    }
}

#[derive(Debug)]
struct TextureParam {
    levels: minwindef::UINT,
    format: dxgiformat::DXGI_FORMAT,
    bytes_per_texel: minwindef::UINT,
    bind: d3d11::D3D11_BIND_FLAG,
    usage: d3d11::D3D11_USAGE,
    cpu_access: d3d11::D3D11_CPU_ACCESS_FLAG,
}

pub struct Factory {
    device: *mut d3d11::ID3D11Device,
    share: Arc<Share>,
    frame_handles: h::Manager<R>,
    vs_cache: Map<u64, Vec<u8>>,
    /// Create typed surface formats for the textures. This is useful for debugging
    /// with PIX, since it doesn't understand typeless formats. This may also prevent
    /// some valid views to be created because the typed formats can't be reinterpret.
    use_texture_format_hint: bool,
    sub_data_array: Vec<d3d11::D3D11_SUBRESOURCE_DATA>,
}

impl Clone for Factory {
    fn clone(&self) -> Factory {
        unsafe { (*self.device).AddRef(); }
        Factory::new(self.device, self.share.clone())
    }
}

impl Drop for Factory {
    fn drop(&mut self) {
        unsafe { (*self.device).Release(); }
    }
}

impl Factory {
    /// Create a new `Factory`.
    pub fn new(device: *mut d3d11::ID3D11Device, share: Arc<Share>) -> Factory {
        Factory {
            device: device,
            share: share,
            frame_handles: h::Manager::new(),
            vs_cache: Map::new(),
            use_texture_format_hint: false,
            sub_data_array: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn wrap_back_buffer(&mut self, back_buffer: *mut d3d11::ID3D11Texture2D, info: texture::Info,
                            desc: texture::RenderDesc) -> h::RawRenderTargetView<R> {
        use core::Factory;
        let raw_tex = Texture(native::Texture::D2(back_buffer));
        let color_tex = self.share.handles.borrow_mut().make_texture(raw_tex, info);
        self.view_texture_as_render_target_raw(&color_tex, desc).unwrap()
    }

    pub fn create_command_buffer(&self) -> CommandBuffer<CommandList> {
        CommandList::new().into()
    }

    pub fn create_command_buffer_native(&self) -> CommandBuffer<DeferredContext> {
        let mut dc = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateDeferredContext(0, &mut dc)
        };
        if winerror::SUCCEEDED(hr) {
            DeferredContext::new(dc).into()
        }else {
            panic!("Failed to create a deferred context")
        }
    }

    fn create_buffer_internal(&self, info: buffer::Info, raw_data: Option<*const c_void>)
                              -> Result<h::RawBuffer<R>, buffer::CreationError> {
        use data::{map_bind, map_usage};

        // we are not allowed to pass size=0, 
        // otherwise it panics
        let buffer_size = if info.size == 0 {
            1
        } else {
            info.size
        };

        let (subind, size) = match info.role {
            buffer::Role::Vertex   =>
                (d3d11::D3D11_BIND_VERTEX_BUFFER, buffer_size),
            buffer::Role::Index    => {
                if info.stride != 2 && info.stride != 4 {
                    error!("Only U16 and U32 index buffers are allowed");
                    return Err(buffer::CreationError::Other);
                }
                (d3d11::D3D11_BIND_INDEX_BUFFER, buffer_size)
            },
            buffer::Role::Constant  => // 16 bit alignment
                (d3d11::D3D11_BIND_CONSTANT_BUFFER, (buffer_size + 0xF) & !0xF),
            buffer::Role::Staging =>
                (0, buffer_size)
        };

        assert!(size >= info.size);
        let (usage, cpu) = map_usage(info.usage, info.bind);
        let bind = map_bind(info.bind) | subind;
        if info.bind.contains(Bind::RENDER_TARGET) | info.bind.contains(Bind::DEPTH_STENCIL) {
            return Err(buffer::CreationError::UnsupportedBind(info.bind))
        }
        let native_desc = d3d11::D3D11_BUFFER_DESC {
            ByteWidth: size as _,
            Usage: usage,
            BindFlags: bind,
            CPUAccessFlags: cpu,
            MiscFlags: 0,
            StructureByteStride: 0, //TODO
        };
        let mut sub = d3d11::D3D11_SUBRESOURCE_DATA {
            pSysMem: ptr::null(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let sub_raw = match raw_data {
            Some(data) => {
                sub.pSysMem = data as _;
                &sub as *const _
            },
            None => ptr::null(),
        };

        debug!("Creating Buffer with info {:?} and sub-data ?", info/*, sub*/);
        let mut raw_buf = native::Buffer(ptr::null_mut());
        let hr = unsafe {
            (*self.device).CreateBuffer(&native_desc, sub_raw, &mut raw_buf.0)
        };
        if winerror::SUCCEEDED(hr) {
            let buf = Buffer(raw_buf);

            let mapping = match info.usage {
                Usage::Data | Usage::Dynamic => None,
                Usage::Upload | Usage::Download => Some(MappingGate { pointer: ptr::null_mut() }),
            };

            Ok(self.share.handles.borrow_mut().make_buffer(buf, info, mapping))
        } else {
            error!("Failed to create a buffer with desc ?, error {:x}"/*, native_desc*/, hr);
            Err(buffer::CreationError::Other)
        }
    }

    fn update_sub_data(&mut self, w: texture::Size, h: texture::Size, bpt: minwindef::UINT)
                       -> *const d3d11::D3D11_SUBRESOURCE_DATA {
        for sub in self.sub_data_array.iter_mut() {
            sub.SysMemPitch = w as minwindef::UINT * bpt;
            sub.SysMemSlicePitch = (h as minwindef::UINT) * sub.SysMemPitch;
        }
        self.sub_data_array.as_ptr()
    }

    fn create_texture_1d(&mut self, size: texture::Size, array: texture::Layer,
                         tp: TextureParam, misc: d3d11::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winerror::HRESULT>
    {
        let native_desc = d3d11::D3D11_TEXTURE1D_DESC {
            Width: size as _,
            MipLevels: tp.levels,
            ArraySize: array as _,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind,
            CPUAccessFlags: tp.cpu_access,
            MiscFlags: misc,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            let num_data = array as usize * cmp::max(1, tp.levels) as usize;
            if num_data != self.sub_data_array.len() {
                error!("Texture1D with {} slices and {} levels is given {} data chunks",
                    array, tp.levels, self.sub_data_array.len());
                return Err(winerror::S_OK)
            }
            self.update_sub_data(size, 0, tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture1D with size {:?}, layer {}, param {:?}, and sub-data {:p}",
            size, array, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture1D(&native_desc, sub_data, &mut raw)
        };
        if winerror::SUCCEEDED(hr) {
            Ok(native::Texture::D1(raw))
        }else {
            error!("CreateTexture1D failed on ? with error {:x}"/*, native_desc*/, hr);
            Err(hr)
        }
    }

    fn create_texture_2d(&mut self, size: [texture::Size; 2], array: texture::Layer, aa: texture::AaMode,
                         tp: TextureParam, misc: d3d11::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winerror::HRESULT>
    {
        use data::map_anti_alias;

        let native_desc = d3d11::D3D11_TEXTURE2D_DESC {
            Width: size[0] as _,
            Height: size[1] as _,
            MipLevels: tp.levels,
            ArraySize: array as _,
            Format: tp.format,
            SampleDesc: map_anti_alias(aa),
            Usage: tp.usage,
            BindFlags: tp.bind,
            CPUAccessFlags: tp.cpu_access,
            MiscFlags: misc,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            let num_data = array as usize * cmp::max(1, tp.levels) as usize;
            if num_data != self.sub_data_array.len() {
                error!("Texture2D with {} slices and {} levels is given {} data chunks",
                    array, tp.levels, self.sub_data_array.len());
                return Err(winerror::S_OK)
            }
            self.update_sub_data(size[0], size[1], tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture2D with size {:?}, layer {}, param {:?}, and sub-data {:p}",
            size, array, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture2D(&native_desc, sub_data, &mut raw)
        };
        if winerror::SUCCEEDED(hr) {
            Ok(native::Texture::D2(raw))
        }else {
            error!("CreateTexture2D failed on ? with error {:x}"/*, native_desc*/, hr);
            Err(hr)
        }
    }

    fn create_texture_3d(&mut self, size: [texture::Size; 3],
                         tp: TextureParam, misc: d3d11::D3D11_RESOURCE_MISC_FLAG)
                         -> Result<native::Texture, winerror::HRESULT>
    {
        let native_desc = d3d11::D3D11_TEXTURE3D_DESC {
            Width: size[0] as _,
            Height: size[1] as _,
            Depth: size[2] as _,
            MipLevels: tp.levels,
            Format: tp.format,
            Usage: tp.usage,
            BindFlags: tp.bind,
            CPUAccessFlags: tp.cpu_access,
            MiscFlags: misc,
        };
        let sub_data = if self.sub_data_array.len() > 0 {
            if cmp::max(1, tp.levels) as usize != self.sub_data_array.len() {
                error!("Texture3D with {} levels is given {} data chunks",
                    tp.levels, self.sub_data_array.len());
                return Err(winerror::S_OK)
            }
            self.update_sub_data(size[0], size[1], tp.bytes_per_texel)
        }else {
            ptr::null()
        };

        debug!("Creating Texture3D with size {:?}, param {:?}, and sub-data {:p}",
            size, tp, sub_data);
        let mut raw = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateTexture3D(&native_desc, sub_data, &mut raw)
        };
        if winerror::SUCCEEDED(hr) {
            Ok(native::Texture::D3(raw))
        }else {
            error!("CreateTexture3D failed on ? with error {:x}"/*, native_desc*/, hr);
            Err(hr)
        }
    }

    pub fn cleanup(&mut self) {
        self.frame_handles.clear();
    }

    /// Read a download-usage texture contents. Useful for screenshot readbacks.
    pub fn map_texture_read<'a, 'b, S: Copy + SurfaceTyped>(
        &'a mut self, texture: &'b h::Texture<R, S>
    ) -> (&'b [S::DataType], usize) {
        let info = texture.get_info();
        let (width, height, _, aa) = info.kind.get_dimensions();
        assert_eq!(info.usage, Usage::Download);
        assert_eq!(aa, texture::AaMode::Single);
        let _mip0_texels = width as usize * height as usize;
        let texel_size = S::get_surface_type().get_total_bits() / 8;

        let mut ctx = ptr::null_mut();
        unsafe {
            (*self.device).GetImmediateContext(&mut ctx);
        }

        let mut sres = d3d11::D3D11_MAPPED_SUBRESOURCE {
            pData: ptr::null_mut(),
            RowPitch: 0,
            DepthPitch: 0,
        };

        let resource = self.frame_handles
            .ref_texture(texture.raw())
            .as_resource();
        let hr = unsafe {
            (*ctx).Map(resource as *mut _, 0, d3d11::D3D11_MAP_READ, 0, &mut sres)
        };

        if winerror::SUCCEEDED(hr) {
            unsafe {
                (slice::from_raw_parts(sres.pData as *const _, sres.DepthPitch as usize / texel_size as usize), sres.RowPitch as usize / texel_size as usize)
            }
        } else {
            panic!("Unable to map a texture {:?}, error {:x}", texture, hr);
        }
    }

    pub fn unmap_texture<S>(&mut self, texture: &h::Texture<R, S>) {
        let resource = self.frame_handles
            .ref_texture(texture.raw())
            .as_resource()
            as *mut d3d11::ID3D11Resource;

        let mut ctx = ptr::null_mut();
        unsafe {
            (*self.device).GetImmediateContext(&mut ctx);
            (*ctx).Unmap(resource as *mut _, 0);
        }
    }
}

impl core::Factory<R> for Factory {
    fn get_capabilities(&self) -> &core::Capabilities {
        &self.share.capabilities
    }

    fn create_buffer_raw(&mut self, info: buffer::Info) -> Result<h::RawBuffer<R>, buffer::CreationError> {
        self.create_buffer_internal(info, None)
    }

    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, role: buffer::Role, bind: Bind)
                                -> Result<h::RawBuffer<R>, buffer::CreationError> {
        let info = buffer::Info {
            role: role,
            usage: Usage::Data,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        self.create_buffer_internal(info, Some(data.as_ptr() as *const c_void))
    }

    fn create_shader(&mut self, stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use core::shade::{CreateShaderError, Stage};
        use mirror::reflect_shader;

        let dev = self.device;
        let (hr, object) = match stage {
            Stage::Vertex => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateVertexShader(code.as_ptr() as *const _, code.len() as _, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut d3d11::ID3D11DeviceChild)
            },
            Stage::Hull => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateHullShader(code.as_ptr() as *const _, code.len() as _, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut d3d11::ID3D11DeviceChild)
            },
            Stage::Domain => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateDomainShader(code.as_ptr() as *const _, code.len() as _, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut d3d11::ID3D11DeviceChild)
            },
            Stage::Geometry => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreateGeometryShader(code.as_ptr() as *const _, code.len() as _, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut d3d11::ID3D11DeviceChild)
            },
            Stage::Pixel => {
                let mut ret = ptr::null_mut();
                let hr = unsafe {
                    (*dev).CreatePixelShader(code.as_ptr() as *const _, code.len() as _, ptr::null_mut(), &mut ret)
                };
                (hr, ret as *mut d3d11::ID3D11DeviceChild)
            },
            //_ => return Err(CreateShaderError::StageNotSupported(stage))
        };

        if winerror::SUCCEEDED(hr) {
            let reflection = reflect_shader(code);
            let hash = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
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
        use core::shade::{ProgramInfo, Stage};
        use mirror::populate_info;

        let mut info = ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            output_depth: false,
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
                    vs: vs.object as *mut d3d11::ID3D11VertexShader,
                    hs: ptr::null_mut(),
                    ds: ptr::null_mut(),
                    gs: ptr::null_mut(),
                    ps: ps.object as *mut d3d11::ID3D11PixelShader,
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
                    vs: vs.object as *mut d3d11::ID3D11VertexShader,
                    hs: ptr::null_mut(),
                    ds: ptr::null_mut(),
                    gs: gs.object as *mut d3d11::ID3D11GeometryShader,
                    ps: ps.object as *mut d3d11::ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
            &core::ShaderSet::Tessellated(ref vs, ref hs, ref ds, ref ps) => {
                let (vs, hs, ds, ps) = (vs.reference(fh), hs.reference(fh), ds.reference(fh), ps.reference(fh));

                populate_info(&mut info, Stage::Vertex, vs.reflection);
                populate_info(&mut info, Stage::Hull,   hs.reflection);
                populate_info(&mut info, Stage::Domain, ds.reflection);
                populate_info(&mut info, Stage::Pixel,  ps.reflection);
                unsafe { (*vs.object).AddRef(); (*hs.object).AddRef(); (*ds.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut d3d11::ID3D11VertexShader,
                    hs: hs.object as *mut d3d11::ID3D11HullShader,
                    ds: ds.object as *mut d3d11::ID3D11DomainShader,
                    gs: ptr::null_mut(),
                    ps: ps.object as *mut d3d11::ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            },
            &core::ShaderSet::TessellatedGeometry(ref vs, ref hs, ref ds, ref gs, ref ps) => {
                let (vs, hs, ds, gs, ps) = (vs.reference(fh), hs.reference(fh), ds.reference(fh), gs.reference(fh), ps.reference(fh));

                populate_info(&mut info, Stage::Vertex, vs.reflection);
                populate_info(&mut info, Stage::Hull,   hs.reflection);
                populate_info(&mut info, Stage::Domain, ds.reflection);
                populate_info(&mut info, Stage::Geometry, gs.reflection);
                populate_info(&mut info, Stage::Pixel,  ps.reflection);
                unsafe { (*vs.object).AddRef(); (*hs.object).AddRef(); (*ds.object).AddRef(); (*gs.object).AddRef(); (*ps.object).AddRef(); }
                Program {
                    vs: vs.object as *mut d3d11::ID3D11VertexShader,
                    hs: hs.object as *mut d3d11::ID3D11HullShader,
                    ds: ds.object as *mut d3d11::ID3D11DomainShader,
                    gs: gs.object as *mut d3d11::ID3D11GeometryShader,
                    ps: ps.object as *mut d3d11::ID3D11PixelShader,
                    vs_hash: vs.code_hash,
                }
            }

        };
        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        use core::Primitive::*;
        use data::map_format;
        use state;

        let mut layouts = Vec::new();
        let mut charbuf = [0; 256];
        let mut charpos = 0;
        for (attrib, at_desc) in program.get_info().vertex_attributes.iter().zip(desc.attributes.iter()) {
            let (bdesc, elem) = match at_desc {
                &Some((buf_id, ref el)) => match desc.vertex_buffers[buf_id as usize] {
                    Some(ref bd) => (bd, el),
                    None => return Err(core::pso::CreationError),
                },
                &None => continue,
            };
            if elem.offset & 1 != 0 {
                error!("Vertex attribute {} must be aligned to 2 bytes, has offset {}",
                    attrib.name, elem.offset);
                return Err(core::pso::CreationError);
            }
            let vertex_semantic: VertexSemantic = attrib.name.as_str().into();
            layouts.push(d3d11::D3D11_INPUT_ELEMENT_DESC {
                SemanticName: &charbuf[charpos],
                SemanticIndex: vertex_semantic.index,
                Format: match map_format(elem.format, false) {
                    Some(fm) => fm,
                    None => {
                        error!("Unable to find DXGI format for {:?}", elem.format);
                        return Err(core::pso::CreationError);
                    }
                },
                InputSlot: attrib.slot as _, // NOTE: gfx_backend_dx11 has a vertex buffer binding per attribute.
                AlignedByteOffset: elem.offset as _,
                InputSlotClass: if bdesc.rate == 0 {
                    d3d11::D3D11_INPUT_PER_VERTEX_DATA
                }else {
                    d3d11::D3D11_INPUT_PER_INSTANCE_DATA
                },
                InstanceDataStepRate: bdesc.rate as _,
            });
            for (out, inp) in charbuf[charpos..].iter_mut().zip(vertex_semantic.name.as_bytes().iter()) {
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

        let dev = self.device;
        let mut vertex_layout = ptr::null_mut();
        let hr = unsafe {
            (*dev).CreateInputLayout(
                layouts.as_ptr(), layouts.len() as _,
                vs_bin.as_ptr() as *const _, vs_bin.len() as _,
                &mut vertex_layout)
        };
        if !winerror::SUCCEEDED(hr) {
            error!("Failed to create input layout from ?, error {:x}"/*, layouts*/, hr);
            return Err(core::pso::CreationError);
        }
        let dummy_dsi = core::pso::DepthStencilInfo { depth: None, front: None, back: None };
        //TODO: cache rasterizer, depth-stencil, and blend states
        let caps = &self.share.capabilities;

        let pso = Pipeline {
            topology: match desc.primitive {
                PointList       => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_POINTLIST,
                LineList        => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_LINELIST,
                LineStrip       => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP,
                TriangleList    => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                TriangleStrip   => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
                LineListAdjacency        => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
                LineStripAdjacency       => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
                TriangleListAdjacency    => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
                TriangleStripAdjacency   => d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ,
                PatchList(num)  => {
                    if num == 0 || (num as usize) > caps.max_patch_size {
                        return Err(core::pso::CreationError)
                    }
                    d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST + (num as u32) - 1
                },
            },
            layout: vertex_layout,
            vertex_buffers: desc.vertex_buffers,
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

    fn create_texture_raw(&mut self, desc: texture::Info, hint: Option<core::format::ChannelType>,
                          data_opt: Option<(&[&[u8]], texture::Mipmap)>) -> Result<h::RawTexture<R>, texture::CreationError> {
        use core::texture::{AaMode, CreationError, Kind};
        use data::{map_bind, map_usage, map_surface, map_format};
        
        if let Some((_, texture::Mipmap::Allocated)) = data_opt {
        	return Err(texture::CreationError::Mipmap);
        }

        let (usage, cpu_access) = map_usage(desc.usage, desc.bind);
        let tparam = TextureParam {
            levels: desc.levels as _,
            format: match hint {
                Some(channel) if self.use_texture_format_hint && !desc.bind.contains(Bind::DEPTH_STENCIL) => {
                    match map_format(core::format::Format(desc.format, channel), true) {
                        Some(f) => f,
                        None => return Err(CreationError::Format(desc.format, Some(channel)))
                    }
                },
                _ => match map_surface(desc.format) {
                    Some(f) => f,
                    None => return Err(CreationError::Format(desc.format, None))
                },
            },
            bytes_per_texel: (desc.format.get_total_bits() >> 3) as _,
            bind: map_bind(desc.bind),
            usage: usage,
            cpu_access: cpu_access,
        };

        self.sub_data_array.clear();
        if let Some(data) = data_opt {
            for sub in data.0.iter() {
                self.sub_data_array.push(d3d11::D3D11_SUBRESOURCE_DATA {
                    pSysMem: sub.as_ptr() as *const _,
                    SysMemPitch: 0,
                    SysMemSlicePitch: 0,
                });
            }
        };
        let misc = if usage != d3d11::D3D11_USAGE_IMMUTABLE &&
            desc.bind.contains(Bind::RENDER_TARGET | Bind::SHADER_RESOURCE) &&
            desc.levels > 1 && data_opt.is_none() {
            d3d11::D3D11_RESOURCE_MISC_GENERATE_MIPS
        }else {
            0
        };

        let texture_result = match desc.kind {
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
                self.create_texture_2d([w,w], 6*1, AaMode::Single, tparam, misc | d3d11::D3D11_RESOURCE_MISC_TEXTURECUBE),
            Kind::CubeArray(w, d) =>
                self.create_texture_2d([w,w], 6*d, AaMode::Single, tparam, misc | d3d11::D3D11_RESOURCE_MISC_TEXTURECUBE),
        };

        match texture_result {
            Ok(native) => {
                let tex = Texture(native);
                Ok(self.share.handles.borrow_mut().make_texture(tex, desc))
            },
            Err(_) => Err(CreationError::Kind),
        }
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &h::RawBuffer<R>, _: core::format::Format)
                                      -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::ResourceDesc)
                                       -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        use core::texture::{AaMode, Kind};
        use data::map_format;
        //TODO: support desc.layer parsing

        let (dim, layers, has_levels) = match htex.get_info().kind {
            Kind::D1(_) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE1D, 1, true),
            Kind::D1Array(_, d) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE1DARRAY, d, true),
            Kind::D2(_, _, AaMode::Single) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D, 1, true),
            Kind::D2(_, _, _) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2DMS, 1, false),
            Kind::D2Array(_, _, d, AaMode::Single) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2DARRAY, d, true),
            Kind::D2Array(_, _, d, _) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2DMSARRAY, d, false),
            Kind::D3(_, _, _) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURE3D, 1, true),
            Kind::Cube(_) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURECUBE, 1, true),
            Kind::CubeArray(_, d) =>
                (d3dcommon::D3D11_SRV_DIMENSION_TEXTURECUBEARRAY, d, true),
        };

        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: match map_format(format, false) {
                Some(fm) => fm,
                None => return Err(f::ResourceViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: unsafe {
                let array = if has_levels {
                    assert!(desc.max >= desc.min);
                    [desc.min as _, (desc.max + 1 - desc.min) as _, 0, layers as _]
                }else {
                    [0, layers as _, 0, 0]
                };
                mem::transmute(array) //TODO
            },
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateShaderResourceView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winerror::SUCCEEDED(hr) {
            error!("Failed to create SRV from ?, error {:x}"/*, native_desc*/, hr);
            return Err(f::ResourceViewError::Unsupported);
        }
        Ok(self.share.handles.borrow_mut().make_texture_srv(native::Srv(raw_view), htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::RenderDesc)
                                         -> Result<h::RawRenderTargetView<R>, f::TargetViewError>
    {
        use core::texture::{AaMode, Kind};
        use data::map_format;

        let level = desc.level as minwindef::UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::D1Array(_, nlayers), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as _]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as _]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as _, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY, [lid as _, (1+lid) as _, 0]),
            (Kind::D3(_, _, depth), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE3D, [level, 0, depth as _]),
            (Kind::D3(_, _, depth), Some(lid)) if lid < depth =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE3D, [level, lid as _, (1+lid) as _]),
            (Kind::Cube(..), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::CubeArray(_, nlayers), None) =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, 0, (6 * nlayers) as _]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_RTV_DIMENSION_TEXTURE2DARRAY, [level, (6 * lid) as _, (6 * (1+lid)) as _]),
            (_, None) => return Err(f::TargetViewError::Level(desc.level)),
            (_, Some(lid)) => return Err(f::TargetViewError::Layer(texture::LayerError::OutOfBounds(lid, 0))), //TODO
        };
        let format = core::format::Format(htex.get_info().format, desc.channel);
        let native_desc = d3d11::D3D11_RENDER_TARGET_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(f::TargetViewError::Channel(desc.channel)),
            },
            ViewDimension: dim,
            u: unsafe { mem::transmute(extra) }, //TODO
        };
        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateRenderTargetView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winerror::SUCCEEDED(hr) {
            error!("Failed to create RTV from ?, error {:x}"/*, native_desc*/, hr);
            return Err(f::TargetViewError::Unsupported);
        }
        let size = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_rtv(native::Rtv(raw_view), htex, size))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::DepthStencilDesc)
                                         -> Result<h::RawDepthStencilView<R>, f::TargetViewError>
    {
        use core::texture::{AaMode, Kind};
        use data::{map_format, map_dsv_flags};

        let level = desc.level as minwindef::UINT;
        let (dim, extra) = match (htex.get_info().kind, desc.layer) {
            (Kind::D1(..), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE1D, [level, 0, 0]),
            (Kind::D1Array(_, nlayers), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::D1Array(_, nlayers), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE1DARRAY, [level, 0, nlayers as _]),
            (Kind::D2(_, _, AaMode::Single), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2D, [level, 0, 0]),
            (Kind::D2(_, _, _), None) if level == 0 =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DMS, [0, 0, 0]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, nlayers as _]),
            (Kind::D2Array(_, _, nlayers, AaMode::Single), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::D2Array(_, _, nlayers, _), None) if level == 0 =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [0, nlayers as _, 0]),
            (Kind::D2Array(_, _, nlayers, _), Some(lid)) if level == 0 && lid < nlayers =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY, [lid as _, (1+lid) as _, 0]),
            (Kind::D3(..), _) => return Err(f::TargetViewError::Unsupported),
            (Kind::Cube(..), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, 6]),
            (Kind::Cube(..), Some(lid)) if lid < 6 =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, lid as _, (1+lid) as _]),
            (Kind::CubeArray(_, nlayers), None) =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, 0, (6 * nlayers) as _]),
            (Kind::CubeArray(_, nlayers), Some(lid)) if lid < nlayers =>
                (d3d11::D3D11_DSV_DIMENSION_TEXTURE2DARRAY, [level, (6 * lid) as _, (6 * (1+lid)) as _]),
            (_, None) => return Err(f::TargetViewError::Level(desc.level)),
            (_, Some(lid)) => return Err(f::TargetViewError::Layer(texture::LayerError::OutOfBounds(lid, 0))), //TODO
        };

        let channel = core::format::ChannelType::Uint; //doesn't matter
        let format = core::format::Format(htex.get_info().format, channel);
        let native_desc = d3d11::D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: match map_format(format, true) {
                Some(fm) => fm,
                None => return Err(f::TargetViewError::Channel(channel)),
            },
            ViewDimension: dim,
            Flags: map_dsv_flags(desc.flags),
            u: unsafe { mem::transmute(extra) }, //TODO
        };

        let mut raw_view = ptr::null_mut();
        let raw_tex = self.frame_handles.ref_texture(htex).as_resource();
        let hr = unsafe {
            (*self.device).CreateDepthStencilView(raw_tex, &native_desc, &mut raw_view)
        };
        if !winerror::SUCCEEDED(hr) {
            error!("Failed to create DSV from ?, error {:x}"/*, native_desc*/, hr);
            return Err(f::TargetViewError::Unsupported);
        }
        let dim = htex.get_info().kind.get_level_dimensions(desc.level);
        Ok(self.share.handles.borrow_mut().make_dsv(native::Dsv(raw_view), htex, dim))
    }

    fn create_sampler(&mut self, info: texture::SamplerInfo) -> h::Sampler<R> {
        use core::texture::FilterMethod;
        use data::{FilterOp, map_function, map_filter, map_wrap};

        let op = if info.comparison.is_some() {FilterOp::Comparison} else {FilterOp::Product};
        let native_desc = d3d11::D3D11_SAMPLER_DESC {
            Filter: map_filter(info.filter, op),
            AddressU: map_wrap(info.wrap_mode.0),
            AddressV: map_wrap(info.wrap_mode.1),
            AddressW: map_wrap(info.wrap_mode.2),
            MipLODBias: info.lod_bias.into(),
            MaxAnisotropy: match info.filter {
                FilterMethod::Anisotropic(max) => max as _,
                _ => 0,
            },
            ComparisonFunc: map_function(info.comparison.unwrap_or(core::state::Comparison::Always)),
            BorderColor: info.border.into(),
            MinLOD: info.lod_range.0.into(),
            MaxLOD: info.lod_range.1.into(),
        };

        let mut raw_sampler = ptr::null_mut();
        let hr = unsafe {
            (*self.device).CreateSamplerState(&native_desc, &mut raw_sampler)
        };
        if winerror::SUCCEEDED(hr) {
            self.share.handles.borrow_mut().make_sampler(native::Sampler(raw_sampler), info)
        } else {
            error!("Unable to create a sampler with desc {:#?}, error {:x}", info, hr);
            unimplemented!()
        }
    }

    fn read_mapping<'a, 'b, T>(&'a mut self, buf: &'b h::Buffer<R, T>)
                               -> Result<mapping::Reader<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        unsafe {
            mapping::read(buf.raw(), |mut m| {
                ensure_mapped(&mut m, buf.raw(), d3d11::D3D11_MAP_READ, self)
            })
        }
    }

    fn write_mapping<'a, 'b, T>(&'a mut self, buf: &'b h::Buffer<R, T>)
                               -> Result<mapping::Writer<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        unsafe {
            mapping::write(buf.raw(), |mut m| {
                // not MAP_WRITE_DISCARD because we are STAGING
                ensure_mapped(&mut m, buf.raw(), d3d11::D3D11_MAP_WRITE, self)
            })
        }
    }
}

pub fn ensure_mapped(mapping: &mut MappingGate,
                     buffer: &h::RawBuffer<R>,
                     map_type: d3d11::D3D11_MAP,
                     factory: &Factory) {
    if mapping.pointer.is_null() {
        let raw_handle = *buffer.resource();
        let mut ctx = ptr::null_mut();

        unsafe {
            (*factory.device).GetImmediateContext(&mut ctx);
        }

        let mut sres = d3d11::D3D11_MAPPED_SUBRESOURCE {
            pData: ptr::null_mut(),
            RowPitch: 0,
            DepthPitch: 0,
        };

        let dst = raw_handle.as_resource() as *mut d3d11::ID3D11Resource;
        let hr = unsafe {
            (*ctx).Map(dst, 0, map_type, 0, &mut sres)
        };

        if winerror::SUCCEEDED(hr) {
            mapping.pointer = sres.pData as _;
        } else {
            panic!("Unable to map a buffer {:?}, error {:x}", buffer, hr);
        }
    }
}

pub fn ensure_unmapped(mapping: &mut MappingGate,
                       buffer: &buffer::Raw<R>,
                       context: *mut d3d11::ID3D11DeviceContext) {
    if !mapping.pointer.is_null() {
        let raw_handle = *buffer.resource();
        unsafe {
            (*context).Unmap(raw_handle.as_resource() as *mut d3d11::ID3D11Resource, 0);
        }

        mapping.pointer = ptr::null_mut();
    }
}

/// Hack: We're packing the semantic name/index into a `String` in the format that HLSL parses the information from (i.e. the name and index are simply concatted together). This loses some type safety and incurs a cost to marshal in and out of this struct. We're doing this to work around not changing `AttributeVar` (which could be parameterized over the `name` type and be specialized to this struct for DX11). When the next minor version of `gfx_core` is released we may be able to remove this hack.
/// 
/// `'s` is the lifetime of the `str` pointing at the semantic name.
#[derive(PartialEq, Debug)]
pub struct VertexSemantic<'s> {
    pub name: &'s str,
    pub index: u32,
}

impl<'s> From<&'s str> for VertexSemantic<'s> {
    fn from(packed: &'s str) -> Self {
        fn is_not_ascii_digit(c: char) -> bool {
            !char::is_ascii_digit(&c)
        }

        // Note: a semantic name can't be just numeric, so this will succeed if called correctly.
        let partition_index = packed.rfind(is_not_ascii_digit).map(|i| i + 1).unwrap();

        Self {
            name: &packed[..partition_index],
            index: str::parse(&packed[partition_index..]).unwrap_or(0),
        }
    }
}

impl Into<String> for VertexSemantic<'_> {
    fn into(self) -> String {
        format!("{}{}", self.name, self.index)
    }
}

#[cfg(test)]
mod vertex_semantic_tests {
    use super::*;

    #[test]
    fn pack() {
        let vertex_semantic = VertexSemantic {
            name: "TEST",
            index: 1,
        };
        let packed: String = vertex_semantic.into();
        assert_eq!("TEST1", packed);
    }

    #[test]
    fn unpack() {
        let test1 = "TEST1";
        let packed: VertexSemantic = test1.into();
        assert_eq!(
            VertexSemantic {
                name: "TEST",
                index: 1,
            },
            packed,
        );
    }

    #[test]
    fn unpack_inner_numbers() {
        let test1 = "TEST1A1";
        let packed: VertexSemantic = test1.into();
        assert_eq!(
            VertexSemantic {
                name: "TEST1A",
                index: 1,
            },
            packed,
        );
    }

    #[test]
    fn unpack_implied_index() {
        let test = "TEST";
        let packed: VertexSemantic = test.into();
        assert_eq!(
            VertexSemantic {
                name: "TEST",
                index: 0,
            },
            packed,
        );
    }
}
