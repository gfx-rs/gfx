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
use tex;
use attrib;

/// A token produced by the `bind_texture` that allows following up
/// with a GL-compatibility sampler settings in `bind_sampler`
pub struct BindAnchor(GLenum);

fn kind_to_gl(kind: tex::TextureKind)
              -> Result<GLenum, ()> {
    Ok(match kind {
        tex::Texture1D => gl::TEXTURE_1D,
        tex::Texture1DArray => gl::TEXTURE_1D_ARRAY,
        tex::Texture2D => gl::TEXTURE_2D,
        tex::Texture2DArray => gl::TEXTURE_2D_ARRAY,
        tex::Texture2DMultiSample(_) => gl::TEXTURE_2D_MULTISAMPLE,
        tex::Texture2DMultiSampleArray(_) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        tex::TextureCube => gl::TEXTURE_CUBE_MAP,
        tex::Texture3D => gl::TEXTURE_3D,
    })
}

fn format_to_gl(t: tex::Format) -> Result<GLenum, ()> {
    Ok(match t {
        // floating-point
        tex::Float(tex::R,    attrib::F16) => gl::R16F,
        tex::Float(tex::R,    attrib::F32) => gl::R32F,
        tex::Float(tex::RG,   attrib::F16) => gl::RG16F,
        tex::Float(tex::RG,   attrib::F32) => gl::RG32F,
        tex::Float(tex::RGB,  attrib::F16) => gl::RGB16F,
        tex::Float(tex::RGB,  attrib::F32) => gl::RGB32F,
        tex::Float(tex::RGBA, attrib::F16) => gl::RGBA16F,
        tex::Float(tex::RGBA, attrib::F32) => gl::RGBA32F,
        tex::Float(_, attrib::F64) => return Err(()),

        // signed normalized
        tex::Integer(tex::R, 8, attrib::IntNormalized) => gl::R8_SNORM,
        tex::Integer(tex::RG, 8, attrib::IntNormalized) => gl::RG8_SNORM,
        tex::Integer(tex::RGB, 8, attrib::IntNormalized) => gl::RGB8_SNORM,
        tex::Integer(tex::RGBA, 8, attrib::IntNormalized) => gl::RGBA8_SNORM,

        tex::Integer(tex::R, 16, attrib::IntNormalized) => gl::R16_SNORM,
        tex::Integer(tex::RG, 16, attrib::IntNormalized) => gl::RG16_SNORM,
        tex::Integer(tex::RGB, 16, attrib::IntNormalized) => gl::RGB16_SNORM,
        tex::Integer(tex::RGBA, 16, attrib::IntNormalized) => gl::RGBA16_SNORM,

        // signed integral
        tex::Integer(tex::R, 8, attrib::IntRaw) => gl::R8I,
        tex::Integer(tex::RG, 8, attrib::IntRaw) => gl::RG8I,
        tex::Integer(tex::RGB, 8, attrib::IntRaw) => gl::RGB8I,
        tex::Integer(tex::RGBA, 8, attrib::IntRaw) => gl::RGBA8I,

        tex::Integer(tex::R, 16, attrib::IntRaw) => gl::R16I,
        tex::Integer(tex::RG, 16, attrib::IntRaw) => gl::RG16I,
        tex::Integer(tex::RGB, 16, attrib::IntRaw) => gl::RGB16I,
        tex::Integer(tex::RGBA, 16, attrib::IntRaw) => gl::RGBA16I,

        tex::Integer(tex::R, 32, attrib::IntRaw) => gl::R32I,
        tex::Integer(tex::RG, 32, attrib::IntRaw) => gl::RG32I,
        tex::Integer(tex::RGB, 32, attrib::IntRaw) => gl::RGB32I,
        tex::Integer(tex::RGBA, 32, attrib::IntRaw) => gl::RGBA32I,

        tex::Integer(_, _, _) => unimplemented!(),

        // unsigned normalized
        tex::Unsigned(tex::RGBA, 2, attrib::IntNormalized) => gl::RGBA2,

        tex::Unsigned(tex::RGB, 4, attrib::IntNormalized) => gl::RGB4,
        tex::Unsigned(tex::RGBA, 4, attrib::IntNormalized) => gl::RGBA4,

        tex::Unsigned(tex::RGB, 5, attrib::IntNormalized) => gl::RGB5,
        //tex::Unsigned(tex::RGBA, 5, attrib::IntNormalized) => gl::RGBA5,

        tex::Unsigned(tex::R, 8, attrib::IntNormalized) => gl::R8,
        tex::Unsigned(tex::RG, 8, attrib::IntNormalized) => gl::RG8,
        tex::Unsigned(tex::RGB, 8, attrib::IntNormalized) => gl::RGB8,
        tex::Unsigned(tex::RGBA, 8, attrib::IntNormalized) => gl::RGBA8,

        tex::Unsigned(tex::RGB, 10, attrib::IntNormalized) => gl::RGB10,

        tex::Unsigned(tex::RGB, 12, attrib::IntNormalized) => gl::RGB12,
        tex::Unsigned(tex::RGBA, 12, attrib::IntNormalized) => gl::RGBA12,

        tex::Unsigned(tex::R, 16, attrib::IntNormalized) => gl::R16,
        tex::Unsigned(tex::RG, 16, attrib::IntNormalized) => gl::RG16,
        tex::Unsigned(tex::RGB, 16, attrib::IntNormalized) => gl::RGB16,
        tex::Unsigned(tex::RGBA, 16, attrib::IntNormalized) => gl::RGBA16,

        // unsigned integral
        tex::Unsigned(tex::R, 8, attrib::IntRaw) => gl::R8UI,
        tex::Unsigned(tex::RG, 8, attrib::IntRaw) => gl::RG8UI,
        tex::Unsigned(tex::RGB, 8, attrib::IntRaw) => gl::RGB8UI,
        tex::Unsigned(tex::RGBA, 8, attrib::IntRaw) => gl::RGBA8UI,

        tex::Unsigned(tex::R, 16, attrib::IntRaw) => gl::R16UI,
        tex::Unsigned(tex::RG, 16, attrib::IntRaw) => gl::RG16UI,
        tex::Unsigned(tex::RGB, 16, attrib::IntRaw) => gl::RGB16UI,
        tex::Unsigned(tex::RGBA, 16, attrib::IntRaw) => gl::RGBA16UI,

        tex::Unsigned(tex::R, 32, attrib::IntRaw) => gl::R32UI,
        tex::Unsigned(tex::RG, 32, attrib::IntRaw) => gl::RG32UI,
        tex::Unsigned(tex::RGB, 32, attrib::IntRaw) => gl::RGB32UI,
        tex::Unsigned(tex::RGBA, 32, attrib::IntRaw) => gl::RGBA32UI,

        tex::Unsigned(_, _, _) => unimplemented!(),
        // special
        tex::R3G3B2       => gl::R3_G3_B2,
        tex::RGB5A1       => gl::RGB5_A1,
        tex::RGB10A2      => gl::RGB10_A2,
        tex::RGB10A2UI    => gl::RGB10_A2UI,
        tex::R11FG11FB10F => gl::R11F_G11F_B10F,
        tex::RGB9E5       => gl::RGB9_E5,
        tex::DEPTH24STENCIL8 => gl::DEPTH24_STENCIL8,
    })
}

