extern crate gfx;
#[macro_use] extern crate gfx_macros;

#[derive(GfxVertexFormat)]
struct Vertex {
    pad: [u8; 4],
}

#[derive(GfxConstantBuffer)]
struct Constant {
    transform: [[f32; 4]; 4],
}
