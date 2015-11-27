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

use super::{gl, Surface, Texture, Sampler};
use super::gl::types::{GLenum, GLuint, GLint, GLfloat, GLsizei, GLvoid};
use super::state;
use gfx::device::tex::*;
use gfx::device::attrib::{FloatSize, IntSubType};


/// A token produced by the `bind_texture` that allows following up
/// with a GL-compatibility sampler settings in `bind_sampler`
#[derive(Copy, Clone)]
pub struct BindAnchor(GLenum);

fn create_kind_to_gl(kind: Kind) -> GLenum {
    match kind {
        Kind::D1 => gl::TEXTURE_1D,
        Kind::D1Array => gl::TEXTURE_1D_ARRAY,
        Kind::D2 => gl::TEXTURE_2D,
        Kind::D2Array => gl::TEXTURE_2D_ARRAY,
        Kind::D2MultiSample(_) => gl::TEXTURE_2D_MULTISAMPLE,
        Kind::D2MultiSampleArray(_) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        Kind::Cube(CubeFace::PosZ) => gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
        Kind::Cube(CubeFace::NegZ) => gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        Kind::Cube(CubeFace::PosX) => gl::TEXTURE_CUBE_MAP_POSITIVE_X,
        Kind::Cube(CubeFace::NegX) => gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
        Kind::Cube(CubeFace::PosY) => gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
        Kind::Cube(CubeFace::NegY) => gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
        Kind::D3 => gl::TEXTURE_3D,
    }
}

fn bind_kind_to_gl(kind: Kind) -> GLenum {
    match kind {
        Kind::Cube(_) => gl::TEXTURE_CUBE_MAP,
        other => create_kind_to_gl(other)
    }
}

