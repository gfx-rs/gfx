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

use comptr::ComPtr;
use std::ptr;
use std::os::raw::c_void;
use std::os::windows::ffi::OsStringExt;
use std::ffi::OsString;

#[derive(Clone)]
pub struct PhysicalDevice {
    adapter: ComPtr<winapi::IDXGIAdapter2>,
    info: core::PhysicalDeviceInfo,
}

impl core::PhysicalDevice for PhysicalDevice {
    type B = Backend;

    fn open(&self) -> (Device, Vec<CommandQueue>) {
        unimplemented!()
    }

    fn get_info(&self) -> core::PhysicalDeviceInfo {
        self.info.clone()
    }
}

pub struct Device {

}

impl core::Device for Device {

}

pub struct CommandQueue {

}

impl core::CommandQueue for CommandQueue {
    type B = Backend;

    fn submit(&mut self, cmd_buffer: &()) {
        unimplemented!()
    }
}

pub struct SwapChain {

}

impl core::SwapChain for SwapChain {
    type B = Backend;

    fn present(&mut self) {
        unimplemented!()
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
