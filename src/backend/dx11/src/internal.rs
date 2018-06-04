use hal::pso::{Stage};
use hal::command;

use winapi::shared::winerror;
use winapi::um::d3d11;
use wio::com::ComPtr;

use std::{mem, ptr};

use spirv_cross;
use shader;

#[repr(C)]
struct BufferImageCopyInfo {
    data: [u32; 4],
}

#[derive(Clone)]
pub struct BufferImageCopy {
    cs: ComPtr<d3d11::ID3D11ComputeShader>,
    copy_info: ComPtr<d3d11::ID3D11Buffer>,
}

impl BufferImageCopy {
    pub fn new(device: ComPtr<d3d11::ID3D11Device>) -> Self {
        let cs = {
            let shader_src = include_bytes!("../shaders/copy.hlsl");
            let bytecode = unsafe { ComPtr::from_raw(shader::compile_hlsl_shader(Stage::Compute, spirv_cross::hlsl::ShaderModel::V5_0, "cs_copy_buffer_image_2d", shader_src).unwrap()) };
            let mut shader = ptr::null_mut();
            let hr = unsafe {
                device.CreateComputeShader(
                    bytecode.GetBufferPointer(),
                    bytecode.GetBufferSize(),
                    ptr::null_mut(),
                    &mut shader as *mut *mut _ as *mut *mut _
                )
            };
            assert_eq!(true, winerror::SUCCEEDED(hr));

            unsafe { ComPtr::from_raw(shader) }
        };

        let copy_info = {
            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: mem::size_of::<BufferImageCopyInfo>() as _,
                Usage: d3d11::D3D11_USAGE_DYNAMIC,
                BindFlags: d3d11::D3D11_BIND_CONSTANT_BUFFER,
                CPUAccessFlags: d3d11::D3D11_CPU_ACCESS_WRITE,
                MiscFlags: 0,
                StructureByteStride: 0,
            };

            let mut buffer = ptr::null_mut();
            let hr = unsafe {
                device.CreateBuffer(
                    &desc,
                    ptr::null_mut(),
                    &mut buffer as *mut *mut _ as *mut *mut _
                )
            };
            assert_eq!(true, winerror::SUCCEEDED(hr));

            unsafe { ComPtr::from_raw(buffer) }
        };

        BufferImageCopy {
            cs,
            copy_info
        }
    }

    fn update_buffer(&mut self, context: ComPtr<d3d11::ID3D11DeviceContext>, info: command::BufferImageCopy) {
        let mut mapped = unsafe { mem::zeroed::<d3d11::D3D11_MAPPED_SUBRESOURCE>() };
        let hr = unsafe {
            context.Map(
                self.copy_info.as_raw() as _,
                0,
                d3d11::D3D11_MAP_WRITE_DISCARD,
                0,
                &mut mapped
            )
        };

        let info_struct = BufferImageCopyInfo {
            data: [
                info.buffer_offset as u32,
                info.buffer_width as u32,
                info.image_offset.x as u32,
                info.image_offset.y as u32,
            ],
        };

        unsafe { ptr::copy(&info_struct, mem::transmute(mapped.pData), 1) };

        unsafe {
            context.Unmap(
                self.copy_info.as_raw() as _,
                0,
            );
        }
    }

    pub fn copy_2d(&mut self,
                context: ComPtr<d3d11::ID3D11DeviceContext>,
                buffer: ComPtr<d3d11::ID3D11ShaderResourceView>,
                image: ComPtr<d3d11::ID3D11UnorderedAccessView>,
                info: command::BufferImageCopy) {
        self.update_buffer(context.clone(), info.clone());

        unsafe {
            context.CSSetShader(self.cs.as_raw(), ptr::null_mut(), 0);
            context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());
            context.CSSetShaderResources(0, 1, &buffer.as_raw());
            context.CSSetUnorderedAccessViews(0, 1, &image.as_raw(), ptr::null_mut());

            context.Dispatch(
                info.image_extent.width,
                info.image_extent.height,
                1
            );

            // unbind external resources
            context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
            context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
        }
    }
}
