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

//#[deny(missing_docs)]

extern crate gfx_core;
extern crate d3d11;
extern crate winapi;

use std::os::raw::c_void;
use std::ptr;

pub mod native {
    use winapi::*;

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Rtv(pub *mut ID3D11RenderTargetView);
    unsafe impl Send for Rtv {}

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    pub struct Texture(pub *mut ID3D11Texture2D);
    unsafe impl Send for Texture {}
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl gfx_core::Resources for Resources {
    type Buffer              = ();
    type Shader              = ();
    type Program             = ();
    type PipelineStateObject = ();
    type Texture             = native::Texture;
    type RenderTargetView    = native::Rtv;
    type DepthStencilView    = ();
    type ShaderResourceView  = ();
    type UnorderedAccessView = ();
    type Sampler             = ();
    type Fence               = ();
}

pub struct Device {
    device: *mut winapi::ID3D11Device,
    context: *mut winapi::ID3D11DeviceContext,
    feature_level: winapi::D3D_FEATURE_LEVEL,
    manager: gfx_core::handle::Manager<Resources>,
}

static FEATURE_LEVELS: [winapi::D3D_FEATURE_LEVEL; 3] = [
    winapi::D3D_FEATURE_LEVEL_11_0,
    winapi::D3D_FEATURE_LEVEL_10_1,
    winapi::D3D_FEATURE_LEVEL_10_0,
];

impl Device {
    pub fn create(driver_type: winapi::D3D_DRIVER_TYPE, desc: &winapi::DXGI_SWAP_CHAIN_DESC)
                  -> Result<(Device, *mut winapi::IDXGISwapChain, gfx_core::handle::RawRenderTargetView<Resources>), winapi::HRESULT> {
        use gfx_core::handle::Producer;
        use gfx_core::tex;

        let mut swap_chain = ptr::null_mut();
        let create_flags = 0;
        let mut ret = Device {
            device: ptr::null_mut(),
            context: ptr::null_mut(),
            feature_level: winapi::D3D_FEATURE_LEVEL_10_0,
            manager: gfx_core::handle::Manager::new(),
        };
        let hr = unsafe {
            d3d11::D3D11CreateDeviceAndSwapChain(ptr::null_mut(), driver_type, ptr::null_mut(), create_flags,
                &FEATURE_LEVELS[0], FEATURE_LEVELS.len() as winapi::UINT, winapi::D3D11_SDK_VERSION, desc,
                &mut swap_chain, &mut ret.device, &mut ret.feature_level, &mut ret.context)
        };
        if !winapi::SUCCEEDED(hr) {
            return Err(hr)
        }

        let mut back_buffer: *mut winapi::ID3D11Texture2D = ptr::null_mut();
        let mut raw_color: *mut winapi::ID3D11RenderTargetView = ptr::null_mut();
        unsafe {
            (*swap_chain).GetBuffer(0, &winapi::IID_ID3D11Texture2D, &mut back_buffer
                as *mut *mut winapi::ID3D11Texture2D as *mut *mut c_void);
            (*ret.device).CreateRenderTargetView(back_buffer as *mut winapi::ID3D11Resource,
                ptr::null_mut(), &mut raw_color);
        }

        let color_tex = ret.manager.make_texture(native::Texture(back_buffer), gfx_core::tex::Descriptor {
            kind: tex::Kind::D2(desc.BufferDesc.Width as tex::Size, desc.BufferDesc.Height as tex::Size, tex::AaMode::Single),
            levels: 1,
            format: gfx_core::format::SurfaceType::R8_G8_B8_A8,
            bind: gfx_core::factory::RENDER_TARGET,
        });
        let color_target = ret.manager.make_rtv(native::Rtv(raw_color), &color_tex,
            color_tex.get_info().kind.get_dimensions());

        Ok((ret, swap_chain, color_target))
    }
}
