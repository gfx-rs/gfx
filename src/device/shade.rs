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

//! Shader handling.

#![allow(missing_docs)]

use std::fmt;

// Describing shader parameters
// TOOD: Remove GL-isms, especially in the documentation.

/// Number of components in a container type (vectors/matrices)
pub type Dimension = u8;

/// Whether the sampler samples an array texture.
#[deriving(Clone, PartialEq, Show)]
pub enum IsArray { Array, NoArray }

/// Whether the sampler samples a shadow texture (texture with a depth comparison)
#[deriving(Clone, PartialEq, Show)]
pub enum IsShadow { Shadow, NoShadow }

/// Whether the sampler samples a multisample texture.
#[deriving(Clone, PartialEq, Show)]
pub enum IsMultiSample { MultiSample, NoMultiSample }

/// Whether the sampler samples a rectangle texture.
///
/// Rectangle textures are the same as 2D textures, but accessed with absolute texture coordinates
/// (as opposed to the usual, normalized to [0, 1]).
#[deriving(Clone, PartialEq, Show)]
pub enum IsRect { Rect, NoRect }

/// Whether the matrix is column or row major.
#[deriving(Clone, PartialEq, Show)]
pub enum MatrixFormat { ColumnMajor, RowMajor }

/// What texture type this sampler samples from.
///
/// A single sampler cannot be used with multiple texture types.
#[deriving(Clone, PartialEq, Show)]
pub enum SamplerType {
    /// Sample from a buffer.
    SamplerBuffer,
    /// Sample from a 1D texture
    Sampler1D(IsArray, IsShadow),
    /// Sample from a 2D texture
    Sampler2D(IsArray, IsShadow, IsMultiSample, IsRect),
    /// Sample from a 3D texture
    Sampler3D,
    /// Sample from a cubemap.
    SamplerCube(IsShadow),
}

/// Base type of this shader parameter.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, Show)]
pub enum BaseType {
    F32,
    F64,
    I32,
    U32,
    Bool,
}

/// Number of components this parameter represents.
#[deriving(Clone, PartialEq, Show)]
pub enum ContainerType {
    /// Scalar value
    Single,
    /// A vector with `Dimension` components.
    Vector(Dimension),
    /// A matrix.
    Matrix(MatrixFormat, Dimension, Dimension),
}

// Describing object data

/// Which program stage this shader represents.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, Show)]
pub enum Stage {
    Vertex,
    Geometry,
    Fragment,
}

// Describing program data

/// Location of a parameter in the program.
pub type Location = uint;

// unable to derive anything for fixed arrays
/// A value that can be uploaded to the device as a uniform.
#[allow(missing_docs)]
pub enum UniformValue {
    I32(i32),
    F32(f32),

    I32Vector2([i32, ..2]),
    I32Vector3([i32, ..3]),
    I32Vector4([i32, ..4]),

    F32Vector2([f32, ..2]),
    F32Vector3([f32, ..3]),
    F32Vector4([f32, ..4]),

    F32Matrix2([[f32, ..2], ..2]),
    F32Matrix3([[f32, ..3], ..3]),
    F32Matrix4([[f32, ..4], ..4]),
}

impl UniformValue {
    /// Whether two `UniformValue`s have the same type.
    pub fn is_same_type(&self, other: &UniformValue) -> bool {
        match (*self, *other) {
            (UniformValue::I32(_), UniformValue::I32(_)) => true,
            (UniformValue::F32(_), UniformValue::F32(_)) => true,

            (UniformValue::I32Vector2(_), UniformValue::I32Vector2(_)) => true,
            (UniformValue::I32Vector3(_), UniformValue::I32Vector3(_)) => true,
            (UniformValue::I32Vector4(_), UniformValue::I32Vector4(_)) => true,

            (UniformValue::F32Vector2(_), UniformValue::F32Vector2(_)) => true,
            (UniformValue::F32Vector3(_), UniformValue::F32Vector3(_)) => true,
            (UniformValue::F32Vector4(_), UniformValue::F32Vector4(_)) => true,

            (UniformValue::F32Matrix2(_), UniformValue::F32Matrix2(_)) => true,
            (UniformValue::F32Matrix3(_), UniformValue::F32Matrix3(_)) => true,
            (UniformValue::F32Matrix4(_), UniformValue::F32Matrix4(_)) => true,

            _ => false,
        }
    }
}

