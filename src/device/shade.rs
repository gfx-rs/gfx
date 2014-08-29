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

#![allow(missing_doc)]

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
#[allow(missing_doc)]
#[deriving(Clone, PartialEq, Show)]
pub enum BaseType {
    BaseF32,
    BaseF64,
    BaseI32,
    BaseU32,
    BaseBool,
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
#[allow(missing_doc)]
#[deriving(Show)]
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
#[allow(missing_doc)]
pub enum UniformValue {
    ValueI32(i32),
    ValueF32(f32),

    ValueI32Vector2([i32, ..2]),
    ValueI32Vector3([i32, ..3]),
    ValueI32Vector4([i32, ..4]),

    ValueF32Vector2([f32, ..2]),
    ValueF32Vector3([f32, ..3]),
    ValueF32Vector4([f32, ..4]),

    ValueF32Matrix2([[f32, ..2], ..2]),
    ValueF32Matrix3([[f32, ..3], ..3]),
    ValueF32Matrix4([[f32, ..4], ..4]),
}

impl UniformValue {
    /// Whether two `UniformValue`s have the same type.
    pub fn is_same_type(&self, other: &UniformValue) -> bool {
        match (*self, *other) {
            (ValueI32(_), ValueI32(_)) => true,
            (ValueF32(_), ValueF32(_)) => true,

            (ValueI32Vector2(_), ValueI32Vector2(_)) => true,
            (ValueI32Vector3(_), ValueI32Vector3(_)) => true,
            (ValueI32Vector4(_), ValueI32Vector4(_)) => true,

            (ValueF32Vector2(_), ValueF32Vector2(_)) => true,
            (ValueF32Vector3(_), ValueF32Vector3(_)) => true,
            (ValueF32Vector4(_), ValueF32Vector4(_)) => true,

            (ValueF32Matrix2(_), ValueF32Matrix2(_)) => true,
            (ValueF32Matrix3(_), ValueF32Matrix3(_)) => true,
            (ValueF32Matrix4(_), ValueF32Matrix4(_)) => true,

            _ => false,
        }
    }
}

impl Clone for UniformValue {
    fn clone(&self) -> UniformValue {
        match *self {
            ValueI32(val)      => ValueI32(val),
            ValueF32(val)      => ValueF32(val),

            ValueI32Vector2(v) => ValueI32Vector2(v),
            ValueI32Vector3(v) => ValueI32Vector3(v),
            ValueI32Vector4(v) => ValueI32Vector4(v),

            ValueF32Vector2(v) => ValueF32Vector2(v),
            ValueF32Vector3(v) => ValueF32Vector3(v),
            ValueF32Vector4(v) => ValueF32Vector4(v),

            ValueF32Matrix2(m) => ValueF32Matrix2(m),
            ValueF32Matrix3(m) => ValueF32Matrix3(m),
            ValueF32Matrix4(m) => ValueF32Matrix4(m),
        }
    }
}

impl fmt::Show for UniformValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ValueI32(x)            => write!(f, "ValueI32({})", x),
            ValueF32(x)            => write!(f, "ValueF32({})", x),

            ValueI32Vector2(ref v) => write!(f, "ValueI32Vector2({})", v.as_slice()),
            ValueI32Vector3(ref v) => write!(f, "ValueI32Vector3({})", v.as_slice()),
            ValueI32Vector4(ref v) => write!(f, "ValueI32Vector4({})", v.as_slice()),

            ValueF32Vector2(ref v) => write!(f, "ValueF32Vector2({})", v.as_slice()),
            ValueF32Vector3(ref v) => write!(f, "ValueF32Vector3({})", v.as_slice()),
            ValueF32Vector4(ref v) => write!(f, "ValueF32Vector4({})", v.as_slice()),

            ValueF32Matrix2(ref m) => {
                try!(write!(f, "ValueF32Matrix2("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
            ValueF32Matrix3(ref m) => {
                try!(write!(f, "ValueF32Matrix3("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
            ValueF32Matrix4(ref m) => {
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
            return Err(ErrorArraySize)
        }
        match (self.base_type, self.container, *value) {
            (BaseI32, Single, ValueI32(_)) => Ok(()),
            (BaseF32, Single, ValueF32(_)) => Ok(()),

            (BaseF32, Vector(2), ValueF32Vector2(_)) => Ok(()),
            (BaseF32, Vector(3), ValueF32Vector3(_)) => Ok(()),
            (BaseF32, Vector(4), ValueF32Vector4(_)) => Ok(()),

            (BaseI32, Vector(2), ValueI32Vector2(_)) => Ok(()),
            (BaseI32, Vector(3), ValueI32Vector3(_)) => Ok(()),
            (BaseI32, Vector(4), ValueI32Vector4(_)) => Ok(()),

            (BaseF32, Matrix(_, 2,2), ValueF32Matrix2(_)) => Ok(()),
            (BaseF32, Matrix(_, 3,3), ValueF32Matrix3(_)) => Ok(()),
            (BaseF32, Matrix(_, 4,4), ValueF32Matrix4(_)) => Ok(()),

            _ => Err(ErrorBaseType)
        }
    }
}

/// Like `MaybeOwned` but for u8.
#[allow(missing_doc)]
#[deriving(Show, PartialEq, Clone)]
pub enum Bytes {
    StaticBytes(&'static [u8]),
    OwnedBytes(Vec<u8>),
}

impl Bytes {
    /// Get the byte data as a slice.
    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        match *self {
            StaticBytes(ref b) => b.as_slice(),
            OwnedBytes(ref b) => b.as_slice(),
        }
    }
}

/// A type storing shader source for different graphics APIs and versions.
#[allow(missing_doc)]
#[deriving(Clone, PartialEq, Show)]
pub struct ShaderSource {
    pub glsl_120: Option<Bytes>,
    pub glsl_150: Option<Bytes>,
    // TODO: hlsl_sm_N...
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
#[allow(missing_doc)]
#[deriving(Clone, PartialEq, PartialOrd, Show)]
pub enum ShaderModel {
    ModelUnsupported,
    Model30,
    Model40,
    Model41,
    Model50,
}

impl ShaderModel {
    /// Return the shader model as a numeric value.
    ///
    /// Model30 turns to 30, etc.
    pub fn to_number(&self) -> u8 {
        match *self {
            ModelUnsupported => 0,  //ModelAncient, ModelPreHistoric, ModelMyGrandpaLikes
            Model30 => 30,
            Model40 => 40,
            Model41 => 41,
            Model50 => 50,
        }
    }
}
