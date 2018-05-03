use conversions::map_format;

use metal;
use hal::format::Format;
use hal::image::Filter;

use std::mem;
use std::collections::HashMap;
use std::path::Path;


pub struct BlitVertex {
    pub uv: [f32; 4],
    pub pos: [f32; 4],
}

//#[derive(Clone)]
pub struct ServicePipes {
    library: metal::Library,
    sampler_nearest: metal::SamplerState,
    sampler_linear: metal::SamplerState,
    blits: HashMap<Format, metal::RenderPipelineState>,
}

impl ServicePipes {
    pub fn new(device: &metal::DeviceRef) -> Self {
        let lib_path = Path::new(env!("OUT_DIR"))
            .join("gfx_shaders.metallib");
        let library = device.new_library_with_file(lib_path).unwrap();

        let sampler_desc = metal::SamplerDescriptor::new();
        sampler_desc.set_min_filter(metal::MTLSamplerMinMagFilter::Nearest);
        sampler_desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Nearest);
        sampler_desc.set_mip_filter(metal::MTLSamplerMipFilter::Nearest);
        let sampler_nearest = device.new_sampler(&sampler_desc);
        sampler_desc.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        let sampler_linear = device.new_sampler(&sampler_desc);

        ServicePipes {
            blits: HashMap::new(),
            sampler_nearest,
            sampler_linear,
            library,
        }
    }

    pub fn get_sampler(&self, filter: Filter) -> metal::SamplerState {
        match filter {
            Filter::Nearest => self.sampler_nearest.clone(),
            Filter::Linear => self.sampler_linear.clone(),
        }
    }

    pub fn get_blit(
        &mut self, format: Format, device: &metal::DeviceRef
    ) -> &metal::RenderPipelineStateRef {
        let lib = &self.library;
        self.blits
            .entry(format)
            .or_insert_with(|| Self::create_blit(format, lib, device))
    }

    fn create_blit(
        format: Format, library: &metal::LibraryRef, device: &metal::DeviceRef
    ) -> metal::RenderPipelineState {
        let pipeline = metal::RenderPipelineDescriptor::new();
        pipeline.set_input_primitive_topology(metal::MTLPrimitiveTopologyClass::Triangle);

        let vs_blit = library.get_function("vs_blit", None).unwrap();
        let ps_blit = library.get_function("ps_blit", None).unwrap();
        pipeline.set_vertex_function(Some(&vs_blit));
        pipeline.set_fragment_function(Some(&ps_blit));

        pipeline
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(map_format(format).unwrap());

        // Vertex buffers
        let vertex_descriptor = metal::VertexDescriptor::new();
        let mtl_buffer_desc = vertex_descriptor
            .layouts()
            .object_at(0)
            .unwrap();
        mtl_buffer_desc.set_stride(mem::size_of::<BlitVertex>() as _);
        for i in 0 .. 2 {
            let mtl_attribute_desc = vertex_descriptor
                .attributes()
                .object_at(i)
                .expect("too many vertex attributes");
            mtl_attribute_desc.set_buffer_index(0);
            mtl_attribute_desc.set_offset((i * mem::size_of::<[f32; 2]>()) as _);
            mtl_attribute_desc.set_format(metal::MTLVertexFormat::Float2);
        }
        pipeline.set_vertex_descriptor(Some(&vertex_descriptor));

        device.new_render_pipeline_state(&pipeline).unwrap()
    }
}
