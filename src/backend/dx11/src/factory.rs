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
use {Resources as R, Share, Pipeline, Program, Shader};
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
        let (usage, cpu) = match info.usage {
            f::BufferUsage::GpuOnly => (D3D11_USAGE_DEFAULT, D3D11_CPU_ACCESS_FLAG(0)),
            f::BufferUsage::Const   => (D3D11_USAGE_IMMUTABLE, D3D11_CPU_ACCESS_FLAG(0)),
            f::BufferUsage::Dynamic => (D3D11_USAGE_DYNAMIC, D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE),
            f::BufferUsage::Stream  => (D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_WRITE),
        };
        let mut bind = match info.role {
            f::BufferRole::Vertex   => D3D11_BIND_VERTEX_BUFFER,
            f::BufferRole::Index    => D3D11_BIND_INDEX_BUFFER,
            f::BufferRole::Uniform  => D3D11_BIND_CONSTANT_BUFFER,
        };
        if info.bind.contains(f::SHADER_RESOURCE) {
            bind = bind | D3D11_BIND_SHADER_RESOURCE;
        }
        if info.bind.contains(f::UNORDERED_ACCESS) {
            bind = bind | D3D11_BIND_UNORDERED_ACCESS;
        }
        if info.bind.contains(f::RENDER_TARGET) {
            return Err(f::BufferError::UnsupportedBind(f::RENDER_TARGET))
        }
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: info.size as winapi::UINT,
            Usage: usage,
            BindFlags: bind.0,
            CPUAccessFlags: cpu.0,
            MiscFlags: 0,
            StructureByteStride: info.stride as winapi::UINT,
        };
        let sub = D3D11_SUBRESOURCE_DATA {
            pSysMem: raw_data.unwrap_or(ptr::null()),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut buf = native::Buffer(ptr::null_mut());
        let hr = unsafe {
            (*self.share.device).CreateBuffer(&desc, &sub, &mut buf.0)
        };
        if winapi::SUCCEEDED(hr) {
            Ok(self.share.handles.borrow_mut().make_buffer(buf, info))
        }else {
            error!("Buffer creation error code {:x}, info: {:?}", hr, info);
            Err(f::BufferError::Other)
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

    fn create_buffer_static_raw(&mut self, data: &[u8], stride: usize, role: f::BufferRole, bind: f::Bind)
                                -> Result<h::RawBuffer<R>, f::BufferError> {
        let info = f::BufferInfo {
            role: role,
            usage: f::BufferUsage::Const,
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
        let info = core::shade::ProgramInfo { //TODO
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
                let vs_ = vs.reference(fh);
                let (vs, ps) = (vs_.object, ps.reference(fh).object);
                unsafe { (*vs).AddRef(); (*ps).AddRef(); }
                Program {
                    vs: vs as *mut ID3D11VertexShader,
                    gs: ptr::null_mut(),
                    ps: ps as *mut ID3D11PixelShader,
                    vs_hash: vs_.code_hash,
                }
            },
            &core::ShaderSet::Geometry(ref vs, ref gs, ref ps) => {
                let vs_ = vs.reference(fh);
                let (vs, gs, ps) = (vs_.object, gs.reference(fh).object, ps.reference(fh).object);
                unsafe { (*vs).AddRef(); (*gs).AddRef(); (*ps).AddRef(); }
                Program {
                    vs: vs as *mut ID3D11VertexShader,
                    gs: vs as *mut ID3D11GeometryShader,
                    ps: ps as *mut ID3D11PixelShader,
                    vs_hash: vs_.code_hash,
                }
            },
        };

        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        use std::mem; //temporary
        use winapi::d3dcommon::*;
        use gfx_core::Primitive::*;
        use state;

        let layouts = Vec::new();
        /*layouts.push(D3D11_INPUT_ELEMENT_DESC {
            SemanticName: c_str!("POSITION"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        });*/
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
        let _hr = unsafe {
            (*dev).CreateInputLayout(
                layouts.as_ptr(), layouts.len() as winapi::UINT,
                vs_bin.as_ptr() as *const c_void, vs_bin.len() as winapi::SIZE_T,
                &mut vertex_layout)
        };
        let dummy_dsi = core::pso::DepthStencilInfo { depth: None, front: None, back: None };

        let pso = Pipeline {
            topology: unsafe{mem::transmute(match desc.primitive {
                PointList       => D3D11_PRIMITIVE_TOPOLOGY_POINTLIST,
                LineList        => D3D11_PRIMITIVE_TOPOLOGY_LINELIST,
                LineStrip       => D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP,
                TriangleList    => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                TriangleStrip   => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            })},
            layout: vertex_layout,
            program: prog,
            rasterizer: state::make_rasterizer(dev, &desc.rasterizer, desc.scissor),
            depth_stencil: state::make_depth_stencil(dev, match desc.depth_stencil {
                Some((_, ref dsi)) => dsi,
                None => &dummy_dsi,
            }),
            blend: ptr::null(),
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
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
