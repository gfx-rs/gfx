// Copyright 2014 The Gfx-rs Developers.
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

use {gl, Surface, Texture, NewTexture, Buffer, Sampler};
use gl::types::{GLenum, GLuint, GLint, GLfloat, GLsizei, GLvoid};
use state;
use info::PrivateCaps;
use core::memory::SHADER_RESOURCE;
use core::format::{Format as NewFormat, ChannelType};
use core::texture as t;


fn cube_face_to_gl(face: t::CubeFace) -> GLenum {
    match face {
        t::CubeFace::PosZ => gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
        t::CubeFace::NegZ => gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        t::CubeFace::PosX => gl::TEXTURE_CUBE_MAP_POSITIVE_X,
        t::CubeFace::NegX => gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
        t::CubeFace::PosY => gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
        t::CubeFace::NegY => gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
    }
}

pub fn kind_to_gl(kind: t::Kind) -> GLenum {
    match kind {
        t::Kind::D1(_) => gl::TEXTURE_1D,
        t::Kind::D1Array(_, _) => gl::TEXTURE_1D_ARRAY,
        t::Kind::D2(_, _, t::AaMode::Single) => gl::TEXTURE_2D,
        t::Kind::D2(_, _, _) => gl::TEXTURE_2D_MULTISAMPLE,
        t::Kind::D2Array(_, _, _, t::AaMode::Single) => gl::TEXTURE_2D_ARRAY,
        t::Kind::D2Array(_, _, _, _) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        t::Kind::D3(_, _, _) => gl::TEXTURE_3D,
        t::Kind::Cube(_) => gl::TEXTURE_CUBE_MAP,
        t::Kind::CubeArray(_, _) => gl::TEXTURE_CUBE_MAP_ARRAY,
    }
}

fn kind_face_to_gl(kind: t::Kind, face: Option<t::CubeFace>) -> GLenum {
    match face {
        Some(f) => cube_face_to_gl(f),
        None => kind_to_gl(kind),
    }
}

fn format_to_glpixel(format: NewFormat) -> GLenum {
    use core::format::SurfaceType as S;
    use core::format::ChannelType as C;
    let (r, rg, rgb, rgba, bgra) = match format.1 {
        C::Int | C::Uint => (gl::RED_INTEGER, gl::RG_INTEGER, gl::RGB_INTEGER, gl::RGBA_INTEGER, gl::BGRA_INTEGER),
        _ => (gl::RED, gl::RG, gl::RGB, gl::RGBA, gl::BGRA),
    };
    match format.0 {
        S::R8 | S::R16 | S::R32=> r,
        S::R4_G4 | S::R8_G8 | S::R16_G16 | S::R32_G32 => rg,
        S::R16_G16_B16 | S::R32_G32_B32 | S::R5_G6_B5 | S::R11_G11_B10 => rgb,
        S::R8_G8_B8_A8 | S::R16_G16_B16_A16 | S::R32_G32_B32_A32 |
        S::R4_G4_B4_A4 | S::R5_G5_B5_A1 | S::R10_G10_B10_A2 => rgba,
        S::D24_S8 => gl::DEPTH_STENCIL,
        S::D16 | S::D24 | S::D32 => gl::DEPTH_COMPONENT,
        S::B8_G8_R8_A8 => bgra,
    }
}

fn format_to_gltype(format: NewFormat) -> Result<GLenum, ()> {
    use core::format::SurfaceType as S;
    use core::format::ChannelType as C;
    let (fm8, fm16, fm32) = match format.1 {
        C::Int | C::Inorm =>
            (gl::BYTE, gl::SHORT, gl::INT),
        C::Uint | C::Unorm | C::Srgb =>
            (gl::UNSIGNED_BYTE, gl::UNSIGNED_SHORT, gl::UNSIGNED_INT),
        C::Float => (gl::ZERO, gl::HALF_FLOAT, gl::FLOAT),
    };
    Ok(match format.0 {
        //S::R3_G3_B2 => gl::UNSIGNED_BYTE_3_3_2,
        S::R4_G4 => return Err(()),
        S::R4_G4_B4_A4 => gl::UNSIGNED_SHORT_4_4_4_4,
        S::R5_G5_B5_A1 => gl::UNSIGNED_SHORT_5_5_5_1,
        S::R5_G6_B5 => gl::UNSIGNED_SHORT_5_6_5,
        S::B8_G8_R8_A8 | S::R8 | S::R8_G8 | S::R8_G8_B8_A8 => fm8,
        S::R10_G10_B10_A2 => gl::UNSIGNED_INT_10_10_10_2,
        S::R11_G11_B10 => return Err(()),
        S::R16 | S::R16_G16 | S::R16_G16_B16 | S::R16_G16_B16_A16 => fm16,
        S::R32 | S::R32_G32 | S::R32_G32_B32 | S::R32_G32_B32_A32 => fm32,
        S::D16 => gl::UNSIGNED_SHORT,
        S::D24 => gl::UNSIGNED_INT,
        S::D24_S8 => gl::UNSIGNED_INT_24_8,
        S::D32 => gl::FLOAT,
    })
}

