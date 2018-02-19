use std::{mem, ptr};
use winapi::{self, UINT};
use core::{self, texture as tex};
use command;
use {Buffer, Texture};
use wio::com::ComPtr;

fn copy_buffer(context: &mut ComPtr<winapi::ID3D11DeviceContext>,
               src: &Buffer, dst: &Buffer,
               src_offset: UINT, dst_offset: UINT,
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
        context.CopySubresourceRegion(dst_resource, 0, dst_offset, 0, 0,
                                         src_resource, 0, &src_box)
    };
}

pub fn update_buffer(context: &mut ComPtr<winapi::ID3D11DeviceContext>, buffer: &Buffer,
                     data: &[u8], offset_bytes: usize) {
    let dst_resource = (buffer.0).0 as *mut winapi::ID3D11Resource;

    // DYNAMIC only
    let map_type = winapi::D3D11_MAP_WRITE_DISCARD;
    let hr = unsafe {
        let mut sub = mem::zeroed();
        let hr = context.Map(dst_resource, 0, map_type, 0, &mut sub);
        let dst = (sub.pData as *mut u8).offset(offset_bytes as isize);
        ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        context.Unmap(dst_resource, 0);
        hr
    };
    if !winapi::SUCCEEDED(hr) {
        error!("Buffer {:?} failed to map, error {:x}", buffer, hr);
    }
}

