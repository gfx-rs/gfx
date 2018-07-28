use hal::pso::{Stage};
use hal::{image, command};

use winapi::shared::dxgiformat;
use winapi::shared::winerror;
use winapi::um::d3d11;
use winapi::um::d3dcommon;
use wio::com::ComPtr;

use std::{mem, ptr};
use std::borrow::Borrow;

use spirv_cross;
use shader;

use {Buffer, Image};

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

#[repr(C)]
struct BlitInfo {
    offset: [f32; 2],
    extent: [f32; 2],
    z: f32,
    level: f32,
}

#[derive(Clone)]
pub struct Internal {
    vs_blit_2d: ComPtr<d3d11::ID3D11VertexShader>,

    sampler_nearest: ComPtr<d3d11::ID3D11SamplerState>,
    sampler_linear: ComPtr<d3d11::ID3D11SamplerState>,

    // blit permutations
    ps_blit_2d_uint: ComPtr<d3d11::ID3D11PixelShader>,
    ps_blit_2d_int: ComPtr<d3d11::ID3D11PixelShader>,
    ps_blit_2d_float: ComPtr<d3d11::ID3D11PixelShader>,

    // Image<->Image not covered by `CopySubresourceRegion`
    cs_copy_image2d_r8g8_image2d_r16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16_image2d_r8g8: ComPtr<d3d11::ID3D11ComputeShader>,

    cs_copy_image2d_r8g8b8a8_image2d_r32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8g8b8a8_image2d_r16g16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16g16_image2d_r32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16g16_image2d_r8g8b8a8: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r32_image2d_r16g16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r32_image2d_r8g8b8a8: ComPtr<d3d11::ID3D11ComputeShader>,

    // Buffer<->Image
    cs_copy_image2d_r32g32b32a32_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r32g32_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16g16b16a16_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r32_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16g16_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8g8b8a8_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r16_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8g8_buffer: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_image2d_r8_buffer: ComPtr<d3d11::ID3D11ComputeShader>,

    cs_copy_buffer_image2d_r32g32b32a32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r32g32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r16g16b16a16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r32: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r16g16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r8g8b8a8: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r16: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r8g8: ComPtr<d3d11::ID3D11ComputeShader>,
    cs_copy_buffer_image2d_r8: ComPtr<d3d11::ID3D11ComputeShader>,

    copy_info: ComPtr<d3d11::ID3D11Buffer>,

    pub working_buffer: ComPtr<d3d11::ID3D11Buffer>,
    pub working_buffer_size: u64,
}

fn compile_blob(src: &[u8], entrypoint: &str, stage: Stage) -> ComPtr<d3dcommon::ID3DBlob> {
    unsafe {
        ComPtr::from_raw(shader::compile_hlsl_shader(
            stage,
            spirv_cross::hlsl::ShaderModel::V5_0,
            entrypoint,
            src
        ).unwrap())
    }
}

fn compile_vs(device: &ComPtr<d3d11::ID3D11Device>, src: &[u8], entrypoint: &str) -> ComPtr<d3d11::ID3D11VertexShader> {
    let bytecode = compile_blob(src, entrypoint, Stage::Vertex);
    let mut shader = ptr::null_mut();
    let hr = unsafe {
        device.CreateVertexShader(
            bytecode.GetBufferPointer(),
            bytecode.GetBufferSize(),
            ptr::null_mut(),
            &mut shader as *mut *mut _ as *mut *mut _
        )
    };
    assert_eq!(true, winerror::SUCCEEDED(hr));

    unsafe { ComPtr::from_raw(shader) }
}

fn compile_ps(device: &ComPtr<d3d11::ID3D11Device>, src: &[u8], entrypoint: &str) -> ComPtr<d3d11::ID3D11PixelShader> {
    let bytecode = compile_blob(src, entrypoint, Stage::Fragment);
    let mut shader = ptr::null_mut();
    let hr = unsafe {
        device.CreatePixelShader(
            bytecode.GetBufferPointer(),
            bytecode.GetBufferSize(),
            ptr::null_mut(),
            &mut shader as *mut *mut _ as *mut *mut _
        )
    };
    assert_eq!(true, winerror::SUCCEEDED(hr));

    unsafe { ComPtr::from_raw(shader) }
}