fn format_to_glfull(format: NewFormat) -> Result<GLenum, ()> {
    use core::format::SurfaceType as S;
    use core::format::ChannelType as C;
    let cty = format.1;
    Ok(match format.0 {
        //S::R3_G3_B2 => gl::R3_G3_B2,
        S::R4_G4 => return Err(()),
        S::R4_G4_B4_A4 => match cty {
            C::Unorm => gl::RGBA4,
            _ => return Err(()),
        },
        S::R5_G5_B5_A1 => match cty {
            C::Unorm => gl::RGB5_A1,
            _ => return Err(()),
        },
        S::R5_G6_B5 => match cty {
            C::Unorm => gl::RGB565,
            _ => return Err(()),
        },
        // 8 bits
        S::R8 => match cty {
            C::Int => gl::R8I,
            C::Inorm => gl::R8_SNORM,
            C::Uint => gl::R8UI,
            C::Unorm => gl::R8,
            _ => return Err(()),
        },
        S::R8_G8 => match cty {
            C::Int => gl::RG8I,
            C::Inorm => gl::RG8_SNORM,
            C::Uint => gl::RG8UI,
            C::Unorm => gl::RG8,
            _ => return Err(()),
        },
        //S::R8_G8_B8 |
        S::R8_G8_B8_A8 => match cty {
            C::Int => gl::RGBA8I,
            C::Inorm => gl::RGBA8_SNORM,
            C::Uint => gl::RGBA8UI,
            C::Unorm => gl::RGBA8,
            C::Srgb => gl::SRGB8_ALPHA8,
            _ => return Err(()),
        },
        // 10+ bits
        S::R10_G10_B10_A2 => match cty {
            C::Uint => gl::RGB10_A2UI,
            C::Unorm => gl::RGB10_A2,
            _ => return Err(()),
        },
        S::R11_G11_B10 => return Err(()),
        // 16 bits
        S::R16 => match cty {
            C::Int => gl::R16I,
            C::Inorm => gl::R16_SNORM,
            C::Uint => gl::R16UI,
            C::Unorm => gl::R16,
            C::Float => gl::R16F,
            _ => return Err(()),
        },
        S::R16_G16 => match cty {
            C::Int => gl::RG16I,
            C::Inorm => gl::RG16_SNORM,
            C::Uint => gl::RG16UI,
            C::Unorm => gl::RG16,
            C::Float => gl::RG16F,
            _ => return Err(()),
        },
        S::R16_G16_B16 => match cty {
            C::Int => gl::RGB16I,
            C::Inorm => gl::RGB16_SNORM,
            C::Uint => gl::RGB16UI,
            C::Unorm => gl::RGB16,
            C::Float => gl::RGB16F,
            _ => return Err(()),
        },
        S::R16_G16_B16_A16 => match cty {
            C::Int => gl::RGBA16I,
            C::Inorm => gl::RGBA16_SNORM,
            C::Uint => gl::RGBA16UI,
            C::Unorm => gl::RGBA16,
            C::Float => gl::RGBA16F,
            _ => return Err(()),
        },
        // 32 bits
        S::R32 => match cty {
            C::Int => gl::R32I,
            C::Uint => gl::R32UI,
            C::Float => gl::R32F,
            _ => return Err(()),
        },
        S::R32_G32 => match cty {
            C::Int => gl::RG32I,
            C::Uint => gl::RG32UI,
            C::Float => gl::RG32F,
            _ => return Err(()),
        },
        S::R32_G32_B32 => match cty {
            C::Int => gl::RGB32I,
            C::Uint => gl::RGB32UI,
            C::Float => gl::RGB32F,
            _ => return Err(()),
        },
        S::R32_G32_B32_A32 => match cty {
            C::Int => gl::RGBA32I,
            C::Uint => gl::RGBA32UI,
            C::Float => gl::RGBA32F,
            _ => return Err(()),
        },
        S::B8_G8_R8_A8 => match cty {
            C::Unorm => gl::RGBA8,
            _ => return Err(()),
        },
        // depth-stencil
        S::D16 => gl::DEPTH_COMPONENT16,
        S::D24 => gl::DEPTH_COMPONENT24,
        S::D24_S8 => gl::DEPTH24_STENCIL8,
        S::D32 => gl::DEPTH_COMPONENT32F,
    })
}

