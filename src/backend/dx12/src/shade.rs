use wio::com::ComPtr;
use d3dcompiler;
use dxguid;
use winapi;

use std::{mem, ptr};

pub fn reflect_shader(code: &winapi::D3D12_SHADER_BYTECODE) -> ComPtr<winapi::ID3D12ShaderReflection> {
    let mut reflection = ptr::null_mut();
    let hr = unsafe {
        d3dcompiler::D3DReflect(
            code.pShaderBytecode,
            code.BytecodeLength,
            &dxguid::IID_ID3D12ShaderReflection,
            &mut reflection as *mut *mut _ as *mut *mut winapi::c_void)
    };
    if !winapi::SUCCEEDED(hr) {
        panic!("Shader reflection failed with code {:x}", hr);
    }

    unsafe { ComPtr::new(reflection) }
}

#[derive(Debug)]
pub struct InputElemDesc {
    pub semantic_name: winapi::LPCSTR,
    pub semantic_index: winapi::UINT,
    pub input_slot: winapi::UINT,
}

pub fn reflect_input_elements(
    vertex_reflection: &mut ComPtr<winapi::ID3D12ShaderReflection>
) -> Vec<InputElemDesc> {
    let shader_desc = unsafe {
        let mut desc = mem::zeroed();
        vertex_reflection.GetDesc(&mut desc);
        desc
    };

    (0 .. shader_desc.InputParameters).map(|i| {
        let input_desc = unsafe {
            let mut desc = mem::zeroed();
            vertex_reflection.GetInputParameterDesc(i, &mut desc);
            desc
        };

        InputElemDesc {
            semantic_name: input_desc.SemanticName,
            semantic_index: input_desc.SemanticIndex,
            input_slot: input_desc.Register,
        }
    }).collect()
}
