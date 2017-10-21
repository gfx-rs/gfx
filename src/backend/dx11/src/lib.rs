//#[deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate gfx_core as core;
extern crate d3d11;
extern crate d3dcompiler;
extern crate dxgi;
extern crate dxguid;
extern crate winapi;
extern crate wio;

pub use self::data::map_format;
pub use self::device::Device;

mod command;
pub mod data;
mod execute;
mod device;
mod mirror;
pub mod native;
mod pool;
mod state;

use core::{command as com, handle};
use wio::com::ComPtr;
use std::cell::RefCell;
use std::ptr;
use std::sync::Arc;
use core::{handle as h, texture as tex};
use core::{QueueType, SubmissionResult};
use core::command::{AccessInfo, AccessGuard};
use std::os::raw::c_void;

static FEATURE_LEVELS: [winapi::D3D_FEATURE_LEVEL; 3] = [
    winapi::D3D_FEATURE_LEVEL_11_0,
    winapi::D3D_FEATURE_LEVEL_10_1,
    winapi::D3D_FEATURE_LEVEL_10_0,
];

#[doc(hidden)]
pub struct Instance(pub ComPtr<winapi::IDXGIFactory2>);

impl Instance {
    #[doc(hidden)]
    pub fn create() -> Self {
        // Create DXGI factory
        let mut dxgi_factory: *mut winapi::IDXGIFactory2 = ptr::null_mut();
        let hr = unsafe {
            dxgi::CreateDXGIFactory1(
                &dxguid::IID_IDXGIFactory2,
                &mut dxgi_factory as *mut *mut _ as *mut *mut c_void)
        };

        if !winapi::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        Instance(unsafe { ComPtr::new(dxgi_factory) })
    }

    #[doc(hidden)]
    pub fn enumerate_adapters(&mut self) -> Vec<Adapter> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let mut cur_index = 0;
        let mut adapters = Vec::new();
        loop {
            let mut adapter = {
                let mut adapter: *mut winapi::IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe {
                    self.0.EnumAdapters1(
                        cur_index,
                        &mut adapter as *mut *mut _)
                };

                if hr == winapi::DXGI_ERROR_NOT_FOUND {
                    break;
                }

                unsafe { ComPtr::new(adapter) }
            };

            // We have found a possible adapter
            // acquire the device information
            let mut desc: winapi::DXGI_ADAPTER_DESC1 = unsafe { std::mem::uninitialized() };
            unsafe { adapter.GetDesc1(&mut desc); }

            let device_name = {
                let len = desc.Description.iter().take_while(|&&c| c != 0).count();
                let name = <OsString as OsStringExt>::from_wide(&desc.Description[..len]);
                name.to_string_lossy().into_owned()
            };

            let info = core::AdapterInfo {
                name: device_name,
                vendor: desc.VendorId as usize,
                device: desc.DeviceId as usize,
                software_rendering: false, // TODO
            };

            adapters.push(
                Adapter {
                    adapter: adapter,
                    info: info,
                    queue_family: [(QueueFamily, QueueType::General)],
                }
            );

            cur_index += 1;
        }