fn format_to_gl(t: Format) -> Result<GLenum, ()> {
    Ok(match t {
        // floating-point
        Format::Float(Components::R,    FloatSize::F16) => gl::R16F,
        Format::Float(Components::R,    FloatSize::F32) => gl::R32F,
        Format::Float(Components::RG,   FloatSize::F16) => gl::RG16F,
        Format::Float(Components::RG,   FloatSize::F32) => gl::RG32F,
        Format::Float(Components::RGB,  FloatSize::F16) => gl::RGB16F,
        Format::Float(Components::RGB,  FloatSize::F32) => gl::RGB32F,
        Format::Float(Components::RGBA, FloatSize::F16) => gl::RGBA16F,
        Format::Float(Components::RGBA, FloatSize::F32) => gl::RGBA32F,
        Format::Float(_, FloatSize::F64) => return Err(()),

        // signed normalized
        Format::Integer(Components::R,    8, IntSubType::Normalized) => gl::R8_SNORM,
        Format::Integer(Components::RG,   8, IntSubType::Normalized) => gl::RG8_SNORM,
        Format::Integer(Components::RGB,  8, IntSubType::Normalized) => gl::RGB8_SNORM,
        Format::Integer(Components::RGBA, 8, IntSubType::Normalized) => gl::RGBA8_SNORM,

        Format::Integer(Components::R,    16, IntSubType::Normalized) => gl::R16_SNORM,
        Format::Integer(Components::RG,   16, IntSubType::Normalized) => gl::RG16_SNORM,
        Format::Integer(Components::RGB,  16, IntSubType::Normalized) => gl::RGB16_SNORM,
        Format::Integer(Components::RGBA, 16, IntSubType::Normalized) => gl::RGBA16_SNORM,

        // signed integral
        Format::Integer(Components::R,    8, IntSubType::Raw) => gl::R8I,
        Format::Integer(Components::RG,   8, IntSubType::Raw) => gl::RG8I,
        Format::Integer(Components::RGB,  8, IntSubType::Raw) => gl::RGB8I,
        Format::Integer(Components::RGBA, 8, IntSubType::Raw) => gl::RGBA8I,

        Format::Integer(Components::R,    16, IntSubType::Raw) => gl::R16I,
        Format::Integer(Components::RG,   16, IntSubType::Raw) => gl::RG16I,
        Format::Integer(Components::RGB,  16, IntSubType::Raw) => gl::RGB16I,
        Format::Integer(Components::RGBA, 16, IntSubType::Raw) => gl::RGBA16I,

        Format::Integer(Components::R,    32, IntSubType::Raw) => gl::R32I,
        Format::Integer(Components::RG,   32, IntSubType::Raw) => gl::RG32I,
        Format::Integer(Components::RGB,  32, IntSubType::Raw) => gl::RGB32I,
        Format::Integer(Components::RGBA, 32, IntSubType::Raw) => gl::RGBA32I,

        Format::Integer(_, _, _) => unimplemented!(),

        // unsigned normalized
        Format::Unsigned(Components::RGBA, 2,  IntSubType::Normalized) => gl::RGBA2,

        Format::Unsigned(Components::RGB,  4,  IntSubType::Normalized) => gl::RGB4,
        Format::Unsigned(Components::RGBA, 4,  IntSubType::Normalized) => gl::RGBA4,

        Format::Unsigned(Components::RGB,  5,  IntSubType::Normalized) => gl::RGB5,
        //tex::Unsigned(tex::RGBA, 5, attrib::IntNormalized) => gl::RGBA5,

        Format::Unsigned(Components::R,    8,  IntSubType::Normalized) => gl::R8,
        Format::Unsigned(Components::RG,   8,  IntSubType::Normalized) => gl::RG8,
        Format::Unsigned(Components::RGB,  8,  IntSubType::Normalized) => gl::RGB8,
        Format::Unsigned(Components::RGBA, 8,  IntSubType::Normalized) => gl::RGBA8,

        Format::Unsigned(Components::RGB,  10, IntSubType::Normalized) => gl::RGB10,

        Format::Unsigned(Components::RGB,  12, IntSubType::Normalized) => gl::RGB12,
        Format::Unsigned(Components::RGBA, 12, IntSubType::Normalized) => gl::RGBA12,

        Format::Unsigned(Components::R,    16, IntSubType::Normalized) => gl::R16,
        Format::Unsigned(Components::RG,   16, IntSubType::Normalized) => gl::RG16,
        Format::Unsigned(Components::RGB,  16, IntSubType::Normalized) => gl::RGB16,
        Format::Unsigned(Components::RGBA, 16, IntSubType::Normalized) => gl::RGBA16,

        // unsigned integral
        Format::Unsigned(Components::R,    8,  IntSubType::Raw) => gl::R8UI,
        Format::Unsigned(Components::RG,   8,  IntSubType::Raw) => gl::RG8UI,
        Format::Unsigned(Components::RGB,  8,  IntSubType::Raw) => gl::RGB8UI,
        Format::Unsigned(Components::RGBA, 8,  IntSubType::Raw) => gl::RGBA8UI,

        Format::Unsigned(Components::R,    16, IntSubType::Raw) => gl::R16UI,
        Format::Unsigned(Components::RG,   16, IntSubType::Raw) => gl::RG16UI,
        Format::Unsigned(Components::RGB,  16, IntSubType::Raw) => gl::RGB16UI,
        Format::Unsigned(Components::RGBA, 16, IntSubType::Raw) => gl::RGBA16UI,

        Format::Unsigned(Components::R,    32, IntSubType::Raw) => gl::R32UI,
        Format::Unsigned(Components::RG,   32, IntSubType::Raw) => gl::RG32UI,
        Format::Unsigned(Components::RGB,  32, IntSubType::Raw) => gl::RGB32UI,
        Format::Unsigned(Components::RGBA, 32, IntSubType::Raw) => gl::RGBA32UI,

        Format::Unsigned(_, _, _) => unimplemented!(),
        // special
        Format::Compressed(Compression::ETC2_RGB) => gl::COMPRESSED_RGB8_ETC2,
        Format::Compressed(Compression::ETC2_SRGB) => gl::COMPRESSED_SRGB8_ETC2,
        Format::Compressed(Compression::ETC2_EAC_RGBA8) => gl::COMPRESSED_RGBA8_ETC2_EAC,
        Format::R3_G3_B2          => gl::R3_G3_B2,
        Format::R5_G6_B5          => gl::RGB565,
        Format::RGB5_A1           => gl::RGB5_A1,
        Format::RGB10_A2          => gl::RGB10_A2,
        Format::RGB10_A2UI        => gl::RGB10_A2UI,
        Format::R11F_G11F_B10F    => gl::R11F_G11F_B10F,
        Format::RGB9_E5           => gl::RGB9_E5,
        Format::BGRA8             => gl::RGBA8,
        Format::SRGB8             => gl::SRGB8,
        Format::SRGB8_A8          => gl::SRGB8_ALPHA8,
        Format::DEPTH16           => gl::DEPTH_COMPONENT16,
        Format::DEPTH24           => gl::DEPTH_COMPONENT24,
        Format::DEPTH32F          => gl::DEPTH_COMPONENT32F,
        Format::DEPTH24_STENCIL8  => gl::DEPTH24_STENCIL8,
        Format::DEPTH32F_STENCIL8 => gl::DEPTH32F_STENCIL8,
    })
}

