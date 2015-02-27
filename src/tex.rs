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
use gfx::device::tex;
use gfx::device::tex::{SurfaceError, TextureError, FilterMethod, TextureKind,
                       AaMode, WrapMode, ComparisonMode, Components, Format,
                       Compression, CubeFace};
use gfx::device::attrib::{FloatSize, IntSubType};

use std::num::Float;

/// A token produced by the `bind_texture` that allows following up
/// with a GL-compatibility sampler settings in `bind_sampler`
#[derive(Copy)]
pub struct BindAnchor(GLenum);

fn create_kind_to_gl(kind: TextureKind) -> GLenum {
    match kind {
        TextureKind::Texture1D => gl::TEXTURE_1D,
        TextureKind::Texture1DArray => gl::TEXTURE_1D_ARRAY,
        TextureKind::Texture2D => gl::TEXTURE_2D,
        TextureKind::Texture2DArray => gl::TEXTURE_2D_ARRAY,
        TextureKind::Texture2DMultiSample(_) => gl::TEXTURE_2D_MULTISAMPLE,
        TextureKind::Texture2DMultiSampleArray(_) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        TextureKind::TextureCube(CubeFace::PosZ) => gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
        TextureKind::TextureCube(CubeFace::NegZ) => gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        TextureKind::TextureCube(CubeFace::PosX) => gl::TEXTURE_CUBE_MAP_POSITIVE_X,
        TextureKind::TextureCube(CubeFace::NegX) => gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
        TextureKind::TextureCube(CubeFace::PosY) => gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
        TextureKind::TextureCube(CubeFace::NegY) => gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
        TextureKind::Texture3D => gl::TEXTURE_3D,
    }
}

fn bind_kind_to_gl(kind: TextureKind) -> GLenum {
    match kind {
        TextureKind::TextureCube(_) => gl::TEXTURE_CUBE_MAP,
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
        Format::R3G3B2       => gl::R3_G3_B2,
        Format::RGB5A1       => gl::RGB5_A1,
        Format::RGB10A2      => gl::RGB10_A2,
        Format::RGB10A2UI    => gl::RGB10_A2UI,
        Format::R11FG11FB10F => gl::R11F_G11F_B10F,
        Format::RGB9E5       => gl::RGB9_E5,
        Format::BGRA8        => gl::RGBA8,
        Format::DEPTH24STENCIL8 => gl::DEPTH24_STENCIL8,
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

fn components_to_count(c: Components) -> usize {
    match c {
        Components::R    => 1,
        Components::RG   => 2,
        Components::RGB  => 3,
        Components::RGBA => 4,
    }
}

fn format_to_glpixel(t: Format) -> GLenum {
    match t {
        Format::Float(c, _)       => components_to_glpixel(c),
        Format::Integer(c, _, _)  => components_to_glpixel(c),
        Format::Unsigned(c, _, _) => components_to_glpixel(c),
        // this is wrong, but it's not used anyway
        Format::Compressed(_)     => panic!("Tried to get components of a compressed texel!"),
        Format::R3G3B2       => gl::RGB,
        Format::RGB5A1       => gl::RGBA,
        Format::RGB10A2      => gl::RGBA,
        Format::RGB10A2UI    => gl::RGBA,
        Format::R11FG11FB10F => gl::RGB,
        Format::RGB9E5       => gl::RGB,
        Format::BGRA8        => gl::BGRA,
        Format::DEPTH24STENCIL8 => gl::DEPTH_STENCIL,
    }
}

fn format_to_gltype(t: Format) -> Result<GLenum, ()> {
    match t {
        Format::Float(_, FloatSize::F32) => Ok(gl::FLOAT),
        Format::Integer(_, 8, _)   => Ok(gl::BYTE),
        Format::Unsigned(_, 8, _)  => Ok(gl::UNSIGNED_BYTE),
        Format::Integer(_, 16, _)  => Ok(gl::SHORT),
        Format::Unsigned(_, 16, _) => Ok(gl::UNSIGNED_SHORT),
        Format::Integer(_, 32, _)  => Ok(gl::INT),
        Format::Unsigned(_, 32, _) => Ok(gl::UNSIGNED_INT),
        Format::BGRA8              => Ok(gl::UNSIGNED_BYTE),
        Format::DEPTH24STENCIL8    => Ok(gl::UNSIGNED_INT_24_8),
        _ => Err(()),
    }
}

fn format_to_size(t: tex::Format) -> usize {
    match t {
        Format::Float(c, FloatSize::F16) => 2 * components_to_count(c),
        Format::Float(c, FloatSize::F32) => 4 * components_to_count(c),
        Format::Float(c, FloatSize::F64) => 8 * components_to_count(c),
        Format::Integer(c, bits, _)  => bits as usize * components_to_count(c) >> 3,
        Format::Unsigned(c, bits, _) => bits as usize * components_to_count(c) >> 3,
        Format::Compressed(_) => panic!("Tried to get size of a compressed texel!"),
        Format::R3G3B2       => 1,
        Format::RGB5A1       => 2,
        Format::RGB10A2      => 4,
        Format::RGB10A2UI    => 4,
        Format::R11FG11FB10F => 4,
        Format::RGB9E5       => 4,
        Format::BGRA8        => 4,
        Format::DEPTH24STENCIL8 => 4,
    }
}

fn set_mipmap_range(gl: &gl::Gl, target: GLenum, (base, max): (u8, u8)) { unsafe {
    gl.TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
}}

/// Create a render surface.
pub fn make_surface(gl: &gl::Gl, info: &tex::SurfaceInfo) ->
                    Result<Surface, SurfaceError> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(SurfaceError::UnsupportedSurfaceFormat),
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
        Some(_) => return Err(SurfaceError::UnsupportedSurfaceFormat),
    }

    Ok(name)
}

