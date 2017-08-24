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

#[deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate dxguid;
extern crate winapi;
extern crate winit;
extern crate gfx_core as core;
extern crate gfx_device_dx11 as device_dx11;
extern crate gfx_device_dx12 as device_dx12;
extern crate wio;

use std::ptr;
use std::rc::Rc;
use std::os::raw::c_void;
use std::collections::VecDeque;
use winit::os::windows::WindowExt;
use core::{handle as h, memory, texture as tex};
use wio::com::ComPtr;

/*
pub fn resize_swap_chain<Cf>(&mut self, factory: &mut Factory, width: Size, height: Size)
                            -> Result<h::RenderTargetView<Resources, Cf>, winapi::HRESULT>
where Cf: format::RenderFormat
{
    let result = unsafe {
        (*self.swap_chain).ResizeBuffers(0,
            width as winapi::UINT, height as winapi::UINT,
            winapi::DXGI_FORMAT_UNKNOWN, 0)
    };
    if result == winapi::S_OK {
        self.size = (width, height);
        let raw = self.make_back_buffer(factory);
        Ok(memory::Typed::new(raw))
    } else {
        Err(result)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InitError {
    /// Unable to create a window.
    Window,
    /// Unable to map format to DXGI.
    Format(format::Format),
    /// Unable to find a supported driver type.
    DriverType,
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf>(window: &mut Window, factory: &mut Factory, width: u16, height: u16)
            -> Result<h::RenderTargetView<Resources, Cf>, f::TargetViewError>
where Cf: format::RenderFormat
{

    factory.cleanup();
    // device.clear_state();
    // device.cleanup();

    window.resize_swap_chain::<Cf>(factory, width, height)
        .map_err(|hr| {
            error!("Resize failed with code {:X}", hr);
            f::TargetViewError::NotDetached
        }
    )
}
*/

fn get_window_dimensions(window: &winit::Window) -> tex::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    ((width as f32 * window.hidpi_factor()) as tex::Size, (height as f32 * window.hidpi_factor()) as tex::Size, 1, 1.into())
}

pub struct Surface11 {
    factory: ComPtr<winapi::IDXGIFactory2>,
    window: Rc<winit::Window>,
    manager: h::Manager<device_dx11::Resources>,
}

impl core::Surface<device_dx11::Backend> for Surface11 {
    type Swapchain = Swapchain11;

    fn supports_queue(&self, _: &device_dx11::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Swapchain11
        where Q: AsRef<device_dx11::CommandQueue>
    {
        use core::handle::Producer;

        let present_queue = present_queue.as_ref();
        let dim = get_window_dimensions(&self.window);

        let mut swap_chain = {
            let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
            let buffer_count = 2; // TODO: user-defined value

            // TODO: double-check values
            let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
                AlphaMode: winapi::DXGI_ALPHA_MODE(0),
                BufferCount: buffer_count,
                Width: dim.0 as u32,
                Height: dim.1 as u32,
                Format: device_dx11::data::map_format(config.color_format, true).unwrap(), // TODO: error handling
                Flags: 0,
                BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SampleDesc: winapi::DXGI_SAMPLE_DESC { // TODO
                    Count: 1,
                    Quality: 0,
                },
                Scaling: winapi::DXGI_SCALING(0),
                Stereo: false as winapi::BOOL,
                SwapEffect: winapi::DXGI_SWAP_EFFECT(4), // TODO: FLIP_DISCARD
            };

            let hr = unsafe {
                self.factory.as_mut().CreateSwapChainForHwnd(
                    present_queue.device.as_mut() as *mut _ as *mut winapi::IUnknown,
                    self.window.get_hwnd() as *mut _,
                    &desc,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut swap_chain as *mut *mut _,
                )
            };

            if !winapi::SUCCEEDED(hr) {
                error!("error on swapchain creation {:x}", hr);
            }

            unsafe { ComPtr::new(swap_chain) }
        };

        let backbuffer = {
            let mut back_buffer: *mut winapi::ID3D11Texture2D = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    0,
                    &dxguid::IID_ID3D11Texture2D,
                    &mut back_buffer as *mut *mut winapi::ID3D11Texture2D as *mut *mut _);
            }

