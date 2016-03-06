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

//#[deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate gfx_core;
extern crate d3d11;
extern crate d3dcompiler;
extern crate dxguid;
extern crate winapi;

pub use self::command::CommandBuffer;
pub use self::data::map_format;

mod command;
mod data;
mod factory;
mod mirror;
mod state;


#[doc(hidden)]
pub mod native {
    use winapi::*;

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Buffer(pub *mut ID3D11Buffer);
    unsafe impl Send for Buffer {}
    unsafe impl Sync for Buffer {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Rtv(pub *mut ID3D11RenderTargetView);
    unsafe impl Send for Rtv {}
    unsafe impl Sync for Rtv {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Dsv(pub *mut ID3D11DepthStencilView);
    unsafe impl Send for Dsv {}
    unsafe impl Sync for Dsv {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Srv(pub *mut ID3D11ShaderResourceView);
    unsafe impl Send for Srv {}
    unsafe impl Sync for Srv {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Sampler(pub *mut ID3D11SamplerState);
    unsafe impl Send for Sampler {}
    unsafe impl Sync for Sampler {}
}

use std::cell::RefCell;
use std::os::raw::c_void;
use std::{mem, ptr};
use std::sync::Arc;
pub use self::factory::Factory;
use gfx_core::handle as h;


#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Texture {
    D1(*mut winapi::ID3D11Texture1D),
    D2(*mut winapi::ID3D11Texture2D),
    D3(*mut winapi::ID3D11Texture3D),
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

impl Texture {
    pub fn to_resource(&self) -> *mut winapi::ID3D11Resource {
        type Res = *mut winapi::ID3D11Resource;
        match *self {
            Texture::D1(t) => t as Res,
            Texture::D2(t) => t as Res,
            Texture::D3(t) => t as Res,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Shader {
    object: *mut winapi::ID3D11DeviceChild,
    reflection: *mut winapi::ID3D11ShaderReflection,
    code_hash: u64,
}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Program {
    vs: *mut winapi::ID3D11VertexShader,
    gs: *mut winapi::ID3D11GeometryShader,
    ps: *mut winapi::ID3D11PixelShader,
    vs_hash: u64,
}
unsafe impl Send for Program {}
unsafe impl Sync for Program {}

pub type InputLayout = *mut winapi::ID3D11InputLayout;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Pipeline {
    topology: winapi::D3D11_PRIMITIVE_TOPOLOGY,
    layout: InputLayout,
    attributes: [Option<gfx_core::pso::AttributeDesc>; gfx_core::MAX_VERTEX_ATTRIBUTES],
    program: Program,
    rasterizer: *const winapi::ID3D11RasterizerState,
    depth_stencil: *const winapi::ID3D11DepthStencilState,
    blend: *const winapi::ID3D11BlendState,
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl gfx_core::Resources for Resources {
    type Buffer              = native::Buffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = Pipeline;
    type Texture             = Texture;
    type RenderTargetView    = native::Rtv;
    type DepthStencilView    = native::Dsv;
    type ShaderResourceView  = native::Srv;
    type UnorderedAccessView = ();
    type Sampler             = native::Sampler;
    type Fence               = ();
}

/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    capabilities: gfx_core::Capabilities,
    handles: RefCell<h::Manager<Resources>>,
}

pub struct Device {
    context: *mut winapi::ID3D11DeviceContext,
    feature_level: winapi::D3D_FEATURE_LEVEL,
    share: Arc<Share>,
    frame_handles: h::Manager<Resources>,
    max_resource_count: Option<usize>,
}

static FEATURE_LEVELS: [winapi::D3D_FEATURE_LEVEL; 3] = [
    winapi::D3D_FEATURE_LEVEL_11_0,
    winapi::D3D_FEATURE_LEVEL_10_1,
    winapi::D3D_FEATURE_LEVEL_10_0,
];


pub fn create(driver_type: winapi::D3D_DRIVER_TYPE, desc: &winapi::DXGI_SWAP_CHAIN_DESC)
              -> Result<(Device, Factory, *mut winapi::IDXGISwapChain, h::RawRenderTargetView<Resources>), winapi::HRESULT> {
    use gfx_core::handle::Producer;
    use gfx_core::tex;

    let mut swap_chain = ptr::null_mut();
    let create_flags = winapi::D3D11_CREATE_DEVICE_FLAG(0); //D3D11_CREATE_DEVICE_DEBUG;
    let mut device = ptr::null_mut();
    let share = Share {
        capabilities: gfx_core::Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: true,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
        },
        handles: RefCell::new(h::Manager::new()),
    };

    let mut context = ptr::null_mut();
    let mut feature_level = winapi::D3D_FEATURE_LEVEL_10_0;
    let hr = unsafe {
        d3d11::D3D11CreateDeviceAndSwapChain(ptr::null_mut(), driver_type, ptr::null_mut(), create_flags.0,
            &FEATURE_LEVELS[0], FEATURE_LEVELS.len() as winapi::UINT, winapi::D3D11_SDK_VERSION, desc,
            &mut swap_chain, &mut device, &mut feature_level, &mut context)
    };
    if !winapi::SUCCEEDED(hr) {
        return Err(hr)
    }

    let mut back_buffer: *mut winapi::ID3D11Texture2D = ptr::null_mut();
    unsafe {
        (*swap_chain).GetBuffer(0, &dxguid::IID_ID3D11Texture2D, &mut back_buffer
            as *mut *mut winapi::ID3D11Texture2D as *mut *mut c_void);
    }
    let color_tex = share.handles.borrow_mut().make_texture(Texture::D2(back_buffer), gfx_core::tex::Descriptor {
        kind: tex::Kind::D2(desc.BufferDesc.Width as tex::Size, desc.BufferDesc.Height as tex::Size, tex::AaMode::Single),
        levels: 1,
        format: gfx_core::format::SurfaceType::R8_G8_B8_A8,
        bind: gfx_core::factory::RENDER_TARGET,
        usage: gfx_core::factory::Usage::GpuOnly,
    });

    let dev = Device {
        context: context,
        feature_level: feature_level,
        share: Arc::new(share),
        frame_handles: h::Manager::new(),
        max_resource_count: None,
    };
    let mut factory = Factory::new(device, dev.share.clone());

    let color_target = {
        use gfx_core::Factory;
        let desc = gfx_core::tex::RenderDesc {
            channel: gfx_core::format::ChannelType::Unorm,
            level: 0,
            layer: None,
        };
        factory.view_texture_as_render_target_raw(&color_tex, desc).unwrap()
    };

    Ok((dev, factory, swap_chain, color_target))
}

pub type ShaderModel = u16;

impl Device {
    /// Return the maximum supported shader model.
    pub fn get_shader_model(&self) -> ShaderModel {
        match self.feature_level {
            winapi::D3D_FEATURE_LEVEL_10_0 => 40,
            winapi::D3D_FEATURE_LEVEL_10_1 => 41,
            winapi::D3D_FEATURE_LEVEL_11_0 => 50,
            winapi::D3D_FEATURE_LEVEL_11_1 => 51,
            _ => {
                error!("Unknown feature level {:?}", self.feature_level);
                0
            },
        }
    }

    fn process(&mut self, command: &command::Command, data_buf: &gfx_core::draw::DataBuffer) {
        use gfx_core::shade::Stage;
        use command::Command::*;
        let max_cb  = gfx_core::MAX_CONSTANT_BUFFERS as winapi::UINT;
        let max_srv = gfx_core::MAX_RESOURCE_VIEWS   as winapi::UINT;
        let max_sm  = gfx_core::MAX_SAMPLERS         as winapi::UINT;
        debug!("Processing {:?}", command);
        match *command {
            BindProgram(ref prog) => unsafe {
                (*self.context).VSSetShader(prog.vs, ptr::null_mut(), 0);
                (*self.context).GSSetShader(prog.gs, ptr::null_mut(), 0);
                (*self.context).PSSetShader(prog.ps, ptr::null_mut(), 0);
            },
            BindInputLayout(layout) => unsafe {
                (*self.context).IASetInputLayout(layout);
            },
            BindIndex(ref buf, format) => unsafe {
                (*self.context).IASetIndexBuffer(buf.0, format, 0);
            },
            BindVertexBuffers(ref buffers, ref strides, ref offsets) => unsafe {
                (*self.context).IASetVertexBuffers(0, gfx_core::MAX_VERTEX_ATTRIBUTES as winapi::UINT,
                    &buffers[0].0, strides.as_ptr(), offsets.as_ptr());
            },
            BindConstantBuffers(stage, ref buffers) => match stage {
                Stage::Vertex => unsafe {
                    (*self.context).VSSetConstantBuffers(0, max_cb, &buffers[0].0);
                },
                Stage::Geometry => unsafe {
                    (*self.context).GSSetConstantBuffers(0, max_cb, &buffers[0].0);
                },
                Stage::Pixel => unsafe {
                    (*self.context).PSSetConstantBuffers(0, max_cb, &buffers[0].0);
                },
            },
            BindShaderResources(stage, ref views) => match stage {
                Stage::Vertex => unsafe {
                    (*self.context).VSSetShaderResources(0, max_srv, &views[0].0);
                },
                Stage::Geometry => unsafe {
                    (*self.context).GSSetShaderResources(0, max_srv, &views[0].0);
                },
                Stage::Pixel => unsafe {
                    (*self.context).PSSetShaderResources(0, max_srv, &views[0].0);
                },
            },
            BindSamplers(stage, ref samplers) => match stage {
                Stage::Vertex => unsafe {
                    (*self.context).VSSetSamplers(0, max_sm, &samplers[0].0);
                },
                Stage::Geometry => unsafe {
                    (*self.context).GSSetSamplers(0, max_sm, &samplers[0].0);
                },
                Stage::Pixel => unsafe {
                    (*self.context).PSSetSamplers(0, max_sm, &samplers[0].0);
                },
            },
            BindPixelTargets(ref colors, ds) => unsafe {
                (*self.context).OMSetRenderTargets(gfx_core::MAX_COLOR_TARGETS as winapi::UINT,
                    &colors[0].0, ds.0);
            },
            SetPrimitive(topology) => unsafe {
                (*self.context).IASetPrimitiveTopology(topology);
            },
            SetViewport(ref viewport) => unsafe {
                (*self.context).RSSetViewports(1, viewport);
            },
            SetScissor(ref rect) => unsafe {
                (*self.context).RSSetScissorRects(1, rect);
            },
            SetRasterizer(rast) => unsafe {
                (*self.context).RSSetState(rast as *mut _);
            },
            SetDepthStencil(ds, value) => unsafe {
                (*self.context).OMSetDepthStencilState(ds as *mut _, value);
            },
            SetBlend(blend, ref value, mask) => unsafe {
                (*self.context).OMSetBlendState(blend as *mut _, value, mask);
            },
            UpdateBuffer(buffer, pointer, offset) => {
                let data = data_buf.get_ref(pointer);
                let map_type = winapi::D3D11_MAP_WRITE_DISCARD;
                let resource = buffer.0 as *mut winapi::ID3D11Resource;
                let hr = unsafe {
                    let mut sub = mem::zeroed();
                    let hr = (*self.context).Map(resource, 0, map_type, 0, &mut sub);
                    let dst = (sub.pData as *mut u8).offset(offset as isize);
                    ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
                    (*self.context).Unmap(resource, 0);
                    hr
                };
                if !winapi::SUCCEEDED(hr) {
                    error!("Buffer {:?} failed to map, error {:x}", buffer, hr);
                }
            },
            ClearColor(target, ref data) => unsafe {
                (*self.context).ClearRenderTargetView(target.0, data);
            },
            ClearDepthStencil(target, flags, depth, stencil) => unsafe {
                (*self.context).ClearDepthStencilView(target.0, flags.0, depth, stencil);
            },
            Draw(nvert, svert) => unsafe {
                (*self.context).Draw(nvert, svert);
            },
            DrawInstanced(nvert, ninst, svert, sinst) => unsafe {
                (*self.context).DrawInstanced(nvert, ninst, svert, sinst);
            },
            DrawIndexed(nind, svert, base) => unsafe {
                (*self.context).DrawIndexed(nind, svert, base);
            },
            DrawIndexedInstanced(nind, ninst, sind, base, sinst) => unsafe {
                (*self.context).DrawIndexedInstanced(nind, ninst, sind, base, sinst);
            },
        }
    }
}

impl gfx_core::Device for Device {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer;

    fn get_capabilities<'a>(&'a self) -> &'a gfx_core::Capabilities {
        &self.share.capabilities
    }

