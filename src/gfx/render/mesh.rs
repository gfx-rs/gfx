use device::dev;

pub type MaterialHandle = int;	//placeholder
pub type VertexCount = u16;
pub type ElementCount = u16;

pub static MAX_ATTRIBUTES : uint = 8;


/// Vertex attribute descriptor, goes into the vertex shader input
pub struct Attribute {
    pub buffer  : dev::Buffer,
    pub count   : uint,         /// number of elements
    pub offset  : uint,         /// can be the middle of the buffer
    pub stride  : u8,           /// should be enough
    pub is_normalized   : bool, /// treat unsigned as fixed-point
    pub is_interpolated : bool, /// allow shader interpolation
    pub name    : String,
}

pub enum PolygonType {
    Point,
    Line,
    LineStrip,
    TriangleList,
    TriangleStrip,
    //Quad,
}

/// Mesh descriptor, as a collection of attributes
pub struct Mesh {
    pub poly_type       : PolygonType,
    pub num_vertices    : VertexCount,
    pub attributes      : [Attribute, ..MAX_ATTRIBUTES],
}

pub enum Slice  {
	VertexSlice(VertexCount, VertexCount),
	IndexSlice(dev::Buffer, ElementCount, ElementCount),
}

/// Slice descriptor with an assigned material
pub struct SubMesh {
    pub mesh: Mesh,
    pub material: MaterialHandle,
    pub slice: Slice,
}
