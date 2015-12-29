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

gfx_vertex_struct!(Vertex {
    _x: i8 = "x",
    _y: f32 = "y",
});

gfx_vertex_struct!(Instance {
    pos: [f32; 2] = "pos",
    color: [f32; 3] = "color",
});

gfx_constant_struct!(Local {
    _pos: [u32; 4] = "pos",
});

gfx_pipeline_init!( _Data _Meta _Init {
    _vertex: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    _instance: gfx::VertexBuffer<Instance> = gfx::PER_INSTANCE,
    _const_locals: gfx::ConstantBuffer<Local> = "Locals",
    _global: gfx::Global<[f32; 4]> = "Global",
    tex_diffuse: gfx::ResourceView<[f32; 4]> = "Diffuse",
    sampler_linear: gfx::Sampler = "Linear",
    buf_frequency: gfx::UnorderedView<[f32; 4]> = "Frequency",
    pixel_color: gfx::RenderTarget<gfx::format::Rgba8> = ("Color", gfx::state::MASK_ALL),
    depth: gfx::DepthTarget<gfx::format::DepthStencil> = gfx::state::Depth {
        fun: gfx::state::Comparison::LessEqual,
        write: false,
    },
});

fn _test_pso<R, F>(factory: &mut F, shaders: &gfx::ShaderSet<R>)
             -> gfx::PipelineState<R, _Meta>  where
    R: gfx::Resources,
    F: gfx::traits::FactoryExt<R>,
{
    factory.create_pipeline_state(shaders,
        gfx::Primitive::Point,
        gfx::state::Rasterizer::new_fill(gfx::state::CullFace::Nothing),
        &_Init::new()
        ).unwrap()
}