    fn reset_state(&mut self) {
        unsafe { (*self.context).ClearState(); }
    }

    fn submit(&mut self, submit_info: gfx_core::SubmitInfo<Self>) {
        let gfx_core::SubmitInfo(cb, db, handles) = submit_info;
        self.frame_handles.extend(handles);
        self.reset_state();
        for com in &cb.buf {
            self.process(com, db);
        }
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn cleanup(&mut self) {
        use gfx_core::handle::Producer;
        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut (),
            |_, v| unsafe { (*v.0).Release(); }, //buffer
            |_, s| unsafe { //shader
                (*s.object).Release();
                (*s.reflection).Release();
            },
            |_, p| unsafe {
                if p.vs != ptr::null_mut() { (*p.vs).Release(); }
                if p.gs != ptr::null_mut() { (*p.gs).Release(); }
                if p.ps != ptr::null_mut() { (*p.ps).Release(); }
            }, //program
            |_, v| unsafe { //PSO
                type Child = *mut winapi::ID3D11DeviceChild;
                (*v.layout).Release();
                (*(v.rasterizer as Child)).Release();
                (*(v.depth_stencil as Child)).Release();
                (*(v.blend as Child)).Release();
            },
            |_, v| unsafe { (*v.to_resource()).Release(); },  //texture
            |_, v| unsafe { (*v.0).Release(); }, //SRV
            |_, _| {}, //UAV
            |_, v| unsafe { (*v.0).Release(); }, //RTV
            |_, v| unsafe { (*v.0).Release(); }, //DSV
            |_, v| unsafe { (*v.0).Release(); }, //sampler
            |_, _| {}, //fence
        );
    }
}