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
use Blob;

/// A token produced by the `bind_texture` that allows following up
/// with a GL-compatibility sampler settings in `bind_sampler`
pub struct BindAnchor(GLenum);

fn kind_to_gl(t: ::tex::TextureKind) -> GLenum {
    match t {
        ::tex::Texture1D => gl::TEXTURE_1D,
        ::tex::Texture1DArray => gl::TEXTURE_1D_ARRAY,
        ::tex::Texture2D => gl::TEXTURE_2D,
        ::tex::Texture2DArray => gl::TEXTURE_2D_ARRAY,
        ::tex::TextureCube => gl::TEXTURE_CUBE_MAP,
        ::tex::Texture3D => gl::TEXTURE_3D,
    }
}

fn format_to_gl(t: ::tex::Format) -> Result<GLenum, ()> {
    Ok(match t {
        // floating-point
        ::tex::Float(::tex::R,    ::attrib::F16) => gl::R16F,
        ::tex::Float(::tex::R,    ::attrib::F32) => gl::R32F,
        ::tex::Float(::tex::RG,   ::attrib::F16) => gl::RG16F,
        ::tex::Float(::tex::RG,   ::attrib::F32) => gl::RG32F,
        ::tex::Float(::tex::RGB,  ::attrib::F16) => gl::RGB16F,
        ::tex::Float(::tex::RGB,  ::attrib::F32) => gl::RGB32F,
        ::tex::Float(::tex::RGBA, ::attrib::F16) => gl::RGBA16F,
        ::tex::Float(::tex::RGBA, ::attrib::F32) => gl::RGBA32F,
        ::tex::Float(_, ::attrib::F64) => return Err(()),
        // integer
        ::tex::Integer(_, _, _) => unimplemented!(),
        // unsigned integer
        ::tex::Unsigned(::tex::RGBA, 8, ::attrib::IntNormalized) => gl::RGBA8,
        ::tex::Unsigned(_, _, _) => unimplemented!(),
        // special
        ::tex::R3G3B2       => gl::R3_G3_B2,
        ::tex::RGB5A1       => gl::RGB5_A1,
        ::tex::RGB10A2      => gl::RGB10_A2,
        ::tex::RGB10A2UI    => gl::RGB10_A2UI,
        ::tex::R11FG11FB10F => gl::R11F_G11F_B10F,
        ::tex::RGB9E5       => gl::RGB9_E5,
    })
}

fn components_to_glpixel(c: ::tex::Components) -> GLenum {
    match c {
        ::tex::R    => gl::RED,
        ::tex::RG   => gl::RG,
        ::tex::RGB  => gl::RGB,
        ::tex::RGBA => gl::RGBA,
    }
}

fn components_to_count(c: ::tex::Components) -> uint {
    match c {
        ::tex::R    => 1,
        ::tex::RG   => 2,
        ::tex::RGB  => 3,
        ::tex::RGBA => 4,
    }
}

fn format_to_glpixel(t: ::tex::Format) -> GLenum {
    match t {
        ::tex::Float(c, _)       => components_to_glpixel(c),
        ::tex::Integer(c, _, _)  => components_to_glpixel(c),
        ::tex::Unsigned(c, _, _) => components_to_glpixel(c),
        ::tex::R3G3B2       => gl::RGB,
        ::tex::RGB5A1       => gl::RGBA,
        ::tex::RGB10A2      => gl::RGBA,
        ::tex::RGB10A2UI    => gl::RGBA,
        ::tex::R11FG11FB10F => gl::RGB,
        ::tex::RGB9E5       => gl::RGB,
    }
}

fn format_to_gltype(t: ::tex::Format) -> Result<GLenum, ()> {
    match t {
        ::tex::Float(_, ::attrib::F32) => Ok(gl::FLOAT),
        ::tex::Integer(_, 8, _)   => Ok(gl::BYTE),
        ::tex::Unsigned(_, 8, _)  => Ok(gl::UNSIGNED_BYTE),
        ::tex::Integer(_, 16, _)  => Ok(gl::SHORT),
        ::tex::Unsigned(_, 16, _) => Ok(gl::UNSIGNED_SHORT),
        ::tex::Integer(_, 32, _)  => Ok(gl::INT),
        ::tex::Unsigned(_, 32, _) => Ok(gl::UNSIGNED_INT),
        _ => Err(()),
    }
}