        adapters
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(native::Buffer);
impl Buffer {
    pub fn as_resource(&self) -> *mut winapi::ID3D11Resource {
        type Res = *mut winapi::ID3D11Resource;
        match self.0 {
            native::Buffer(t) => t as Res,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Texture(native::Texture);
impl Texture {
    #[doc(hidden)]
    pub fn new(tex: native::Texture) -> Self {
        Texture(tex)
    }

    pub fn as_resource(&self) -> *mut winapi::ID3D11Resource {
        type Res = *mut winapi::ID3D11Resource;
        match self.0 {
            native::Texture::D1(t) => t as Res,
            native::Texture::D2(t) => t as Res,
            native::Texture::D3(t) => t as Res,
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
    hs: *mut winapi::ID3D11HullShader,
    ds: *mut winapi::ID3D11DomainShader,
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
    vertex_buffers: [Option<core::pso::VertexBufferDesc>; core::pso::MAX_VERTEX_BUFFERS],
    attributes: [Option<core::pso::AttributeDesc>; core::MAX_VERTEX_ATTRIBUTES],
    program: Program,
    rasterizer: *const winapi::ID3D11RasterizerState,
    depth_stencil: *const winapi::ID3D11DepthStencilState,
    blend: *const winapi::ID3D11BlendState,
}
unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Resources = Resources;
    type CommandQueue = CommandQueue;
    type RawCommandBuffer = command::RawCommandBuffer<CommandList>; // TODO: deferred?
    type SubpassCommandBuffer = command::SubpassCommandBuffer<CommandList>;
    type SubmitInfo = command::SubmitInfo<CommandList>;
    type Device = Device;
    type QueueFamily = QueueFamily;

    type RawCommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;
}

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
    type Semaphore           = (); // TODO
    type Mapping             = device::MappingGate;
}

/// Internal struct of shared data between the device and its factories.
#[doc(hidden)]
pub struct Share {
    capabilities: core::Capabilities,
    handles: RefCell<h::Manager<Resources>>,
}

pub type ShaderModel = u16;

#[derive(Clone)]
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
    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>, data: &[u8], image: tex::RawImageInfo) {
        let ptr = self.1.add(data);
        self.0.push(command::Command::UpdateTexture(tex, kind, face, ptr, image));
    }
}

pub struct DeferredContext(ComPtr<winapi::ID3D11DeviceContext>, Option<*mut winapi::ID3D11CommandList>);
unsafe impl Send for DeferredContext {}
impl DeferredContext {
    pub fn new(dc: ComPtr<winapi::ID3D11DeviceContext>) -> DeferredContext {
        DeferredContext(dc, None)
    }
}
impl Drop for DeferredContext {
    fn drop(&mut self) {
        unsafe { self.0.Release() };
    }
}
impl command::Parser for DeferredContext {
    fn reset(&mut self) {
        if let Some(cl) = self.1 {
            unsafe { (*cl).Release() };
            self.1 = None;
        }
        unsafe {
            self.0.ClearState()
        };
    }
    fn parse(&mut self, com: command::Command) {
        let db = command::DataBuffer::new(); //not used
        execute::process(&mut self.0, &com, &db);
    }
    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        execute::update_buffer(&mut self.0, &buf, data, offset);
    }
    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>, data: &[u8], image: tex::RawImageInfo) {
        execute::update_texture(&mut self.0, &tex, kind, face, data, &image);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fence;

pub struct Adapter {
    adapter: ComPtr<winapi::IDXGIAdapter1>,
    info: core::AdapterInfo,
    queue_family: [(QueueFamily, QueueType); 1],
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)]) -> core::Gpu<Backend> {
        // Only support a single queue
        assert_eq!(queue_descs.len(), 1);
        assert!(queue_descs[0].2 <= 1);

        // Create D3D11 device
        let mut feature_level = winapi::D3D_FEATURE_LEVEL_10_0;

        let (mut dev, context) = {
            let mut device: *mut winapi::ID3D11Device         = ptr::null_mut();
            let mut context: *mut winapi::ID3D11DeviceContext = ptr::null_mut();

            let hr = unsafe {
                d3d11::D3D11CreateDevice(
                    self.adapter.as_mut() as *mut _ as *mut winapi::IDXGIAdapter,
                    winapi::D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(),
                    0, // TODO
                    &FEATURE_LEVELS[0],
                    FEATURE_LEVELS.len() as winapi::UINT,
                    winapi::D3D11_SDK_VERSION,
                    &mut device as *mut *mut winapi::ID3D11Device,
                    &mut feature_level as *mut _,
                    &mut context as *mut *mut winapi::ID3D11DeviceContext,
                )
            };

            if !winapi::SUCCEEDED(hr) {
                error!("error on device creation: {:x}", hr);
            }

            unsafe { (ComPtr::new(device), ComPtr::new(context)) }
        };

        let share = Arc::new(Share {
            capabilities: core::Capabilities {
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
        });

        let device = Device::new(dev.clone(), feature_level, share.clone());
        let mut gpu = core::Gpu {
            device,
            general_queues: Vec::new(),
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types: Vec::new(),
            memory_heaps: Vec::new(),
        };

        let raw_queue = || {
            CommandQueue {
                device: dev.clone(),
                context: context.clone(),
                share: share.clone(),
                frame_handles: handle::Manager::new(),
                max_resource_count: Some(999999),
            }
        };

        let (_, queue_type, num_queues) = queue_descs[0];
        for _ in 0..num_queues {
            unsafe {
                match queue_type {
                    QueueType::General => {
                        gpu.general_queues.push(core::GeneralQueue::new(raw_queue()));
                    }
                    QueueType::Graphics => {
                        gpu.graphics_queues.push(core::GraphicsQueue::new(raw_queue()));
                    }
                    QueueType::Compute => {
                        gpu.compute_queues.push(core::ComputeQueue::new(raw_queue()));
                    }
                    QueueType::Transfer => {
                        gpu.transfer_queues.push(core::TransferQueue::new(raw_queue()));
                    }
                }
            }
        }

        gpu
    }

    fn info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn queue_families(&self) -> &[(QueueFamily, QueueType)] {
        &self.queue_family
    }
}

