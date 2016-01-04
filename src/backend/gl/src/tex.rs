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

use {gl, Surface, Texture, Sampler};
use gl::types::{GLenum, GLuint, GLint, GLfloat, GLsizei, GLvoid};
use state;
use gfx_core::factory::SHADER_RESOURCE;
use gfx_core::format::{Format as NewFormat, ChannelType};
use gfx_core::tex::{Format, CubeFace, Kind, TextureError, SurfaceError,
                    SamplerInfo, ImageInfo, ImageInfoCommon, RawImageInfo,
                    AaMode, Components, FilterMethod, WrapMode,
                    Level, Dimensions, Descriptor};


fn cube_face_to_gl(face: CubeFace) -> GLenum {
    match face {
        CubeFace::PosZ => gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
        CubeFace::NegZ => gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        CubeFace::PosX => gl::TEXTURE_CUBE_MAP_POSITIVE_X,
        CubeFace::NegX => gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
        CubeFace::PosY => gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
        CubeFace::NegY => gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
    }
}

pub fn kind_to_gl(kind: Kind) -> GLenum {
    match kind {
        Kind::D1(_) => gl::TEXTURE_1D,
        Kind::D1Array(_, _) => gl::TEXTURE_1D_ARRAY,
        Kind::D2(_, _, AaMode::Single) => gl::TEXTURE_2D,
        Kind::D2(_, _, _) => gl::TEXTURE_2D_MULTISAMPLE,
        Kind::D2Array(_, _, _, AaMode::Single) => gl::TEXTURE_2D_ARRAY,
        Kind::D2Array(_, _, _, _) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        Kind::D3(_, _, _) => gl::TEXTURE_3D,
        Kind::Cube(_, _) => gl::TEXTURE_CUBE_MAP,
        Kind::CubeArray(_, _, _) => gl::TEXTURE_CUBE_MAP_ARRAY,
    }
}

fn kind_face_to_gl(kind: Kind, face: Option<CubeFace>) -> GLenum {
    match face {
        Some(f) => cube_face_to_gl(f),
        None => kind_to_gl(kind),
    }
}

fn components_to_glpixel(c: Components) -> GLenum {
    match c {
        Components::R    => gl::RED,
        Components::RG   => gl::RG,
        Components::RGB  => gl::RGB,
        Components::RGBA => gl::RGBA,
    }
}

