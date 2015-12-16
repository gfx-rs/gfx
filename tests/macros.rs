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

gfx_structure!(Vertex {
    x@ _x: i8,
    y@ _y: f32,
});

gfx_structure!(Instance {
    alpha@ _alpha: f32,
});

gfx_structure!(Local {
    pos@ _pos: [u32; 4],
});

gfx_tex_format!(Float4 = gfx::tex::RGBA8);
gfx_tex_format!(Rgba = gfx::tex::RGBA8);
gfx_tex_format!(Depth = gfx::tex::Format::DEPTH24);
/// These should not be allowed, TODO
impl gfx::DepthStencilFormat for Depth {}
impl gfx::DepthFormat for Depth {}

gfx_pipeline_init!( _Data _Meta _Init {
    _vertex: gfx::VertexBuffer<Vertex> = gfx::PER_VERTEX,
    _instance: gfx::VertexBuffer<Instance> = gfx::PER_INSTANCE,
    _const_locals: gfx::ConstantBuffer<Local> = "Locals",
    _global: gfx::Global<[f32; 4]> = "Global",
    tex_diffuse: gfx::ResourceView<Float4> = "Diffuse",
    sampler_linear: gfx::Sampler = "Linear",
    buf_frequency: gfx::UnorderedView<Float4> = "Frequency",
    pixel_color: gfx::RenderTarget<Rgba> = ("Color", gfx::state::MASK_ALL),
    depth: gfx::DepthTarget<Depth> = gfx::state::Depth {
        fun: gfx::state::Comparison::LessEqual,
        write: false,
    },
});

fn _test_pso<R, F>(factory: &mut F, shaders: &gfx::ShaderSet<R>)
             -> gfx::PipelineState<R, _Meta>  where
    R: gfx::Resources,
    F: gfx::traits::FactoryExt<R>,
{
    use std::default::Default;
    factory.create_pipeline_state(shaders, gfx::Primitive::Point,
        Default::default(), &_Init::new()).unwrap()
}
