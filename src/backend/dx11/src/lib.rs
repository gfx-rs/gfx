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
extern crate gfx_core as core;
extern crate winapi;

use winapi::shared::{dxgi, minwindef, winerror};
use winapi::um::{d3d11, d3d11shader, d3dcommon};

pub use self::command::CommandBuffer;
pub use self::data::map_format;
pub use self::factory::Factory;

mod debug;
mod command;
mod data;
mod execute;
mod factory;
mod mirror;
mod state;

#[doc(hidden)]
pub mod native {
    use winapi::um::d3d11::*;

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Buffer(pub *mut ID3D11Buffer);
    unsafe impl Send for Buffer {}
    unsafe impl Sync for Buffer {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub enum Texture {
        D1(*mut ID3D11Texture1D),
        D2(*mut ID3D11Texture2D),
        D3(*mut ID3D11Texture3D),
    }
    unsafe impl Send for Texture {}
    unsafe impl Sync for Texture {}

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
use std::ptr;
use std::sync::Arc;
use core::{handle as h, texture as tex};
use core::SubmissionResult;
use core::command::{AccessInfo, AccessGuard};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(native::Buffer);
impl Buffer {
    pub fn as_resource(&self) -> *mut d3d11::ID3D11Resource {
        type Res = *mut d3d11::ID3D11Resource;
        match self.0 {
            native::Buffer(t) => t as Res,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Texture(native::Texture);
impl Texture {
    pub fn as_resource(&self) -> *mut d3d11::ID3D11Resource {
        type Res = *mut d3d11::ID3D11Resource;
        match self.0 {
            native::Texture::D1(t) => t as Res,
            native::Texture::D2(t) => t as Res,
            native::Texture::D3(t) => t as Res,
        }
    }
}


#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Shader {
    object: *mut d3d11::ID3D11DeviceChild,
    reflection: *mut d3d11shader::ID3D11ShaderReflection,
    code_hash: u64,
}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Program {
    vs: *mut d3d11::ID3D11VertexShader,
    hs: *mut d3d11::ID3D11HullShader,
    ds: *mut d3d11::ID3D11DomainShader,
    gs: *mut d3d11::ID3D11GeometryShader,
    ps: *mut d3d11::ID3D11PixelShader,
    vs_hash: u64,
}
unsafe impl Send for Program {}
unsafe impl Sync for Program {}

pub type InputLayout = *mut d3d11::ID3D11InputLayout;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Pipeline {
    topology: d3d11::D3D11_PRIMITIVE_TOPOLOGY,
    layout: InputLayout,
    vertex_buffers: [Option<core::pso::VertexBufferDesc>; core::pso::MAX_VERTEX_BUFFERS],
    attributes: [Option<core::pso::AttributeDesc>; core::MAX_VERTEX_ATTRIBUTES],
    program: Program,
    rasterizer: *const d3d11::ID3D11RasterizerState,
    depth_stencil: *const d3d11::ID3D11DepthStencilState,
    blend: *const d3d11::ID3D11BlendState,
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl core::Resources for Resources {
    type Buffer              = Buffer;
    type Shader              = Shader;
    type Program             = Program;
    type PipelineStateObject = Pipeline;
    type Texture             = Texture;
    type RenderTargetView    = native::Rtv;
    type DepthStencilView    = native::Dsv;
    type ShaderResourceView  = native::Srv;
    type UnorderedAccessView = ();
    type Sampler             = native::Sampler;
    type Fence               = Fence;
    type Mapping             = factory::MappingGate;
}

/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    capabilities: core::Capabilities,
    handles: RefCell<h::Manager<Resources>>,
}

pub struct Device {
    context: *mut d3d11::ID3D11DeviceContext,
    feature_level: d3dcommon::D3D_FEATURE_LEVEL,
    share: Arc<Share>,
    frame_handles: h::Manager<Resources>,
    max_resource_count: Option<usize>,
    infoqueue: Option<debug::InfoQueue>,
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { (*self.context).Release(); }
    }
}

static FEATURE_LEVELS: [d3dcommon::D3D_FEATURE_LEVEL; 3] = [
    d3dcommon::D3D_FEATURE_LEVEL_11_0,
    d3dcommon::D3D_FEATURE_LEVEL_10_1,
    d3dcommon::D3D_FEATURE_LEVEL_10_0,
];

pub fn create(driver_type: d3dcommon::D3D_DRIVER_TYPE, desc: &dxgi::DXGI_SWAP_CHAIN_DESC)
              -> Result<(Device, Factory, *mut dxgi::IDXGISwapChain), winerror::HRESULT> {
    let mut swap_chain = ptr::null_mut();
    let create_flags = debug::add_debug_flag_if_needed(0);
    let mut device = ptr::null_mut();
    let mut context = ptr::null_mut();
    let mut feature_level = d3dcommon::D3D_FEATURE_LEVEL_10_0;
    let hr = unsafe {
        d3d11::D3D11CreateDeviceAndSwapChain(ptr::null_mut(), driver_type, ptr::null_mut(), create_flags,
            &FEATURE_LEVELS[0], FEATURE_LEVELS.len() as _, d3d11::D3D11_SDK_VERSION, desc,
            &mut swap_chain, &mut device, &mut feature_level, &mut context)
    };
    if !winerror::SUCCEEDED(hr) {
        return Err(hr);
    }

    let (dev, factory) = create_from_existing(device, context)?;
    Ok((dev, factory, swap_chain))
}

pub fn create_from_existing(device: *mut winapi::um::d3d11::ID3D11Device, context: *mut winapi::um::d3d11::ID3D11DeviceContext)
    -> Result<(Device, Factory), winerror::HRESULT>
{
    if device.is_null() || context.is_null() {
        return Err(winerror::E_INVALIDARG);
    }

    let share = Share {
        capabilities: core::Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            max_patch_size: 32, //hard-coded in D3D11
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: true,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
            copy_buffer_supported: true,
        },
        handles: RefCell::new(h::Manager::new()),
    };

