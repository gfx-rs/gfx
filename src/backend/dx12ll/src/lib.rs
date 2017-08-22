// Copyright 2017 The Gfx-rs Developers.
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

#[macro_use]
extern crate log;
extern crate comptr;
extern crate d3d12;
extern crate d3dcompiler;
extern crate dxgi;
extern crate dxguid;
#[macro_use]
extern crate gfx_corell as core;
extern crate kernel32;
extern crate winapi;
extern crate winit;

use comptr::ComPtr;
use std::{mem, ptr};
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::collections::VecDeque;
use std::ffi::OsString;
use winapi::BOOL;
use winit::os::windows::WindowExt;

use core::{image, memory, Capabilities};

mod command;
mod factory;
mod mirror;
mod native;
mod pool;
mod state;

pub use pool::{GeneralCommandPool, GraphicsCommandPool,
    ComputeCommandPool, TransferCommandPool, SubpassCommandPool};

const _HEAP_TYPES: [winapi::D3D12_HEAP_TYPE; 3] = [
    winapi::D3D12_HEAP_TYPE_DEFAULT,
    winapi::D3D12_HEAP_TYPE_UPLOAD,
    winapi::D3D12_HEAP_TYPE_READBACK,
];

#[derive(Clone)]
pub struct QueueFamily;

impl core::QueueFamily for QueueFamily {
    type Surface = Surface;

    fn supports_present(&self, _surface: &Surface) -> bool {
        //
        true
    }

    fn num_queues(&self) -> u32 {
        // TODO: actually infinite, need to find a good way to handle this
        1
    }
}

#[derive(Clone)]
pub struct Adapter {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    info: core::AdapterInfo,
    queue_families: Vec<QueueFamily>,
}

impl core::Adapter for Adapter {
    type CommandQueue = CommandQueue;
    type Resources = Resources;
    type Factory = Factory;
    type QueueFamily = QueueFamily;

    fn open<'a, I>(&self, queue_descs: I) -> core::Device<Resources, Factory, CommandQueue>
        where I: Iterator<Item=(&'a QueueFamily, u32)>
    {
        // Create D3D12 device
        let mut device = ComPtr::<winapi::ID3D12Device>::new(ptr::null_mut());
        let hr = unsafe {
            d3d12::D3D12CreateDevice(
                self.adapter.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
                winapi::D3D_FEATURE_LEVEL_12_0, // TODO: correct feature level?
                &dxguid::IID_ID3D12Device,
                device.as_mut() as *mut *mut _ as *mut *mut c_void,
            )
        };
        if !winapi::SUCCEEDED(hr) {
            error!("error on device creation: {:x}", hr);
        }

        let mut features: winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS = unsafe { mem::zeroed() };
        assert_eq!(winapi::S_OK, unsafe {
            device.CheckFeatureSupport(winapi::D3D12_FEATURE_D3D12_OPTIONS,
                &mut features as *mut _ as *mut c_void,
                mem::size_of::<winapi::D3D12_FEATURE_DATA_D3D12_OPTIONS>() as u32)
        });

        // TODO: other queue types
        // Create command queues
        let general_queues = queue_descs.flat_map(|(_family, queue_count)| {
            (0..queue_count).map(|_| {
                let mut queue = ComPtr::<winapi::ID3D12CommandQueue>::new(ptr::null_mut());
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
                        queue.as_mut() as *mut *mut _ as *mut *mut c_void,
                    )
                };

                if !winapi::SUCCEEDED(hr) {
                    error!("error on queue creation: {:x}", hr);
                }

                unsafe {
                    core::GeneralQueue::new(
                        CommandQueue {
                            inner: queue,
                            device: device.clone(),
                            list_type: queue_desc.Type,
                        }
                    )
                }
            }).collect::<Vec<_>>()
        }).collect();

