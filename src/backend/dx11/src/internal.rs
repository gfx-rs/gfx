use hal::pso::{Stage};
use hal::command;

use winapi::shared::dxgiformat;
use winapi::shared::winerror;
use winapi::um::d3d11;
use wio::com::ComPtr;

use std::{mem, ptr};

use spirv_cross;
use shader;

#[repr(C)]
struct BufferCopy {
    src: u32,
    dst: u32,
    _padding: [u32; 2]
}

#[repr(C)]
struct ImageCopy {
    src: [u32; 4],
    dst: [u32; 4],
}

#[repr(C)]
struct BufferImageCopy {
    buffer_offset: u32,
    buffer_size: [u32; 2],
    _padding: u32,
    image_offset: [u32; 4],
    image_extent: [u32; 4],
}

#[repr(C)]
struct BufferImageCopyInfo {
    buffer: BufferCopy,
    image: ImageCopy,
    buffer_image: BufferImageCopy,
}

#[derive(Clone)]
pub struct Internal {
    cs_copy_image2d_r32_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16g16_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8g8_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8_buffer: ComPtr<d3d11::ID3D11ComputeShader>,

    cs_copy_buffer_image2d_r32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r16g16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r8g8: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r8: ComPtr<d3d11::ID3D11ComputeShader>,

    copy_info: ComPtr<d3d11::ID3D11Buffer>,
}

fn compile(device: ComPtr<d3d11::ID3D11Device>, entrypoint: &str) -> ComPtr<d3d11::ID3D11ComputeShader> {
    let shader_src = include_bytes!("../shaders/copy.hlsl");
    let bytecode = unsafe {
        ComPtr::from_raw(shader::compile_hlsl_shader(
            Stage::Compute,
            spirv_cross::hlsl::ShaderModel::V5_0,
            entrypoint,
            shader_src
        ).unwrap())
    };

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
}

impl Internal {
    pub fn new(device: ComPtr<d3d11::ID3D11Device>) -> Self {
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

        Internal {
            cs_copy_image2d_r32_buffer:    compile(device.clone(), "cs_copy_image2d_r32_buffer"),
            cs_copy_image2d_r16g16_buffer: compile(device.clone(), "cs_copy_image2d_r16g16_buffer"),
            cs_copy_image2d_r16_buffer:    compile(device.clone(), "cs_copy_image2d_r16_buffer"),
            cs_copy_image2d_r8g8_buffer:   compile(device.clone(), "cs_copy_image2d_r8g8_buffer"),
            cs_copy_image2d_r8_buffer:     compile(device.clone(), "cs_copy_image2d_r8_buffer"),

            cs_copy_buffer_image2d_r32:    compile(device.clone(), "cs_copy_buffer_image2d_r32"),
            cs_copy_buffer_image2d_r16g16: compile(device.clone(), "cs_copy_buffer_image2d_r16g16"),
            cs_copy_buffer_image2d_r16:    compile(device.clone(), "cs_copy_buffer_image2d_r16"),
            cs_copy_buffer_image2d_r8g8:   compile(device.clone(), "cs_copy_buffer_image2d_r8g8"),
            cs_copy_buffer_image2d_r8:     compile(device.clone(), "cs_copy_buffer_image2d_r8"),
            copy_info
        }
    }

    fn update_buffer(&mut self, context: ComPtr<d3d11::ID3D11DeviceContext>, info: command::BufferCopy) {
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

        unsafe { ptr::copy(&BufferImageCopyInfo {
            buffer: BufferCopy {
                src: info.src as _,
                dst: info.dst as _,
                _padding: [0u32; 2]
            },
            .. mem::zeroed()
        }, mem::transmute(mapped.pData), 1) };

        unsafe {
            context.Unmap(
                self.copy_info.as_raw() as _,
                0,
            );
        }
    }

    fn update_image(&mut self, context: ComPtr<d3d11::ID3D11DeviceContext>, info: command::ImageCopy) {
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

        unsafe { ptr::copy(&BufferImageCopyInfo {
            image: ImageCopy {
                src: [info.src_offset.x as _, info.src_offset.y as _, info.src_offset.z as _, 0],
                dst: [info.dst_offset.x as _, info.dst_offset.y as _, info.dst_offset.z as _, 0],
            },
            .. mem::zeroed()
        }, mem::transmute(mapped.pData), 1) };

        unsafe {
            context.Unmap(
                self.copy_info.as_raw() as _,
                0,
            );
        }
    }

