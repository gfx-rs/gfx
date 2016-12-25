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

use winit::os::windows::WindowExt;
use core::format;
use core::memory::Typed;
use core::texture::Size;
use device_dx11::{Device, Factory, Resources};


pub struct Window {
    inner: winit::Window,
    swap_chain: *mut winapi::IDXGISwapChain,
    driver_type: winapi::D3D_DRIVER_TYPE,
    pub size: (Size, Size),
}

impl Window {
    pub fn is_accelerated(&self) -> bool {
        self.driver_type == winapi::D3D_DRIVER_TYPE_HARDWARE
    }

    pub fn swap_buffers(&self, wait: u8) {
        unsafe{ (*self.swap_chain).Present(wait as winapi::UINT, 0) };
    }

    pub fn poll_events(&self) -> winit::PollEventsIterator {
        self.inner.poll_events()
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
pub fn init<Cf>(wb: winit::WindowBuilder)
           -> Result<(Window, Device, Factory, core::handle::RenderTargetView<Resources, Cf>), InitError>
where Cf: format::RenderFormat
{
    init_raw(wb, Cf::get_format())
        .map(|(window, device, factory, color)| (window, device, factory, Typed::new(color)))
}

/// Initialize with a given size. Raw format version.
pub fn init_raw(wb: winit::WindowBuilder, color_format: format::Format)
                -> Result<(Window, Device, Factory, core::handle::RawRenderTargetView<Resources>), InitError> {
    let inner = match wb.build() {
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
        match device_dx11::create(*dt, &swap_desc, color_format) {
            Ok((device, factory, chain, color)) => {
                info!("Success with driver {:?}, shader model {}", *dt, device.get_shader_model());
                let win = Window {
                    inner: inner,
                    swap_chain: chain,
                    driver_type: *dt,
                    size: (width as Size, height as Size),
                };
                return Ok((win, device, factory, color))
            },
            Err(hres) => {
                info!("Failure with driver {:?}: code {:x}", *dt, hres);
            },
        }
    }
    Err(InitError::DriverType)
}
