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

use super::{gl, GlBackEnd, Texture, Sampler};
use super::gl::types::{GLenum, GLuint, GLint, GLfloat, GLsizei, GLvoid};
use tex::*;
use Blob;

fn kind_to_gl(t: ::tex::TextureKind) -> GLenum {
    match t {
        Texture1D => gl::TEXTURE_1D,
        Texture1DArray => gl::TEXTURE_1D_ARRAY,
        Texture2D => gl::TEXTURE_2D,
        Texture2DArray => gl::TEXTURE_2D_ARRAY,
        TextureCube => gl::TEXTURE_CUBE_MAP,
        Texture3D => gl::TEXTURE_3D,
    }
}

fn format_to_gl(t: ::tex::TextureFormat) -> GLenum {
    match t {
        RGB8 => gl::RGB8,
        RGBA8 => gl::RGBA8,
    }
}

fn format_to_glpixel(t: ::tex::TextureFormat) -> GLenum {
    match t {
        RGB8 => gl::RGB,
        RGBA8 => gl::RGBA
    }
}

fn format_to_gltype(t: ::tex::TextureFormat) -> GLenum {
    match t {
        RGB8 | RGBA8 => gl::UNSIGNED_BYTE,
    }
}

/// Create a texture, assuming TexStorage* isn't available.
pub fn make_without_storage(info: ::tex::TextureInfo) -> Texture {
    let name = make_texture(info);

    let fmt = format_to_gl(info.format) as GLint;
    let pix = format_to_glpixel(info.format);
    let typ = format_to_gltype(info.format);

    let kind = kind_to_gl(info.kind);

    unsafe {
        match info.kind {
            Texture1D => {
                gl::TexImage1D(
                    kind,
                    0,
                    fmt,
                    info.width as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null(),
                );
            },
            Texture1DArray => {
                gl::TexImage2D(
                    kind,
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
            Texture2D => {
                gl::TexImage2D(
                    kind,
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
            TextureCube => unimplemented!(),
            Texture2DArray | Texture3D => {
                gl::TexImage3D(
                    kind,
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

    name
}

/// Create a texture, assuming TexStorage is available.
pub fn make_with_storage(info: ::tex::TextureInfo) -> Texture {
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

    let fmt = format_to_gl(info.format);
    let kind = kind_to_gl(info.kind);

    match info.kind {
        Texture1D => {
            gl::TexStorage1D(
                kind,
                min(info.mipmap_range.val1(), mip_level1(info.width)),
                fmt,
                info.width as GLsizei
            );
        },
        Texture1DArray => {
            gl::TexStorage2D(
                kind,
                min(info.mipmap_range.val1(), mip_level1(info.width)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
            );
        },
        Texture2D => {
            gl::TexStorage2D(
                kind,
                min(info.mipmap_range.val1(), mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
            );
        },
        TextureCube => unimplemented!(),
        Texture2DArray => {
            gl::TexStorage3D(
                kind,
                min(info.mipmap_range.val1(), mip_level2(info.width, info.height)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
            );
        },
        Texture3D => {
            gl::TexStorage3D(
                kind,
                min(info.mipmap_range.val1(), mip_level3(info.width, info.height, info.depth)),
                fmt,
                info.width as GLsizei,
                info.height as GLsizei,
                info.depth as GLsizei,
            );
        },
    }

    name
}

/// Bind a texture + sampler to a given slot.
pub fn bind_texture(loc: GLuint, tex: Texture, sam: Sampler, backend: &GlBackEnd) {
    let info = backend.samplers[sam as uint];
    let kind = tex.kind;
    let tex = tex.name;

    gl::ActiveTexture(gl::TEXTURE0 + loc as GLenum);

    gl::BindTexture(kind, tex);

    if backend.caps.sampler_objects_supported {
        gl::BindSampler(loc, sam);
    } else {
        let (min, mag) = filter_to_gl(info.filtering);
        let target = kind;

        match info.filtering {
            Anisotropic(fac) =>
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

        let (base, max) = info.mipmap_range;
        gl::TexParameteri(target, gl::TEXTURE_BASE_LEVEL, base as GLint);
        gl::TexParameteri(target, gl::TEXTURE_MAX_LEVEL, max as GLint);
    }
}

pub fn update_texture(tex: Texture, img: ::tex::ImageInfo, data: Box<Blob + Send>) {
    debug_assert!(img.width as u32 * img.height as u32 * img.depth as u32 == data.get_size() as u32);

    let data = data.get_address() as *const GLvoid;
    let pix = format_to_glpixel(img.format);
    let typ = format_to_gltype(img.format);
    let kind = tex.kind;

    gl::BindTexture(kind, tex.name);

    unsafe {
        match kind {
            gl::TEXTURE_1D => {
                gl::TexSubImage1D(
                    kind,
                    img.mipmap as GLint,
                    img.xoffset as GLint,
                    img.width as GLint,
                    pix,
                    typ,
                    data,
                );
            },
            gl::TEXTURE_1D_ARRAY | gl::TEXTURE_2D => {
                gl::TexSubImage2D(
                    kind,
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
            gl::TEXTURE_CUBE_MAP => unimplemented!(),
            gl::TEXTURE_2D_ARRAY | gl::TEXTURE_3D => {
                gl::TexSubImage3D(
                    kind,
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
            _ => fail!("invalid target stored in texture {}", tex)
        }
    }
}

/// Common texture creation routine, just creates and binds.
fn make_texture(info: ::tex::TextureInfo) -> Texture {
    let mut name = 0 as GLuint;
    unsafe {
        gl::GenTextures(1, &mut name);
    }

    let k = kind_to_gl(info.kind);
    gl::BindTexture(k, name);

    Texture { name: name, kind: k }
}

fn wrap_to_gl(w: WrapMode) -> GLenum {
    match w {
        Tile => gl::REPEAT,
        Mirror => gl::MIRRORED_REPEAT,
        Clamp => gl::CLAMP_TO_EDGE
    }
}

fn filter_to_gl(f: ::tex::FilterMethod) -> (GLenum, GLenum) {
    match f {
        Scale => (gl::NEAREST, gl::NEAREST),
        Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        Bilinear => (gl::LINEAR_MIPMAP_NEAREST, gl::LINEAR),
        Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        Anisotropic(..) => {
            (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR)
        }
    }
}

pub fn make_sampler(info: ::tex::SamplerInfo) -> Sampler {
    let mut name = 0 as Sampler;
    unsafe {
        gl::GenSamplers(1, &mut name);
    }

    let (min, mag) = filter_to_gl(info.filtering);

    match info.filtering {
        Anisotropic(fac) =>
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

    let (base, max) = info.mipmap_range;
    gl::SamplerParameteri(name, gl::TEXTURE_BASE_LEVEL, base as GLint);
    gl::SamplerParameteri(name, gl::TEXTURE_MAX_LEVEL, max as GLint);

    name
}