/// Create a texture, assuming TexStorage* isn't available.
pub fn make_without_storage(gl: &gl::Gl, info: &tex::TextureInfo) ->
                            Result<Texture, tex::TextureError> {
    let (name, target) = make_texture(gl, info);

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(TextureError::UnsupportedTextureFormat),
    };
    let pix = format_to_glpixel(info.format);
    let typ = match format_to_gltype(info.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedTextureFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        TextureKind::Texture1D => unsafe {
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
        TextureKind::Texture1DArray => unsafe {
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
        TextureKind::Texture2D => unsafe {
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
        TextureKind::Texture2DMultiSample(AaMode::Msaa(samples)) => { unsafe {
            gl.TexImage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,  //GL spec bug
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        }},
        TextureKind::TextureCube(_) =>
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
            },
        TextureKind::Texture2DArray | TextureKind::Texture3D => unsafe {
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
        TextureKind::Texture2DMultiSampleArray(AaMode::Msaa(samples)) => { unsafe {
            gl.TexImage3DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,  //GL spec bug
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
                fixed_sample_locations
            );
        }},
        _ => return Err(TextureError::UnsupportedTextureSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels));

    Ok(name)
}

/// Create a texture, assuming TexStorage is available.
pub fn make_with_storage(gl: &gl::Gl, info: &tex::TextureInfo) ->
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
        Err(_) => return Err(TextureError::UnsupportedTextureFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        TextureKind::Texture1D => { unsafe {
            gl.TexStorage1D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei
            );
        }},
        TextureKind::Texture1DArray => { unsafe {
            gl.TexStorage2D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        TextureKind::Texture2D | TextureKind::TextureCube(_) => { unsafe {
            gl.TexStorage2D(
                // to create storage for a texture cube, we don't do individual faces
                match info.kind {
                    TextureKind::TextureCube(_) => gl::TEXTURE_CUBE_MAP,
                    _ => target
                },
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        TextureKind::Texture2DArray => { unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        }},
        TextureKind::Texture2DMultiSample(AaMode::Msaa(samples)) => { unsafe {
            gl.TexStorage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        }},
        TextureKind::Texture2DMultiSampleArray(AaMode::Msaa(samples)) => { unsafe {
            gl.TexStorage3DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
                fixed_sample_locations
            );
        }},
        TextureKind::Texture3D => { unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level3(info.width, info.height, info.depth)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        }},
        _ => return Err(TextureError::UnsupportedTextureSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels));

    Ok(name)
}

/// Bind a texture to the specified slot
pub fn bind_texture(gl: &gl::Gl, slot: GLenum, kind: TextureKind,
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
pub fn bind_sampler(gl: &gl::Gl, anchor: BindAnchor, info: &tex::SamplerInfo) { unsafe {
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
        ComparisonMode::NoComparison => gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        ComparisonMode::CompareRefToTexture(cmp) => {
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }
}}

pub fn update_texture(gl: &gl::Gl, kind: TextureKind, name: Texture,
                      img: &tex::ImageInfo, address: *const u8, size: usize)
                      -> Result<(), TextureError> {
    if !img.format.is_compressed() {
        // TODO: can we compute the expected size for compressed formats?
        let expected_size = img.width as usize * img.height as usize *
                            img.depth as usize * format_to_size(img.format);
        if size != expected_size {
            return Err(TextureError::IncorrectTextureSize(expected_size));
        }
    }

    let data = address as *const GLvoid;
    let pix = format_to_glpixel(img.format);
    let typ = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedTextureFormat),
    };
    let target = bind_kind_to_gl(kind);

    unsafe { gl.BindTexture(target, name) };

    if img.format.is_compressed() {
        return compressed_update(gl, kind, target, img, data, typ, size as GLint);
    }

    unsafe {
        match kind {
            TextureKind::Texture1D => {
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
            TextureKind::Texture1DArray | TextureKind::Texture2D => {
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
            TextureKind::TextureCube(_) => {
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
            TextureKind::Texture2DArray | TextureKind::Texture3D => {
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
            TextureKind::Texture2DMultiSample(_) | TextureKind::Texture2DMultiSampleArray(_) =>
                return Err(TextureError::UnsupportedTextureSampling),
        }
    }

    Ok(())
}

pub fn compressed_update(gl: &gl::Gl, kind: TextureKind, target: GLenum, img: &tex::ImageInfo,
                         data: *const GLvoid, typ: GLenum, size: GLint)
                         -> Result<(), TextureError> {
    unsafe {
        match kind {
            TextureKind::Texture1D => {
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
            TextureKind::Texture1DArray | TextureKind::Texture2D => {
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
            TextureKind::TextureCube(_) => {
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
            TextureKind::Texture2DArray | TextureKind::Texture3D => {
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
            TextureKind::Texture2DMultiSample(_) | TextureKind::Texture2DMultiSampleArray(_) =>
                return Err(TextureError::UnsupportedTextureSampling),
        }
    }

    Ok(())
}
/// Common texture creation routine, just creates and binds.
fn make_texture(gl: &gl::Gl, info: &tex::TextureInfo) -> (Texture, GLuint) {
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

pub fn make_sampler(gl: &gl::Gl, info: &tex::SamplerInfo) -> Sampler { unsafe {
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
        ComparisonMode::NoComparison => gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        ComparisonMode::CompareRefToTexture(cmp) => {
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }

    name
}}

pub fn generate_mipmap(gl: &gl::Gl, kind: tex::TextureKind, name: Texture) { unsafe {
    //can't fail here, but we need to check for integer formats too
    debug_assert!(kind.get_aa_mode().is_none());
    let target = bind_kind_to_gl(kind);
    gl.BindTexture(target, name);
    gl.GenerateMipmap(target);
}}