fn compile_cs(device: &ComPtr<d3d11::ID3D11Device>, src: &[u8], entrypoint: &str) -> ComPtr<d3d11::ID3D11ComputeShader> {
    let bytecode = compile_blob(src, entrypoint, Stage::Compute);
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
    pub fn new(device: &ComPtr<d3d11::ID3D11Device>) -> Self {
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

        let (sampler_nearest, sampler_linear) = {
            let mut desc = d3d11::D3D11_SAMPLER_DESC {
                Filter: d3d11::D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: d3d11::D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: d3d11::D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: d3d11::D3D11_TEXTURE_ADDRESS_CLAMP,
                MipLODBias: 0f32,
                MaxAnisotropy: 0,
                ComparisonFunc: 0,
                BorderColor: [0f32; 4],
                MinLOD: 0f32,
                MaxLOD: d3d11::D3D11_FLOAT32_MAX,
            };

            let mut nearest = ptr::null_mut();
            let mut linear = ptr::null_mut();

            assert_eq!(winerror::S_OK, unsafe {
                device.CreateSamplerState(
                    &desc,
                    &mut nearest as *mut *mut _ as *mut *mut _
                )
            });

            desc.Filter = d3d11::D3D11_FILTER_MIN_MAG_MIP_LINEAR;

            assert_eq!(winerror::S_OK, unsafe {
                device.CreateSamplerState(
                    &desc,
                    &mut linear as *mut *mut _ as *mut *mut _
                )
            });

            unsafe { (ComPtr::from_raw(nearest), ComPtr::from_raw(linear)) }
        };

        let (working_buffer, working_buffer_size) = {
            let working_buffer_size = 1 << 16;

            let desc = d3d11::D3D11_BUFFER_DESC {
                ByteWidth: working_buffer_size,
                Usage: d3d11::D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: d3d11::D3D11_CPU_ACCESS_READ | d3d11::D3D11_CPU_ACCESS_WRITE,
                MiscFlags:0,
                StructureByteStride: 0,

            };
            let mut working_buffer = ptr::null_mut();

            assert_eq!(winerror::S_OK, unsafe {
                device.CreateBuffer(
                    &desc,
                    ptr::null_mut(),
                    &mut working_buffer as *mut *mut _ as *mut *mut _
                )
            });

            (unsafe { ComPtr::from_raw(working_buffer) }, working_buffer_size)
        };

        let copy_shaders = include_bytes!("../shaders/copy.hlsl");
        let blit_shaders = include_bytes!("../shaders/blit.hlsl");

        Internal {
            vs_blit_2d: compile_vs(device, blit_shaders, "vs_blit_2d"),

            sampler_nearest,
            sampler_linear,

            cs_copy_image2d_r8g8_image2d_r16: compile_cs(device, copy_shaders, "cs_copy_image2d_r8g8_image2d_r16"),
            cs_copy_image2d_r16_image2d_r8g8: compile_cs(device, copy_shaders, "cs_copy_image2d_r16_image2d_r8g8"),

            cs_copy_image2d_r8g8b8a8_image2d_r32: compile_cs(device, copy_shaders, "cs_copy_image2d_r8g8b8a8_image2d_r32"),
            cs_copy_image2d_r8g8b8a8_image2d_r16g16: compile_cs(device, copy_shaders, "cs_copy_image2d_r8g8b8a8_image2d_r16g16"),
            cs_copy_image2d_r16g16_image2d_r32: compile_cs(device, copy_shaders, "cs_copy_image2d_r16g16_image2d_r32"),
            cs_copy_image2d_r16g16_image2d_r8g8b8a8: compile_cs(device, copy_shaders, "cs_copy_image2d_r16g16_image2d_r8g8b8a8"),
            cs_copy_image2d_r32_image2d_r16g16: compile_cs(device, copy_shaders, "cs_copy_image2d_r32_image2d_r16g16"),
            cs_copy_image2d_r32_image2d_r8g8b8a8: compile_cs(device, copy_shaders, "cs_copy_image2d_r32_image2d_r8g8b8a8"),

            ps_blit_2d_uint: compile_ps(device, blit_shaders, "ps_blit_2d_uint"),
            ps_blit_2d_int: compile_ps(device, blit_shaders, "ps_blit_2d_int"),
            ps_blit_2d_float: compile_ps(device, blit_shaders, "ps_blit_2d_float"),

            cs_copy_image2d_r32g32b32a32_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r32g32b32a32_buffer"),
            cs_copy_image2d_r32g32_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r32g32_buffer"),
            cs_copy_image2d_r16g16b16a16_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r16g16b16a16_buffer"),
            cs_copy_image2d_r32_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r32_buffer"),
            cs_copy_image2d_r16g16_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r16g16_buffer"),
            cs_copy_image2d_r8g8b8a8_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r8g8b8a8_buffer"),
            cs_copy_image2d_r16_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r16_buffer"),
            cs_copy_image2d_r8g8_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r8g8_buffer"),
            cs_copy_image2d_r8_buffer: compile_cs(device, copy_shaders, "cs_copy_image2d_r8_buffer"),

            cs_copy_buffer_image2d_r32g32b32a32: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r32g32b32a32"),
            cs_copy_buffer_image2d_r32g32: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r32g32"),
            cs_copy_buffer_image2d_r16g16b16a16: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r16g16b16a16"),
            cs_copy_buffer_image2d_r32: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r32"),
            cs_copy_buffer_image2d_r16g16: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r16g16"),
            cs_copy_buffer_image2d_r8g8b8a8: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r8g8b8a8"),
            cs_copy_buffer_image2d_r16: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r16"),
            cs_copy_buffer_image2d_r8g8: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r8g8"),
            cs_copy_buffer_image2d_r8: compile_cs(device, copy_shaders, "cs_copy_buffer_image2d_r8"),

            copy_info,
            working_buffer,
            working_buffer_size: working_buffer_size as _
        }
    }

    fn map(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>) -> *mut u8 {
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

        assert_eq!(winerror::S_OK, hr);

        mapped.pData as _
    }

    fn unmap(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>) {
        unsafe {
            context.Unmap(
                self.copy_info.as_raw() as _,
                0,
            );
        }
    }

    fn update_image(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, info: &command::ImageCopy) {
        unsafe { ptr::copy(&BufferImageCopyInfo {
            image: ImageCopy {
                src: [info.src_offset.x as _, info.src_offset.y as _, info.src_offset.z as _, 0],
                dst: [info.dst_offset.x as _, info.dst_offset.y as _, info.dst_offset.z as _, 0],
            },
            .. mem::zeroed()
        }, self.map(context) as *mut _, 1) };

        self.unmap(context);
    }

    fn update_buffer_image(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, info: &command::BufferImageCopy) {
        unsafe { ptr::copy(&BufferImageCopyInfo {
            buffer_image: BufferImageCopy {
                buffer_offset: info.buffer_offset as _,
                buffer_size: [info.buffer_width, info.buffer_height],
                _padding: 0,
                image_offset: [info.image_offset.x as _, info.image_offset.y as _, info.image_offset.z as _, 0],
                image_extent: [info.image_extent.width, info.image_extent.height, info.image_extent.depth, 0],
            },
            .. mem::zeroed()
        }, self.map(context) as *mut _, 1) };

        self.unmap(context);
    }


    fn update_blit(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, src: &Image, info: &command::ImageBlit) {
        let (sx, dx) = if info.dst_bounds.start.x > info.dst_bounds.end.x {
            (info.src_bounds.end.x, info.src_bounds.start.x - info.src_bounds.end.x)
        } else {
            (info.src_bounds.start.x, info.src_bounds.end.x - info.src_bounds.start.x)
        };
        let (sy, dy) = if info.dst_bounds.start.y > info.dst_bounds.end.y {
            (info.src_bounds.end.y, info.src_bounds.start.y - info.src_bounds.end.y)
        } else {
            (info.src_bounds.start.y, info.src_bounds.end.y - info.src_bounds.start.y)
        };
        let image::Extent { width, height, .. } = src.kind.level_extent(info.src_subresource.level);

        unsafe {
            ptr::copy(
                &BlitInfo {
                    offset: [
                        sx as f32 / width as f32,
                        sy as f32 / height as f32,
                    ],
                    extent: [
                        dx as f32 / width as f32,
                        dy as f32 / height as f32,
                    ],
                    z: 0f32, // TODO
                    level: info.src_subresource.level as _,
                },
                self.map(context) as *mut _, 1
            )
        };

        self.unmap(context);
    }

    fn find_image_copy_shader(&self, src: &Image, dst: &Image) -> Option<*mut d3d11::ID3D11ComputeShader> {
        use dxgiformat::*;

        match (src.typed_raw_format, dst.typed_raw_format) {
            (DXGI_FORMAT_R8G8_UINT, DXGI_FORMAT_R16_UINT) => Some(self.cs_copy_image2d_r8g8_image2d_r16.as_raw()),
            (DXGI_FORMAT_R16_UINT, DXGI_FORMAT_R8G8_UINT) => Some(self.cs_copy_image2d_r16_image2d_r8g8.as_raw()),
            (DXGI_FORMAT_R8G8B8A8_UINT, DXGI_FORMAT_R32_UINT) => Some(self.cs_copy_image2d_r8g8b8a8_image2d_r32.as_raw()),
            (DXGI_FORMAT_R8G8B8A8_UINT, DXGI_FORMAT_R16G16_UINT) => Some(self.cs_copy_image2d_r8g8b8a8_image2d_r16g16.as_raw()),
            (DXGI_FORMAT_R16G16_UINT, DXGI_FORMAT_R32_UINT) => Some(self.cs_copy_image2d_r16g16_image2d_r32.as_raw()),
            (DXGI_FORMAT_R16G16_UINT, DXGI_FORMAT_R8G8B8A8_UINT) => Some(self.cs_copy_image2d_r16g16_image2d_r8g8b8a8.as_raw()),
            (DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R16G16_UINT) => Some(self.cs_copy_image2d_r32_image2d_r16g16.as_raw()),
            (DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R8G8B8A8_UINT) => Some(self.cs_copy_image2d_r32_image2d_r8g8b8a8.as_raw()),
            _ => None
        }
    }

    pub fn copy_image_2d<T>(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, src: &Image, dst: &Image, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        if let Some(shader) = self.find_image_copy_shader(src, dst) {
            // Some formats cant go through default path, since they cant
            // be cast between formats of different component types (eg.
            // Rg16 <-> Rgba8)

            // TODO: subresources
            let srv = src.internal.copy_srv.clone().unwrap().as_raw();

            unsafe {
                context.CSSetShader(shader, ptr::null_mut(), 0);
                context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());
                context.CSSetShaderResources(0, 1, [srv].as_ptr());


                for region in regions.into_iter() {
                    let info = region.borrow();
                    self.update_image(context, &info);

                    let uav = dst.get_uav(info.dst_subresource.level, 0).unwrap().as_raw();
                    context.CSSetUnorderedAccessViews(0, 1, [uav].as_ptr(), ptr::null_mut());

                    context.Dispatch(
                        info.extent.width as u32,
                        info.extent.height as u32,
                        1
                    );
                }

                // unbind external resources
                context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
                context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
            }
        } else {
            // Default copy path
            for region in regions.into_iter() {
                let info = region.borrow();

                // TODO: layer subresources
                unsafe {
                    context.CopySubresourceRegion(

                        dst.internal.raw as _,
                        src.calc_subresource(info.src_subresource.level as _, 0),
                        info.dst_offset.x as _,
                        info.dst_offset.y as _,
                        info.dst_offset.z as _,
                        src.internal.raw as _,
                        dst.calc_subresource(info.dst_subresource.level as _, 0),
                        &d3d11::D3D11_BOX {
                            left: info.src_offset.x as _,
                            top: info.src_offset.y as _,
                            front: info.src_offset.z as _,
                            right: info.extent.width as _,
                            bottom: info.extent.height as _,
                            back: info.extent.depth as _,
                        }
                    );
                }
            }
        }
    }

    fn find_image_to_buffer_shader(&self, format: dxgiformat::DXGI_FORMAT) -> Option<(*mut d3d11::ID3D11ComputeShader, f32, f32)> {
        use dxgiformat::*;

        match format {
            DXGI_FORMAT_R32G32B32A32_UINT => Some((self.cs_copy_image2d_r32g32b32a32_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R32G32_UINT =>       Some((self.cs_copy_image2d_r32g32_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16G16B16A16_UINT => Some((self.cs_copy_image2d_r16g16b16a16_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R32_UINT =>          Some((self.cs_copy_image2d_r32_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16G16_UINT =>       Some((self.cs_copy_image2d_r16g16_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R8G8B8A8_UINT =>     Some((self.cs_copy_image2d_r8g8b8a8_buffer.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16_UINT =>          Some((self.cs_copy_image2d_r16_buffer.as_raw(), 2f32, 1f32)),
            DXGI_FORMAT_R8G8_UINT =>         Some((self.cs_copy_image2d_r8g8_buffer.as_raw(), 2f32, 1f32)),
            DXGI_FORMAT_R8_UINT =>           Some((self.cs_copy_image2d_r8_buffer.as_raw(), 4f32, 1f32)),
            _ => None
        }
    }

    pub fn copy_image_2d_to_buffer<T>(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, src: &Image, dst: &Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        let (shader, scale_x, scale_y) = self.find_image_to_buffer_shader(src.typed_raw_format).unwrap();

        let srv = src.internal.copy_srv.clone().unwrap().as_raw();
        let uav = dst.internal.uav.unwrap();

        unsafe {
            context.CSSetShader(shader, ptr::null_mut(), 0);
            context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());

            context.CSSetShaderResources(0, 1, [srv].as_ptr());
            context.CSSetUnorderedAccessViews(0, 1, [uav].as_ptr(), ptr::null_mut());

            for copy in regions {
                let copy = copy.borrow();
                self.update_buffer_image(context, &copy);

                context.Dispatch(
                    (copy.image_extent.width as f32 / scale_x) as u32,
                    (copy.image_extent.height as f32 / scale_y) as u32,
                    1
                );
            }

            // unbind external resources
            context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
            context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
        }
    }

    fn find_buffer_to_image_shader(&self, format: dxgiformat::DXGI_FORMAT) -> Option<(*mut d3d11::ID3D11ComputeShader, f32, f32)> {
        use dxgiformat::*;

        match format {
            DXGI_FORMAT_R32G32B32A32_UINT => Some((self.cs_copy_buffer_image2d_r32g32b32a32.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R32G32_UINT =>       Some((self.cs_copy_buffer_image2d_r32g32.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16G16B16A16_UINT => Some((self.cs_copy_buffer_image2d_r16g16b16a16.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R32_UINT =>          Some((self.cs_copy_buffer_image2d_r32.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16G16_UINT =>       Some((self.cs_copy_buffer_image2d_r16g16.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R8G8B8A8_UINT =>     Some((self.cs_copy_buffer_image2d_r8g8b8a8.as_raw(), 1f32, 1f32)),
            DXGI_FORMAT_R16_UINT =>          Some((self.cs_copy_buffer_image2d_r16.as_raw(), 2f32, 1f32)),
            DXGI_FORMAT_R8G8_UINT =>         Some((self.cs_copy_buffer_image2d_r8g8.as_raw(), 2f32, 1f32)),
            DXGI_FORMAT_R8_UINT =>           Some((self.cs_copy_buffer_image2d_r8.as_raw(), 4f32, 1f32)),
            _ => None
        }
    }

    pub fn copy_buffer_to_image_2d<T>(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, src: &Buffer, dst: &Image, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        // NOTE: we have two separate paths for Buffer -> Image transfers. we need to special case
        //       uploads to compressed formats through `UpdateSubresource` since we cannot get a
        //       UAV of any compressed format.

        let format_desc = dst.format.base_format().0.desc();
        if format_desc.is_compressed() {
            // we dont really care about non-4x4 block formats..
            assert_eq!(format_desc.dim, (4, 4));
            assert!(!src.host_ptr.is_null());

            for copy in regions {
                let info = copy.borrow();

                let bytes_per_texel  = format_desc.bits as u32 / 8;

                let row_pitch = bytes_per_texel * info.image_extent.width / 4;
                let depth_pitch = row_pitch * info.image_extent.height / 4;

                unsafe {
                    context.UpdateSubresource(
                        dst.internal.raw,
                        dst.calc_subresource(info.image_layers.level as _, info.image_layers.layers.start as _),
                        &d3d11::D3D11_BOX {
                            left: info.image_offset.x as _,
                            top: info.image_offset.y as _,
                            front: info.image_offset.z as _,
                            right: info.image_extent.width,
                            bottom: info.image_extent.height,
                            back: info.image_extent.depth,
                        },
                        src.host_ptr.offset(src.bound_range.start as isize + info.buffer_offset as isize) as _,
                        row_pitch,
                        depth_pitch
                    );
                }
            }
        } else {
            let (shader, scale_x, scale_y) = self.find_buffer_to_image_shader(dst.typed_raw_format).unwrap();

            let srv = src.internal.srv.unwrap();

            unsafe {
                context.CSSetShader(shader, ptr::null_mut(), 0);
                context.CSSetConstantBuffers(0, 1, &self.copy_info.as_raw());
                context.CSSetShaderResources(0, 1, [srv].as_ptr());


                for copy in regions {
                    let info = copy.borrow();
                    self.update_buffer_image(context, &info);

                    let uav = dst.get_uav(info.image_layers.level, 0).unwrap().as_raw();
                    context.CSSetUnorderedAccessViews(0, 1, [uav].as_ptr(), ptr::null_mut());

                    context.Dispatch(
                        (info.image_extent.width as f32 / scale_x) as u32,
                        (info.image_extent.height as f32 / scale_y) as u32,
                        1
                    );
                }

                // unbind external resources
                context.CSSetShaderResources(0, 1, [ptr::null_mut(); 1].as_ptr());
                context.CSSetUnorderedAccessViews(0, 1, [ptr::null_mut(); 1].as_ptr(), ptr::null_mut());
            }
        }
    }

    fn find_blit_shader(&self, src: &Image) -> Option<*mut d3d11::ID3D11PixelShader> {
        use format::ChannelType::*;

        match src.format.base_format().1 {
            Uint => Some(self.ps_blit_2d_uint.as_raw()),
            Int => Some(self.ps_blit_2d_int.as_raw()),
            Unorm | Inorm | Float | Srgb => Some(self.ps_blit_2d_float.as_raw()),
            _ => None
        }
    }

    pub fn blit_2d_image<T>(&mut self, context: &ComPtr<d3d11::ID3D11DeviceContext>, src: &Image, dst: &Image, filter: image::Filter, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::ImageBlit>
    {

        use std::cmp;

        let shader = self.find_blit_shader(src).unwrap();

        let srv = src.internal.srv.clone().unwrap().as_raw();

        unsafe {
            context.IASetPrimitiveTopology(d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(self.vs_blit_2d.as_raw(), ptr::null_mut(), 0);
            context.VSSetConstantBuffers(0, 1, [self.copy_info.as_raw()].as_ptr());
            context.PSSetShader(shader, ptr::null_mut(), 0);
            context.PSSetShaderResources(0, 1, [srv].as_ptr());
            context.PSSetSamplers(0, 1, match filter {
                image::Filter::Nearest => [self.sampler_nearest.as_raw()],
                image::Filter::Linear => [self.sampler_linear.as_raw()],
            }.as_ptr());


            for region in regions {
                let region = region.borrow();
                self.update_blit(context, src, &region);

                // TODO: more layers
                let rtv = dst.get_rtv(region.dst_subresource.level, region.dst_subresource.layers.start).unwrap().as_raw();

                context.RSSetViewports(1, [d3d11::D3D11_VIEWPORT {
                    TopLeftX: cmp::min(region.dst_bounds.start.x, region.dst_bounds.end.x) as _,
                    TopLeftY: cmp::min(region.dst_bounds.start.y, region.dst_bounds.end.y) as _,
                    Width: (region.dst_bounds.end.x - region.dst_bounds.start.x).abs() as _,
                    Height: (region.dst_bounds.end.y - region.dst_bounds.start.y).abs() as _,
                    MinDepth: 0.0f32,
                    MaxDepth: 1.0f32,
                }].as_ptr());
                context.OMSetRenderTargets(1, [rtv].as_ptr(), ptr::null_mut());
                context.Draw(3, 0);
            }


            context.PSSetShaderResources(0, 1, [ptr::null_mut()].as_ptr());
            context.OMSetRenderTargets(1, [ptr::null_mut()].as_ptr(), ptr::null_mut());
        }
    }
}
