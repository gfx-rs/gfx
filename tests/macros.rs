#[macro_use]
extern crate gfx;


gfx_vertex!(_Foo {
    x@ _x: i8,
    y@ _y: f32,
    z@ _z: [u32; 4],
});

gfx_parameters!(_Bar {
    x@ _x: i32,
    y@ _y: [f32; 4],
    b@ _b: gfx::handle::RawBuffer<R>,
    t@ _t: gfx::shade::TextureParam<R>,
});

gfx_structure!(Vertex: _VertexDef {
    x@ _x: i8,
    y@ _y: f32,
    z@ _z: [u32; 4],
});

gfx_structure!(Instance: _InstanceDef {
    alpha@ _alpha: f32,
});

gfx_shader_link!( _Shader: _ShaderDef {
    v@ _vertex: gfx::VertexBuffer<R, Vertex>,
    i@ _instance: gfx::InstanceBuffer<R, Instance>,
    //const_locals: ConstantBuffer<Local>,
    //const_globals: ConstantBuffer<Global>,
    //tex_diffuse: TextureView<Dim2, Float4>,
    //tex_normal: TextureView<Dim2, Float3>,
    //sampler_linear: Sampler,
    //buf_noise: BufferView<Int4>,
    //buf_frequency: UnorderedView<Dim2, Int>,
    //pixel_color: RenderView<Float4>,
    //depth: DepthStencilView,
});
