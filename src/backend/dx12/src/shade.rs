use winapi::um::{d3d12, d3dcompiler, d3d12shader, winnt};
use winapi::shared::winerror::SUCCEEDED;
use winapi::shared::minwindef::UINT;
use wio::com::ComPtr;

use std::{mem, ptr};

pub fn reflect_shader(code: &d3d12::D3D12_SHADER_BYTECODE) -> ComPtr<d3d12shader::ID3D12ShaderReflection> {
    let mut reflection = ptr::null_mut();
    let hr = unsafe {
        d3dcompiler::D3DReflect(
            code.pShaderBytecode,
            code.BytecodeLength,
            &d3d12shader::IID_ID3D12ShaderReflection,
            &mut reflection as *mut *mut _ as *mut *mut _)
    };
    if !SUCCEEDED(hr) {
        panic!("Shader reflection failed with code {:x}", hr);
    }

    unsafe { ComPtr::new(reflection) }
}

#[derive(Debug)]
pub struct InputElemDesc {
    pub semantic_name: winnt::LPCSTR,
    pub semantic_index: UINT,
    pub input_slot: UINT,
}

pub fn reflect_input_elements(
    vertex_reflection: &mut ComPtr<d3d12shader::ID3D12ShaderReflection>
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
