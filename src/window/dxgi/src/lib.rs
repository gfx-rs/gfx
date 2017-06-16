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
extern crate comptr;

use std::ptr;
use std::os::raw::c_void;
use winit::os::windows::WindowExt;
use core::{format, handle as h, factory as f, memory, texture as tex};
use core::texture::Size;
use device_dx11::{data, native, Factory, Resources, Texture};
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

pub struct Surface<'a> {
    factory: ComPtr<winapi::IDXGIFactory2>,
    window: &'a winit::Window,
    manager: h::Manager<Resources>,
}

impl<'a> core::Surface<device_dx11::Backend> for Surface<'a> {
    type SwapChain = SwapChain;

    fn supports_queue(&self, queue_family: &device_dx11::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> SwapChain
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
            Format: data::map_format(config.color_format, true).unwrap(), // TODO: error handling
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
            let raw_tex = Texture::new(native::Texture::D2(back_buffer));
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

                let (usage, cpu_access) = data::map_usage(info.usage, info.bind);

                let desc = winapi::D3D11_TEXTURE2D_DESC {
                    Width: dim.0 as winapi::UINT,
                    Height: dim.1 as winapi::UINT,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: data::map_surface(info.format).unwrap(),
                    SampleDesc: data::map_anti_alias(dim.3),
                    Usage: usage,
                    BindFlags: data::map_bind(info.bind).0,
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
                    Texture::new(native::Texture::D2(raw)),
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

        SwapChain {
            swap_chain,
            images: [backbuffer],
        }
    }
}

pub struct SwapChain {
    swap_chain: ComPtr<winapi::IDXGISwapChain1>,
    images: [core::Backbuffer<device_dx11::Backend>; 1],
}

impl core::SwapChain<device_dx11::Backend> for SwapChain {
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

pub struct Window<'a>(pub &'a winit::Window);

impl<'a> core::WindowExt<device_dx11::Backend> for Window<'a> {
    type Surface = Surface<'a>;
    type Adapter = device_dx11::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface<'a>, Vec<device_dx11::Adapter>) {
        let mut instance = device_dx11::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            Surface {
                factory: instance.0,
                window: self.0,
                manager: h::Manager::new()
            }
        };

        (surface, adapters)
    }
}