fn components_to_glpixel(c: tex::Components) -> GLenum {
    match c {
        tex::R    => gl::RED,
        tex::RG   => gl::RG,
        tex::RGB  => gl::RGB,
        tex::RGBA => gl::RGBA,
    }
}

fn components_to_count(c: tex::Components) -> uint {
    match c {
        tex::R    => 1,
        tex::RG   => 2,
        tex::RGB  => 3,
        tex::RGBA => 4,
    }
}

fn format_to_glpixel(t: tex::Format) -> GLenum {
    match t {
        tex::Float(c, _)       => components_to_glpixel(c),
        tex::Integer(c, _, _)  => components_to_glpixel(c),
        tex::Unsigned(c, _, _) => components_to_glpixel(c),
        tex::R3G3B2       => gl::RGB,
        tex::RGB5A1       => gl::RGBA,
        tex::RGB10A2      => gl::RGBA,
        tex::RGB10A2UI    => gl::RGBA,
        tex::R11FG11FB10F => gl::RGB,
        tex::RGB9E5       => gl::RGB,
        tex::DEPTH24STENCIL8 => gl::DEPTH_STENCIL,
    }
}

fn format_to_gltype(t: tex::Format) -> Result<GLenum, ()> {
    match t {
        tex::Float(_, attrib::F32) => Ok(gl::FLOAT),
        tex::Integer(_, 8, _)   => Ok(gl::BYTE),
        tex::Unsigned(_, 8, _)  => Ok(gl::UNSIGNED_BYTE),
        tex::Integer(_, 16, _)  => Ok(gl::SHORT),
        tex::Unsigned(_, 16, _) => Ok(gl::UNSIGNED_SHORT),
        tex::Integer(_, 32, _)  => Ok(gl::INT),
        tex::Unsigned(_, 32, _) => Ok(gl::UNSIGNED_INT),
        tex::DEPTH24STENCIL8   => Ok(gl::UNSIGNED_INT_24_8),
        _ => Err(()),
    }
}

