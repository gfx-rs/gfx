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

//use std::ptr;
//use d3dcompiler;
//use dxguid;
//use winapi;
use gfx_core::shade;
use Program;


pub fn reflect_shader(_: &[u8]) -> () {}
/*
pub fn reflect_shader(code: &[u8]) -> *const winapi::ID3D11ShaderReflection {
    let mut reflection = ptr::null_mut();
    let hr = unsafe {
        d3dcompiler::D3DReflect(code.as_ptr() as *const winapi::VOID,
            code.len() as winapi::SIZE_T, dxguid::IID_ID3D11ShaderReflection, &mut reflection)
    };
    if !winapi::SUCCEEDED(hr) {
        error!("Shader reflection failed with code {:x}", hr);
    }
    reflection
}*/


pub fn reflect_program(_prog: &Program) -> shade::ProgramInfo {
    let info = shade::ProgramInfo {
        vertex_attributes: Vec::new(),
        globals: Vec::new(),
        constant_buffers: Vec::new(),
        textures: Vec::new(),
        unordereds: Vec::new(),
        samplers: Vec::new(),
        outputs: Vec::new(),
        knows_outputs: true,
    };
    info
}