    fn update_buffer_image(&mut self, context: ComPtr<d3d11::ID3D11DeviceContext>, info: command::BufferImageCopy) {
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

        unsafe { ptr::copy(&BufferImageCopyInfo {
            buffer_image: BufferImageCopy {
                buffer_offset: info.buffer_offset as _,
                buffer_size: [info.buffer_width, info.buffer_height],
                _padding: 0,
                image_offset: [info.image_offset.x as _, info.image_offset.y as _, info.image_offset.z as _, 0],
                image_extent: [info.image_extent.width, info.image_extent.height, info.image_extent.depth, 0],
            },
            .. mem::zeroed()
        }, mem::transmute(mapped.pData), 1) };

        unsafe {
            context.Unmap(
                self.copy_info.as_raw() as _,
                0,
            );
        }
    }

    fn find_image_to_buffer_shader(&self, format: dxgiformat::DXGI_FORMAT) -> Option<(*mut d3d11::ID3D11ComputeShader, u32, u32)> {
        use dxgiformat::*;

        match format {
            DXGI_FORMAT_R32_UINT =>    Some((self.cs_copy_image2d_r32_buffer.as_raw(), 1, 1)),
            DXGI_FORMAT_R16G16_UINT => Some((self.cs_copy_image2d_r16g16_buffer.as_raw(), 1, 1)),
            DXGI_FORMAT_R16_UINT =>    Some((self.cs_copy_image2d_r16_buffer.as_raw(), 2, 1)),
            DXGI_FORMAT_R8G8_UINT =>   Some((self.cs_copy_image2d_r8g8_buffer.as_raw(), 2, 1)),
            DXGI_FORMAT_R8_UINT =>     Some((self.cs_copy_image2d_r8_buffer.as_raw(), 4, 1)),
            _ => None
        }
    }

    fn find_buffer_to_image_shader(&self, format: dxgiformat::DXGI_FORMAT) -> Option<(*mut d3d11::ID3D11ComputeShader, u32, u32)> {
        use dxgiformat::*;

        match format {
            DXGI_FORMAT_R32_UINT =>    Some((self.cs_copy_buffer_image2d_r32.as_raw(), 1, 1)),
            DXGI_FORMAT_R16G16_UINT => Some((self.cs_copy_buffer_image2d_r16g16.as_raw(), 1, 1)),
            DXGI_FORMAT_R16_UINT =>    Some((self.cs_copy_buffer_image2d_r16.as_raw(), 2, 1)),
            DXGI_FORMAT_R8G8_UINT =>   Some((self.cs_copy_buffer_image2d_r8g8.as_raw(), 2, 1)),
            DXGI_FORMAT_R8_UINT =>     Some((self.cs_copy_buffer_image2d_r8.as_raw(), 4, 1)),
            _ => None
        }
    }

    pub fn copy_image_2d_buffer(&mut self,
                context: ComPtr<d3d11::ID3D11DeviceContext>,
                image: ComPtr<d3d11::ID3D11ShaderResourceView>,
                image_format: dxgiformat::DXGI_FORMAT,
                buffer: *mut d3d11::ID3D11UnorderedAccessView,
                info: command::BufferImageCopy) {
        self.update_buffer_image(context.clone(), info.clone());
        let (shader, stride_x, stride_y) = self.find_image_to_buffer_shader(image_format).unwrap();

        unsafe {
            context.CSSetShader(shader, ptr::null_mut(), 0);
            context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());
            context.CSSetShaderResources(0, 1, &image.as_raw());
            context.CSSetUnorderedAccessViews(0, 1, &buffer, ptr::null_mut());

            context.Dispatch(
                info.image_extent.width / stride_x,
                info.image_extent.height / stride_y,
                1
            );

            // unbind external resources
            context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
            context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
        }
    }

    pub fn copy_buffer_image_2d(&mut self,
                context: ComPtr<d3d11::ID3D11DeviceContext>,
                buffer: *mut d3d11::ID3D11ShaderResourceView,
                image: ComPtr<d3d11::ID3D11UnorderedAccessView>,
                image_format: dxgiformat::DXGI_FORMAT,
                info: command::BufferImageCopy) {
        self.update_buffer_image(context.clone(), info.clone());
        let (shader, stride_x, stride_y) = self.find_buffer_to_image_shader(image_format).unwrap();

        unsafe {
            context.CSSetShader(shader, ptr::null_mut(), 0);
            context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());
            context.CSSetShaderResources(0, 1, &buffer);
            context.CSSetUnorderedAccessViews(0, 1, &image.as_raw(), ptr::null_mut());

            context.Dispatch(
                info.image_extent.width / stride_x,
                info.image_extent.height / stride_y,
                1
            );

            // unbind external resources
            context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
            context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
        }
    }
}