pub fn update_texture(context: &mut ComPtr<winapi::ID3D11DeviceContext>, texture: &Texture, kind: tex::Kind,
                      face: Option<tex::CubeFace>, data: &[u8], image: &tex::RawImageInfo) {
    let subres = texture_subres(face, image);
    let dst_resource = texture.as_resource();
    let (width, height, _, _) = kind.level_dimensions(image.mipmap);
    let stride = image.format.0.get_total_bits() as usize;
    let row_pitch = width as usize * stride;
    let depth_pitch = height as usize * row_pitch;

    // DYNAMIC only
    let offset_bytes = image.xoffset as usize +
                       image.yoffset as usize * row_pitch +
                       image.zoffset as usize * depth_pitch;
    let map_type = winapi::D3D11_MAP_WRITE_DISCARD;
    let hr = unsafe {
        let mut sub = mem::zeroed();
        let hr = context.Map(dst_resource, subres, map_type, 0, &mut sub);
        let dst = (sub.pData as *mut u8).offset(offset_bytes as isize);
        ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        context.Unmap(dst_resource, 0);
        hr
    };
    if !winapi::SUCCEEDED(hr) {
        error!("Texture {:?} failed to map, error {:x}", texture, hr);
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

pub fn process(ctx: &mut ComPtr<winapi::ID3D11DeviceContext>, command: &command::Command, data_buf: &command::DataBuffer) {
    use winapi::UINT;
    use core::shade::Stage;
    use command::Command::*;

    let max_cb  = core::MAX_CONSTANT_BUFFERS as UINT;
    let max_srv = core::MAX_RESOURCE_VIEWS   as UINT;
    let max_sm  = core::MAX_SAMPLERS         as UINT;
    debug!("Processing {:?}", command);
    match *command {
        BindProgram(ref prog) => unsafe {
            ctx.VSSetShader(prog.vs, ptr::null_mut(), 0);
            ctx.HSSetShader(prog.hs, ptr::null_mut(), 0);
            ctx.DSSetShader(prog.ds, ptr::null_mut(), 0);
            ctx.GSSetShader(prog.gs, ptr::null_mut(), 0);
            ctx.PSSetShader(prog.ps, ptr::null_mut(), 0);
        },
        BindInputLayout(layout) => unsafe {
            ctx.IASetInputLayout(layout);
        },
        BindIndex(ref buf, format) => unsafe {
            ctx.IASetIndexBuffer((buf.0).0, format, 0);
        },
        BindVertexBuffers(ref buffers, ref strides, ref offsets) => unsafe {
            ctx.IASetVertexBuffers(0, core::MAX_VERTEX_ATTRIBUTES as UINT,
                &buffers[0].0, strides.as_ptr(), offsets.as_ptr());
        },
        BindConstantBuffers(stage, ref buffers) => match stage {
            Stage::Vertex => unsafe {
                ctx.VSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Hull => unsafe {
                ctx.HSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Domain => unsafe {
                ctx.DSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Geometry => unsafe {
                ctx.GSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
            Stage::Pixel => unsafe {
                ctx.PSSetConstantBuffers(0, max_cb, &buffers[0].0);
            },
        },
        BindShaderResources(stage, ref views) => match stage {
            Stage::Vertex => unsafe {
                ctx.VSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Hull => unsafe {
                ctx.HSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Domain => unsafe {
                ctx.DSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Geometry => unsafe {
                ctx.GSSetShaderResources(0, max_srv, &views[0].0);
            },
            Stage::Pixel => unsafe {
                ctx.PSSetShaderResources(0, max_srv, &views[0].0);
            },
        },
        BindSamplers(stage, ref samplers) => match stage {
            Stage::Vertex => unsafe {
                ctx.VSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Hull => unsafe {
                ctx.HSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Domain => unsafe {
                ctx.DSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Geometry => unsafe {
                ctx.GSSetSamplers(0, max_sm, &samplers[0].0);
            },
            Stage::Pixel => unsafe {
                ctx.PSSetSamplers(0, max_sm, &samplers[0].0);
            },
        },
        BindPixelTargets(ref colors, ds) => unsafe {
            ctx.OMSetRenderTargets(core::MAX_COLOR_TARGETS as UINT,
                &colors[0].0, ds.0);
        },
        SetPrimitive(topology) => unsafe {
            ctx.IASetPrimitiveTopology(topology);
        },
        SetViewport(ref viewport) => unsafe {
            ctx.RSSetViewports(1, viewport);
        },
        SetScissor(ref rect) => unsafe {
            ctx.RSSetScissorRects(1, rect);
        },
        SetRasterizer(rast) => unsafe {
            ctx.RSSetState(rast as *mut _);
        },
        SetDepthStencil(ds, value) => unsafe {
            ctx.OMSetDepthStencilState(ds as *mut _, value);
        },
        SetBlend(blend, ref value, mask) => unsafe {
            ctx.OMSetBlendState(blend as *mut _, value, mask);
        },
        CopyBuffer(ref src, ref dst, src_offset, dst_offset, size) => {
            copy_buffer(ctx, src, dst, src_offset, dst_offset, size);
        },
        UpdateBuffer(ref buffer, pointer, offset) => {
            let data = data_buf.get(pointer);
            update_buffer(ctx, buffer, data, offset);
        },
        UpdateTexture(ref tex, kind, face, pointer, ref image) => {
            let data = data_buf.get(pointer);
            update_texture(ctx, tex, kind, face, data, image);
        },
        GenerateMips(ref srv) => unsafe {
            ctx.GenerateMips(srv.0);
        },
        ClearColor(target, ref data) => unsafe {
            ctx.ClearRenderTargetView(target.0, data);
        },
        ClearDepthStencil(target, flags, depth, stencil) => unsafe {
            ctx.ClearDepthStencilView(target.0, flags.0, depth, stencil);
        },
        Draw(nvert, svert) => unsafe {
            ctx.Draw(nvert, svert);
        },
        DrawInstanced(nvert, ninst, svert, sinst) => unsafe {
            ctx.DrawInstanced(nvert, ninst, svert, sinst);
        },
        DrawIndexed(nind, svert, base) => unsafe {
            ctx.DrawIndexed(nind, svert, base);
        },
        DrawIndexedInstanced(nind, ninst, sind, base, sinst) => unsafe {
            ctx.DrawIndexedInstanced(nind, ninst, sind, base, sinst);
        },
    }
}