fn format_to_size(t: ::tex::Format) -> uint {
    match t {
        ::tex::Float(c, ::attrib::F16) => 2 * components_to_count(c),
        ::tex::Float(c, ::attrib::F32) => 4 * components_to_count(c),
        ::tex::Float(c, ::attrib::F64) => 8 * components_to_count(c),
        ::tex::Integer(c, bits, _)  => bits as uint * components_to_count(c) >> 3,
        ::tex::Unsigned(c, bits, _) => bits as uint * components_to_count(c) >> 3,
        ::tex::R3G3B2       => 1,
        ::tex::RGB5A1       => 2,
        ::tex::RGB10A2      => 4,
        ::tex::RGB10A2UI    => 4,
        ::tex::R11FG11FB10F => 4,
        ::tex::RGB9E5       => 4,
    }
}

fn set_mipmap_range(target: GLenum, (base, max): (u8, u8)) {
    gl::TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl::TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
}

/// Create a render surface.
pub fn make_surface(info: &::tex::SurfaceInfo) -> Result<Surface, ::SurfaceError> {
    let mut name = 0 as GLuint;
    unsafe {
        gl::GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(::UnsupportedSurfaceFormat),
    };

    gl::BindRenderbuffer(target, name);
    gl::RenderbufferStorage(
        target,
        fmt,
        info.width as GLsizei,
        info.height as GLsizei,
    );

    Ok(name)
}

/// Create a texture, assuming TexStorage* isn't available.
pub fn make_without_storage(info: &::tex::TextureInfo) -> Result<Texture, ::TextureError> {
    let name = make_texture(info);

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(::UnsupportedTextureFormat),
    };
    let pix = format_to_glpixel(info.format);
    let typ = match format_to_gltype(info.format) {
        Ok(t) => t,
        Err(_) => return Err(::UnsupportedTextureFormat),
    };
    let target = kind_to_gl(info.kind);

    unsafe {
        match info.kind {
            ::tex::Texture1D => {
                gl::TexImage1D(
                    target,
                    0,
                    fmt,
                    info.width as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null(),
                );
            },
            ::tex::Texture1DArray => {
                gl::TexImage2D(
                    target,
                    0,
                    fmt,
                    info.width as GLsizei,
                    info.height as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null(),
                );
            },
            ::tex::Texture2D => {
                gl::TexImage2D(
                    target,
                    0,
                    fmt,
                    info.width as GLsizei,
                    info.height as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null(),
                );
            },
            ::tex::TextureCube => unimplemented!(),
            ::tex::Texture2DArray | ::tex::Texture3D => {
                gl::TexImage3D(
                    target,
                    0,
                    fmt,
                    info.width as GLsizei,
                    info.height as GLsizei,
                    info.depth as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null(),
                );
            },
        }
    }

    set_mipmap_range(target, info.mipmap_range);

    Ok(name)
}