    let feature_level = unsafe {
        let device_obj = & *device;
        device_obj.GetFeatureLevel()
    };

    let dev = Device {
        context: context,
        feature_level: feature_level,
        share: Arc::new(share),
        frame_handles: h::Manager::new(),
        max_resource_count: None,
        infoqueue: debug::InfoQueue::try_create(device),
    };
    let factory = Factory::new(device, dev.share.clone());

    Ok((dev, factory))
}

pub type ShaderModel = u16;

impl Device {
    /// Return the maximum supported shader model.
    pub fn get_shader_model(&self) -> ShaderModel {
        match self.feature_level {
            d3dcommon::D3D_FEATURE_LEVEL_10_0 => 40,
            d3dcommon::D3D_FEATURE_LEVEL_10_1 => 41,
            d3dcommon::D3D_FEATURE_LEVEL_11_0 => 50,
            d3dcommon::D3D_FEATURE_LEVEL_11_1 => 51,
            _ => {
                error!("Unknown feature level {:?}", self.feature_level);
                0
            },
        }
    }

    pub fn before_submit<'a>(&mut self, gpu_access: &'a AccessInfo<Resources>)
                             -> core::SubmissionResult<AccessGuard<'a, Resources>> {
        let mut gpu_access = try!(gpu_access.take_accesses());
        for (buffer, mut mapping) in gpu_access.access_mapped() {
            factory::ensure_unmapped(&mut mapping, buffer, self.context);
        }
        Ok(gpu_access)
    }

    pub fn clear_state(&self) {
        unsafe {
            (*self.context).ClearState();
        }
    }
}

pub struct CommandList(Vec<command::Command>, command::DataBuffer);
impl CommandList {
    pub fn new() -> CommandList {
        CommandList(Vec::new(), command::DataBuffer::new())
    }
}
impl command::Parser for CommandList {
    fn reset(&mut self) {
        self.0.clear();
        self.1.reset();
    }
    fn parse(&mut self, com: command::Command) {
        self.0.push(com);
    }
    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        let ptr = self.1.add(data);
        self.0.push(command::Command::UpdateBuffer(buf, ptr, offset));
    }
    fn update_texture(&mut self, dst: tex::TextureCopyRegion<Texture>, data: &[u8]) {
        let ptr = self.1.add(data);
        self.0.push(command::Command::UpdateTexture(dst, ptr));
    }
}

pub struct DeferredContext(*mut d3d11::ID3D11DeviceContext, Option<*mut d3d11::ID3D11CommandList>);
unsafe impl Send for DeferredContext {}
impl DeferredContext {
    pub fn new(dc: *mut d3d11::ID3D11DeviceContext) -> Self {
        DeferredContext(dc, None)
    }
}
impl Drop for DeferredContext {
    fn drop(&mut self) {
        unsafe { (*self.0).Release() };
    }
}
impl command::Parser for DeferredContext {
    fn reset(&mut self) {
        if let Some(cl) = self.1 {
            unsafe { (*cl).Release() };
            self.1 = None;
        }
        unsafe {
            (*self.0).ClearState()
        };
    }
    fn parse(&mut self, com: command::Command) {
        let db = command::DataBuffer::new(); //not used
        execute::process(self.0, &com, &db);
    }
    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        execute::update_buffer(self.0, &buf, data, offset);
    }
    fn update_texture(&mut self, dst: tex::TextureCopyRegion<Texture>, data: &[u8]) {
        execute::update_texture(self.0, &dst, data);
    }
}


