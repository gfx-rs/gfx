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

use std::{fmt, cmp, hash};
use std::error::Error;
use {Resources};
use {AttributeSlot, ColorSlot, ConstantBufferSlot, ResourceViewSlot, SamplerSlot, UnorderedViewSlot};

#[cfg(feature = "cgmath-types")]
use cgmath::{Deg, Matrix2, Matrix3, Matrix4, Point2, Point3, Rad, Vector2, Vector3, Vector4};

/// Number of components in a container type (vectors/matrices)
pub type Dimension = u8;

/// Whether the sampler samples an array texture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum IsArray { Array, NoArray }

/// Whether the sampler compares the depth value upon sampling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum IsComparison { Compare, NoCompare }

/// Whether the sampler samples a multisample texture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum IsMultiSample { MultiSample, NoMultiSample }

/// Whether the sampler samples a rectangle texture.
///
/// Rectangle textures are the same as 2D textures, but accessed with absolute texture coordinates
/// (as opposed to the usual, normalized to [0, 1]).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum IsRect { Rect, NoRect }

/// Whether the matrix is column or row major.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum MatrixFormat { ColumnMajor, RowMajor }

/// A type of the texture variable.
/// This has to match the actual data we bind to the shader.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum TextureType {
    /// Sample from a buffer.
    Buffer,
    /// Sample from a 1D texture
    D1(IsArray),
    /// Sample from a 2D texture
    D2(IsArray, IsMultiSample),
    /// Sample from a 3D texture
    D3,
    /// Sample from a cubemap.
    Cube(IsArray),
}

impl TextureType {
    /// Check if this texture can be used with a sampler.
    pub fn can_sample(&self) -> bool {
        match self {
            &TextureType::Buffer => false,
            &TextureType::D1(_) => true,
            &TextureType::D2(_, IsMultiSample::MultiSample) => false,
            &TextureType::D2(_, IsMultiSample::NoMultiSample) => true,
            &TextureType::D3 => true,
            &TextureType::Cube(_) => true,
        }
    }
}

/// A type of the sampler variable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SamplerType(pub IsComparison, pub IsRect);

/// Base type of this shader parameter.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum BaseType {
    I32,
    U32,
    F32,
    F64,
    Bool,
}

/// Number of components this parameter represents.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Stage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
}

/// A constant static array of all shader stages.
pub const STAGES: [Stage; 5] = [Stage::Vertex, Stage::Hull, Stage::Domain, Stage::Geometry, Stage::Pixel];

// Describing program data

/// Location of a parameter in the program.
pub type Location = usize;

// unable to derive anything for fixed arrays
/// A value that can be uploaded to the device as a uniform.
#[allow(missing_docs)]
#[derive(Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum UniformValue {
    I32(i32),
    F32(f32),

    I32Vector2([i32; 2]),
    I32Vector3([i32; 3]),
    I32Vector4([i32; 4]),

    F32Vector2([f32; 2]),
    F32Vector3([f32; 3]),
    F32Vector4([f32; 4]),

    F32Matrix2([[f32; 2]; 2]),
    F32Matrix3([[f32; 3]; 3]),
    F32Matrix4([[f32; 4]; 4]),
}

impl fmt::Debug for UniformValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UniformValue::I32(x)            => write!(f, "ValueI32({:?})", x),
            UniformValue::F32(x)            => write!(f, "ValueF32({:?})", x),

            UniformValue::I32Vector2(ref v) => write!(f, "ValueI32Vector2({:?})", &v[..]),
            UniformValue::I32Vector3(ref v) => write!(f, "ValueI32Vector3({:?})", &v[..]),
            UniformValue::I32Vector4(ref v) => write!(f, "ValueI32Vector4({:?})", &v[..]),

            UniformValue::F32Vector2(ref v) => write!(f, "ValueF32Vector2({:?})", &v[..]),
            UniformValue::F32Vector3(ref v) => write!(f, "ValueF32Vector3({:?})", &v[..]),
            UniformValue::F32Vector4(ref v) => write!(f, "ValueF32Vector4({:?})", &v[..]),

            UniformValue::F32Matrix2(ref m) => {
                try!(write!(f, "ValueF32Matrix2("));
                for v in m.iter() {
                    try!(write!(f, "{:?}", &v[..]));
                }
                write!(f, ")")
            },
            UniformValue::F32Matrix3(ref m) => {
                try!(write!(f, "ValueF32Matrix3("));
                for v in m.iter() {
                    try!(write!(f, "{:?}", &v[..]));
                }
                write!(f, ")")
            },
            UniformValue::F32Matrix4(ref m) => {
                try!(write!(f, "ValueF32Matrix4("));
                for v in m.iter() {
                    try!(write!(f, "{:?}", &v[..]));
                }
                write!(f, ")")
            },
        }
    }
}

/// Format of a shader constant.
pub type ConstFormat = (BaseType, ContainerType);

/// A trait that statically links simple data types to
/// base types of the shader constants.
pub trait BaseTyped {
    fn get_base_type() -> BaseType;
}

/// A trait that statically links simple data types to
/// constant formats.
pub trait Formatted {
    /// Get the associated constant format.
    fn get_format() -> ConstFormat;
}