pub struct CommandQueue {
    #[doc(hidden)]
    pub device: ComPtr<winapi::ID3D11Device>,
    context: ComPtr<winapi::ID3D11DeviceContext>,
    share: Arc<Share>,
    frame_handles: handle::Manager<Resources>,
    max_resource_count: Option<usize>,
}

impl CommandQueue {
    pub fn before_submit<'a>(&mut self, gpu_access: &'a AccessInfo<Resources>)
                             -> core::SubmissionResult<AccessGuard<'a, Resources>> {
        let mut gpu_access = try!(gpu_access.take_accesses());
        for (buffer, mut mapping) in gpu_access.access_mapped() {
            device::ensure_unmapped(&mut mapping, buffer, &mut self.context);
        }
        Ok(gpu_access)
    }
}

impl core::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<'a, I>(
        &mut self,
        submit_infos: I,
        fence: Option<&h::Fence<Resources>>,
        access: &com::AccessInfo<Resources>,
    ) where I: Iterator<Item=core::RawSubmission<'a, Backend>> {
        let _guard = self.before_submit(access).unwrap();
        for submit in submit_infos {
            for cb in submit.cmd_buffers {
                unsafe { self.context.ClearState(); }
                for com in &cb.parser.0 {
                    execute::process(&mut self.context, com, &cb.parser.1);
                }
            }
        }

        // TODO: handle sync
    }

    fn pin_submitted_resources(&mut self, man: &handle::Manager<Resources>) {
        self.frame_handles.extend(man);
        match self.max_resource_count {
            Some(c) if self.frame_handles.count() > c => {
                error!("Way too many resources in the current frame. Did you call Device::cleanup()?");
                self.max_resource_count = None;
            },
            _ => (),
        }
    }

    fn cleanup(&mut self) {
        use core::handle::Producer;

        self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut self.context,
            |ctx, buffer| {
                buffer.mapping().map(|raw| {
                    // we have exclusive access because it's the last reference
                    let mut mapping = unsafe { raw.use_access() };
                    device::ensure_unmapped(&mut mapping, buffer, ctx);
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
                type Child = *mut winapi::ID3D11DeviceChild;
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
            |_, _| {}, // Semaphore
        );
    }
}

///
#[derive(Debug)]
pub struct QueueFamily;
impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 { 1 }
}
