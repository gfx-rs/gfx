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

use std::{cmp, mem, ptr};
use winapi::{self, UINT};
use core::{self, texture as tex};
use command;
use {Buffer, Texture};


fn copy_buffer(context: *mut winapi::ID3D11DeviceContext,
               src: &Buffer, src_offset: UINT,
               dst: &Buffer, dst_offset: UINT,
               size: UINT) {
    let src_resource = src.as_resource();
    let dst_resource = dst.as_resource();
    let src_box = winapi::D3D11_BOX {
        left: src_offset,
        right: src_offset + size,
        top: 0,
        bottom: 1,
        front: 0,
        back: 1,
    };
    unsafe {
        (*context).CopySubresourceRegion(dst_resource, 0, dst_offset, 0, 0,
                                         src_resource, 0, &src_box)
    };
}

fn copy_texture(context: *mut winapi::ID3D11DeviceContext,
                src: &tex::TextureCopyRegion<Texture>,
                dst: &tex::TextureCopyRegion<Texture>) {
    assert_eq!((src.info.width, src.info.height, src.info.depth),
               (dst.info.width, dst.info.height, dst.info.depth));
    assert_eq!(src.kind.get_num_slices(), None);
    assert_eq!(dst.kind.get_num_slices(), None);

    let src_resource = src.texture.as_resource();
    let dst_resource = dst.texture.as_resource();
    let src_box = winapi::D3D11_BOX {
        left: src.info.xoffset as _,
        right: (src.info.xoffset + src.info.width) as _,
        top: src.info.yoffset as _,
        bottom: (src.info.yoffset + src.info.height) as _,
        front: src.info.zoffset as _,
        back: (src.info.zoffset + cmp::max(1, src.info.depth)) as _,
    };
    unsafe {
        (*context).CopySubresourceRegion(dst_resource, dst.info.mipmap as _,
                                         dst.info.xoffset as _, dst.info.yoffset as _, dst.info.zoffset as _,
                                         src_resource, src.info.mipmap as _, &src_box)
    };
}

pub fn update_buffer(context: *mut winapi::ID3D11DeviceContext, buffer: &Buffer,
                     data: &[u8], offset_bytes: usize) {
    let dst_resource = (buffer.0).0 as *mut winapi::ID3D11Resource;

    // DYNAMIC only
    let map_type = winapi::D3D11_MAP_WRITE_DISCARD;
    let hr = unsafe {
        let mut sub = mem::zeroed();
        let hr = (*context).Map(dst_resource, 0, map_type, 0, &mut sub);
        let dst = (sub.pData as *mut u8).offset(offset_bytes as isize);
        ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        (*context).Unmap(dst_resource, 0);
        hr
    };
    if !winapi::SUCCEEDED(hr) {
        error!("Buffer {:?} failed to map, error {:x}", buffer, hr);
    }
}

pub fn update_texture(context: *mut winapi::ID3D11DeviceContext,
                      tex: &tex::TextureCopyRegion<Texture>,
                      data: &[u8]) {
    let subres = texture_subres(tex.cube_face, &tex.info);
    let dst_resource = tex.texture.as_resource();
    // DYNAMIC only; This only works if the whole texture is covered.
    assert_eq!(tex.info.xoffset + tex.info.yoffset + tex.info.zoffset, 0);
    let map_type = winapi::D3D11_MAP_WRITE_DISCARD;
    let hr = unsafe {
        let mut sub = mem::zeroed();
        let hr = (*context).Map(dst_resource, subres, map_type, 0, &mut sub);
        let dst = sub.pData as *mut u8;
        ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        (*context).Unmap(dst_resource, 0);
        hr
    };
    if !winapi::SUCCEEDED(hr) {
        error!("Texture {:?} failed to map, error {:x}", tex.texture, hr);
    }
}

fn texture_subres(face: Option<tex::CubeFace>, image: &tex::RawImageInfo) -> winapi::UINT {
    use core::texture::CubeFace::*;

    let array_slice = match face {
        Some(PosX) => 0,
        Some(NegX) => 1,
        Some(PosY) => 2,
        Some(NegY) => 3,
        Some(PosZ) => 4,
        Some(NegZ) => 5,
        None => 0,
    };
    let num_mipmap_levels = 1; //TODO
    array_slice * num_mipmap_levels + (image.mipmap as UINT)
}

