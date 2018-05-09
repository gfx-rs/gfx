use metal;
use hal::image::{Filter};

use std::mem;
use std::collections::HashMap;
use std::path::Path;


#[derive(Debug)]
pub struct BlitVertex {
    pub uv: [f32; 4],
    pub pos: [f32; 4],
}

pub type BlitKey = (metal::MTLTextureType, metal::MTLPixelFormat);

//#[derive(Clone)]
pub struct ServicePipes {
    library: metal::Library,
    sampler_nearest: metal::SamplerState,
    sampler_linear: metal::SamplerState,
    blits: HashMap<BlitKey, metal::RenderPipelineState>,
    copy_buffer: metal::ComputePipelineState,
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

        let copy_buffer = Self::create_copy_buffer(&library, device);

        ServicePipes {
            blits: HashMap::new(),
            sampler_nearest,
            sampler_linear,
            library,
            copy_buffer
        }
    }

    pub fn get_sampler(&self, filter: Filter) -> metal::SamplerState {
        match filter {
            Filter::Nearest => self.sampler_nearest.clone(),
            Filter::Linear => self.sampler_linear.clone(),
        }
    }

    pub fn get_blit_image(
        &mut self,
        ty: metal::MTLTextureType,
        format: metal::MTLPixelFormat,
        device: &metal::DeviceRef,
    ) -> &metal::RenderPipelineStateRef {
        let lib = &self.library;
        self.blits
            .entry((ty, format))
            .or_insert_with(|| Self::create_blit_image(ty, format, lib, device))
    }

    fn create_blit_image(
        ty: metal::MTLTextureType, format: metal::MTLPixelFormat,
        library: &metal::LibraryRef, device: &metal::DeviceRef,
    ) -> metal::RenderPipelineState {
        use metal::MTLTextureType as Tt;

        let pipeline = metal::RenderPipelineDescriptor::new();
        pipeline.set_input_primitive_topology(metal::MTLPrimitiveTopologyClass::Triangle);

        let ps_name = match ty {
            Tt::D1 => "ps_blit_1d",
            Tt::D1Array => "ps_blit_1d_array",
            Tt::D2 => "ps_blit_2d",
            Tt::D2Array => "ps_blit_2d_array",
            Tt::D3 => "ps_blit_3d",
            Tt::D2Multisample => panic!("Can't blit MSAA surfaces"),
            Tt::Cube |
            Tt::CubeArray => unimplemented!()
        };

        let vs_blit = library.get_function("vs_blit", None).unwrap();
        let ps_blit = library.get_function(ps_name, None).unwrap();
        pipeline.set_vertex_function(Some(&vs_blit));
        pipeline.set_fragment_function(Some(&ps_blit));

        pipeline
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(format);

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
            mtl_attribute_desc.set_offset((i * mem::size_of::<[f32; 4]>()) as _);
            mtl_attribute_desc.set_format(metal::MTLVertexFormat::Float4);
        }
        pipeline.set_vertex_descriptor(Some(&vertex_descriptor));

        device.new_render_pipeline_state(&pipeline).unwrap()
    }

    pub fn get_copy_buffer(&self) -> &metal::ComputePipelineStateRef {
        &self.copy_buffer
    }

    fn create_copy_buffer(
        library: &metal::LibraryRef, device: &metal::DeviceRef
    ) -> metal::ComputePipelineState {
        let pipeline = metal::ComputePipelineDescriptor::new();

        let cs_fill_buffer = library.get_function("cs_copy_buffer", None).unwrap();
        pipeline.set_compute_function(Some(&cs_fill_buffer));
        pipeline.set_thread_group_size_is_multiple_of_thread_execution_width(true);

        if let Some(buffers) = pipeline.buffers() {
            buffers.object_at(0).unwrap().set_mutability(metal::MTLMutability::Mutable);
            buffers.object_at(1).unwrap().set_mutability(metal::MTLMutability::Immutable);
            buffers.object_at(2).unwrap().set_mutability(metal::MTLMutability::Immutable);
        }

        device.new_compute_pipeline_state(&pipeline).unwrap()
    }
}
