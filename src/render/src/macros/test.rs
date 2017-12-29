#![allow(dead_code)]

use format;

gfx_buffer_struct! {
    Vertex {
        a_Pos: [f32; 2],
    }
}

gfx_descriptors! {
    desc {
        sampled_image: pso::SampledImage,
        sampler: pso::Sampler,
    }
}

gfx_graphics_pipeline! {
    pipe {
        desc: desc::Component,
        color: pso::RenderTarget<format::Rgba8Srgb>,
        vertices: pso::VertexBuffer<Vertex>,
    }
}

#[test]
fn test_macros() {}