fn set_mipmap_range(gl: &gl::Gl, target: GLenum, (base, max): (u8, u8)) { unsafe {
    gl.TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
}}

fn make_surface_impl(gl: &gl::Gl, format: GLenum, dim: t::Dimensions)
                     -> Result<Surface, ()> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    unsafe {
        gl.BindRenderbuffer(target, name);
    }
    match dim.3 {
        t::AaMode::Single => unsafe {
            gl.RenderbufferStorage(
                target,
                format,
                dim.0 as GLsizei,
                dim.1 as GLsizei
            );
        },
        t::AaMode::Multi(samples) => unsafe {
            gl.RenderbufferStorageMultisample(
                target,
                samples as GLsizei,
                format,
                dim.0 as GLsizei,
                dim.1 as GLsizei
            );
        },
        t::AaMode::Coverage(_, _) => return Err(()),
    }

    Ok(name)
}

/// Create a render surface.
pub fn make_surface(gl: &gl::Gl, desc: &t::Info, cty: ChannelType) ->
                        Result<Surface, t::CreationError> {
    let format = NewFormat(desc.format, cty);
    let format_error = t::CreationError::Format(desc.format, Some(cty));
    let fmt = match format_to_glfull(format) {
        Ok(f) => f,
        Err(_) => return Err(format_error),
    };
    make_surface_impl(gl, fmt, desc.kind.get_dimensions())
        .map_err(|_| format_error)
}