fn components_to_glpixel(c: Components) -> GLenum {
    match c {
        Components::R    => gl::RED,
        Components::RG   => gl::RG,
        Components::RGB  => gl::RGB,
        Components::RGBA => gl::RGBA,
    }
}

fn format_to_glpixel(t: Format) -> GLenum {
    match t {
        Format::Float(c, _)       => components_to_glpixel(c),
        Format::Integer(c, _, _)  => components_to_glpixel(c),
        Format::Unsigned(c, _, _) => components_to_glpixel(c),
        // this is wrong, but it's not used anyway
        Format::Compressed(_)     => {
            error!("Tried to get components of a compressed texel!");
            gl::RGBA
        },
        Format::R3_G3_B2          |
        Format::R5_G6_B5          |
        Format::R11F_G11F_B10F    |
        Format::RGB9_E5           |
        Format::SRGB8             => gl::RGB,
        Format::RGB5_A1           |
        Format::RGB10_A2          |
        Format::RGB10_A2UI        |
        Format::SRGB8_A8          => gl::RGBA,
        Format::BGRA8             => gl::BGRA,
        Format::DEPTH16           |
        Format::DEPTH24           |
        Format::DEPTH32F          => gl::DEPTH_COMPONENT,
        Format::DEPTH24_STENCIL8  |
        Format::DEPTH32F_STENCIL8 => gl::DEPTH_STENCIL,
    }
}

/// This function produces the pixel type for a give internal format.
/// Note that the pixel types are only needed for transfer in/out of the texture data.
/// It is not used for rendering at all.
/// Also note that in OpenGL there are multiple allowed formats of data, while this
/// function only gives you only the most compact representation.
fn format_to_gltype(t: Format) -> Result<GLenum, ()> {
    match t {
        Format::Float(_, FloatSize::F16) => Ok(gl::HALF_FLOAT),
        Format::Float(_, FloatSize::F32) => Ok(gl::FLOAT),
        Format::Unsigned(_, 4, _)  => Ok(gl::UNSIGNED_SHORT_4_4_4_4),
        Format::Integer(_, 8, _)   => Ok(gl::BYTE),
        Format::Unsigned(_, 8, _)  => Ok(gl::UNSIGNED_BYTE),
        Format::Integer(_, 16, _)  => Ok(gl::SHORT),
        Format::Unsigned(_, 16, _) => Ok(gl::UNSIGNED_SHORT),
        Format::Integer(_, 32, _)  => Ok(gl::INT),
        Format::Unsigned(_, 32, _) => Ok(gl::UNSIGNED_INT),
        Format::R3_G3_B2           => Ok(gl::UNSIGNED_BYTE_3_3_2),
        Format::R5_G6_B5           => Ok(gl::UNSIGNED_SHORT_5_6_5),
        Format::R11F_G11F_B10F     => Ok(gl::UNSIGNED_INT_10F_11F_11F_REV),
        Format::RGB9_E5            => Ok(gl::UNSIGNED_INT_5_9_9_9_REV),
        Format::RGB5_A1            => Ok(gl::UNSIGNED_SHORT_5_5_5_1),
        Format::RGB10_A2           |
        Format::RGB10_A2UI         => Ok(gl::UNSIGNED_INT_10_10_10_2),
        Format::SRGB8              |
        Format::SRGB8_A8           |
        Format::BGRA8              => Ok(gl::UNSIGNED_BYTE),
        Format::DEPTH16            => Ok(gl::UNSIGNED_SHORT),
        Format::DEPTH24            => Ok(gl::UNSIGNED_INT),
        Format::DEPTH32F           => Ok(gl::FLOAT),
        Format::DEPTH24_STENCIL8   => Ok(gl::UNSIGNED_INT_24_8),
        Format::DEPTH32F_STENCIL8  => Ok(gl::FLOAT_32_UNSIGNED_INT_24_8_REV),
        _ => Err(()),
    }
}

fn set_mipmap_range(gl: &gl::Gl, target: GLenum, (base, max): (u8, u8)) { unsafe {
    gl.TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
}}

