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

//! Shader parameter handling.

#[cfg(feature = "cgmath-types")]
use cgmath::{Deg, Matrix2, Matrix3, Matrix4, Rad, Point2, Point3, Vector2, Vector3, Vector4};

use std::error::Error;
use std::fmt;
pub use core::shade::{self as core, ConstFormat, Formatted, Usage};

#[allow(missing_docs)]
pub trait ToUniform: Copy {
    fn convert(self) -> core::UniformValue;
}

macro_rules! impl_uniforms {
    ( $( $ty_src:ty = $ty_dst:ident ,)* ) => {
        $(
            impl ToUniform for $ty_src {
                fn convert(self) -> core::UniformValue {
                    core::UniformValue::$ty_dst(self.into())
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

#[cfg(feature = "cgmath-types")]
impl ToUniform for Deg<f32> {
    fn convert(self) -> core::UniformValue {
        core::UniformValue::F32(self.0)
    }
}

#[cfg(feature = "cgmath-types")]
impl ToUniform for Rad<f32> {
    fn convert(self) -> core::UniformValue {
        core::UniformValue::F32(self.0)
    }
}

#[cfg(feature = "cgmath-types")]
impl_uniforms! {
    Point2<f32> = F32Vector2,
    Point3<f32> = F32Vector3,
    Vector2<f32> = F32Vector2,
    Vector3<f32> = F32Vector3,
    Vector4<f32> = F32Vector4,
    Matrix2<f32> = F32Matrix2,
    Matrix3<f32> = F32Matrix3,
    Matrix4<f32> = F32Matrix4,
}

/// Program linking error
#[derive(Clone, Debug, PartialEq)]
pub enum ProgramError {
    /// Unable to compile the vertex shader
    Vertex(core::CreateShaderError),
    /// Unable to compile the hull shader
    Hull(core::CreateShaderError),
    /// Unable to compile the domain shader
    Domain(core::CreateShaderError),
    /// Unable to compile the geometry shader
    Geometry(core::CreateShaderError),
    /// Unable to compile the pixel shader
    Pixel(core::CreateShaderError),
    /// Unable to link
    Link(core::CreateProgramError),
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