fn format_to_size(t: tex::Format) -> uint {
    match t {
        tex::Float(c, attrib::F16) => 2 * components_to_count(c),
        tex::Float(c, attrib::F32) => 4 * components_to_count(c),
        tex::Float(c, attrib::F64) => 8 * components_to_count(c),
        tex::Integer(c, bits, _)  => bits as uint * components_to_count(c) >> 3,
        tex::Unsigned(c, bits, _) => bits as uint * components_to_count(c) >> 3,
        tex::R3G3B2       => 1,
        tex::RGB5A1       => 2,
        tex::RGB10A2      => 4,
        tex::RGB10A2UI    => 4,
        tex::R11FG11FB10F => 4,
        tex::RGB9E5       => 4,
        tex::DEPTH24STENCIL8 => 4,
    }
}

fn set_mipmap_range(gl: &gl::Gl, target: GLenum, (base, max): (u8, u8)) { unsafe {
    gl.TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl.TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
}}

/// Create a render surface.
pub fn make_surface(gl: &gl::Gl, info: &tex::SurfaceInfo) ->
                    Result<Surface, tex::SurfaceError> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(tex::UnsupportedSurfaceFormat),
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
        Some(tex::Msaa(samples)) => { unsafe {
            gl.RenderbufferStorageMultisample(
                target,
                samples as GLsizei,
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        Some(_) => return Err(tex::UnsupportedSurfaceFormat),
    }

    Ok(name)
}

/// Create a texture, assuming TexStorage* isn't available.
pub fn make_without_storage(gl: &gl::Gl, info: &tex::TextureInfo) ->
                            Result<Texture, tex::TextureError> {
    let (name, target) = match make_texture(gl, info) {
        Ok((n, t)) => (n, t),
        Err(_) => return Err(tex::UnsupportedTextureSampling),
    };

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(tex::UnsupportedTextureFormat),
    };
    let pix = format_to_glpixel(info.format);
    let typ = match format_to_gltype(info.format) {
        Ok(t) => t,
        Err(_) => return Err(tex::UnsupportedTextureFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        tex::Texture1D => unsafe {
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
        tex::Texture1DArray => unsafe {
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
        tex::Texture2D => unsafe {
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
        tex::Texture2DMultiSample(tex::Msaa(samples)) => { unsafe {
            gl.TexImage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,  //GL spec bug
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        }},
        tex::TextureCube => unimplemented!(),
        tex::Texture2DArray | tex::Texture3D => unsafe {
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
        tex::Texture2DMultiSampleArray(tex::Msaa(samples)) => { unsafe {
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
        _ => return Err(tex::UnsupportedTextureSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels));

    Ok(name)
}

/// Create a texture, assuming TexStorage is available.
pub fn make_with_storage(gl: &gl::Gl, info: &tex::TextureInfo) ->
                         Result<Texture, tex::TextureError> {
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

    let (name, target) = match make_texture(gl, info) {
        Ok((n, t)) => (n, t),
        Err(_) => return Err(tex::UnsupportedTextureSampling),
    };

    let fmt = match format_to_gl(info.format) {
        Ok(f) => f,
        Err(_) => return Err(tex::UnsupportedTextureFormat),
    };

    // since it's a texture, we want to read from it
    let fixed_sample_locations = gl::TRUE;

    match info.kind {
        tex::Texture1D => { unsafe {
            gl.TexStorage1D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei
            );
        }},
        tex::Texture1DArray => { unsafe {
            gl.TexStorage2D(
                target,
                min(info.levels, mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        tex::Texture2D => { unsafe {
            gl.TexStorage2D(
                target,
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei
            );
        }},
        tex::Texture2DArray => { unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        }},
        tex::Texture2DMultiSample(tex::Msaa(samples)) => { unsafe {
            gl.TexStorage2DMultisample(
                target,
                samples as GLsizei,
                fmt as GLenum,
                info.width as GLsizei,
                info.height as GLsizei,
                fixed_sample_locations
            );
        }},
        tex::Texture2DMultiSampleArray(tex::Msaa(samples)) => { unsafe {
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
        tex::TextureCube => unimplemented!(),
        tex::Texture3D => { unsafe {
            gl.TexStorage3D(
                target,
                min(info.levels, mip_level3(info.width, info.height, info.depth)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei
            );
        }},
        _ => return Err(tex::UnsupportedTextureSampling),
    }

    set_mipmap_range(gl, target, (0, info.levels));

    Ok(name)
}

/// Bind a texture to the specified slot
pub fn bind_texture(gl: &gl::Gl, slot: GLenum, kind: tex::TextureKind,
                    name: Texture) -> Result<BindAnchor, tex::TextureError> {
    match kind_to_gl(kind) {
        Ok(target) => { unsafe {
            gl.ActiveTexture(slot);
            gl.BindTexture(target, name);
            Ok(BindAnchor(target))
        }},
        Err(_) => Err(tex::UnsupportedTextureSampling),
    }

}

/// Bind a sampler using a given binding anchor.
/// Used for GL compatibility profile only. The core profile has sampler objects
pub fn bind_sampler(gl: &gl::Gl, anchor: BindAnchor, info: &tex::SamplerInfo) { unsafe {
    let BindAnchor(target) = anchor;
    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        tex::Anisotropic(fac) =>
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
        tex::NoComparsion => gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        tex::CompareRefToTexture(cmp) => {
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.TexParameteri(target, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }
}}

pub fn update_texture(gl: &gl::Gl, kind: tex::TextureKind, name: Texture,
                      img: &tex::ImageInfo, address: *const u8, size: uint)
                      -> Result<(), tex::TextureError> {
    let expected_size = img.width as uint * img.height as uint *
                        img.depth as uint * format_to_size(img.format);
    if size != expected_size {
        return Err(tex::IncorrectTextureSize(expected_size));
    }

    let data = address as *const GLvoid;
    let pix = format_to_glpixel(img.format);
    let typ = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(tex::UnsupportedTextureFormat),
    };
    let target = match kind_to_gl(kind) {
        Ok(t) => t,
        Err(_) => return Err(tex::UnsupportedTextureSampling),
    };

    unsafe { gl.BindTexture(target, name) };

    unsafe {
        match kind {
            tex::Texture1D => {
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
            tex::Texture1DArray | tex::Texture2D => {
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
            tex::TextureCube => unimplemented!(),
            tex::Texture2DArray | tex::Texture3D => {
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
            tex::Texture2DMultiSample(_) | tex::Texture2DMultiSampleArray(_) =>
                return Err(tex::UnsupportedTextureSampling),
        }
    }

    Ok(())
}

/// Common texture creation routine, just creates and binds.
fn make_texture(gl: &gl::Gl, info: &tex::TextureInfo)
                -> Result<(Texture, GLuint), ()> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenTextures(1, &mut name);
    }

    kind_to_gl(info.kind).map(|k| {
        unsafe { gl.BindTexture(k, name) };
        (name, k)
    })
}

fn wrap_to_gl(w: tex::WrapMode) -> GLenum {
    match w {
        tex::Tile   => gl::REPEAT,
        tex::Mirror => gl::MIRRORED_REPEAT,
        tex::Clamp  => gl::CLAMP_TO_EDGE,
    }
}

fn filter_to_gl(f: tex::FilterMethod) -> (GLenum, GLenum) {
    match f {
        tex::Scale => (gl::NEAREST, gl::NEAREST),
        tex::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        tex::Bilinear => (gl::LINEAR, gl::LINEAR),
        tex::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        tex::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
}

pub fn make_sampler(gl: &gl::Gl, info: &tex::SamplerInfo) -> Sampler { unsafe {
    let mut name = 0 as Sampler;
    gl.GenSamplers(1, &mut name);

    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        tex::Anisotropic(fac) =>
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
        tex::NoComparsion => gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::NONE as GLint),
        tex::CompareRefToTexture(cmp) => {
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as GLint);
            gl.SamplerParameteri(name, gl::TEXTURE_COMPARE_FUNC, state::map_comparison(cmp) as GLint);
        }
    }

    name
}}

pub fn generate_mipmap(gl: &gl::Gl, kind: tex::TextureKind, name: Texture) { unsafe {
    //can't fail here, but we need to check for integer formats too
    debug_assert!(kind.get_aa_mode().is_none());
    let target = kind_to_gl(kind).unwrap();
    gl.BindTexture(target, name);
    gl.GenerateMipmap(target);
}}