        // https://msdn.microsoft.com/en-us/library/windows/desktop/dn788678(v=vs.85).aspx
        let heap_types = vec![
            core::HeapType {
                id: 0,
                properties: memory::DEVICE_LOCAL,
                heap_index: 1,
            },
            core::HeapType {
                id: 1,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::HeapType {
                id: 2,
                properties: memory::CPU_VISIBLE | memory::COHERENT,
                heap_index: 0,
            },
        ];

        let memory_heaps = Vec::new(); //TODO

        let caps = Capabilities {
            heterogeneous_resource_heaps: features.ResourceHeapTier != winapi::D3D12_RESOURCE_HEAP_TIER_1,
            buffer_copy_offset_alignment: winapi::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as usize,
            buffer_copy_row_pitch_alignment: winapi::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize,
        };
        let factory = Factory::new(device, caps.clone());

        core::Device {
            factory,
            general_queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types,
            memory_heaps,
            caps,
            _marker: std::marker::PhantomData,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn get_queue_families(&self) -> std::slice::Iter<QueueFamily> {
        self.queue_families.iter()
    }
}

pub struct Factory {
    inner: ComPtr<winapi::ID3D12Device>,
    rtv_pool: native::DescriptorSetPool,
    srv_pool: native::DescriptorSetPool,
    sampler_pool: native::DescriptorSetPool,
    events: Vec<winapi::HANDLE>,
    caps: core::Capabilities,
}

impl Factory {
    fn new(mut device: ComPtr<winapi::ID3D12Device>, caps: Capabilities) -> Factory {
        let max_rtvs = 64;
        let rtv_pool = native::DescriptorSetPool {
            heap: Self::create_descriptor_heap_impl(&mut device, winapi::D3D12_DESCRIPTOR_HEAP_TYPE_RTV, false, max_rtvs),
            pools: Vec::new(),
            offset: 0,
            size: 0,
            max_size: max_rtvs as u64,
        };

        let max_srvs = 0x1000; //TODO
        let srv_pool = native::DescriptorSetPool {
            heap: Self::create_descriptor_heap_impl(&mut device, winapi::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, false, max_srvs),
            pools: Vec::new(),
            offset: 0,
            size: 0,
            max_size: max_srvs as u64,
        };

        let max_samplers = 0x100;
        let sampler_pool = native::DescriptorSetPool {
            heap: Self::create_descriptor_heap_impl(&mut device, winapi::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, false, max_samplers),
            pools: Vec::new(),
            offset: 0,
            size: 0,
            max_size: max_samplers as u64,
        };

        Factory {
            inner: device,
            rtv_pool,
            srv_pool,
            sampler_pool,
            events: Vec::new(),
            caps,
        }
    }
}

pub struct CommandQueue {
    inner: ComPtr<winapi::ID3D12CommandQueue>,
    device: ComPtr<winapi::ID3D12Device>,
    list_type: winapi::D3D12_COMMAND_LIST_TYPE,
}

impl core::CommandQueue for CommandQueue {
    type R = Resources;
    type SubmitInfo = command::SubmitInfo;
    type GeneralCommandBuffer = native::GeneralCommandBuffer;
    type GraphicsCommandBuffer = native::GraphicsCommandBuffer;
    type ComputeCommandBuffer = native::ComputeCommandBuffer;
    type TransferCommandBuffer = native::TransferCommandBuffer;
    type SubpassCommandBuffer = native::SubpassCommandBuffer;

    unsafe fn submit<C>(&mut self, submit_infos: &[core::QueueSubmit<C, Resources>], fence: Option<&mut native::Fence>)
        where C: core::CommandBuffer<SubmitInfo = command::SubmitInfo>
    {
        let mut lists = Vec::new();
        for submit in submit_infos {
            lists.clear();
            lists.extend(submit.cmd_buffers.iter()
                .map(|cmd| cmd.get_info().0.as_mut_ptr() as *mut _));
            self.inner.ExecuteCommandLists(lists.len() as u32, lists.as_mut_ptr());
        }

        if let Some(fence) = fence {
            assert_eq!(winapi::S_OK,
                self.inner.Signal(fence.inner.as_mut_ptr(), 1)
            );
        }
    }
}

pub struct Surface {
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
    width: u32,
    height: u32,
}

impl core::Surface for Surface {
    type Queue = CommandQueue;
    type Swapchain = Swapchain;

    fn build_swapchain<T: core::format::RenderFormat>(&self, present_queue: &CommandQueue) -> Swapchain {
        let mut swap_chain = ComPtr::<winapi::IDXGISwapChain1>::new(ptr::null_mut());
        let buffer_count = 2; // TODO: user-defined value
        let mut format = T::get_format();
        if format.1 == core::format::ChannelType::Srgb {
            // Apparently, swap chain doesn't like sRGB, but the RTV can still have some:
            // https://www.gamedev.net/forums/topic/670546-d3d12srgb-buffer-format-for-swap-chain/
            // [15716] DXGI ERROR: IDXGIFactory::CreateSwapchain: Flip model swapchains (DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL and DXGI_SWAP_EFFECT_FLIP_DISCARD) only support the following Formats: (DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM), assuming the underlying Device does as well.
            format.1 = core::format::ChannelType::Unorm;
        }
        let dxgi_format = state::map_format(format, true).unwrap();
        let bits_per_texel = format.0.get_total_bits();

        let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: winapi::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: buffer_count,
            Width: self.width,
            Height: self.height,
            Format: dxgi_format,
            Flags: 0,
            BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Scaling: winapi::DXGI_SCALING_STRETCH,
            Stereo: false as BOOL,
            SwapEffect: winapi::DXGI_SWAP_EFFECT(4), //DXGI_SWAP_EFFECT_FLIP_DISCARD,
        };

        let hr = unsafe {
            self.factory.clone().CreateSwapChainForHwnd(
                present_queue.inner.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
                self.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                swap_chain.as_mut() as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            panic!("error on swapchain creation {:x}", hr);
        }

        let mut swap_chain3 = ComPtr::<winapi::IDXGISwapChain3>::new(ptr::null_mut());
        assert_eq!(winapi::S_OK, unsafe {
            swap_chain.QueryInterface(&dxguid::IID_IDXGISwapChain3,
                swap_chain3.as_mut() as *mut *mut _ as *mut *mut c_void)
        });

        let kind = image::Kind::D2(self.width as image::Size, self.height as image::Size, image::AaMode::Single);

        // Get backbuffer images
        let backbuffers = (0..buffer_count).map(|i| {
            let mut resource = ComPtr::<winapi::ID3D12Resource>::new(ptr::null_mut());
            unsafe {
                swap_chain3.GetBuffer(
                    i,
                    &dxguid::IID_ID3D12Resource,
                    resource.as_mut() as *mut *mut _ as *mut *mut c_void);
            }

            native::Image { resource, kind, dxgi_format, bits_per_texel }
        }).collect::<Vec<_>>();

        Swapchain {
            inner: swap_chain3,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            images: backbuffers,
        }
    }
}

pub struct Swapchain {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
    images: Vec<native::Image>,
}

impl<'a> core::Swapchain for Swapchain {
    type R = Resources;
    type Image = native::Image;

    fn get_images(&mut self) -> &[native::Image] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<Resources>) -> core::Frame {
        //TODO: use sync
        // TODO: we need to block this at some point? (running out of backbuffers)
        let num_images = self.images.len();
        let index = self.next_frame;
        self.frame_queue.push_back(index);
        self.next_frame = (self.next_frame + 1) % num_images;
        unsafe { core::Frame::new(index) };

        // TODO:
        let index = unsafe { self.inner.GetCurrentBackBufferIndex() };
        unsafe { core::Frame::new(index as usize) }
    }