            let kind = tex::Kind::D2(dim.0, dim.1, dim.3);
            let raw_tex = device_dx11::Texture::new(device_dx11::native::Texture::D2(back_buffer));
            let color_tex = self.manager.make_texture(
                                raw_tex,
                                tex::Info {
                                    kind,
                                    levels: 1,
                                    format: config.color_format.0,
                                    bind: memory::RENDER_TARGET,
                                    usage: memory::Usage::Data,
                                });

            let ds_tex = config.depth_stencil_format.map(|ds_format| {
                let info = tex::Info {
                    kind: tex::Kind::D2(dim.0, dim.1, dim.3),
                    levels: 1,
                    format: ds_format.0,
                    bind: memory::DEPTH_STENCIL,
                    usage: memory::Usage::Data,
                };

                let (usage, cpu_access) = device_dx11::data::map_usage(info.usage, info.bind);

                let desc = winapi::D3D11_TEXTURE2D_DESC {
                    Width: dim.0 as winapi::UINT,
                    Height: dim.1 as winapi::UINT,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: device_dx11::data::map_surface(info.format).unwrap(),
                    SampleDesc: device_dx11::data::map_anti_alias(dim.3),
                    Usage: usage,
                    BindFlags: device_dx11::data::map_bind(info.bind).0,
                    CPUAccessFlags: cpu_access.0,
                    MiscFlags: 0,
                };

                let mut raw = ptr::null_mut();
                let hr = unsafe {
                    present_queue.device.as_mut().CreateTexture2D(&desc, ptr::null(), &mut raw)
                };

                if !winapi::SUCCEEDED(hr) {
                    error!("DS texture creation failed on {:#?} with error {:x}", desc, hr);
                }

                self.manager.make_texture(
                    device_dx11::Texture::new(device_dx11::native::Texture::D2(raw)),
                    tex::Info {
                        kind: tex::Kind::D2(dim.0, dim.1, dim.3),
                        levels: 1,
                        format: ds_format.0,
                        bind: memory::DEPTH_STENCIL,
                        usage: memory::Usage::Data,
                    })
            });

            (color_tex, ds_tex)

        };

        Swapchain11 {
            swap_chain,
            images: [backbuffer],
        }
    }
}

pub struct Surface12 {
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
    manager: h::Manager<device_dx12::Resources>,
    width: u32,
    height: u32,
}

impl core::Surface<device_dx12::Backend> for Surface12 {
    type Swapchain = Swapchain12;

    fn supports_queue(&self, _: &device_dx12::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Swapchain12
        where Q: AsRef<device_dx12::CommandQueue>
    {
        use core::handle::Producer;
        let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
        let buffer_count = 2; // TODO: user-defined value

        // TODO: double-check values
        let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: winapi::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: buffer_count,
            Width: self.width,
            Height: self.height,
            Format: device_dx12::data::map_format(config.color_format, true).unwrap(), // TODO: error handling
            Flags: 0,
            BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Scaling: winapi::DXGI_SCALING_STRETCH,
            Stereo: false as winapi::BOOL,
            SwapEffect: winapi::DXGI_SWAP_EFFECT(4), // TODO: DXGI_SWAP_EFFECT_FLIP_DISCARD missing in winapi
        };

        let hr = unsafe {
            self.factory.CreateSwapChainForHwnd(
                present_queue.as_ref().raw.as_mut() as *mut _ as *mut winapi::IUnknown,
                self.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                &mut swap_chain as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            error!("error on swapchain creation {:x}", hr);
        }

        let mut swap_chain = unsafe { ComPtr::<winapi::IDXGISwapChain3>::new(swap_chain as *mut winapi::IDXGISwapChain3) };

        // Get backbuffer images
        let backbuffers = (0..buffer_count).map(|i| {
            let mut resource: *mut winapi::ID3D12Resource = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    i,
                    &dxguid::IID_ID3D12Resource,
                    &mut resource as *mut *mut _ as *mut *mut c_void);
            }

            // TODO: correct AA mode
            let kind = tex::Kind::D2(self.width as u16, self.height as u16, 1.into());
            let color_tex = self.manager.make_texture(
                                (),
                                tex::Info {
                                    kind,
                                    levels: 1,
                                    format: config.color_format.0,
                                    bind: memory::RENDER_TARGET,
                                    usage: memory::Usage::Data,
                                });

            let ds_tex = config.depth_stencil_format.map(|ds_format| {
                self.manager.make_texture(
                    (),
                    tex::Info {
                        kind,
                        levels: 1,
                        format: ds_format.0,
                        bind: memory::DEPTH_STENCIL,
                        usage: memory::Usage::Data,
                    })
            });

            (color_tex, ds_tex)
        }).collect::<Vec<_>>();

