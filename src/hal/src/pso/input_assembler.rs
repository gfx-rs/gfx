//! Input Assembler (IA) stage description.
//! The input assembler collects raw vertex and index data.

use format;
use {Primitive};

/// Shader binding location.
pub type Location = u32;
/// Index of a vertex buffer.
pub type BufferIndex = u32;
/// Offset of an attribute from the start of the buffer, in bytes
pub type ElemOffset = u32;
/// Offset between attribute values, in bytes
pub type ElemStride = u32;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;

/// A struct element descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Element<F> {
    /// Element format
    pub format: F,
    /// Offset from the beginning of the container, in bytes
    pub offset: ElemOffset,
}

/// Vertex buffer descriptor
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VertexBufferDesc {
    /// Binding number of this vertex buffer descriptor.
    pub binding: BufferIndex,
    /// Total container size, in bytes.
    /// Specifies the byte distance between two consecutive elements.
    pub stride: ElemStride,
    /// Rate of the input for the given buffer
    pub rate: InstanceRate,
}

/// PSO vertex attribute descriptor
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AttributeDesc {
    /// Attribute binding location in the shader.
    pub location: Location,
    /// Binding number of the associated vertex buffer descriptor.
    pub binding: BufferIndex,
    /// Attribute element description.
    pub element: Element<format::Format>,
}

/// Describes whether or not primitive restart is supported for
/// an input assembler. Primitive restart is a feature that
/// allows a mark to be placed in an index buffer where it is
/// is "broken" into multiple pieces of geometry.
///
/// See <https://www.khronos.org/opengl/wiki/Vertex_Rendering#Primitive_Restart>
/// for more detail.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PrimitiveRestart {
    /// No primitive restart.
    Disabled,
    /// Primitive restart using a 16-bit index value (`std::u16::MAX`).
    U16,
    /// Primitive restart using a 32-bit index value (`std::u32::MAX`)
    U32,
}

/// All the information needed to create an input assembler.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InputAssemblerDesc {
    /// Type of the primitive
    pub primitive: Primitive,
    /// The primitive restart specification.
    pub primitive_restart: PrimitiveRestart,
}

impl InputAssemblerDesc {
    /// Create a new IA descriptor without primitive restart
    pub fn new(primitive: Primitive) -> Self {
        InputAssemblerDesc {
            primitive,
            primitive_restart: PrimitiveRestart::Disabled,
        }
    }
}
