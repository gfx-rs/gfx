extern crate d3d12;
extern crate dxguid;
extern crate dxgi;
extern crate gfx_core as core;
#[macro_use]
extern crate log;
extern crate smallvec;
extern crate winapi;
extern crate wio;

mod command;
pub mod data;
mod device;
mod native;
mod pool;

use core::{command as com, handle, QueueType};
use wio::com::ComPtr;

use std::ptr;
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;

pub type ShaderModel = u16;

#[derive(Clone)]
pub struct QueueFamily;

impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 { 1 } // TODO: infinite software queues actually
}

#[derive(Clone)]
pub struct Adapter {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    info: core::AdapterInfo,
    queue_families: Vec<(QueueFamily, QueueType)>,
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)]) -> core::Gpu<Backend>
    {
        // Create D3D12 device
        let mut device = unsafe { ComPtr::<winapi::ID3D12Device>::new(ptr::null_mut()) };
        let hr = unsafe {
            d3d12::D3D12CreateDevice(
                self.adapter.as_mut() as *mut _ as *mut winapi::IUnknown,
                winapi::D3D_FEATURE_LEVEL_12_0, // TODO: correct feature level?
                &dxguid::IID_ID3D12Device,
                &mut device.as_mut() as *mut &mut _ as *mut *mut c_void,
            )
        };
        if !winapi::SUCCEEDED(hr) {
            error!("error on device creation: {:x}", hr);
        }

        // TODO: other queue types
        // Create command queues
        let mut general_queues = queue_descs.iter().flat_map(|&(_family, _ty, queue_count)| {
            (0..queue_count).map(|_| {
                let mut queue = unsafe { ComPtr::<winapi::ID3D12CommandQueue>::new(ptr::null_mut()) };
                let queue_desc = winapi::D3D12_COMMAND_QUEUE_DESC {
                    Type: winapi::D3D12_COMMAND_LIST_TYPE_DIRECT, // TODO: correct queue type
                    Priority: 0,
                    Flags: winapi::D3D12_COMMAND_QUEUE_FLAG_NONE,
                    NodeMask: 0,
                };

                let hr = unsafe {
                    device.CreateCommandQueue(
                        &queue_desc,
                        &dxguid::IID_ID3D12CommandQueue,
                        &mut queue.as_mut() as *mut &mut _ as *mut *mut c_void,
                    )
                };

                if !winapi::SUCCEEDED(hr) {
                    error!("error on queue creation: {:x}", hr);
                }

                unsafe {
                    core::GeneralQueue::new(
                        CommandQueue {
                            raw: queue,
                            device: device.clone(),
                            list_type: winapi::D3D12_COMMAND_LIST_TYPE_DIRECT, // TODO
                            frame_handles: handle::Manager::new(),
                            max_resource_count: Some(999999),
                        }
                    )
                }
            }).collect::<Vec<_>>()
        }).collect();

        let device = Device::new(device);

        core::Gpu {
            device,
            general_queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types: Vec::new(), // TODO
            memory_heaps: Vec::new(), // TODO
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        unimplemented!()
    }

    fn get_queue_families(&self) -> &[(QueueFamily, QueueType)] {
        unimplemented!()
    }
}

pub struct CommandQueue {
    #[doc(hidden)]
    pub raw: ComPtr<winapi::ID3D12CommandQueue>,
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,

    frame_handles: handle::Manager<Backend>,
    max_resource_count: Option<usize>,
}

impl core::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<'a, I>(
        &mut self,
        submit_infos: I,
        fence: Option<&handle::Fence<Backend>>,
        access: &com::AccessInfo<Backend>,
    ) where I: Iterator<Item=core::RawSubmission<'a, Backend>> {
        unimplemented!()
    }

    fn pin_submitted_resources(&mut self, man: &handle::Manager<Backend>) {
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
        // TODO
    }
}

pub struct Device {
    device: ComPtr<winapi::ID3D12Device>,
}

impl Device {
    fn new(device: ComPtr<winapi::ID3D12Device>) -> Device {
        Device {
            device: device,
        }
    }

