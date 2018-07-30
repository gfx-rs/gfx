pub mod buffer;
pub mod image;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw vertex base offset.
pub type VertexOffset = i32;
/// Draw number of indices.
pub type IndexCount = u32;
/// Draw number of instances.
pub type InstanceCount = u32;
/// Number of work groups.
pub type WorkGroupCount = [u32; 3];