        Swapchain12 {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            images: backbuffers,
        }
    }
}

pub struct Swapchain11 {
    swap_chain: ComPtr<winapi::IDXGISwapChain1>,
    images: [core::Backbuffer<device_dx11::Backend>; 1],
}

impl core::Swapchain<device_dx11::Backend> for Swapchain11 {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_dx11::Backend>] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_dx11::Resources>) -> core::Frame {
        // TODO: sync
        core::Frame::new(0)
    }

    fn present<Q>(&mut self, _present_queue: &mut Q, wait_semaphores: &[&h::Semaphore<device_dx11::Resources>])
        where Q: AsMut<device_dx11::CommandQueue>
    {
        // TODO: wait semaphores
        unsafe { self.swap_chain.Present(1, 0); }
    }
}

pub struct Swapchain12 {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
    images: Vec<core::Backbuffer<device_dx12::Backend>>,
}

impl core::Swapchain<device_dx12::Backend> for Swapchain12 {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_dx12::Backend>] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_dx12::Resources>) -> core::Frame {
        // TODO: sync
        // TODO: we need to block this at some point? (running out of backbuffers)
        // let num_images = self.images.len();
        // let index = self.next_frame;
        // self.frame_queue.push_back(index);
        // self.next_frame = (self.next_frame + 1) % num_images;
        // unsafe { core::Frame::new(index) };

        // TODO:
        let index = unsafe { self.inner.GetCurrentBackBufferIndex() };
        unsafe { core::Frame::new(index as usize) }
    }

    fn present<Q>(&mut self, _present_queue: &mut Q, wait_semaphores: &[&h::Semaphore<device_dx12::Resources>])
        where Q: AsMut<device_dx12::CommandQueue>
    {
        // TODO: wait semaphores
        unsafe { self.inner.Present(1, 0); }
    }
}

pub struct Window(Rc<winit::Window>);

impl Window {
    /// Create a new window.
    pub fn new(window: winit::Window) -> Self {
        Window(Rc::new(window))
    }

    /// Get internal winit window.
    pub fn raw(&self) -> &winit::Window {
        &self.0
    }
}

impl core::WindowExt<device_dx11::Backend> for Window {
    type Surface = Surface11;
    type Adapter = device_dx11::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface11, Vec<device_dx11::Adapter>) {
        let mut instance = device_dx11::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            Surface11 {
                factory: instance.0,
                window: self.0.clone(),
                manager: h::Manager::new()
            }
        };

        (surface, adapters)
    }
}

impl core::WindowExt<device_dx12::Backend> for Window {
    type Surface = Surface12;
    type Adapter = device_dx12::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface12, Vec<device_dx12::Adapter>) {
        let mut instance = device_dx12::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            let (width, height) = self.0.get_inner_size_pixels().unwrap();
            Surface12 {
                factory: instance.factory.clone(),
                wnd_handle: self.0.get_hwnd() as *mut _,
                manager: h::Manager::new(),
                width: width,
                height: height,
            }
        };

        (surface, adapters)
    }
}