/// Create a texture, assuming TexStorage is available.
pub fn make_with_storage(info: &::tex::TextureInfo) -> Result<Texture, ::TextureError> {
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

    let name = make_texture(info);

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(::UnsupportedTextureFormat),
    };
    let target = kind_to_gl(info.kind);

    match info.kind {
        ::tex::Texture1D => {
            gl::TexStorage1D(
                target,
                min(info.mipmap_range.val1(), mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
            );
        },
        ::tex::Texture1DArray => {
            gl::TexStorage2D(
                target,
                min(info.mipmap_range.val1(), mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
            );
        },
        ::tex::Texture2D => {
            gl::TexStorage2D(
                target,
                min(info.mipmap_range.val1(), mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
            );
        },
        ::tex::TextureCube => unimplemented!(),
        ::tex::Texture2DArray => {
            gl::TexStorage3D(
                target,
                min(info.mipmap_range.val1(), mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
            );
        },
        ::tex::Texture3D => {
            gl::TexStorage3D(
                target,
                min(info.mipmap_range.val1(), mip_level3(info.width, info.height, info.depth)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
            );
        },
    }

    set_mipmap_range(target, info.mipmap_range);

    Ok(name)
}

/// Bind a texture to the specified slot
pub fn bind_texture(slot: GLenum, kind: ::tex::TextureKind, name: Texture) -> BindAnchor {
    let target = kind_to_gl(kind);
    gl::ActiveTexture(slot);
    gl::BindTexture(target, name);
    BindAnchor(target)
}

/// Bind a sampler using a given binding anchor.
/// Used for GL compatibility profile only. The core profile has sampler objects
pub fn bind_sampler(anchor: BindAnchor, info: &::tex::SamplerInfo) {
    let BindAnchor(target) = anchor;
    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        ::tex::Anisotropic(fac) =>
            gl::TexParameterf(target, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl::TexParameteri(target, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl::TexParameteri(target, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl::TexParameteri(target, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl::TexParameteri(target, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl::TexParameteri(target, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    gl::TexParameterf(target, gl::TEXTURE_LOD_BIAS, info.lod_bias);

    let (min, max) = info.lod_range;
    gl::TexParameterf(target, gl::TEXTURE_MIN_LOD, min);
    gl::TexParameterf(target, gl::TEXTURE_MAX_LOD, max);
}

pub fn update_texture(kind: ::tex::TextureKind, name: Texture, img: &::tex::ImageInfo,
                      data: Box<Blob + Send>) -> Result<(), ::TextureError> {
    debug_assert!(img.width as uint * img.height as uint * img.depth as uint *
        format_to_size(img.format) == data.get_size());

    let data = data.get_address() as *const GLvoid;
    let pix = format_to_glpixel(img.format);
    let typ = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(::UnsupportedTextureFormat),
    };
    let target = kind_to_gl(kind);

    gl::BindTexture(target, name);

    unsafe {
        match kind {
            ::tex::Texture1D => {
                gl::TexSubImage1D(
                    target,
                    img.mipmap as GLint,
                    img.xoffset as GLint,
                    img.width as GLint,
                    pix,
                    typ,
                    data,
                );
            },
            ::tex::Texture1DArray | ::tex::Texture2D => {
                gl::TexSubImage2D(
                    target,
                    img.mipmap as GLint,
                    img.xoffset as GLint,
                    img.yoffset as GLint,
                    img.width as GLint,
                    img.height as GLint,
                    pix,
                    typ,
                    data,
                );
            },
            ::tex::TextureCube => unimplemented!(),
            ::tex::Texture2DArray | ::tex::Texture3D => {
                gl::TexSubImage3D(
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
                    data,
                );
            },
        }
    }

    Ok(())
}

/// Common texture creation routine, just creates and binds.
fn make_texture(info: &::tex::TextureInfo) -> Texture {
    let mut name = 0 as GLuint;
    unsafe {
        gl::GenTextures(1, &mut name);
    }

    let k = kind_to_gl(info.kind);
    gl::BindTexture(k, name);

    name
}

fn wrap_to_gl(w: ::tex::WrapMode) -> GLenum {
    match w {
        ::tex::Tile   => gl::REPEAT,
        ::tex::Mirror => gl::MIRRORED_REPEAT,
        ::tex::Clamp  => gl::CLAMP_TO_EDGE,
    }
}

fn filter_to_gl(f: ::tex::FilterMethod) -> (GLenum, GLenum) {
    match f {
        ::tex::Scale => (gl::NEAREST, gl::NEAREST),
        ::tex::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        ::tex::Bilinear => (gl::LINEAR, gl::LINEAR),
        ::tex::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        ::tex::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
}

pub fn make_sampler(info: &::tex::SamplerInfo) -> Sampler {
    let mut name = 0 as Sampler;
    unsafe {
        gl::GenSamplers(1, &mut name);
    }

    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        ::tex::Anisotropic(fac) =>
            gl::SamplerParameterf(name, gl::TEXTURE_MAX_ANISOTROPY_EXT, fac as GLfloat),
        _ => ()
    }

    gl::SamplerParameteri(name, gl::TEXTURE_MIN_FILTER, min as GLint);
    gl::SamplerParameteri(name, gl::TEXTURE_MAG_FILTER, mag as GLint);

    let (s, t, r) = info.wrap_mode;
    gl::SamplerParameteri(name, gl::TEXTURE_WRAP_S, wrap_to_gl(s) as GLint);
    gl::SamplerParameteri(name, gl::TEXTURE_WRAP_T, wrap_to_gl(t) as GLint);
    gl::SamplerParameteri(name, gl::TEXTURE_WRAP_R, wrap_to_gl(r) as GLint);

    gl::SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias);

    let (min, max) = info.lod_range;
    gl::SamplerParameterf(name, gl::TEXTURE_MIN_LOD, min);
    gl::SamplerParameterf(name, gl::TEXTURE_MAX_LOD, max);

    name
}
