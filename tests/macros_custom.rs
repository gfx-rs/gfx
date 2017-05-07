#[macro_use]
extern crate gfx as mygfx;
pub use mygfx::format as fm;

#[derive(Clone, Debug)]
pub struct Rg16;
gfx_format!(Rg16: R16_G16 = Vec2<Float>);

gfx_defines!{
    vertex Vertex {
        _x: i8 = "x",
        _y: f32 = "y",
    }
    
    vertex Instance {
        pos: [f32; 2] = "pos",
        color: [f32; 3] = "color",
    }
    
    constant Local {
        pos: [u32; 4] = "pos",
    }
    
    pipeline mygfx:testpipe {
        vertex: mygfx::VertexBuffer<Vertex> = (),
        instance: mygfx::InstanceBuffer<Instance> = (),
        const_locals: mygfx::ConstantBuffer<Local> = "Locals",
        global: mygfx::Global<[f32; 4]> = "Global",
        tex_diffuse: mygfx::ShaderResource<[f32; 4]> = "Diffuse",
        sampler_linear: mygfx::Sampler = "Linear",
        buf_frequency: mygfx::UnorderedAccess<[f32; 4]> = "Frequency",
        pixel_color: mygfx::RenderTarget<fm::Rgba8> = "Color",
        blend_target: mygfx::BlendTarget<Rg16> =
            ("o_Color1", mygfx::state::MASK_ALL, mygfx::preset::blend::ADD),
        depth: mygfx::DepthTarget<mygfx::format::DepthStencil> =
            mygfx::preset::depth::LESS_EQUAL_TEST,
        blend_ref: mygfx::BlendRef = (),
        scissor: mygfx::Scissor = (),
    }
}

fn _test_pso<R, F>(factory: &mut F) -> mygfx::PipelineState<R, testpipe::Meta> where
    R: mygfx::Resources,
    F: mygfx::traits::FactoryExt<R>,
{
    factory.create_pipeline_simple(&[], &[], testpipe::new()).unwrap()
}


gfx_pipeline_base!( mygfx:testraw {
    vertex: mygfx::RawVertexBuffer,
    cbuf: mygfx::RawConstantBuffer,
    tex: mygfx::RawShaderResource,
    target: mygfx::RawRenderTarget,
});

fn _test_raw<R, F>(factory: &mut F) -> mygfx::PipelineState<R, testraw::Meta> where
    R: mygfx::Resources,
    F: mygfx::traits::FactoryExt<R>,
{
    let special = mygfx::pso::buffer::Element {
        format: fm::Format(fm::SurfaceType::R32, fm::ChannelType::Float),
        offset: 0,
    };
    let init = testraw::Init {
        vertex: (&[("a_Special", special)], 12, 0),
        cbuf: "Locals",
        tex: "Specular",
        target: ("o_Color2",
            fm::Format(fm::SurfaceType::R8_G8_B8_A8, fm::ChannelType::Unorm),
            mygfx::state::MASK_ALL, None),
    };
    factory.create_pipeline_simple(&[], &[], init).unwrap()
}
