//! Input Assembler (IA) stage description.
//! The input assembler collects raw vertex and index data.

use crate::format;
use crate::Primitive;

/// Shader binding location.
pub type Location = u32;
/// Index of a vertex buffer.
pub type BufferIndex = u32;
/// Offset of an attribute from the start of the buffer, in bytes
pub type ElemOffset = u32;
/// Offset between attribute values, in bytes
pub type ElemStride = u32;
/// Number of instances between each advancement of the vertex buffer.
pub type InstanceRate = u8;

/// The rate at which to advance input data to shaders for the given buffer
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum VertexInputRate {
    /// Advance the buffer after every vertex
    Vertex,
    /// Advance the buffer after every instance
    Instance(InstanceRate),
}

impl VertexInputRate {
    /// Get the numeric representation of the rate
    pub fn as_uint(&self) -> u8 {
        match *self {
            VertexInputRate::Vertex => 0,
            VertexInputRate::Instance(divisor) => divisor,
        }
    }
}

/// A struct element descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Element<F> {
    /// Element format
    pub format: F,
    /// Offset from the beginning of the container, in bytes
    pub offset: ElemOffset,
}

/// Vertex buffer description. Notably, completely separate from resource `Descriptor`s
/// used in `DescriptorSet`s.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VertexBufferDesc {
    /// Binding number of this vertex buffer. This binding number is
    /// used only for vertex buffers, and is completely separate from
    /// `Descriptor` and `DescriptorSet` bind points.
    pub binding: BufferIndex,
    /// Total container size, in bytes.
    /// Specifies the byte distance between two consecutive elements.
    pub stride: ElemStride,
    /// The rate at which to advance data for the given buffer
    ///
    /// i.e. the rate at which data passed to shaders will get advanced by
    /// `stride` bytes
    pub rate: VertexInputRate,
}

/// Vertex attribute description. Notably, completely separate from resource `Descriptor`s
/// used in `DescriptorSet`s.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AttributeDesc {
    /// Attribute binding location in the shader. Attribute locations are
    /// shared between all vertex buffers in a pipeline, meaning that even if the
    /// data for this attribute comes from a different vertex buffer, it still cannot
    /// share the same location with another attribute.
    pub location: Location,
    /// Binding number of the associated vertex buffer.
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
