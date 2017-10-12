use gl::{self, types as t};
use hal::{buffer, image as i};

pub fn image_kind_to_gl(kind: i::Kind) -> t::GLenum {
    match kind {
        i::Kind::D1(_) => gl::TEXTURE_1D,
        i::Kind::D1Array(_, _) => gl::TEXTURE_1D_ARRAY,
        i::Kind::D2(_, _, i::AaMode::Single) => gl::TEXTURE_2D,
        i::Kind::D2(_, _, _) => gl::TEXTURE_2D_MULTISAMPLE,
        i::Kind::D2Array(_, _, _, i::AaMode::Single) => gl::TEXTURE_2D_ARRAY,
        i::Kind::D2Array(_, _, _, _) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        i::Kind::D3(_, _, _) => gl::TEXTURE_3D,
        i::Kind::Cube(_) => gl::TEXTURE_CUBE_MAP,
        i::Kind::CubeArray(_, _) => gl::TEXTURE_CUBE_MAP_ARRAY,
    }
}

pub fn filter_to_gl(f: i::FilterMethod) -> (t::GLenum, t::GLenum) {
    match f {
        i::FilterMethod::Scale => (gl::NEAREST, gl::NEAREST),
        i::FilterMethod::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        i::FilterMethod::Bilinear => (gl::LINEAR, gl::LINEAR),
        i::FilterMethod::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        i::FilterMethod::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
}

pub fn wrap_to_gl(w: i::WrapMode) -> t::GLenum {
    match w {
        i::WrapMode::Tile   => gl::REPEAT,
        i::WrapMode::Mirror => gl::MIRRORED_REPEAT,
        i::WrapMode::Clamp  => gl::CLAMP_TO_EDGE,
        i::WrapMode::Border => gl::CLAMP_TO_BORDER,
    }
}

pub fn buffer_usage_to_gl_target(usage: buffer::Usage) -> Option<t::GLenum> {
    match usage & (buffer::UNIFORM | buffer::INDEX | buffer::VERTEX | buffer::INDIRECT) {
        buffer::UNIFORM => Some(gl::UNIFORM_BUFFER),
        buffer::INDEX => Some(gl::ELEMENT_ARRAY_BUFFER),
        buffer::VERTEX => Some(gl::ARRAY_BUFFER),
        buffer::INDIRECT => unimplemented!(),
        _ => None
    }
}