    fn present(&mut self) {
        unsafe { self.inner.Present(1, 0); }
    }
}

pub struct Instance {
    inner: ComPtr<winapi::IDXGIFactory4>,
    adapters: Vec<Adapter>,
}

impl core::Instance for Instance {
    type Adapter = Adapter;
    type Surface = Surface;
    type Window = winit::Window;

    fn create() -> Instance {
        // Enable debug layer
        {
            let mut debug_controller = ComPtr::<winapi::ID3D12Debug>::new(ptr::null_mut());
            let hr = unsafe {
                d3d12::D3D12GetDebugInterface(
                    &dxguid::IID_ID3D12Debug,
                    debug_controller.as_mut() as *mut *mut _ as *mut *mut c_void)
            };

            if winapi::SUCCEEDED(hr) {
                unsafe { debug_controller.EnableDebugLayer() };
            }
        }

        // Create DXGI factory
        let mut dxgi_factory = ComPtr::<winapi::IDXGIFactory4>::new(ptr::null_mut());

        let hr = unsafe {
            dxgi::CreateDXGIFactory2(
                winapi::DXGI_CREATE_FACTORY_DEBUG,
                &dxguid::IID_IDXGIFactory4,
                dxgi_factory.as_mut() as *mut *mut _ as *mut *mut c_void)
        };

        if !winapi::SUCCEEDED(hr) {
            error!("Failed on dxgi factory creation: {:?}", hr);
        }

        // Enumerate adapters
        let mut cur_index = 0;
        let mut devices = Vec::new();
        loop {
            let mut adapter = ComPtr::<winapi::IDXGIAdapter2>::new(ptr::null_mut());
            let hr = unsafe {
                dxgi_factory.EnumAdapters1(
                    cur_index,
                    adapter.as_mut() as *mut *mut _ as *mut *mut winapi::IDXGIAdapter1)
            };

            if hr == winapi::DXGI_ERROR_NOT_FOUND {
                break;
            }

            // Check for D3D12 support
            let hr = unsafe {
                d3d12::D3D12CreateDevice(
                    adapter.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
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
                        queue_families: vec![QueueFamily], // TODO:
                    });
            }

            cur_index += 1;
        }

        Instance {
            inner: dxgi_factory,
            adapters: devices,
        }
    }

    fn enumerate_adapters(&self) -> Vec<Adapter> {
        self.adapters.clone()
    }

    fn create_surface(&self, window: &winit::Window) -> Surface {
        let (width, height) = window.get_inner_size_pixels().unwrap();
        Surface {
            factory: self.inner.clone(),
            wnd_handle: window.get_hwnd() as *mut _,
            width: width,
            height: height,
        }
    }
}

pub enum Backend { }
impl core::Backend for Backend {
    type CommandQueue = CommandQueue;
    type Factory = Factory;
    type Instance = Instance;
    type Adapter = Adapter;
    type Resources = Resources;
    type Surface = Surface;
    type Swapchain = Swapchain;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources { }
impl core::Resources for Resources {
    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type PipelineLayout = native::PipelineLayout;
    type GraphicsPipeline = native::GraphicsPipeline;
    type ComputePipeline = native::ComputePipeline;
    type UnboundBuffer = factory::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = factory::UnboundImage;
    type Image = native::Image;
    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;
    type FrameBuffer = native::FrameBuffer;
    type Sampler = native::Sampler;
    type Semaphore = native::Semaphore;
    type Fence = native::Fence;
    type Heap = native::Heap;
    type Mapping = factory::Mapping;
    type DescriptorHeap = native::DescriptorHeap;
    type DescriptorSet = native::DescriptorSet;
    type DescriptorSetPool = native::DescriptorSetPool;
    type DescriptorSetLayout = native::DescriptorSetLayout;
}