macro_rules! impl_base_type {
    ( $($name:ty = $value:ident ,)* ) => {
        $(
            impl BaseTyped for $name {
                fn get_base_type() -> BaseType {
                    BaseType::$value
                }
            }
        )*
    }
}

macro_rules! impl_const_vector {
    ( $( $num:expr ),* ) => {
        $(
            impl<T: BaseTyped> Formatted for [T; $num] {
                fn get_format() -> ConstFormat {
                    (T::get_base_type(), ContainerType::Vector($num))
                }
            }
        )*
    }
}

macro_rules! impl_const_matrix {
    ( $( [$n:expr, $m:expr] ),* ) => {
        $(
            impl<T: BaseTyped> Formatted for [[T; $n]; $m] {
                fn get_format() -> ConstFormat {
                    let mf = MatrixFormat::ColumnMajor;
                    (T::get_base_type(), ContainerType::Matrix(mf, $n, $m))
                }
            }
        )*
    }
}

#[cfg(feature = "cgmath-types")]
macro_rules! impl_const_vector_cgmath {
    ( $( $name:ident = $num:expr, )* ) => {
        $(
            impl<T: BaseTyped> Formatted for $name<T> {
                fn get_format() -> ConstFormat {
                    (T::get_base_type(), ContainerType::Vector($num))
                }
            }
        )*
    }
}

#[cfg(feature = "cgmath-types")]
macro_rules! impl_const_matrix_cgmath {
    ( $( $name:ident = $size:expr, )* ) => {
        $(
            impl<T: BaseTyped> Formatted for $name<T> {
                fn get_format() -> ConstFormat {
                    let mf = MatrixFormat::ColumnMajor;
                    (T::get_base_type(), ContainerType::Matrix(mf, $size, $size))
                }
            }
        )*
    }
}

impl_base_type! {
    i32 = I32,
    u32 = U32,
    f32 = F32,
    bool = Bool,
}

#[cfg(feature = "cgmath-types")]
impl_base_type! {
    Deg<f32> = F32,
    Rad<f32> = F32,
}

impl<T: BaseTyped> Formatted for T {
    fn get_format() -> ConstFormat {
        (T::get_base_type(), ContainerType::Single)
    }
}

impl_const_vector!(2, 3, 4);
impl_const_matrix!([2,2], [3,3], [4,4], [4,3]);

#[cfg(feature = "cgmath-types")]
impl_const_vector_cgmath! {
    Point2 = 2,
    Point3 = 3,
    Vector2 = 2,
    Vector3 = 3,
    Vector4 = 4,
}

#[cfg(feature = "cgmath-types")]
impl_const_matrix_cgmath! {
    Matrix2 = 2,
    Matrix3 = 3,
    Matrix4 = 4,
}

bitflags!(
    /// Parameter usage flags.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags Usage: u8 {
        /// Used by the vertex shader
        const VERTEX   = 0x1,
        /// Used by the geometry shader
        const GEOMETRY = 0x2,
        /// Used by the pixel shader
        const PIXEL    = 0x4,
        /// Used by the hull shader
        const HULL    = 0x8,
        /// Used by the pixel shader
        const DOMAIN    = 0x16,

    }
);

impl From<Stage> for Usage {
    fn from(stage: Stage) -> Usage {
        match stage {
            Stage::Vertex => VERTEX,
            Stage::Geometry => GEOMETRY,
            Stage::Pixel => PIXEL,
            Stage::Hull => HULL,
            Stage::Domain => DOMAIN,
        }
    }
}

/// Vertex information that a shader takes as input.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct AttributeVar {
    /// Name of this attribute.
    pub name: String,
    /// Slot of the vertex attribute.
    pub slot: AttributeSlot,
    /// Type that this attribute is composed of.
    pub base_type: BaseType,
    /// "Scalarness" of this attribute.
    pub container: ContainerType,
}

/// A constant in the shader - a bit of data that doesn't vary
// between the shader execution units (vertices/pixels/etc).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct ConstVar {
    /// Name of this constant.
    pub name: String,
    /// Location of this constant in the program.
    /// For constant buffer elements, it's the offset in bytes.
    pub location: Location,
    /// Number of elements this constant represents.
    pub count: usize,
    /// Type that this constant is composed of
    pub base_type: BaseType,
    /// "Scalarness" of this constant.
    pub container: ContainerType,
}

/// A constant buffer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct ConstantBufferVar {
    /// Name of this constant buffer.
    pub name: String,
    /// Slot of the constant buffer.
    pub slot: ConstantBufferSlot,
    /// Size (in bytes) of this buffer's data.
    pub size: usize,
    /// What program stage this buffer is used in.
    pub usage: Usage,
    /// List of individual elements in this buffer.
    pub elements: Vec<ConstVar>,
}

/// Texture shader parameter.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct TextureVar {
    /// Name of this texture variable.
    pub name: String,
    /// Slot of this texture variable.
    pub slot: ResourceViewSlot,
    /// Base type for the texture.
    pub base_type: BaseType,
    /// Type of this texture.
    pub ty: TextureType,
    /// What program stage this texture is used in.
    pub usage: Usage,
}