/// Create a render surface.
pub fn make_surface(gl: &gl::Gl, info: &SurfaceInfo) ->
                    Result<Surface, SurfaceError> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(SurfaceError::UnsupportedFormat),
    };

    unsafe {
        gl.BindRenderbuffer(target, name);
    }
    match info.aa_mode {
        None => { unsafe {
            gl.RenderbufferStorage(
                target,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        Some(AaMode::Msaa(samples)) => { unsafe {
            gl.RenderbufferStorageMultisample(
                target,
                samples as GLsizei,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        Some(_) => return Err(SurfaceError::UnsupportedFormat),
    }

    Ok(name)
}

/// Create a texture, assuming TexStorage* isn't available.
pub fn make_without_storage(gl: &gl::Gl, info: &TextureInfo) ->
                            Result<Texture, TextureError> {
    let (name, target) = make_texture(gl, info);

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };
    let pix = format_to_glpixel(info.format);
    let typ = match format_to_gltype(info.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        Kind::D1 => unsafe {
            gl.TexImage1D(
                target,
                0,
                fmt,
                info.width as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        Kind::D1Array => unsafe {
            gl.TexImage2D(
                target,
                0,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        Kind::D2 => unsafe {
            gl.TexImage2D(
                target,
                0,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        Kind::D2MultiSample(AaMode::Msaa(samples)) => unsafe {
            gl.TexImage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,  //GL spec bug
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        },
        Kind::Cube(_) => {
            for &target in [gl::TEXTURE_CUBE_MAP_POSITIVE_X, gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Y, gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Z, gl::TEXTURE_CUBE_MAP_NEGATIVE_Z].iter() {
                unsafe { gl.TexImage2D(
                    target,
                    0,
                    fmt,
                    info.width as GLsizei,
                    info.height as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null()
                )};
            }
        },
        Kind::D2Array | Kind::D3 => unsafe {
            gl.TexImage3D(
                target,
                0,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
                0,
                pix,
                typ,
                ::std::ptr::null()
            );
        },
        Kind::D2MultiSampleArray(AaMode::Msaa(samples)) => unsafe {
            gl.TexImage3DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,  //GL spec bug
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
                fixed_sample_locations
            );
        },
        _ => return Err(TextureError::UnsupportedSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels - 1));

    Ok(name)
}

/// Create a texture, assuming TexStorage is available.
pub fn make_with_storage(gl: &gl::Gl, info: &TextureInfo) ->
                         Result<Texture, TextureError> {
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

    let (name, target) = make_texture(gl, info);

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        Kind::D1 => unsafe {
            gl.TexStorage1D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei
            );
        },
        Kind::D1Array => unsafe {
            gl.TexStorage2D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        },
        Kind::D2 | Kind::Cube(_) => unsafe {
            gl.TexStorage2D(
                // to create storage for a texture cube, we don't do individual faces
                match info.kind {
                    Kind::Cube(_) => gl::TEXTURE_CUBE_MAP,
                    _ => target
                },
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        },
        Kind::D2Array => unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        },
        Kind::D2MultiSample(AaMode::Msaa(samples)) => unsafe {
            gl.TexStorage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        },
        Kind::D2MultiSampleArray(AaMode::Msaa(samples)) => unsafe {
            gl.TexStorage3DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
                fixed_sample_locations
            );
        },
        Kind::D3 => unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level3(info.width, info.height, info.depth)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        },
        _ => return Err(TextureError::UnsupportedSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels - 1));

    Ok(name)
}

/// Bind a texture to the specified slot
pub fn bind_texture(gl: &gl::Gl, slot: GLenum, kind: Kind,
                    name: Texture) -> BindAnchor {
    let target = bind_kind_to_gl(kind);
    unsafe {
        gl.ActiveTexture(slot);
        gl.BindTexture(target, name);
    }
    BindAnchor(target)
}

/// Bind a sampler using a given binding anchor.
/// Used for GL compatibility profile only. The core profile has sampler objects
pub fn bind_sampler(gl: &gl::Gl, anchor: BindAnchor, info: &SamplerInfo) { unsafe {
    let BindAnchor(target) = anchor;
    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        FilterMethod::Anisotropic(fac) =>
            gl.TexParameterf(target, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl.TexParameteri(target, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl.TexParameteri(target, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl.TexParameteri(target, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl.TexParameteri(target, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    gl.TexParameterf(target, gl::TEXTURE_LOD_BIAS, info.lod_bias);

    let (min, max) = info.lod_range;
    gl.TexParameterf(target, gl::TEXTURE_MIN_LOD, min);
    gl.TexParameterf(target, gl::TEXTURE_MAX_LOD, max);

    match info.comparison {
        None => gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        Some(cmp) => {
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }
}}

pub fn update_texture(gl: &gl::Gl, kind: Kind, name: Texture,
                      img: &ImageInfo, address: *const u8, size: usize)
                      -> Result<(), TextureError> {
    if let Some(fmt_size) = img.format.get_size() {
        // TODO: can we compute the expected size for compressed formats?
        let expected_size = img.width as usize * img.height as usize *
                            img.depth as usize * fmt_size as usize;
        if size != expected_size {
            return Err(TextureError::IncorrectSize(expected_size));
        }
    }

    let data = address as *const GLvoid;
    let pix = format_to_glpixel(img.format);
    let typ = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };
    let target = bind_kind_to_gl(kind);

    unsafe { gl.BindTexture(target, name) };

    if img.format.is_compressed() {
        return compressed_update(gl, kind, target, img, data, typ, size as GLint);
    }

    match kind {
        Kind::D1 => unsafe {
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
        Kind::D1Array | Kind::D2 => unsafe {
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
        Kind::Cube(_) => unsafe {
            // get specific face target
            let target = create_kind_to_gl(kind);
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
        Kind::D2Array | Kind::D3 => unsafe {
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
        Kind::D2MultiSample(_) | Kind::D2MultiSampleArray(_) =>
            return Err(TextureError::UnsupportedSampling),
    }

    Ok(())
}

pub fn compressed_update(gl: &gl::Gl, kind: Kind, target: GLenum, img: &ImageInfo,
                         data: *const GLvoid, typ: GLenum, size: GLint)
                         -> Result<(), TextureError> {
    match kind {
        Kind::D1 => unsafe {
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
        Kind::D1Array | Kind::D2 => unsafe {
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
        Kind::Cube(_) => unsafe {
            // get specific face target
            let target = create_kind_to_gl(kind);
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
        Kind::D2Array | Kind::D3 => unsafe {
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
        Kind::D2MultiSample(_) | Kind::D2MultiSampleArray(_) =>
            return Err(TextureError::UnsupportedSampling),
    }

    Ok(())
}
/// Common texture creation routine, just creates and binds.
fn make_texture(gl: &gl::Gl, info: &TextureInfo) -> (Texture, GLuint) {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenTextures(1, &mut name);
    }

    let k = bind_kind_to_gl(info.kind);
    unsafe { gl.BindTexture(k, name) };
    (name, k)
}

fn wrap_to_gl(w: WrapMode) -> GLenum {
    match w {
        WrapMode::Tile   => gl::REPEAT,
        WrapMode::Mirror => gl::MIRRORED_REPEAT,
        WrapMode::Clamp  => gl::CLAMP_TO_EDGE,
    }
}

fn filter_to_gl(f: FilterMethod) -> (GLenum, GLenum) {
    match f {
        FilterMethod::Scale => (gl::NEAREST, gl::NEAREST),
        FilterMethod::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        FilterMethod::Bilinear => (gl::LINEAR, gl::LINEAR),
        FilterMethod::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        FilterMethod::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
}

pub fn make_sampler(gl: &gl::Gl, info: &SamplerInfo) -> Sampler { unsafe {
    let mut name = 0 as Sampler;
    gl.GenSamplers(1, &mut name);

    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        FilterMethod::Anisotropic(fac) =>
            gl.SamplerParameterf(name, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl.SamplerParameteri(name, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl.SamplerParameteri(name, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    gl.SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias);

    let (min, max) = info.lod_range;
    gl.SamplerParameterf(name, gl::TEXTURE_MIN_LOD, min);
    gl.SamplerParameterf(name, gl::TEXTURE_MAX_LOD, max);

    match info.comparison {
        None => gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        Some(cmp) => {
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }

    name
}}

pub fn generate_mipmap(gl: &gl::Gl, kind: Kind, name: Texture) { unsafe {
    //can't fail here, but we need to check for integer formats too
    debug_assert!(kind.get_aa_mode().is_none());
    let target = bind_kind_to_gl(kind);
    gl.BindTexture(target, name);
    gl.GenerateMipmap(target);
}}
