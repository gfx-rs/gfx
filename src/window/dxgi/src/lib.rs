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
extern crate kernel32;
extern crate user32;
extern crate winapi;
extern crate gfx_core;
extern crate gfx_device_dx11;

mod window;

use std::mem;
use gfx_device_dx11::{Device, Factory, Resources};


pub struct Window<Cf> {
    hwnd: winapi::HWND,
    swap_chain: *mut winapi::IDXGISwapChain,
    pub driver_type: winapi::D3D_DRIVER_TYPE,
    pub color_target: gfx_core::handle::RenderTargetView<Resources, Cf>,
}

impl<Cf> Window<Cf> {
    pub fn is_accelerated(&self) -> bool {
        self.driver_type == winapi::D3D_DRIVER_TYPE_HARDWARE
    }

    pub fn swap_buffers(&self, wait: u8) {
        unsafe{ (*self.swap_chain).Present(wait as winapi::UINT, 0) };
    }

    pub fn dispatch(&self) -> bool {unsafe {
        let mut msg: winapi::MSG = mem::uninitialized();
        while user32::PeekMessageW(&mut msg, self.hwnd, 0, 0, winapi::PM_REMOVE) == winapi::TRUE {
            if (msg.message & 0xFFFF) == winapi::WM_QUIT {
                return false
            }
            user32::TranslateMessage(&msg);
            user32::DispatchMessageW(&msg);
        }
        true
    }}
}

#[derive(Copy, Clone, Debug)]
pub struct InitError;

/// Initialize with a given size.
pub fn init<Cf: gfx_core::format::RenderFormat = gfx_core::format::Rgba8>
           (title: &str, requested_width: winapi::INT, requested_height: winapi::INT)
           -> Result<(Window<Cf>, Device, Factory), InitError> {
    let hwnd = window::create(title, requested_width, requested_height).unwrap();
    let (width, height) = window::show(hwnd).unwrap();

    let driver_types = [
        winapi::D3D_DRIVER_TYPE_HARDWARE,
        winapi::D3D_DRIVER_TYPE_WARP,
        winapi::D3D_DRIVER_TYPE_REFERENCE,
    ];

    let swap_desc = winapi::DXGI_SWAP_CHAIN_DESC {
        BufferDesc: winapi::DXGI_MODE_DESC {
            Width: width as winapi::UINT,
            Height: height as winapi::UINT,
            Format: winapi::DXGI_FORMAT_R8G8B8A8_UNORM,
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
        OutputWindow: hwnd,
        Windowed: winapi::TRUE,
        SwapEffect: winapi::DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };

    info!("Creating swap chain of size {}x{}", width, height);
    for dt in driver_types.iter() {
        match gfx_device_dx11::create(*dt, &swap_desc) {
            Ok((device, factory, chain, color)) => {
                use gfx_core::factory::Phantom;
                info!("Success with driver {:?}", *dt);
                let win = Window {
                    hwnd: hwnd,
                    swap_chain: chain,
                    driver_type: *dt,
                    color_target: gfx_core::handle::RenderTargetView::new(color),
                };
                return Ok((win, device, factory))
            },
            Err(hres) => {
                info!("Failure with driver {:?}: code {:x}", *dt, hres);
            },
        }
    }
    Err(InitError)
}