    /// Return the maximum supported shader model.
    pub fn get_shader_model(&self) -> ShaderModel {
        unimplemented!()
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type CommandQueue = CommandQueue;
    type RawCommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type SubmitInfo = command::SubmitInfo;
    type Device = Device;
    type QueueFamily = QueueFamily;

    type RawCommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type Buffer = native::Buffer;
    type ShaderResourceView = ();
    type UnorderedAccessView = ();
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;
    type Sampler = ();
    type Fence = ();
    type Semaphore = ();
    type Mapping = Mapping;

    type ShaderLib = native::ShaderLib;
    type Image = native::Image;
    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSet = ();
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type RenderPass = native::RenderPass;
    type FrameBuffer = native::FrameBuffer;
}

pub struct Instance {
    #[doc(hidden)]
    pub factory: ComPtr<winapi::IDXGIFactory4>,
}

impl Instance {
    pub fn create() -> Instance {
        // Enable debug layer
        {
            let mut debug_controller: *mut winapi::ID3D12Debug = ptr::null_mut();
            let hr = unsafe {
                d3d12::D3D12GetDebugInterface(
                    &dxguid::IID_ID3D12Debug,
                    &mut debug_controller as *mut *mut _ as *mut *mut c_void)
            };

            if winapi::SUCCEEDED(hr) {
                unsafe { (*debug_controller).EnableDebugLayer() };
            }
        }

        // Create DXGI factory
        let mut dxgi_factory: *mut winapi::IDXGIFactory4 = ptr::null_mut();

        let hr = unsafe {
            dxgi::CreateDXGIFactory2(
                winapi::DXGI_CREATE_FACTORY_DEBUG,
                &dxguid::IID_IDXGIFactory4,
                &mut dxgi_factory as *mut *mut _ as *mut *mut c_void)
        };

        if !winapi::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        Instance {
            factory: unsafe { ComPtr::new(dxgi_factory) },
        }
    }

    pub fn enumerate_adapters(&mut self) -> Vec<Adapter> {
        // Enumerate adapters
        let mut cur_index = 0;
        let mut devices = Vec::new();
        loop {
            let mut adapter = {
                let mut adapter: *mut winapi::IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe {
                    self.factory.EnumAdapters1(
                        cur_index,
                        &mut adapter as *mut *mut _)
                };

                if hr == winapi::DXGI_ERROR_NOT_FOUND {
                    break;
                }

                unsafe { ComPtr::new(adapter as *mut winapi::IDXGIAdapter2) }
            };

            // Check for D3D12 support
            let hr = unsafe {
                d3d12::D3D12CreateDevice(
                    adapter.as_mut() as *mut _ as *mut winapi::IUnknown,
                    winapi::D3D_FEATURE_LEVEL_11_0, // TODO: correct feature level?
                    &dxguid::IID_ID3D12Device,
                    ptr::null_mut(),
                )
            };

            if winapi::SUCCEEDED(hr) {
                // We have found a possible adapter
                // acquire the device information
                let mut desc: winapi::DXGI_ADAPTER_DESC2 = unsafe { std::mem::uninitialized() };
                unsafe { adapter.GetDesc2(&mut desc); }

                let device_name = {
                    let len = desc.Description.iter().take_while(|&&c| c != 0).count();
                    let name = <OsString as OsStringExt>::from_wide(&desc.Description[..len]);
                    name.to_string_lossy().into_owned()
                };

                let info = core::AdapterInfo {
                    name: device_name,
                    vendor: desc.VendorId as usize,
                    device: desc.DeviceId as usize,
                    software_rendering: false, // TODO: check for WARP adapter (software rasterizer)?
                };

                devices.push(
                    Adapter {
                        adapter: adapter,
                        info: info,
                        queue_families: vec![(QueueFamily, QueueType::General)], // TODO:
                    });
            }

            cur_index += 1;
        }
        devices
    }
}

// TODO: temporary
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Mapping;

impl core::mapping::Gate<Backend> for Mapping {
    unsafe fn set<T>(&self, index: usize, val: T) {
        unimplemented!()
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        unimplemented!()
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        unimplemented!()
    }
}
