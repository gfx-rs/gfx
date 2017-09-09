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
mod data;
mod device;
mod native;
mod pool;
mod window;

use core::{Features, Limits, QueueType};
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

/// Create associated command queues for a specific queue type
fn collect_queues<C>(
     queue_descs: &[(&QueueFamily, QueueType, u32)],
     device_raw: &ComPtr<winapi::ID3D12Device>,
     collect_type: QueueType,
) -> Vec<core::CommandQueue<Backend, C>> {
    queue_descs.iter()
        .filter(|&&(_, qtype, _)| qtype == collect_type)
        .flat_map(|&(_, _, qcount)| {
            (0..qcount).map(|_| {
                let mut device = device_raw.clone();
                let queue = unsafe { ComPtr::<winapi::ID3D12CommandQueue>::new(ptr::null_mut()) };
                let qtype = match collect_type {
                    QueueType::General | QueueType::Graphics => winapi::D3D12_COMMAND_LIST_TYPE_DIRECT,
                    QueueType::Compute => winapi::D3D12_COMMAND_LIST_TYPE_COMPUTE,
                    QueueType::Transfer => winapi::D3D12_COMMAND_LIST_TYPE_COPY,
                };

                let queue_desc = winapi::D3D12_COMMAND_QUEUE_DESC {
                    Type: qtype,
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
                    core::CommandQueue::new(
                        CommandQueue {
                            raw: queue,
                            device,
                            list_type: qtype,
                        }
                    )
                }
            })
        }).collect()
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
        let device = unsafe { ComPtr::<winapi::ID3D12Device>::new(ptr::null_mut()) };
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

        core::Gpu {
            general_queues: collect_queues(queue_descs, &device, QueueType::General),
            graphics_queues: collect_queues(queue_descs, &device, QueueType::Graphics),
            compute_queues: collect_queues(queue_descs, &device, QueueType::Compute),
            transfer_queues: collect_queues(queue_descs, &device, QueueType::Transfer),
            heap_types: Vec::new(), // TODO
            memory_heaps: Vec::new(), // TODO
            device: Device::new(device),
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
    pub(crate) raw: ComPtr<winapi::ID3D12CommandQueue>,
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}

impl core::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(
        &mut self,
        _submission: core::RawSubmission<Backend>,
        _fence: Option<&native::Fence>,
    ) {
        unimplemented!()
    }
}

pub struct Device {
    device: ComPtr<winapi::ID3D12Device>,
    features: core::Features,
    limits: core::Limits,
}

impl Device {
    fn new(device: ComPtr<winapi::ID3D12Device>) -> Device {
        Device {
            device: device,
            features: Features { // TODO
                indirect_execution: true,
                draw_instanced: true,
                draw_instanced_base: true,
                draw_indexed_base: true,
                draw_indexed_instanced: true,
                draw_indexed_instanced_base_vertex: true,
                draw_indexed_instanced_base: true,
                instance_rate: false,
                vertex_base: false,
                srgb_color: false,
                constant_buffer: false,
                unordered_access_view: false,
                separate_blending_slots: false,
                copy_buffer: false,
                sampler_anisotropy: false,
                sampler_border_color: false,
                sampler_lod_bias: false,
                sampler_objects: false,
            },
            limits: Limits { // TODO
                max_texture_size: 0,
                max_patch_size: 0,
                max_viewports: 0,
                min_buffer_copy_offset_alignment: 0,
                min_buffer_copy_pitch_alignment: 0,
            },
        }
    }

    /// Return the maximum supported shader model.
    pub fn get_shader_model(&self) -> ShaderModel {
        unimplemented!()
    }
}

pub struct Instance {
    pub(crate) factory: ComPtr<winapi::IDXGIFactory4>,
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

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Device = Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type QueueFamily = QueueFamily;

    type Heap = native::Heap;
    type Mapping = device::Mapping;
    type CommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type FrameBuffer = native::FrameBuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type Sampler = native::Sampler;

    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
}
