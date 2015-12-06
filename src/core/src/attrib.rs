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

//! Vertex attribute types.

#![allow(missing_docs)]

use shade::BaseType;

/// Number of elements per attribute, only 1 to 4 are supported
pub type Count = u8;
/// Offset of an attribute from the start of the buffer, in bytes
pub type Offset = u32;
/// Offset between attribute values, in bytes
pub type Stride = u8;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;

/// The signedness of an attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SignFlag {
    Signed,
    Unsigned,
}

/// Describes how an integer value is interpreted by the shader.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum IntSubType {
    Raw,         // un-processed integer
    Normalized,  // normalized either to [0,1] or [-1,1] depending on the sign flag
    AsFloat,     // converted to float on the fly by the hardware
}

/// The size of an integer attribute, in bits.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum IntSize {
    U8,
    U16,
    U32,
}

/// Type of a floating point attribute on the shader side.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum FloatSubType {
    Default,    // 32-bit
    Precision,  // 64-bit
}

/// The size of a floating point attribute, in bits.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum FloatSize {
    F16,
    F32,
    F64,
}

/// The type of an attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Type {
    Int(IntSubType, IntSize, SignFlag),
    Float(FloatSubType, FloatSize),
    Special,
}

impl Type {
    /// Check if the attribute is compatible with a particular shader type.
    pub fn is_compatible(&self, bt: BaseType) -> Result<(), ()> {
        match (*self, bt) {
            (Type::Int(IntSubType::Raw, _, _), BaseType::I32) => Ok(()),
            (Type::Int(IntSubType::Raw, _, SignFlag::Unsigned), BaseType::U32) => Ok(()),
            (Type::Int(IntSubType::Raw, _, _), _) => Err(()),
            (Type::Int(_, _, _), BaseType::F32) => Ok(()),
            (Type::Int(_, _, _), _) => Err(()),
            (Type::Float(_, _), BaseType::F32) => Ok(()),
            (Type::Float(FloatSubType::Precision, FloatSize::F64), BaseType::F64) => Ok(()),
            (Type::Float(_, _), _) => Err(()),
            (_, BaseType::F64) => Err(()),
            (_, BaseType::Bool) => Err(()),
            _ => Err(()),
        }
    }

    /// Return the size of the type in bytes.
    pub fn get_size(&self) -> u8 {
        match *self {
            Type::Int(_, IntSize::U8, _) => 1,
            Type::Int(_, IntSize::U16, _) => 2,
            Type::Int(_, IntSize::U32, _) => 4,
            Type::Float(_, FloatSize::F16) => 2,
            Type::Float(_, FloatSize::F32) => 4,
            Type::Float(_, FloatSize::F64) => 8,
            Type::Special => 0,
        }
    }
}

/// Complete format of a vertex attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Format {
    /// Number of elements per vertex
    pub elem_count: Count,
    /// Type of a single element
    pub elem_type: Type,
    /// Offset in bytes to the first vertex
    pub offset: Offset,
    /// Stride in bytes between consecutive vertices
    pub stride: Stride,
    /// Instance rate per vertex
    pub instance_rate: InstanceRate,
}


/// Fixed-point version of integer attributes.
#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub struct FixedPoint<T>(pub T);

impl<T: Copy> FixedPoint<T> {
    /// Cast a fixed-size2 array to fixed-point.
    pub fn cast2(a: [T; 2]) -> [FixedPoint<T>; 2] {
        [FixedPoint(a[0]), FixedPoint(a[1])]
    }
    /// Cast a fixed-size3 array to fixed-point.
    pub fn cast3(a: [T; 3]) -> [FixedPoint<T>; 3] {
        [FixedPoint(a[0]), FixedPoint(a[1]), FixedPoint(a[2])]
    }
    /// Cast a fixed-size4 array to fixed-point.
    pub fn cast4(a: [T; 4]) -> [FixedPoint<T>; 4] {
        [FixedPoint(a[0]), FixedPoint(a[1]),
         FixedPoint(a[2]), FixedPoint(a[3])]
    }
}

/// Floating-point version of integer attributes.
#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub struct Floater<T>(pub T);