impl core::Device for Device {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer<CommandList>;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.share.capabilities
    }

    fn pin_submitted_resources(&mut self, man: &h::Manager<Resources>) {
        self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn submit(&mut self,
              cb: &mut Self::CommandBuffer,
              access: &AccessInfo<Resources>) -> SubmissionResult<()>
    {
        let _guard = try!(self.before_submit(access));
        unsafe { (*self.context).ClearState(); }
        for com in &cb.parser.0 {
            execute::process(self.context, com, &cb.parser.1);
        }
        Ok(())
    }

    fn fenced_submit(&mut self,
                     _: &mut Self::CommandBuffer,
                     _: &AccessInfo<Resources>,
                     _after: Option<h::Fence<Resources>>)
                     -> SubmissionResult<h::Fence<Resources>>
    {
        unimplemented!()
    }

    fn wait_fence(&mut self, _fence: &h::Fence<Self::Resources>) {
        unimplemented!()
    }

    fn cleanup(&mut self) {
        use core::handle::Producer;

        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut self.context,
            |ctx, buffer| {
                buffer.mapping().map(|raw| {
                    // we have exclusive access because it's the last reference
                    let mut mapping = unsafe { raw.use_access() };
                    factory::ensure_unmapped(&mut mapping, buffer, *ctx);
                });
                unsafe { (*(buffer.resource().0).0).Release(); }
            },
            |_, s| unsafe { //shader
                (*s.object).Release();
                (*s.reflection).Release();
            },
            |_, program| unsafe {
                let p = program.resource();
                if p.vs != ptr::null_mut() { (*p.vs).Release(); }
                if p.hs != ptr::null_mut() { (*p.hs).Release(); }
                if p.ds != ptr::null_mut() { (*p.ds).Release(); }
                if p.gs != ptr::null_mut() { (*p.gs).Release(); }
                if p.ps != ptr::null_mut() { (*p.ps).Release(); }
            },
            |_, v| unsafe { //PSO
                type Child = *mut d3d11::ID3D11DeviceChild;
                (*v.layout).Release();
                (*(v.rasterizer as Child)).Release();
                (*(v.depth_stencil as Child)).Release();
                (*(v.blend as Child)).Release();
            },
            |_, texture| unsafe { (*texture.resource().as_resource()).Release(); },
            |_, v| unsafe { (*v.0).Release(); }, //SRV
            |_, _| {}, //UAV
            |_, v| unsafe { (*v.0).Release(); }, //RTV
            |_, v| unsafe { (*v.0).Release(); }, //DSV
            |_, v| unsafe { (*v.0).Release(); }, //sampler
            |_, _fence| {},
        );

        if let Some(ref infoqueue) = self.infoqueue {
            infoqueue.flush_messages();
        }
    }
}

pub struct Deferred(Device);
impl From<Device> for Deferred {
    fn from(device: Device) -> Deferred {
        Deferred(device)
    }
}
impl Deferred {
    pub fn clear_state(&self) {
        self.0.clear_state();
    }
}

impl core::Device for Deferred {
    type Resources = Resources;
    type CommandBuffer = command::CommandBuffer<DeferredContext>;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.0.share.capabilities
    }

    fn pin_submitted_resources(&mut self, man: &h::Manager<Resources>) {
        self.0.pin_submitted_resources(man);
    }

    fn submit(&mut self,
              cb: &mut Self::CommandBuffer,
              access: &AccessInfo<Resources>) -> SubmissionResult<()>
    {
        let _guard = try!(self.0.before_submit(access));
        let cl = match cb.parser.1 {
            Some(cl) => cl,
            None => {
                let mut cl = ptr::null_mut();
                let hr = unsafe {
                    (*cb.parser.0).FinishCommandList(minwindef::FALSE, &mut cl)
                };
                assert!(winerror::SUCCEEDED(hr));
                cb.parser.1 = Some(cl);
                cl
            },
        };
        unsafe {
            (*self.0.context).ExecuteCommandList(cl, minwindef::TRUE)
        };
        match self.0.max_resource_count {
            Some(c) if self.0.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.0.max_resource_count = None;
            },
            _ => (),
        }
        Ok(())
    }

    fn fenced_submit(&mut self,
                     _: &mut Self::CommandBuffer,
                     _: &AccessInfo<Resources>,
                     _after: Option<h::Fence<Resources>>)
                     -> SubmissionResult<h::Fence<Resources>>
    {
        unimplemented!()
    }

    fn wait_fence(&mut self, _fence: &h::Fence<Self::Resources>) {
        unimplemented!()
    }

    fn cleanup(&mut self) {
        self.0.cleanup();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fence(());