pub fn process(ctx: *mut winapi::ID3D11DeviceContext, command: &command::Command, data_buf: &command::DataBuffer) {
    use winapi::UINT;
    use core::shade::Stage;
    use command::Command::*;

    let max_cb  = core::MAX_CONSTANT_BUFFERS as UINT;
    let max_srv = core::MAX_RESOURCE_VIEWS   as UINT;
    let max_sm  = core::MAX_SAMPLERS         as UINT;
    debug!("Processing {:?}", command);
    match *command {
        BindProgram(ref prog) => unsafe {
            (*ctx).VSSetShader(prog.vs, ptr::null_mut(), 0);
            (*ctx).HSSetShader(prog.hs, ptr::null_mut(), 0);
            (*ctx).DSSetShader(prog.ds, ptr::null_mut(), 0);
            (*ctx).GSSetShader(prog.gs, ptr::null_mut(), 0);
            (*ctx).PSSetShader(prog.ps, ptr::null_mut(), 0);
        },
        BindInputLayout(layout) => unsafe {
            (*ctx).IASetInputLayout(layout);
        },
        BindIndex(ref buf, format) => unsafe {
            (*ctx).IASetIndexBuffer((buf.0).0, format, 0);
        },
        BindVertexBuffers(ref buffers, ref strides, ref offsets) => unsafe {
            (*ctx).IASetVertexBuffers(0, core::MAX_VERTEX_ATTRIBUTES as UINT,
                &buffers[0].0, strides.as_ptr(), offsets.as_ptr());
        },
        BindConstantBuffers(stage, ref buffers) => match stage {
            Stage::Vertex => unsafe {
                (*ctx).VSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Hull => unsafe {
                (*ctx).HSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Domain => unsafe {
                (*ctx).DSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Geometry => unsafe {
                (*ctx).GSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Pixel => unsafe {
                (*ctx).PSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
        },
        BindShaderResources(stage, ref views) => match stage {
            Stage::Vertex => unsafe {
                (*ctx).VSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Hull => unsafe {
                (*ctx).HSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Domain => unsafe {
                (*ctx).DSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Geometry => unsafe {
                (*ctx).GSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Pixel => unsafe {
                (*ctx).PSSetShaderResources(0, max_srv, &views[0].0);
            },
        },
        BindSamplers(stage, ref samplers) => match stage {
            Stage::Vertex => unsafe {
                (*ctx).VSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Hull => unsafe {
                (*ctx).HSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Domain => unsafe {
                (*ctx).DSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Geometry => unsafe {
                (*ctx).GSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Pixel => unsafe {
                (*ctx).PSSetSamplers(0, max_sm, &samplers[0].0);
            },
        },
        BindPixelTargets(ref colors, ds) => unsafe {
            (*ctx).OMSetRenderTargets(core::MAX_COLOR_TARGETS as UINT,
                &colors[0].0, ds.0);
        },
        SetPrimitive(topology) => unsafe {
            (*ctx).IASetPrimitiveTopology(topology);
        },
        SetViewport(ref viewport) => unsafe {
            (*ctx).RSSetViewports(1, viewport);
        },
        SetScissor(ref rect) => unsafe {
            (*ctx).RSSetScissorRects(1, rect);
        },
        SetRasterizer(rast) => unsafe {
            (*ctx).RSSetState(rast as *mut _);
        },
        SetDepthStencil(ds, value) => unsafe {
            (*ctx).OMSetDepthStencilState(ds as *mut _, value);
        },
        SetBlend(blend, ref value, mask) => unsafe {
            (*ctx).OMSetBlendState(blend as *mut _, value, mask);
        },
        CopyBuffer(ref src, src_offset, ref dst, dst_offset, size) => {
            copy_buffer(ctx, src, src_offset, dst, dst_offset, size);
        },
        CopyBufferToTexture(ref _src, _src_offset, ref _dst) => {
            unimplemented!()
        },
        CopyTextureToBuffer(ref _src, ref _dst, _dst_offset) => {
            unimplemented!()
        },
        CopyTexture(ref src, ref dst) => {
            copy_texture(ctx, src, dst);
        },
        UpdateBuffer(ref buffer, pointer, offset) => {
            let data = data_buf.get(pointer);
            update_buffer(ctx, buffer, data, offset);
        },
        UpdateTexture(ref dst, pointer) => {
            let data = data_buf.get(pointer);
            update_texture(ctx, dst, data);
        },
        GenerateMips(ref srv) => unsafe {
            (*ctx).GenerateMips(srv.0);
        },
        ClearColor(target, ref data) => unsafe {
            (*ctx).ClearRenderTargetView(target.0, data);
        },
        ClearDepthStencil(target, flags, depth, stencil) => unsafe {
            (*ctx).ClearDepthStencilView(target.0, flags.0, depth, stencil);
        },
        Draw(nvert, svert) => unsafe {
            (*ctx).Draw(nvert, svert);
        },
        DrawInstanced(nvert, ninst, svert, sinst) => unsafe {
            (*ctx).DrawInstanced(nvert, ninst, svert, sinst);
        },
        DrawIndexed(nind, svert, base) => unsafe {
            (*ctx).DrawIndexed(nind, svert, base);
        },
        DrawIndexedInstanced(nind, ninst, sind, base, sinst) => unsafe {
            (*ctx).DrawIndexedInstanced(nind, ninst, sind, base, sinst);
        },
    }
}
