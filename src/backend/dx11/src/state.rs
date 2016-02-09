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

use std::ptr;
use winapi::*;
use gfx_core::state;

pub fn make_rasterizer(device: *mut ID3D11Device, rast: &state::Rasterizer, use_scissor: bool)
                       -> *const ID3D11RasterizerState {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: match rast.method {
            state::RasterMethod::Point => {
                error!("Point rasterization is not supported");
                D3D11_FILL_WIREFRAME
            },
            state::RasterMethod::Line(_) => D3D11_FILL_WIREFRAME,
            state::RasterMethod::Fill(_) => D3D11_FILL_SOLID,
        },
        CullMode: match rast.method.get_cull_face() {
            state::CullFace::Nothing => D3D11_CULL_NONE,
            state::CullFace::Front => D3D11_CULL_FRONT,
            state::CullFace::Back => D3D11_CULL_BACK,
        },
        FrontCounterClockwise: match rast.front_face {
            state::FrontFace::Clockwise => FALSE,
            state::FrontFace::CounterClockwise => TRUE,
        },
        DepthBias: match rast.offset {
            Some(ref o) => o.1 as INT,
            None => 0,
        },
        DepthBiasClamp: 16.0,
        SlopeScaledDepthBias: match rast.offset {
            Some(ref o) => o.0 as FLOAT,
            None => 0.0,
        },
        DepthClipEnable: TRUE,
        ScissorEnable: if use_scissor {TRUE} else {FALSE},
        MultisampleEnable: match rast.samples {
            Some(_) => TRUE,
            None => FALSE,
        },
        AntialiasedLineEnable: FALSE,
    };
    let mut handle = ptr::null_mut();
    let hr = unsafe {
        (*device).CreateRasterizerState(&desc, &mut handle)
    };
    if !SUCCEEDED(hr) {
        error!("Failed to create rasterizer state {:?}", rast);
    }
    handle as *const _
}
