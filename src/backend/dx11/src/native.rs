// Copyright 2017 The Gfx-rs Developers.
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

use winapi::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Buffer(pub *mut ID3D11Buffer);
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Texture {
    D1(*mut ID3D11Texture1D),
    D2(*mut ID3D11Texture2D),
    D3(*mut ID3D11Texture3D),
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Rtv(pub *mut ID3D11RenderTargetView);
unsafe impl Send for Rtv {}
unsafe impl Sync for Rtv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Dsv(pub *mut ID3D11DepthStencilView);
unsafe impl Send for Dsv {}
unsafe impl Sync for Dsv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Srv(pub *mut ID3D11ShaderResourceView);
unsafe impl Send for Srv {}
unsafe impl Sync for Srv {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Sampler(pub *mut ID3D11SamplerState);
unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}
