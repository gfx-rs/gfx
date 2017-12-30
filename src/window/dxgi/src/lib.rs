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
extern crate winapi;
extern crate winit;
extern crate gfx_core as core;
extern crate gfx_device_dx11 as device_dx11;

use std::ptr;
use winit::os::windows::WindowExt;
use winapi::shared::{dxgi, dxgiformat, dxgitype, winerror};
use winapi::um::{d3d11, d3dcommon};

use core::{format, handle as h, factory as f, memory, texture as tex};
use core::texture::Size;
use device_dx11::{Device, Factory, Resources};


pub struct Window {
    pub inner: winit::Window,
    swap_chain: *mut dxgi::IDXGISwapChain,
    driver_type: d3dcommon::D3D_DRIVER_TYPE,
    color_format: format::Format,
    pub size: (Size, Size),
}

impl Window {
    pub fn is_accelerated(&self) -> bool {
        self.driver_type == d3dcommon::D3D_DRIVER_TYPE_HARDWARE
    }

    pub fn swap_buffers(&self, wait: u8) {
        match unsafe {(*self.swap_chain).Present(wait as _, 0)} {
            winerror::S_OK | winerror::DXGI_STATUS_OCCLUDED => {}
            hr => panic!("Present Error: {:X}", hr)
        }
    }

    fn make_back_buffer(&self, factory: &mut Factory) -> h::RawRenderTargetView<Resources> {
        let mut back_buffer: *mut d3d11::ID3D11Texture2D = ptr::null_mut();
        assert_eq!(winerror::S_OK, unsafe {
            (*self.swap_chain).GetBuffer(0, &d3d11::IID_ID3D11Texture2D,
                &mut back_buffer as *mut *mut d3d11::ID3D11Texture2D as *mut *mut _)
        });

        let info = tex::Info {
            kind: tex::Kind::D2(self.size.0, self.size.1, tex::AaMode::Single),
            levels: 1,
            format: self.color_format.0,
            bind: memory::Bind::RENDER_TARGET | memory::Bind::TRANSFER_SRC,
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
                             -> Result<h::RenderTargetView<Resources, Cf>, winerror::HRESULT>
    where Cf: format::RenderFormat
    {
        let result = unsafe {
            (*self.swap_chain).ResizeBuffers(0,
                width as _, height as _,
                dxgiformat::DXGI_FORMAT_UNKNOWN, 0)
        };
        if result == winerror::S_OK {
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
           -> Result<(Window, Device, Factory, h::RenderTargetView<Resources, Cf>), InitError>
where Cf: format::RenderFormat
{
    init_raw(wb, events_loop, Cf::get_format())
        .map(|(window, device, factory, color)| (window, device, factory, memory::Typed::new(color)))
}

/// Initialize with a given size. Raw format version.
pub fn init_raw(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop, color_format: format::Format)
                -> Result<(Window, Device, Factory, h::RawRenderTargetView<Resources>), InitError>
{
    let inner = match wb.build(events_loop) {
        Ok(w) => w,
        Err(_) => return Err(InitError::Window),
    };
    init_existing_raw(inner, color_format)
}

pub fn init_existing_raw(inner: winit::Window, color_format: format::Format)
                         -> Result<(Window, Device, Factory, h::RawRenderTargetView<Resources>), InitError>
{
    let (width, height) = inner.get_inner_size().unwrap();

    let driver_types = [
        d3dcommon::D3D_DRIVER_TYPE_HARDWARE,
        d3dcommon::D3D_DRIVER_TYPE_WARP,
        d3dcommon::D3D_DRIVER_TYPE_REFERENCE,
    ];

    let swap_desc = dxgi::DXGI_SWAP_CHAIN_DESC {
        BufferDesc: dxgitype::DXGI_MODE_DESC {
            Width: width as _,
            Height: height as _,
            Format: match device_dx11::map_format(color_format, true) {
                Some(fm) => fm,
                None => return Err(InitError::Format(color_format)),
            },
            RefreshRate: dxgitype::DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            ScanlineOrdering: dxgitype::DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: dxgitype::DXGI_MODE_SCALING_UNSPECIFIED,
        },
        SampleDesc: dxgitype::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 1,
        OutputWindow: inner.get_hwnd() as _,
        Windowed: true as _,
        SwapEffect: dxgi::DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };

    info!("Creating swap chain of size {}x{}", width, height);
    for dt in driver_types.iter() {
        match device_dx11::create(*dt, &swap_desc) {
            Ok((device, mut factory, chain)) => {
                info!("Success with driver {:?}, shader model {}", *dt, device.get_shader_model());
                let win = Window {
                    inner: inner,
                    swap_chain: chain,
                    driver_type: *dt,
                    color_format: color_format,
                    size: (width as Size, height as Size),
                };
                let color = win.make_back_buffer(&mut factory);
                return Ok((win, device, factory, color))
            },
            Err(hres) => {
                info!("Failure with driver {:?}: code {:x}", *dt, hres);
            },
        }
    }
    Err(InitError::DriverType)
}

pub trait DeviceExt: core::Device {
    fn clear_state(&self);
}

impl DeviceExt for device_dx11::Deferred {
     fn clear_state(&self) {
         self.clear_state();
     }
}

impl DeviceExt for Device {
    fn clear_state(&self) {
        self.clear_state();
    }
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf, D>(window: &mut Window, factory: &mut Factory, device: &mut D, width: u16, height: u16)
            -> Result<h::RenderTargetView<Resources, Cf>, f::TargetViewError>
where Cf: format::RenderFormat, D: DeviceExt
{
    
    factory.cleanup();
    device.clear_state();
    device.cleanup();

    window.resize_swap_chain::<Cf>(factory, width, height)
        .map_err(|hr| {
            error!("Resize failed with code {:X}", hr);
            f::TargetViewError::NotDetached
        }
    )    
}