impl<T: Copy> Floater<T> {
    /// Cast a fixed-size2 array to floating-point.
    pub fn cast2(a: [T; 2]) -> [Floater<T>; 2] {
        [Floater(a[0]), Floater(a[1])]
    }
    /// Cast a fixed-size3 array to floating-point.
    pub fn cast3(a: [T; 3]) -> [Floater<T>; 3] {
        [Floater(a[0]), Floater(a[1]), Floater(a[2])]
    }
    /// Cast a fixed-size4 array to floating-point.
    pub fn cast4(a: [T; 4]) -> [Floater<T>; 4] {
        [Floater(a[0]), Floater(a[1]),
         Floater(a[2]), Floater(a[3])]
    }
}

/// A service module for deriving `ToFormat` for primitive types.
pub mod format {
    use super::{Count, FixedPoint, Floater, Type};
    use super::Type::*;
    use super::FloatSize::*;
    use super::FloatSubType::*;
    use super::IntSize::*;
    use super::IntSubType::*;
    use super::SignFlag::*;

    /// A trait for getting the format out of vertex element types.
    /// Needed to implement `VertexFormat` with a macro.
    pub trait ToFormat {
        fn describe() -> (Count, Type);
    }

    /// A helper trait for implementing ToFormat.
    pub trait ToType {
        fn describe() -> Type;
    }

    impl<T: ToType> ToFormat for T {
        fn describe() -> (Count, Type) {
            (1, T::describe())
        }
    }
    impl<T: ToType> ToFormat for [T; 2] {
        fn describe() -> (Count, Type) {
            (2, T::describe())
        }
    }
    impl<T: ToType> ToFormat for [T; 3] {
        fn describe() -> (Count, Type) {
            (3, T::describe())
        }
    }
    impl<T: ToType> ToFormat for [T; 4] {
        fn describe() -> (Count, Type) {
            (4, T::describe())
        }
    }

    impl ToType for f32 {
        fn describe() -> Type { Float(Default, F32) }
    }
    impl ToType for f64 {
        fn describe() -> Type { Float(Precision, F64) }
    }
    impl ToType for u8 {
        fn describe() -> Type { Int(Raw, U8, Unsigned) }
    }
    impl ToType for u16 {
        fn describe() -> Type { Int(Raw, U16, Unsigned) }
    }
    impl ToType for u32 {
        fn describe() -> Type { Int(Raw, U32, Unsigned) }
    }
    impl ToType for i8 {
        fn describe() -> Type { Int(Raw, U8, Signed) }
    }
    impl ToType for i16 {
        fn describe() -> Type { Int(Raw, U16, Signed) }
    }
    impl ToType for i32 {
        fn describe() -> Type { Int(Raw, U32, Signed) }
    }

    impl ToType for FixedPoint<u8> {
        fn describe() -> Type { Int(Normalized, U8, Unsigned) }
    }
    impl ToType for FixedPoint<u16> {
        fn describe() -> Type { Int(Normalized, U16, Unsigned) }
    }
    impl ToType for FixedPoint<u32> {
        fn describe() -> Type { Int(Normalized, U32, Unsigned) }
    }
    impl ToType for FixedPoint<i8> {
        fn describe() -> Type { Int(Normalized, U8, Signed) }
    }
    impl ToType for FixedPoint<i16> {
        fn describe() -> Type { Int(Normalized, U16, Signed) }
    }
    impl ToType for FixedPoint<i32> {
        fn describe() -> Type { Int(Normalized, U32, Signed) }
    }

    impl ToType for Floater<u8> {
        fn describe() -> Type { Int(AsFloat, U8, Unsigned) }
    }
    impl ToType for Floater<u16> {
        fn describe() -> Type { Int(AsFloat, U16, Unsigned) }
    }
    impl ToType for Floater<u32> {
        fn describe() -> Type { Int(AsFloat, U32, Unsigned) }
    }
    impl ToType for Floater<i8> {
        fn describe() -> Type { Int(AsFloat, U8, Signed) }
    }
    impl ToType for Floater<i16> {
        fn describe() -> Type { Int(AsFloat, U16, Signed) }
    }
    impl ToType for Floater<i32> {
        fn describe() -> Type { Int(AsFloat, U32, Signed) }
    }
}