fn old_format_to_glpixel(t: Format) -> GLenum {
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
fn old_format_to_gltype(t: Format) -> Result<GLenum, ()> {
    use gfx_core::tex::FloatSize;
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

fn format_to_glpixel(format: NewFormat) -> GLenum {
    use gfx_core::format::SurfaceType as S;
    match format.0 {
        S::R8 | S::R16 | S::R32=> gl::RED,
        S::R4_G4 | S::R8_G8 | S::R16_G16 | S::R32_G32 => gl::RG,
        S::R8_G8_B8 | S::R16_G16_B16 | S::R32_G32_B32 |
        S::R3_G3_B2 | S::R5_G6_B5 => gl::RGB,
        S::R8_G8_B8_A8 | S::R16_G16_B16_A16 | S::R32_G32_B32_A32 |
        S::R4_G4_B4_A4 | S::R5_G5_B5_A1 | S::R10_G10_B10_A2 => gl::RGBA,
        S::D24_S8 => gl::DEPTH_STENCIL,
        S::D16 | S::D24 | S::D32 => gl::DEPTH,
    }
}

fn format_to_gltype(format: NewFormat) -> Result<GLenum, ()> {
    use gfx_core::format::SurfaceType as S;
    use gfx_core::format::ChannelType as C;
    let (fm8, fm16, fm32) = match format.1.ty {
        C::Int | C::IntNormalized | C::IntScaled =>
            (gl::BYTE, gl::SHORT, gl::INT),
        C::Uint | C::UintNormalized | C::UintScaled =>
            (gl::UNSIGNED_BYTE, gl::UNSIGNED_SHORT, gl::UNSIGNED_INT),
        C::Float => (gl::ZERO, gl::HALF_FLOAT, gl::FLOAT),
        C::Srgb => return Err(()),
    };
    Ok(match format.0 {
        S::R3_G3_B2 => gl::UNSIGNED_BYTE_3_3_2,
        S::R4_G4 => return Err(()),
        S::R4_G4_B4_A4 => gl::UNSIGNED_SHORT_4_4_4_4,
        S::R5_G5_B5_A1 => gl::UNSIGNED_SHORT_5_5_5_1,
        S::R5_G6_B5 => gl::UNSIGNED_SHORT_5_6_5,
        S::R8 | S::R8_G8 | S::R8_G8_B8 | S::R8_G8_B8_A8 => fm8,
        S::R10_G10_B10_A2 => gl::UNSIGNED_INT_10_10_10_2,
        S::R16 | S::R16_G16 | S::R16_G16_B16 | S::R16_G16_B16_A16 => fm16,
        S::R32 | S::R32_G32 | S::R32_G32_B32 | S::R32_G32_B32_A32 => fm32,
        S::D16 => gl::UNSIGNED_SHORT,
        S::D24 => gl::UNSIGNED_INT,
        S::D24_S8 => gl::UNSIGNED_INT_24_8,
        S::D32 => gl::FLOAT,
    })
}

fn format_to_glfull(format: NewFormat) -> Result<GLenum, ()> {
    use gfx_core::format::SurfaceType as S;
    use gfx_core::format::ChannelType as C;
    let cty = format.1.ty;
    Ok(match format.0 {
        S::R3_G3_B2 => match cty {
            C::UintNormalized => gl::R3_G3_B2,
            _ => return Err(()),
        },
        S::R4_G4 => return Err(()),
        S::R4_G4_B4_A4 => match cty {
            C::UintNormalized => gl::RGBA4,
            _ => return Err(()),
        },
        S::R5_G5_B5_A1 => match cty {
            C::UintNormalized => gl::RGB5_A1,
            _ => return Err(()),
        },
        S::R5_G6_B5 => match cty {
            C::UintNormalized => gl::RGB565,
            _ => return Err(()),
        },
        // 8 bits
        S::R8 => match cty {
            C::Int => gl::R8I,
            C::IntNormalized => gl::R8_SNORM,
            C::Uint => gl::R8UI,
            C::UintNormalized => gl::R8,
            _ => return Err(()),
        },
        S::R8_G8 => match cty {
            C::Int => gl::RG8I,
            C::IntNormalized => gl::RG8_SNORM,
            C::Uint => gl::RG8UI,
            C::UintNormalized => gl::RG8,
            _ => return Err(()),
        },
        S::R8_G8_B8 => match cty {
            C::Int => gl::RGB8I,
            C::IntNormalized => gl::RGB8_SNORM,
            C::Uint => gl::RGB8UI,
            C::UintNormalized => gl::RGB8,
            C::Srgb => gl::SRGB8,
            _ => return Err(()),
        },
        S::R8_G8_B8_A8 => match cty {
            C::Int => gl::RGBA8I,
            C::IntNormalized => gl::RGBA8_SNORM,
            C::Uint => gl::RGBA8UI,
            C::UintNormalized => gl::RGBA8,
            C::Srgb => gl::SRGB8_ALPHA8,
            _ => return Err(()),
        },
        // 10+ bits
        S::R10_G10_B10_A2 => match cty {
            C::Uint => gl::RGB10_A2UI,
            C::UintNormalized => gl::RGB10_A2,
            _ => return Err(()),
        },
        // 16 bits
        S::R16 => match cty {
            C::Int => gl::R16I,
            C::IntNormalized => gl::R16_SNORM,
            C::Uint => gl::R16UI,
            C::UintNormalized => gl::R16,
            C::Float => gl::R16F,
            _ => return Err(()),
        },
        S::R16_G16 => match cty {
            C::Int => gl::RG16I,
            C::IntNormalized => gl::RG16_SNORM,
            C::Uint => gl::RG16UI,
            C::UintNormalized => gl::RG16,
            C::Float => gl::RG16F,
            _ => return Err(()),
        },
        S::R16_G16_B16 => match cty {
            C::Int => gl::RGB16I,
            C::IntNormalized => gl::RGB16_SNORM,
            C::Uint => gl::RGB16UI,
            C::UintNormalized => gl::RGB16,
            C::Float => gl::RGB16F,
            _ => return Err(()),
        },
        S::R16_G16_B16_A16 => match cty {
            C::Int => gl::RGBA16I,
            C::IntNormalized => gl::RGBA16_SNORM,
            C::Uint => gl::RGBA16UI,
            C::UintNormalized => gl::RGBA16,
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

fn make_surface_impl(gl: &gl::Gl, format: GLenum, dim: Dimensions)
                     -> Result<Surface, SurfaceError> {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenRenderbuffers(1, &mut name);
    }

    let target = gl::RENDERBUFFER;
    unsafe {
        gl.BindRenderbuffer(target, name);
    }
    match dim.3 {
        AaMode::Single => unsafe {
            gl.RenderbufferStorage(
                target,
                format,
                dim.0 as GLsizei,
                dim.1 as GLsizei
            );
        },
        AaMode::Multi(samples) => unsafe {
            gl.RenderbufferStorageMultisample(
                target,
                samples as GLsizei,
                format,
                dim.0 as GLsizei,
                dim.1 as GLsizei
            );
        },
        AaMode::Coverage(_, _) => return Err(SurfaceError::UnsupportedFormat),
    }

    Ok(name)
}

/// Create a render surface.
pub fn make_surface(gl: &gl::Gl, desc: &Descriptor, cty: ChannelType) ->
                        Result<Surface, SurfaceError> {
    let format = NewFormat(desc.format, cty.into());
    let fmt = match format_to_glfull(format) {
        Ok(f) => f,
        Err(_) => return Err(SurfaceError::UnsupportedFormat),
    };
    make_surface_impl(gl, fmt, desc.kind.get_dimensions())
}

fn make_widout_storage_impl(gl: &gl::Gl, kind: Kind, format: GLint, pix: GLenum, typ: GLenum,
                            levels: Level, fixed_sample_locations: bool)
                            -> Result<Texture, TextureError> {
    let (name, target) = make_texture(gl, kind);
    match kind {
        Kind::D1(w) => unsafe {
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
        Kind::D1Array(w, a) => unsafe {
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
        Kind::D2(w, h, AaMode::Single) => unsafe {
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
        Kind::D2(w, h, AaMode::Multi(samples)) => unsafe {
            gl.TexImage2DMultisample(
                target,
                samples as GLsizei,
                format as GLenum,  //GL spec bug
                w as GLsizei,
                h as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        Kind::D2Array(w, h, a, AaMode::Single) => unsafe {
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
        Kind::D2Array(w, h, a, AaMode::Multi(samples)) => unsafe {
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
        Kind::D3(w, h, d)  => unsafe {
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
        Kind::Cube(w, h) => {
            for &target in [gl::TEXTURE_CUBE_MAP_POSITIVE_X, gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Y, gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Z, gl::TEXTURE_CUBE_MAP_NEGATIVE_Z].iter() {
                unsafe { gl.TexImage2D(
                    target,
                    0,
                    format,
                    w as GLsizei,
                    h as GLsizei,
                    0,
                    pix,
                    typ,
                    ::std::ptr::null()
                )};
            }
        },
        _ => return Err(TextureError::UnsupportedSamples),
    }

    set_mipmap_range(gl, target, (0, levels - 1));
    Ok(name)
}

/// Create a texture, using the descriptor, assuming TexStorage* isn't available.
pub fn make_without_storage(gl: &gl::Gl, desc: &Descriptor, cty: ChannelType) ->
                            Result<Texture, TextureError> {
    let format = NewFormat(desc.format, cty.into());
    let gl_format = match format_to_glfull(format) {
        Ok(f) => f as GLint,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };
    let gl_pixel_format = format_to_glpixel(format);
    let gl_data_type = match format_to_gltype(format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };

    let fixed_loc = desc.bind.contains(SHADER_RESOURCE);
    make_widout_storage_impl(gl, desc.kind, gl_format, gl_pixel_format, gl_data_type,
                             desc.levels, fixed_loc)
}

/// Create a texture, assuming TexStorage is available.
fn make_with_storage_impl(gl: &gl::Gl, kind: Kind, format: GLenum,
                          levels: Level, fixed_sample_locations: bool)
                          -> Result<Texture, TextureError> {
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
        Kind::D1(w) => unsafe {
            gl.TexStorage1D(
                target,
                min(levels, mip_level1(w)),
                format,
                w as GLsizei
            );
        },
        Kind::D1Array(w, a) => unsafe {
            gl.TexStorage2D(
                target,
                min(levels, mip_level1(w)),
                format,
                w as GLsizei,
                a as GLsizei
            );
        },
        Kind::D2(w, h, AaMode::Single) => unsafe {
            gl.TexStorage2D(
                target,
                min(levels, mip_level2(w, h)),
                format,
                w as GLsizei,
                h as GLsizei
            );
        },
        Kind::D2Array(w, h, a, AaMode::Single) => unsafe {
            gl.TexStorage3D(
                target,
                min(levels, mip_level2(w, h)),
                format,
                w as GLsizei,
                h as GLsizei,
                a as GLsizei
            );
        },
        Kind::D2(w, h, AaMode::Multi(samples)) => unsafe {
            gl.TexStorage2DMultisample(
                target,
                samples as GLsizei,
                format,
                w as GLsizei,
                h as GLsizei,
                if fixed_sample_locations {gl::TRUE} else {gl::FALSE}
            );
        },
        Kind::D2Array(w, h, a, AaMode::Multi(samples)) => unsafe {
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
        Kind::D3(w, h, d) => unsafe {
            gl.TexStorage3D(
                target,
                min(levels, mip_level3(w, h, d)),
                format,
                w as GLsizei,
                h as GLsizei,
                d as GLsizei
            );
        },
        //Kind::Cube(..) TODO
        _ => return Err(TextureError::UnsupportedSamples),
    }

    set_mipmap_range(gl, target, (0, levels - 1));

    Ok(name)
}

/// Create a texture, using the descriptor, assuming TexStorage is available.
pub fn make_with_storage(gl: &gl::Gl, desc: &Descriptor, cty: ChannelType) ->
                         Result<Texture, TextureError> {
    let format = NewFormat(desc.format, cty.into());
    let gl_format = match format_to_glfull(format) {
        Ok(f) => f,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };
    let fixed_loc = desc.bind.contains(SHADER_RESOURCE);
    make_with_storage_impl(gl, desc.kind, gl_format, desc.levels, fixed_loc)
}

/// Bind a sampler using a given binding anchor.
/// Used for GL compatibility profile only. The core profile has sampler objects
pub fn bind_sampler(gl: &gl::Gl, target: GLenum, info: &SamplerInfo) { unsafe {
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

    gl.TexParameterf(target, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());

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

fn update_texture_impl<F>(gl: &gl::Gl, kind: Kind, target: GLenum, pix: GLenum,
                       typ: GLenum, img: &ImageInfoCommon<F>, data: *const GLvoid)
                       -> Result<(), TextureError> {
    Ok(match kind {
        Kind::D1(_) => unsafe {
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
        Kind::D1Array(_, _) | Kind::D2(_, _, AaMode::Single) => unsafe {
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
        Kind::D2Array(_, _, _, AaMode::Single) | Kind::D3(_, _, _) => unsafe {
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
        Kind::Cube(_, _) => unsafe {
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
        _ => return Err(TextureError::UnsupportedSamples),
    })
}

pub fn update_texture_new(gl: &gl::Gl, name: Texture,
                          kind: Kind, face: Option<CubeFace>,
                          img: &RawImageInfo, slice: &[u8])
                          -> Result<(), TextureError> {
    //TODO: check size
    let data = slice.as_ptr() as *const GLvoid;
    let pixel_format = format_to_glpixel(img.format);
    let data_type = match format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedFormat),
    };

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, name) };

    let target = kind_face_to_gl(kind, face);
    update_texture_impl(gl, kind, target, pixel_format, data_type, img, data)
}

pub fn update_texture(gl: &gl::Gl, kind: Kind, face: Option<CubeFace>,
                      name: Texture, img: &ImageInfo, slice: &[u8])
                      -> Result<(), TextureError> {
    if let Some(fmt_size) = img.format.get_size() {
        // TODO: can we compute the expected size for compressed formats?
        let expected_size = img.width as usize * img.height as usize *
                            img.depth as usize * fmt_size as usize;
        if slice.len() != expected_size {
            return Err(TextureError::IncorrectSize(expected_size));
        }
    }

    let data = slice.as_ptr() as *const GLvoid;
    let pixel_format = old_format_to_glpixel(img.format);
    let data_type = match old_format_to_gltype(img.format) {
        Ok(t) => t,
        Err(_) => return Err(TextureError::UnsupportedFormat),
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
                         -> Result<(), TextureError> {
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
        _ => return Err(TextureError::UnsupportedSamples),
    }

    Ok(())
}
/// Common texture creation routine, just creates and binds.
fn make_texture(gl: &gl::Gl, kind: Kind) -> (Texture, GLuint) {
    let mut name = 0 as GLuint;
    unsafe {
        gl.GenTextures(1, &mut name);
    }

    let target = kind_to_gl(kind);
    unsafe { gl.BindTexture(target, name) };
    (name, target)
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

    gl.SamplerParameterf(name, gl::TEXTURE_LOD_BIAS, info.lod_bias.into());

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

pub fn generate_mipmap(gl: &gl::Gl, kind: Kind, name: Texture) { unsafe {
    //can't fail here, but we need to check for integer formats too
    let (_, _, _, aa) = kind.get_dimensions();
    debug_assert!(!aa.needs_resolve());
    let target = kind_to_gl(kind);
    gl.BindTexture(target, name);
    gl.GenerateMipmap(target);
}}