fn make_widout_storage_impl(gl: &gl::Gl, kind: t::Kind, format: GLint, pix: GLenum, typ: GLenum,
                            levels: t::Level, fixed_sample_locations: bool)
                            -> Result<Texture, t::CreationError> {
    let (name, target) = make_texture(gl, kind);
    match kind {
        t::Kind::D1(w) => unsafe {
            gl.TexImage1D(
                target,
                0,
                format,
                w as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        t::Kind::D1Array(w, a) => unsafe {
            gl.TexImage2D(
                target,
                0,
                format,
                w as GLsizei,
                a as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        t::Kind::D2(w, h, t::AaMode::Single) => unsafe {
            gl.TexImage2D(
                target,
                0,
                format,
                w as GLsizei,
                h as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        t::Kind::D2(w, h, t::AaMode::Multi(samples)) => unsafe {
            gl.TexImage2DMultisample(
                target,
                samples as GLsizei,
                format as GLenum,  //GL spec bug
                w as GLsizei,
                h as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        t::Kind::D2Array(w, h, a, t::AaMode::Single) => unsafe {
            gl.TexImage3D(
                target,
                0,
                format,
                w as GLsizei,
                h as GLsizei,
                a as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        t::Kind::D2Array(w, h, a, t::AaMode::Multi(samples)) => unsafe {
            gl.TexImage3DMultisample(
                target,
                samples as GLsizei,
                format as GLenum,  //GL spec bug
                w as GLsizei,
                h as GLsizei,
                a as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        t::Kind::D3(w, h, d)  => unsafe {
            gl.TexImage3D(
                target,
                0,
                format,
                w as GLsizei,
                h as GLsizei,
                d as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        t::Kind::Cube(w) => {
            for &target in [gl::TEXTURE_CUBE_MAP_POSITIVE_X, gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Y, gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Z, gl::TEXTURE_CUBE_MAP_NEGATIVE_Z].iter() {
                unsafe { gl.TexImage2D(
                    target,
                    0,
                    format,
                    w as GLsizei,
                    w as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null()
                )};
            }
        },
        t::Kind::CubeArray(_, _) => return Err(t::CreationError::Kind),
        t::Kind::D2(_, _, aa) => return Err(t::CreationError::Samples(aa)),
        t::Kind::D2Array(_, _, _, aa) => return Err(t::CreationError::Samples(aa)),
    }

    set_mipmap_range(gl, target, (0, levels - 1));
    Ok(name)
}

/// Create a texture, using the descriptor, assuming TexStorage* isn't available.
pub fn make_without_storage(gl: &gl::Gl, desc: &t::Info, cty: ChannelType) ->
                            Result<Texture, t::CreationError> {
    let format = NewFormat(desc.format, cty);
    let gl_format = match format_to_glfull(format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(t::CreationError::Format(desc.format, Some(cty))),
    };
    let gl_pixel_format = format_to_glpixel(format);
    let gl_data_type = match format_to_gltype(format) {
        Ok(t) => t,
        Err(_) => return Err(t::CreationError::Format(desc.format, Some(cty))),
    };

    let fixed_loc = desc.bind.contains(SHADER_RESOURCE);
    make_widout_storage_impl(gl, desc.kind, gl_format, gl_pixel_format, gl_data_type,
                             desc.levels, fixed_loc)
}

/// Create a texture, assuming TexStorage is available.
fn make_with_storage_impl(gl: &gl::Gl, kind: t::Kind, format: GLenum,
                          levels: t::Level, fixed_sample_locations: bool)
                          -> Result<Texture, t::CreationError> {
    use std::cmp::max;

    fn min(a: u8, b: u8) -> GLint {
        ::std::cmp::min(a, b) as GLint
    }
    fn mip_level1(w: u16) -> u8 {
        ((w as f32).log2() + 1.0) as u8
    }
    fn mip_level2(w: u16, h: u16) -> u8 {
        ((max(w, h) as f32).log2() + 1.0) as u8
    }
    fn mip_level3(w: u16, h: u16, d: u16) -> u8 {
        ((max(w, max(h, d)) as f32).log2() + 1.0) as u8
    }

    let (name, target) = make_texture(gl, kind);
    match kind {
        t::Kind::D1(w) => unsafe {
            gl.TexStorage1D(
                target,
                min(levels, mip_level1(w)),
                format,
                w as GLsizei
            );
        },
        t::Kind::D1Array(w, a) => unsafe {
            gl.TexStorage2D(
                target,
                min(levels, mip_level1(w)),
                format,
                w as GLsizei,
                a as GLsizei
            );
        },
        t::Kind::D2(w, h, t::AaMode::Single) => unsafe {
            gl.TexStorage2D(
                target,
                min(levels, mip_level2(w, h)),
                format,
                w as GLsizei,
                h as GLsizei
            );
        },
        t::Kind::D2Array(w, h, a, t::AaMode::Single) => unsafe {
            gl.TexStorage3D(
                target,
                min(levels, mip_level2(w, h)),
                format,
                w as GLsizei,
                h as GLsizei,
                a as GLsizei
            );
        },
        t::Kind::D2(w, h, t::AaMode::Multi(samples)) => unsafe {
            gl.TexStorage2DMultisample(
                target,
                samples as GLsizei,
                format,
                w as GLsizei,
                h as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        t::Kind::D2Array(w, h, a, t::AaMode::Multi(samples)) => unsafe {
            gl.TexStorage3DMultisample(
                target,
                samples as GLsizei,
                format as GLenum,
                w as GLsizei,
                h as GLsizei,
                a as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        t::Kind::D3(w, h, d) => unsafe {
            gl.TexStorage3D(
                target,
                min(levels, mip_level3(w, h, d)),
                format,
                w as GLsizei,
                h as GLsizei,
                d as GLsizei
            );
        },
        t::Kind::Cube(w) => unsafe {
            gl.TexStorage2D(
                target,
                min(levels, mip_level2(w, w)),
                format,
                w as GLsizei,
                w as GLsizei
            );
        },
        t::Kind::CubeArray(w, d) => unsafe {
            gl.TexStorage3D(
                target,
                min(levels, mip_level2(w, w)),
                format,
                w as GLsizei,
                w as GLsizei,
                d as GLsizei,
            );
        },
        t::Kind::D2(_, _, aa) => return Err(t::CreationError::Samples(aa)),
        t::Kind::D2Array(_, _, _, aa) => return Err(t::CreationError::Samples(aa)),
    }

    set_mipmap_range(gl, target, (0, levels - 1));

    Ok(name)
}

/// Create a texture, using the descriptor, assuming TexStorage is available.
pub fn make_with_storage(gl: &gl::Gl, desc: &t::Info, cty: ChannelType) ->
                         Result<Texture, t::CreationError> {
    let format = NewFormat(desc.format, cty);
    let gl_format = match format_to_glfull(format) {
        Ok(f) => f,
        Err(_) => return Err(t::CreationError::Format(desc.format, Some(cty))),
    };
    let fixed_loc = desc.bind.contains(SHADER_RESOURCE);
    make_with_storage_impl(gl, desc.kind, gl_format, desc.levels, fixed_loc)
}

/// Bind a sampler using a given binding anchor.
/// Used for GL compatibility profile only. The core profile has sampler objects
pub fn bind_sampler(gl: &gl::Gl, target: GLenum, info: &t::SamplerInfo, private_caps: &PrivateCaps) { unsafe {
    let (min, mag) = filter_to_gl(info.filter);

    match info.filter {
        t::FilterMethod::Anisotropic(fac) =>
            gl.TexParameterf(target, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl.TexParameteri(target, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl.TexParameteri(target, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl.TexParameteri(target, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl.TexParameteri(target, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    if private_caps.sampler_lod_bias_supported {
        gl.TexParameterf(target, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());
    }
    let border: [f32; 4] = info.border.into();
    gl.TexParameterfv(target, gl::TEXTURE_BORDER_COLOR, &border[0]);

    let (min, max) = info.lod_range;
    gl.TexParameterf(target, gl::TEXTURE_MIN_LOD, min.into());
    gl.TexParameterf(target, gl::TEXTURE_MAX_LOD, max.into());

    match info.comparison {
        None => gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        Some(cmp) => {
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }
}}

fn tex_sub_image<F>(gl: &gl::Gl, kind: t::Kind, target: GLenum, pix: GLenum,
                       typ: GLenum, img: &t::ImageInfoCommon<F>, data: *const GLvoid)
                       -> Result<(), t::CreationError> {
    Ok(match kind {
        t::Kind::D1(_) => unsafe {
            gl.TexSubImage1D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.width as GLint,
                pix,
                typ,
                data
            );
        },
        t::Kind::D1Array(_, _) | t::Kind::D2(_, _, t::AaMode::Single) => unsafe {
            gl.TexSubImage2D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                pix,
                typ,
                data
            );
        },
        t::Kind::D2Array(_, _, _, t::AaMode::Single) | t::Kind::D3(_, _, _) => unsafe {
            gl.TexSubImage3D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.zoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                img.depth as GLint,
                pix,
                typ,
                data
            );
        },
        t::Kind::Cube(_) => unsafe {
            gl.TexSubImage2D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                pix,
                typ,
                data
            );
        },
        t::Kind::CubeArray(_, _) => return Err(t::CreationError::Kind),
        t::Kind::D2(_, _, aa) => return Err(t::CreationError::Samples(aa)),
        t::Kind::D2Array(_, _, _, aa) => return Err(t::CreationError::Samples(aa)),
    })
}

pub fn copy_from_buffer(gl: &gl::Gl,
                        dst: Texture,
                        kind: t::Kind,
                        face: Option<t::CubeFace>,
                        img: &t::RawImageInfo,
                        src: Buffer, src_offset: gl::types::GLintptr)
                        -> Result<(), t::CreationError>
{
    // will be treated as a byte offset into the buffer object's data store
    let data = src_offset as *const GLvoid;
    unsafe { gl.BindBuffer(gl::PIXEL_UNPACK_BUFFER, src); }

    let pixel_format = format_to_glpixel(img.format);
    let data_type = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(t::CreationError::Format(img.format.0, Some(img.format.1))),
    };

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, dst); }

    let target = kind_face_to_gl(kind, face);
    tex_sub_image(gl, kind, target, pixel_format, data_type, img, data)
}

pub fn copy_to_buffer(gl: &gl::Gl,
                      src: NewTexture,
                      kind: t::Kind,
                      face: Option<t::CubeFace>,
                      img: &t::RawImageInfo,
                      dst: Buffer, dst_offset: gl::types::GLintptr)
                      -> Result<(), t::CreationError>
{
    let data = dst_offset as *mut GLvoid;
    unsafe { gl.BindBuffer(gl::PIXEL_PACK_BUFFER, dst); }

    let pixel_format = format_to_glpixel(img.format);
    let data_type = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(t::CreationError::Format(img.format.0, Some(img.format.1))),
    };

    match src {
        NewTexture::Texture(t) => {
            let target = kind_to_gl(kind);
            unsafe { gl.BindTexture(target, t); }

            let target = kind_face_to_gl(kind, face);
            // FIXME: can't specify image offsets
            let (w, h, d, _) = kind.get_dimensions();
            debug_assert!(img.xoffset == 0 &&
                          img.yoffset == 0 &&
                          img.zoffset == 0 &&
                          img.width == w &&
                          img.height == h &&
                          img.depth == d);
            unsafe {
                gl.GetTexImage(target,
                               img.mipmap as GLint,
                               pixel_format,
                               data_type,
                               data);
            }
        }
        NewTexture::Surface(s) => {
            unsafe {
                gl.BindFramebuffer(gl::READ_FRAMEBUFFER, s);
                gl.ReadPixels(img.xoffset as GLint,
                              img.yoffset as GLint,
                              img.width as GLint,
                              img.height as GLint,
                              pixel_format,
                              data_type,
                              data);
            }
        }
    }

    Ok(())
}

pub fn update_texture(gl: &gl::Gl, name: Texture,
                      kind: t::Kind, face: Option<t::CubeFace>,
                      img: &t::RawImageInfo, slice: &[u8])
                          -> Result<(), t::CreationError> {
    //TODO: check size
    let data = slice.as_ptr() as *const GLvoid;
    let pixel_format = format_to_glpixel(img.format);
    let data_type = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(t::CreationError::Format(img.format.0, Some(img.format.1))),
    };

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, name) };

    let target = kind_face_to_gl(kind, face);
    tex_sub_image(gl, kind, target, pixel_format, data_type, img, data)
}

pub fn init_texture_data(gl: &gl::Gl, name: Texture, desc: t::Info, channel: ChannelType,
                         data: &[&[u8]]) -> Result<(), t::CreationError> {
    let opt_slices = desc.kind.get_num_slices();
    let num_slices = opt_slices.unwrap_or(1) as usize;
    let num_mips = desc.levels as usize;
    let mut cube_faces = [None; 6];
    let faces: &[_] = if desc.kind.is_cube() {
        for (cf, orig) in cube_faces.iter_mut().zip(t::CUBE_FACES.iter()) {
            *cf = Some(*orig);
        }
        &cube_faces
    } else {
        &cube_faces[..1]
    };
    if data.len() != num_slices * faces.len() * num_mips {
        error!("Texture expects {} slices {} faces {} mips, given {} data chunks instead",
            num_slices, faces.len(), num_mips, data.len());
        return Err(t::CreationError::Data(0))
    }

    for i in 0 .. num_slices {
        for (f, &face) in faces.iter().enumerate() {
            for m in 0 .. num_mips {
                let sub = data[(i*faces.len() + f)*num_mips + m];
                let mut image = desc.to_raw_image_info(channel, m as t::Level);
                if opt_slices.is_some() {
                    image.zoffset = i as t::Size;
                    image.depth = 1;
                }
                try!(update_texture(gl, name, desc.kind, face, &image, sub));
            }
        }
    }

    Ok(())
}

/*
pub fn update_texture(gl: &gl::Gl, kind: Kind, face: Option<CubeFace>,
                      name: Texture, img: &ImageInfo, slice: &[u8])
                      -> Result<(), Error> {
    if let Some(fmt_size) = img.format.get_size() {
        // TODO: can we compute the expected size for compressed formats?
        let expected_size = img.width as usize * img.height as usize *
                            img.depth as usize * fmt_size as usize;
        if slice.len() != expected_size {
            return Err(Error::IncorrectSize(expected_size));
        }
    }

    let data = slice.as_ptr() as *const GLvoid;
    let pixel_format = old_format_to_glpixel(img.format);
    let data_type = match old_format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(Error::UnsupportedFormat),
    };

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, name) };

    let target = kind_face_to_gl(kind, face);
    if img.format.is_compressed() {
        compressed_update(gl, kind, target, img, data, data_type, slice.len() as GLint)
    }else {
        update_texture_impl(gl, kind, target, pixel_format, data_type, img, data)
    }
}

pub fn compressed_update(gl: &gl::Gl, kind: Kind, target: GLenum, img: &ImageInfo,
                         data: *const GLvoid, typ: GLenum, size: GLint)
                         -> Result<(), Error> {
    match kind {
        Kind::D1(_) => unsafe {
            gl.CompressedTexSubImage1D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.width as GLint,
                typ,
                size as GLint,
                data
            );
        },
        Kind::D1Array(_, _) | Kind::D2(_, _, AaMode::Single) => unsafe {
            gl.CompressedTexSubImage2D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                typ,
                size as GLint,
                data
            );
        },
        Kind::D2Array(_, _, _, AaMode::Single) | Kind::D3(_, _, _) => unsafe {
            gl.CompressedTexSubImage3D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.zoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                img.depth as GLint,
                typ,
                size as GLint,
                data
            );
        },
        Kind::Cube(_, _) => unsafe {
            gl.CompressedTexSubImage2D(
                target,
                img.mipmap as GLint,
                img.xoffset as GLint,
                img.yoffset as GLint,
                img.width as GLint,
                img.height as GLint,
                typ,
                size as GLint,
                data
            );
        },
        _ => return Err(Error::UnsupportedSamples),
    }

    Ok(())
}
*/

/// Common texture creation routine, just creates and binds.
fn make_texture(gl: &gl::Gl, kind: t::Kind) -> (Texture, GLuint) {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenTextures(1, &mut name);
    }

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, name) };
    (name, target)
}

fn wrap_to_gl(w: t::WrapMode) -> GLenum {
    match w {
        t::WrapMode::Tile   => gl::REPEAT,
        t::WrapMode::Mirror => gl::MIRRORED_REPEAT,
        t::WrapMode::Clamp  => gl::CLAMP_TO_EDGE,
        t::WrapMode::Border => gl::CLAMP_TO_BORDER,
    }
}

fn filter_to_gl(f: t::FilterMethod) -> (GLenum, GLenum) {
    match f {
        t::FilterMethod::Scale => (gl::NEAREST, gl::NEAREST),
        t::FilterMethod::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        t::FilterMethod::Bilinear => (gl::LINEAR, gl::LINEAR),
        t::FilterMethod::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        t::FilterMethod::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
}

pub fn make_sampler(gl: &gl::Gl, info: &t::SamplerInfo, private_caps: &PrivateCaps) -> Sampler { unsafe {
    let mut name = 0 as Sampler;
    gl.GenSamplers(1, &mut name);

    let (min, mag) = filter_to_gl(info.filter);

    match info.filter{
        t::FilterMethod::Anisotropic(fac) =>
            gl.SamplerParameterf(name, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl.SamplerParameteri(name, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    if private_caps.sampler_lod_bias_supported {
        gl.SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());
    }
    let border: [f32; 4] = info.border.into();
    gl.SamplerParameterfv(name, gl::TEXTURE_BORDER_COLOR, &border[0]);

    let (min, max) = info.lod_range;
    gl.SamplerParameterf(name, gl::TEXTURE_MIN_LOD, min.into());
    gl.SamplerParameterf(name, gl::TEXTURE_MAX_LOD, max.into());

    match info.comparison {
        None => gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        Some(cmp) => {
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }

    name
}}

pub fn generate_mipmap(gl: &gl::Gl, name: Texture, target: gl::types::GLenum) { unsafe {
    //can't fail here, but we need to check for integer formats too
    gl.BindTexture(target, name);
    gl.GenerateMipmap(target);
}}
