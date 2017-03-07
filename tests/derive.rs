extern crate gfx;
#[macro_use] extern crate gfx_macros;

#[derive(VertexData)]
struct Vertex {
    pos: [u8; 4],
}

#[derive(ConstantBuffer)]
struct Constant {
    transform: [[f32; 4]; 4],
}
