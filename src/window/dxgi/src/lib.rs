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
extern crate comptr;

use std::ptr;
use std::os::raw::c_void;
use std::collections::VecDeque;
use winit::os::windows::WindowExt;
use core::{format, handle as h, factory as f, memory, texture as tex};
use core::texture::Size;
use comptr::ComPtr;

/*
pub struct Window {
    inner: winit::Window,
    swap_chain: *mut winapi::IDXGISwapChain,
    driver_type: winapi::D3D_DRIVER_TYPE,
    color_format: format::Format,
    pub size: (Size, Size),
}

impl Window {
    pub fn is_accelerated(&self) -> bool {
        self.driver_type == winapi::D3D_DRIVER_TYPE_HARDWARE
    }

    pub fn swap_buffers(&self, wait: u8) {
        match unsafe {(*self.swap_chain).Present(wait as winapi::UINT, 0)} {
            winapi::S_OK | winapi::DXGI_STATUS_OCCLUDED => {}
            hr => panic!("Present Error: {:X}", hr)
        }
    }

    fn make_back_buffer(&self, factory: &mut Factory) -> h::RawRenderTargetView<Resources> {
        let mut back_buffer: *mut winapi::ID3D11Texture2D = ptr::null_mut();
        assert_eq!(winapi::S_OK, unsafe {
            (*self.swap_chain).GetBuffer(0, &dxguid::IID_ID3D11Texture2D,
                &mut back_buffer as *mut *mut winapi::ID3D11Texture2D as *mut *mut _)
        });

        let info = tex::Info {
            kind: tex::Kind::D2(self.size.0, self.size.1, tex::AaMode::Single),
            levels: 1,
            format: self.color_format.0,
            bind: memory::RENDER_TARGET,
            usage: memory::Usage::Data,
        };
        let desc = tex::RenderDesc {
            channel: self.color_format.1,
            level: 0,
            layer: None,
        };
        factory.wrap_back_buffer(back_buffer, info, desc)
    }

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

/// Initialize with a given size. Typed format version.
pub fn init<Cf>(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop)
           -> Result<(Window, Factory, h::RenderTargetView<Resources, Cf>), InitError>
where Cf: format::RenderFormat
{
    init_raw(wb, events_loop, Cf::get_format())
        .map(|(window, factory, color)| (window, factory, memory::Typed::new(color)))
}

/// Initialize with a given size. Raw format version.
pub fn init_raw(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop, color_format: format::Format)
                -> Result<(Window, Factory, h::RawRenderTargetView<Resources>), InitError> {
    let inner = match wb.build(events_loop) {
        Ok(w) => w,
        Err(_) => return Err(InitError::Window),
    };
    let (width, height) = inner.get_inner_size_pixels().unwrap();

    let driver_types = [
        winapi::D3D_DRIVER_TYPE_HARDWARE,
        winapi::D3D_DRIVER_TYPE_WARP,
        winapi::D3D_DRIVER_TYPE_REFERENCE,
    ];

    let swap_desc = winapi::DXGI_SWAP_CHAIN_DESC {
        BufferDesc: winapi::DXGI_MODE_DESC {
            Width: width as winapi::UINT,
            Height: height as winapi::UINT,
            Format: match device_dx11::map_format(color_format, true) {
                Some(fm) => fm,
                None => return Err(InitError::Format(color_format)),
            },
            RefreshRate: winapi::DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            ScanlineOrdering: winapi::DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: winapi::DXGI_MODE_SCALING_UNSPECIFIED,
        },
        SampleDesc: winapi::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 1,
        OutputWindow: inner.get_hwnd() as winapi::HWND,
        Windowed: winapi::TRUE,
        SwapEffect: winapi::DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };

    info!("Creating swap chain of size {}x{}", width, height);
    for dt in driver_types.iter() {
        match device_dx11::create(*dt, &swap_desc) {
            Ok((mut factory, chain)) => {
                // info!("Success with driver {:?}, shader model {}", *dt, device.get_shader_model());
                let win = Window {
                    inner: inner,
                    swap_chain: chain,
                    driver_type: *dt,
                    color_format: color_format,
                    size: (width as Size, height as Size),
                };
                let color = win.make_back_buffer(&mut factory);
                return Ok((win, factory, color))
            },
            Err(hres) => {
                info!("Failure with driver {:?}: code {:x}", *dt, hres);
            },
        }
    }
    Err(InitError::DriverType)
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

pub struct Surface11<'a> {
    factory: ComPtr<winapi::IDXGIFactory2>,
    window: &'a winit::Window,
    manager: h::Manager<device_dx11::Resources>,
}

impl<'a> core::Surface<device_dx11::Backend> for Surface11<'a> {
    type SwapChain = SwapChain11;

    fn supports_queue(&self, queue_family: &device_dx11::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> SwapChain11
        where Q: AsRef<device_dx11::CommandQueue>
    {
        use core::handle::Producer;

        let present_queue = present_queue.as_ref();
        let mut swap_chain = ComPtr::<winapi::IDXGISwapChain1>::new(ptr::null_mut());
        let buffer_count = 2; // TODO: user-defined value
        let dim = get_window_dimensions(self.window);

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
            (**self.factory.as_ref()).CreateSwapChainForHwnd(
                present_queue.device.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
                self.window.get_hwnd() as *mut _,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                swap_chain.as_mut() as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            error!("error on swapchain creation {:x}", hr);
        }

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
                    (*present_queue.device.as_mut_ptr()).CreateTexture2D(&desc, ptr::null(), &mut raw)
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

        SwapChain11 {
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

impl<'a> core::Surface<device_dx12::Backend> for Surface12 {
    type SwapChain = SwapChain12;

    fn supports_queue(&self, queue_family: &device_dx12::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> SwapChain12
        where Q: AsRef<device_dx12::CommandQueue>
    {
        use core::handle::Producer;
        let mut swap_chain = ComPtr::<winapi::IDXGISwapChain1>::new(ptr::null_mut());
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
                present_queue.as_ref().raw.as_mut_ptr() as *mut _ as *mut winapi::IUnknown,
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

        let mut swap_chain = ComPtr::<winapi::IDXGISwapChain3>::new(swap_chain.as_mut_ptr() as *mut winapi::IDXGISwapChain3);

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

        SwapChain12 {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            images: backbuffers,
        }
    }
}

pub struct SwapChain11 {
    swap_chain: ComPtr<winapi::IDXGISwapChain1>,
    images: [core::Backbuffer<device_dx11::Backend>; 1],
}

impl core::SwapChain<device_dx11::Backend> for SwapChain11 {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_dx11::Backend>] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_dx11::Resources>) -> core::Frame {
        // TODO: sync
        core::Frame::new(0)
    }

    fn present<Q>(&mut self, _present_queue: &mut Q)
        where Q: AsMut<device_dx11::CommandQueue>
    {
        unsafe { self.swap_chain.Present(1, 0); }
    }
}

pub struct SwapChain12 {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
    images: Vec<core::Backbuffer<device_dx12::Backend>>,
}

impl core::SwapChain<device_dx12::Backend> for SwapChain12 {
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

    fn present<Q>(&mut self, _present_queue: &mut Q)
        where Q: AsMut<device_dx12::CommandQueue>
    {
        unsafe { self.inner.Present(1, 0); }
    }
}

pub struct Window<'a>(pub &'a winit::Window);

impl<'a> core::WindowExt<device_dx11::Backend> for Window<'a> {
    type Surface = Surface11<'a>;
    type Adapter = device_dx11::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface11<'a>, Vec<device_dx11::Adapter>) {
        let mut instance = device_dx11::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            Surface11 {
                factory: instance.0,
                window: self.0,
                manager: h::Manager::new()
            }
        };

        (surface, adapters)
    }
}

impl<'a> core::WindowExt<device_dx12::Backend> for Window<'a> {
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