/// Unordered access shader parameter.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct UnorderedVar {
    /// Name of this unordered variable.
    pub name: String,
    /// Slot of this unordered variable.
    pub slot: UnorderedViewSlot,
    /// What program stage this UAV is used in.
    pub usage: Usage,
}

/// Sampler shader parameter.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SamplerVar {
    /// Name of this sampler variable.
    pub name: String,
    /// Slot of this sampler variable.
    pub slot: SamplerSlot,
    /// Type of this sampler.
    pub ty: SamplerType,
    /// What program stage this texture is used in.
    pub usage: Usage,
}

/// Target output variable.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct OutputVar {
    /// Name of this output variable.
    pub name: String,
    /// Output color target index.
    pub slot: ColorSlot,
    /// Type of the output component.
    pub base_type: BaseType,
    /// "Scalarness" of this output.
    pub container: ContainerType,
}

/// Metadata about a program.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct ProgramInfo {
    /// Attributes in the program
    pub vertex_attributes: Vec<AttributeVar>,
    /// Global constants in the program
    pub globals: Vec<ConstVar>,
    /// Constant buffers in the program
    pub constant_buffers: Vec<ConstantBufferVar>,
    /// Textures in the program
    pub textures: Vec<TextureVar>,
    /// Unordered access resources in the program
    pub unordereds: Vec<UnorderedVar>,
    /// Samplers in the program
    pub samplers: Vec<SamplerVar>,
    /// Output targets in the program
    pub outputs: Vec<OutputVar>,
    /// A flag indicating that the pixel shader manually assigns the depth.
    pub output_depth: bool,
    /// A hacky flag to make sure the clients know we are
    /// unable to actually get the output variable info
    pub knows_outputs: bool,
}

/// A program
#[derive(Debug)]
pub struct Program<R: Resources> {
    resource: R::Program,
    info: ProgramInfo,
}

impl<R: Resources> Program<R> {
    #[doc(hidden)]
    pub fn new(resource: R::Program, info: ProgramInfo) -> Self {
        Program {
            resource: resource,
            info: info,
        }
    }

    #[doc(hidden)]
    pub fn resource(&self) -> &R::Program { &self.resource }

    /// Get program info
    pub fn get_info(&self) -> &ProgramInfo { &self.info }
}

impl<R: Resources + cmp::PartialEq> cmp::PartialEq for Program<R> {
    fn eq(&self, other: &Program<R>) -> bool {
        self.resource().eq(other.resource())
    }
}

impl<R: Resources + cmp::Eq> cmp::Eq for Program<R> {}

impl<R: Resources + hash::Hash> hash::Hash for Program<R> {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.resource().hash(state);
    }
}

/// Error type for trying to store a UniformValue in a ConstVar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityError {
    /// Array sizes differ between the value and the var (trying to upload a vec2 as a vec4, etc)
    ErrorArraySize,
    /// Base types differ between the value and the var (trying to upload a f32 as a u16, etc)
    ErrorBaseType,
    /// Container-ness differs between the value and the var (trying to upload a scalar as a vec4,
    /// etc)
    ErrorContainer,
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for CompatibilityError {
    fn description(&self) -> &str {
        match *self {
            CompatibilityError::ErrorArraySize =>
                "Array sizes differ between the value and the var \
                 (trying to upload a vec2 as a vec4, etc)",
            CompatibilityError::ErrorBaseType =>
                "Base types differ between the value and the var \
                 (trying to upload a f32 as a u16, etc)",
            CompatibilityError::ErrorContainer =>
                "Container-ness differs between the value and the var \
                 (trying to upload a scalar as a vec4, etc)",
        }
    }
}

impl ConstVar {
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

/// An error type for creating shaders.
#[derive(Clone, Debug, PartialEq)]
pub enum CreateShaderError {
    /// The device does not support the requested shader model.
    ModelNotSupported,
    /// The device does not support the shader stage.
    StageNotSupported(Stage),
    /// The shader failed to compile.
    CompilationFailed(String),
}

impl fmt::Display for CreateShaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let desc = self.description();
        match *self {
            CreateShaderError::StageNotSupported(ref stage) => write!(f, "{}: {:?}", desc, stage),
            CreateShaderError::CompilationFailed(ref string) => write!(f, "{}: {}", desc, string),
            _ => write!(f, "{}", desc),
        }
    }
}

impl Error for CreateShaderError {
    fn description(&self) -> &str {
        match *self {
            CreateShaderError::ModelNotSupported => "The device does not support the requested shader model",
            CreateShaderError::StageNotSupported(_) => "The device does not support the shader stage",
            CreateShaderError::CompilationFailed(_) => "The shader failed to compile",
        }
    }
}

/// An error type for creating programs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProgramError(String);

impl fmt::Display for CreateProgramError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&self.0)
    }
}

impl Error for CreateProgramError {
    fn description(&self) -> &str {
        &self.0
    }
}

impl<S: Into<String>> From<S> for CreateProgramError {
    fn from(s: S) -> CreateProgramError {
        CreateProgramError(s.into())
    }
}
