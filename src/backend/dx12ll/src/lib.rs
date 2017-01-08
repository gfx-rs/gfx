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

#[macro_use]
extern crate log;
extern crate comptr;
extern crate d3d12;
extern crate dxgi;
extern crate dxguid;
extern crate gfx_corell as core;
extern crate winapi;
extern crate winit;

use comptr::ComPtr;
use std::ptr;
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;
use winapi::BOOL;
use winit::os::windows::WindowExt;

mod data;

#[derive(Clone)]
pub struct PhysicalDevice {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    info: core::PhysicalDeviceInfo,
}

impl core::PhysicalDevice for PhysicalDevice {
    type B = Backend;

    fn open(&self) -> (Device, Vec<CommandQueue>) {
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
            error!("error on device creation: {:?}", hr);
        }

        // Create command queues
        // TODO: Let the users decide how many and which queues they want to create
        let mut queue = ComPtr::<winapi::ID3D12CommandQueue>::new(ptr::null_mut());
        let queue_desc = winapi::D3D12_COMMAND_QUEUE_DESC {
            Type: winapi::D3D12_COMMAND_LIST_TYPE_DIRECT,
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
            error!("error on queue creation: {:?}", hr);
        }

        (Device { inner: device }, vec![CommandQueue { inner: queue }])
    }

    fn get_info(&self) -> &core::PhysicalDeviceInfo {
        &self.info
    }
}

pub struct Device {
    inner: ComPtr<winapi::ID3D12Device>,
}

impl core::Device for Device {

}

pub struct CommandQueue {
    inner: ComPtr<winapi::ID3D12CommandQueue>,
}

impl core::CommandQueue for CommandQueue {
    type B = Backend;

    fn submit(&mut self, cmd_buffer: &()) {
        unimplemented!()
    }
}

pub struct Surface {
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
}

impl core::Surface for Surface {
    type B = Backend;
    type Window = winit::Window;

    fn from_window(window: &winit::Window, instance: &Instance) -> Surface {
        Surface {
            factory: instance.inner.clone(),
            wnd_handle: window.get_hwnd() as *mut _,
        }
    }

    fn build_swapchain<T: core::format::RenderFormat>(&self, width: u32, height: u32, present_queue: &CommandQueue) -> SwapChain {
        let mut swap_chain = ComPtr::<winapi::IDXGISwapChain1>::new(ptr::null_mut());

        // TODO: double-check values
        let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: winapi::DXGI_ALPHA_MODE(0),
            BufferCount: 2,
            Width: width,
            Height: height,
            Format: data::map_format(T::get_format(), true).unwrap(), // TODO: error handling
            Flags: 0,
            BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Scaling: winapi::DXGI_SCALING(0),
            Stereo: false as BOOL,
            SwapEffect: winapi::DXGI_SWAP_EFFECT(4), // TODO: FLIP_DISCARD
        };

        let hr = unsafe {
            (**self.factory.as_ref()).CreateSwapChainForHwnd(
                present_queue.inner.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
                self.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                swap_chain.as_mut() as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            error!("error on swapchain creation {:x}", hr);
        }

        SwapChain {
            inner: swap_chain,
        }
    }
}

pub struct SwapChain {
    inner: ComPtr<winapi::IDXGISwapChain1>,
}

impl core::SwapChain for SwapChain {
    type B = Backend;

    fn present(&mut self) {
        unsafe { self.inner.Present(1, 0); }
    }
}

pub struct Instance {
    inner: ComPtr<winapi::IDXGIFactory4>,
    physical_devices: Vec<PhysicalDevice>,
}

impl core::Instance for Instance {
    type B = Backend;

    fn create() -> Instance {
        // Enable debug layer
        let mut debug_controller = ComPtr::<winapi::ID3D12Debug>::new(ptr::null_mut());
        let hr = unsafe {
            d3d12::D3D12GetDebugInterface(
                &dxguid::IID_ID3D12Debug,
                debug_controller.as_mut() as *mut *mut _ as *mut *mut c_void)
        };

        if winapi::SUCCEEDED(hr) {
            unsafe { debug_controller.EnableDebugLayer() };
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

                let info = core::PhysicalDeviceInfo {
                    name: device_name,
                    vendor: desc.VendorId as usize,
                    device: desc.DeviceId as usize,
                    software_rendering: false, // TODO: check for WARP adapter (software rasterizer)?
                };

                devices.push(
                    PhysicalDevice {
                        adapter: adapter,
                        info: info,
                    });
            }

            cur_index += 1;
        }

        Instance {
            inner: dxgi_factory,
            physical_devices: devices,
        }
    }

    fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice> {
        self.physical_devices.clone()
    }
}

pub enum Backend { }

impl core::Backend for Backend {
    type CommandBuffer = ();
    type CommandQueue = CommandQueue;
    type Device = Device;
    type Instance = Instance;
    type PhysicalDevice = PhysicalDevice;
    type Resources = Resources;
    type Surface = Surface;
    type SwapChain = SwapChain;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources { }

impl core::Resources for Resources {
    type Buffer = ();
    type Shader = ();
    type RenderPass = ();
    type PipelineLayout = ();
    type PipelineStateObject = ();
    type Image = ();
    type ShaderResourceView = ();
    type UnorderedAccessView = ();
    type RenderTargetView = ();
    type DepthStencilView = ();
    type Sampler = ();
}
