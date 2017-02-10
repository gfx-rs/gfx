#[macro_use] extern crate gfx;
#[macro_use] extern crate gfx_macros;

#[derive(GfxStruct)]
struct Vertex {
    pad: [u8; 4],
}
