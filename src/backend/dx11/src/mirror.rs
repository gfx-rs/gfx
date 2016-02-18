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

pub fn populate_info(_info: &mut shade::ProgramInfo, _stage: shade::Stage, _reflection: ()) {
    /*TODO: blocked by D3DReflect
    use std::mem;
    let usage = stage.into();
    let shader_desc = unsafe {
        let mut desc = mem::zeroed();
        reflection->GetDesc(&mut desc);
        desc
    };
    for i in 0 .. desc.ConstantBuffers {
        let cb = reflection->GetConstantBufferByIndex(i);
        let desc = unsafe {
            let mut desc = mem::zeroed();
            cb->GetDesc(&mut desc);
            desc
        };
        let var = shade::ConstantBufferVar {
            name: desc.Name,
            slot: i,
            size: desc.Size,
            usage: usage,
        };
        //TODO: search for the existing one
        info.constant_buffers.push(var);
    }
    */
}
