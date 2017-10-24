//! Shader parameter handling.

#[cfg(feature = "mint")]
use mint;

use std::error::Error;
use std::fmt;
pub use hal::shade::{self as core, ConstFormat, Formatted, Usage};

#[allow(missing_docs)]
pub trait ToUniform: Copy {
    fn convert(self) -> hal::UniformValue;
}

macro_rules! impl_uniforms {
    ( $( $ty_src:ty = $ty_dst:ident ,)* ) => {
        $(
            impl ToUniform for $ty_src {
                fn convert(self) -> hal::UniformValue {
                    hal::UniformValue::$ty_dst(self.into())
                }
            }
        )*
    }
}

impl_uniforms! {
    i32 = I32,
    f32 = F32,
    [i32; 2] = I32Vector2,
    [i32; 3] = I32Vector3,
    [i32; 4] = I32Vector4,
    [f32; 2] = F32Vector2,
    [f32; 3] = F32Vector3,
    [f32; 4] = F32Vector4,
    [[f32; 2]; 2] = F32Matrix2,
    [[f32; 3]; 3] = F32Matrix3,
    [[f32; 4]; 4] = F32Matrix4,
}

#[cfg(feature = "mint")]
impl_uniforms! {
    mint::Point2<f32> = F32Vector2,
    mint::Point3<f32> = F32Vector3,
    mint::Vector2<f32> = F32Vector2,
    mint::Vector3<f32> = F32Vector3,
    mint::Vector4<f32> = F32Vector4,
    mint::ColumnMatrix2<f32> = F32Matrix2,
    mint::ColumnMatrix3<f32> = F32Matrix3,
    mint::ColumnMatrix4<f32> = F32Matrix4,
}

/// Program linking error
#[derive(Clone, Debug, PartialEq)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    Vertex(hal::CreateShaderError),
    /// Unable to compile the hull shader
    Hull(hal::CreateShaderError),
    /// Unable to compile the domain shader
    Domain(hal::CreateShaderError),
    /// Unable to compile the geometry shader
    Geometry(hal::CreateShaderError),
    /// Unable to compile the pixel shader
    Pixel(hal::CreateShaderError),
    /// Unable to link
    Link(hal::CreateProgramError),
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProgramError::Vertex(ref e) => write!(f, "{}: {}", self.description(), e),
            ProgramError::Hull(ref e) => write!(f, "{}: {}", self.description(), e),
            ProgramError::Domain(ref e) => write!(f, "{}: {}", self.description(), e),
            ProgramError::Geometry(ref e) => write!(f, "{}: {}", self.description(), e),
            ProgramError::Pixel(ref e) => write!(f, "{}: {}", self.description(), e),
            ProgramError::Link(ref e) => write!(f, "{}: {}", self.description(), e),
        }
    }
}

impl Error for ProgramError {
    fn description(&self) -> &str {
        match *self {
            ProgramError::Vertex(_) => "Unable to compile the vertex shader",
            ProgramError::Hull(_) => "Unable to compile the hull shader",
            ProgramError::Domain(_) => "Unable to compile the domain shader",
            ProgramError::Geometry(_) => "Unable to compile the geometry shader",
            ProgramError::Pixel(_) => "Unable to compile the pixel shader",
            ProgramError::Link(_) => "Unable to link",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ProgramError::Vertex(ref e) => Some(e),
            ProgramError::Hull(ref e) => Some(e),
            ProgramError::Domain(ref e) => Some(e),
            ProgramError::Geometry(ref e) => Some(e),
            ProgramError::Pixel(ref e) => Some(e),
            ProgramError::Link(ref e) => Some(e),
        }
    }
}