impl Clone for UniformValue {
    fn clone(&self) -> UniformValue {
        match *self {
            UniformValue::I32(val)      => UniformValue::I32(val),
            UniformValue::F32(val)      => UniformValue::F32(val),

            UniformValue::I32Vector2(v) => UniformValue::I32Vector2(v),
            UniformValue::I32Vector3(v) => UniformValue::I32Vector3(v),
            UniformValue::I32Vector4(v) => UniformValue::I32Vector4(v),

            UniformValue::F32Vector2(v) => UniformValue::F32Vector2(v),
            UniformValue::F32Vector3(v) => UniformValue::F32Vector3(v),
            UniformValue::F32Vector4(v) => UniformValue::F32Vector4(v),

            UniformValue::F32Matrix2(m) => UniformValue::F32Matrix2(m),
            UniformValue::F32Matrix3(m) => UniformValue::F32Matrix3(m),
            UniformValue::F32Matrix4(m) => UniformValue::F32Matrix4(m),
        }
    }
}

impl fmt::Show for UniformValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UniformValue::I32(x)            => write!(f, "ValueI32({})", x),
            UniformValue::F32(x)            => write!(f, "ValueF32({})", x),

            UniformValue::I32Vector2(ref v) => write!(f, "ValueI32Vector2({})", v.as_slice()),
            UniformValue::I32Vector3(ref v) => write!(f, "ValueI32Vector3({})", v.as_slice()),
            UniformValue::I32Vector4(ref v) => write!(f, "ValueI32Vector4({})", v.as_slice()),

            UniformValue::F32Vector2(ref v) => write!(f, "ValueF32Vector2({})", v.as_slice()),
            UniformValue::F32Vector3(ref v) => write!(f, "ValueF32Vector3({})", v.as_slice()),
            UniformValue::F32Vector4(ref v) => write!(f, "ValueF32Vector4({})", v.as_slice()),

            UniformValue::F32Matrix2(ref m) => {
                try!(write!(f, "ValueF32Matrix2("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
            UniformValue::F32Matrix3(ref m) => {
                try!(write!(f, "ValueF32Matrix3("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
            UniformValue::F32Matrix4(ref m) => {
                try!(write!(f, "ValueF32Matrix4("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
        }
    }
}

/// Vertex information that a shader takes as input.
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    /// Name of this attribute.
    pub name: String,
    /// Vertex attribute binding.
    pub location: uint,
    /// Number of elements this attribute represents.
    pub count: uint,
    /// Type that this attribute is composed of.
    pub base_type: BaseType,
    /// "Scalarness" of this attribute.
    pub container: ContainerType,
}

/// Uniform, a type of shader parameter representing data passed to the program.
#[deriving(Clone, PartialEq, Show)]
pub struct UniformVar {
    /// Name of this uniform.
    pub name: String,
    /// Location of this uniform in the program.
    pub location: Location,
    /// Number of elements this uniform represents.
    pub count: uint,
    /// Type that this uniform is composed of
    pub base_type: BaseType,
    /// "Scalarness" of this uniform.
    pub container: ContainerType,
}

/// A uniform block.
#[deriving(Clone, PartialEq, Show)]
pub struct BlockVar {
    /// Name of this uniform block.
    pub name: String,
    /// Size (in bytes) of this uniform block's data.
    pub size: uint,
    /// What program stage this uniform block can be used in, as a bitflag.
    pub usage: u8,
}

/// Sampler, a type of shader parameter representing a texture that can be sampled.
#[deriving(Clone, PartialEq, Show)]
pub struct SamplerVar {
    /// Name of this sampler variable.
    pub name: String,
    /// Location of this sampler in the program.
    pub location: Location,
    /// Base type for the sampler.
    pub base_type: BaseType,
    /// Type of this sampler.
    pub sampler_type: SamplerType,
}

/// Metadata about a program.
#[deriving(Clone, PartialEq, Show)]
pub struct ProgramInfo {
    /// Attributes in the program.
    pub attributes: Vec<Attribute>,
    /// Uniforms in the program
    pub uniforms: Vec<UniformVar>,
    /// Uniform blocks in the program
    pub blocks: Vec<BlockVar>,
    /// Samplers in the program
    pub textures: Vec<SamplerVar>,
}

/// Error type for trying to store a UniformValue in a UniformVar.
#[deriving(Show)]
pub enum CompatibilityError {
    /// Array sizes differ between the value and the var (trying to upload a vec2 as a vec4, etc)
    ErrorArraySize,
    /// Base types differ between the value and the var (trying to upload a f32 as a u16, etc)
    ErrorBaseType,
    /// Container-ness differs between the value and the var (trying to upload a scalar as a vec4,
    /// etc)
    ErrorContainer,
}

impl UniformVar {
    /// Whether a value is compatible with this variable. That is, whether the value can be stored
    /// in this variable.
    pub fn is_compatible(&self, value: &UniformValue) -> Result<(), CompatibilityError> {
        if self.count != 1 {
            return Err(CompatibilityError::ErrorArraySize)
        }
        match (self.base_type, self.container, *value) {
            (BaseType::I32, ContainerType::Single,         UniformValue::I32(_))        => Ok(()),
            (BaseType::F32, ContainerType::Single,         UniformValue::F32(_))        => Ok(()),

            (BaseType::F32, ContainerType::Vector(2),      UniformValue::F32Vector2(_)) => Ok(()),
            (BaseType::F32, ContainerType::Vector(3),      UniformValue::F32Vector3(_)) => Ok(()),
            (BaseType::F32, ContainerType::Vector(4),      UniformValue::F32Vector4(_)) => Ok(()),

            (BaseType::I32, ContainerType::Vector(2),      UniformValue::I32Vector2(_)) => Ok(()),
            (BaseType::I32, ContainerType::Vector(3),      UniformValue::I32Vector3(_)) => Ok(()),
            (BaseType::I32, ContainerType::Vector(4),      UniformValue::I32Vector4(_)) => Ok(()),

            (BaseType::F32, ContainerType::Matrix(_, 2,2), UniformValue::F32Matrix2(_)) => Ok(()),
            (BaseType::F32, ContainerType::Matrix(_, 3,3), UniformValue::F32Matrix3(_)) => Ok(()),
            (BaseType::F32, ContainerType::Matrix(_, 4,4), UniformValue::F32Matrix4(_)) => Ok(()),

            _ => Err(CompatibilityError::ErrorBaseType)
        }
    }
}

/// A type storing shader source for different graphics APIs and versions.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, Show)]
pub struct ShaderSource<'a> {
    pub glsl_120: Option<&'a [u8]>,
    pub glsl_130: Option<&'a [u8]>,
    pub glsl_140: Option<&'a [u8]>,
    pub glsl_150: Option<&'a [u8]>,
    // TODO: hlsl_sm_N...
    pub glsl_130plus_outputs: Option<[&'a str, ..4]>,
}

/// An error type for creating programs.
#[deriving(Clone, PartialEq, Show)]
pub enum CreateShaderError {
    /// The device does not support any of the shaders supplied.
    NoSupportedShaderProvided,
    /// The shader failed to compile.
    ShaderCompilationFailed
}

/// Shader model supported by the device, corresponds to the HLSL shader models.
#[allow(missing_docs)]
#[deriving(Clone, PartialEq, PartialOrd, Show)]
pub enum ShaderModel {
    Unsupported,
    Version30,
    Version40,
    Version41,
    Version50,
}

impl ShaderModel {
    /// Return the shader model as a numeric value.
    ///
    /// Model30 turns to 30, etc.
    pub fn to_number(&self) -> u8 {
        match *self {
            ShaderModel::Unsupported => 0,  //ModelAncient, ModelPreHistoric, ModelMyGrandpaLikes
            ShaderModel::Version30 => 30,
            ShaderModel::Version40 => 40,
            ShaderModel::Version41 => 41,
            ShaderModel::Version50 => 50,
        }
    }
}
