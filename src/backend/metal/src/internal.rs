use metal;
use hal::backend::FastHashMap;
use hal::command::ClearColorRaw;
use hal::format::{Aspects, ChannelType};
use hal::image::Filter;

use std::mem;
use std::path::Path;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct ClearVertex {
    pub pos: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct BlitVertex {
    pub uv: [f32; 4],
    pub pos: [f32; 4],
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Channel {
    Float,
    Int,
    Uint,
}

impl From<ChannelType> for Channel {
    fn from(channel_type: ChannelType) -> Self {
        match channel_type {
            ChannelType::Unorm |
            ChannelType::Inorm |
            ChannelType::Ufloat |
            ChannelType::Float |
            ChannelType::Uscaled |
            ChannelType::Iscaled |
            ChannelType::Srgb => Channel::Float,
            ChannelType::Uint => Channel::Uint,
            ChannelType::Int => Channel::Int,
        }
    }
}

impl Channel {
    pub fn interpret(&self, raw: ClearColorRaw) -> metal::MTLClearColor {
        unsafe {
            match *self {
                Channel::Float => metal::MTLClearColor::new(
                    raw.float32[0] as _,
                    raw.float32[1] as _,
                    raw.float32[2] as _,
                    raw.float32[3] as _,
                ),
                Channel::Int => metal::MTLClearColor::new(
                    raw.int32[0] as _,
                    raw.int32[1] as _,
                    raw.int32[2] as _,
                    raw.int32[3] as _,
                ),
                Channel::Uint => metal::MTLClearColor::new(
                    raw.uint32[0] as _,
                    raw.uint32[1] as _,
                    raw.uint32[2] as _,
                    raw.uint32[3] as _,
                ),
            }
        }
    }
}


pub struct SamplerStates {
    nearest: metal::SamplerState,
    linear: metal::SamplerState,
}

impl SamplerStates {
    fn new(device: &metal::DeviceRef) -> Self {
        let desc = metal::SamplerDescriptor::new();
        desc.set_min_filter(metal::MTLSamplerMinMagFilter::Nearest);
        desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Nearest);
        desc.set_mip_filter(metal::MTLSamplerMipFilter::Nearest);
        let nearest = device.new_sampler(&desc);
        desc.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        let linear = device.new_sampler(&desc);

        SamplerStates {
            nearest,
            linear,
        }
    }

    pub fn get(&self, filter: Filter) -> &metal::SamplerStateRef {
        match filter {
            Filter::Nearest => &self.nearest,
            Filter::Linear => &self.linear,
        }
    }
}

pub struct DepthStencilStates {
    write_depth: metal::DepthStencilState,
    write_stencil: metal::DepthStencilState,
    write_all: metal::DepthStencilState,
}

impl DepthStencilStates {
    fn new(device: &metal::DeviceRef) -> Self {
        let desc = metal::DepthStencilDescriptor::new();
        desc.set_depth_write_enabled(true);
        desc.set_depth_compare_function(metal::MTLCompareFunction::Always);
        let write_depth = device.new_depth_stencil_state(&desc);
        let stencil_desc = metal::StencilDescriptor::new();
        stencil_desc.set_depth_stencil_pass_operation(metal::MTLStencilOperation::Replace);
        desc.set_front_face_stencil(Some(&stencil_desc));
        desc.set_back_face_stencil(Some(&stencil_desc));
        let write_all = device.new_depth_stencil_state(&desc);
        desc.set_depth_write_enabled(false);
        let write_stencil = device.new_depth_stencil_state(&desc);

        DepthStencilStates {
            write_depth,
            write_stencil,
            write_all,
        }
    }

    //TODO: return `Option<metal::DepthStencilState>` instead?
    pub fn get(&self, aspects: Aspects) -> &metal::DepthStencilStateRef {
        if aspects.contains(Aspects::DEPTH | Aspects::STENCIL) {
            &self.write_all
        } else if aspects.contains(Aspects::DEPTH) {
            &self.write_depth
        } else if aspects.contains(Aspects::STENCIL) {
            &self.write_stencil
        } else {
            panic!("Can't write nothing!")
        }
    }
}


#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ClearKey {
    pub framebuffer_aspects: Aspects,
    pub color_formats: [metal::MTLPixelFormat; 1],
    pub depth_stencil_format: metal::MTLPixelFormat,
    pub target_index: Option<(u8, Channel)>,
}

pub struct ImageClearPipes {
    map: FastHashMap<ClearKey, metal::RenderPipelineState>,
}

impl ImageClearPipes {
    pub fn get(
        &mut self,
        key: ClearKey,
        library: &metal::LibraryRef,
        device: &Mutex<metal::Device>,
    ) -> &metal::RenderPipelineStateRef {
        self.map
            .entry(key)
            .or_insert_with(|| Self::create(key, library, &*device.lock().unwrap()))
    }

    fn create(
        key: ClearKey, library: &metal::LibraryRef, device: &metal::DeviceRef,
    ) -> metal::RenderPipelineState {
        let pipeline = metal::RenderPipelineDescriptor::new();
        pipeline.set_input_primitive_topology(metal::MTLPrimitiveTopologyClass::Triangle);

        let vs_clear = library.get_function("vs_clear", None).unwrap();
        pipeline.set_vertex_function(Some(&vs_clear));

        if key.framebuffer_aspects.contains(Aspects::COLOR) {
            for (i, &format) in key.color_formats.iter().enumerate() {
                pipeline
                    .color_attachments()
                    .object_at(i)
                    .unwrap()
                    .set_pixel_format(format);
            }
        }
        if key.framebuffer_aspects.contains(Aspects::DEPTH) {
            pipeline.set_depth_attachment_pixel_format(key.depth_stencil_format);
        }
        if key.framebuffer_aspects.contains(Aspects::STENCIL) {
            pipeline.set_stencil_attachment_pixel_format(key.depth_stencil_format);
        }

        if let Some((index, channel)) = key.target_index {
            assert!(key.framebuffer_aspects.contains(Aspects::COLOR));
            let s_channel = match channel {
                Channel::Float => "float",
                Channel::Int => "int",
                Channel::Uint => "uint",
            };
            let ps_name = format!("ps_clear{}_{}", index, s_channel);
            let ps_blit = library.get_function(&ps_name, None).unwrap();
            pipeline.set_fragment_function(Some(&ps_blit));
        }

        // Vertex buffers
        let vertex_descriptor = metal::VertexDescriptor::new();
        let mtl_buffer_desc = vertex_descriptor
            .layouts()
            .object_at(0)
            .unwrap();
        mtl_buffer_desc.set_stride(mem::size_of::<ClearVertex>() as _);
        for i in 0 .. 1 {
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
}


pub type BlitKey = (metal::MTLTextureType, metal::MTLPixelFormat, Aspects, Channel);

pub struct ImageBlitPipes {
    map: FastHashMap<BlitKey, metal::RenderPipelineState>,
}

impl ImageBlitPipes {
    pub fn get(
        &mut self,
        key: BlitKey,
        library: &metal::LibraryRef,
        device: &Mutex<metal::Device>,
    ) -> &metal::RenderPipelineStateRef {
        self.map
            .entry(key)
            .or_insert_with(|| Self::create(key, library, &*device.lock().unwrap()))
    }

    fn create(
        key: BlitKey, library: &metal::LibraryRef, device: &metal::DeviceRef,
    ) -> metal::RenderPipelineState {
        use metal::MTLTextureType as Tt;

        let pipeline = metal::RenderPipelineDescriptor::new();
        pipeline.set_input_primitive_topology(metal::MTLPrimitiveTopologyClass::Triangle);

        let s_type = match key.0 {
            Tt::D1 => "1d",
            Tt::D1Array => "1d_array",
            Tt::D2 => "2d",
            Tt::D2Array => "2d_array",
            Tt::D3 => "3d",
            Tt::D2Multisample => panic!("Can't blit MSAA surfaces"),
            Tt::Cube |
            Tt::CubeArray => unimplemented!()
        };
        let s_channel = if key.2.contains(Aspects::COLOR) {
            match key.3 {
                Channel::Float => "float",
                Channel::Int => "int",
                Channel::Uint => "uint",
            }
        } else {
            "depth" //TODO: stencil
        };
        let ps_name = format!("ps_blit_{}_{}", s_type, s_channel);

        let vs_blit = library.get_function("vs_blit", None).unwrap();
        let ps_blit = library.get_function(&ps_name, None).unwrap();
        pipeline.set_vertex_function(Some(&vs_blit));
        pipeline.set_fragment_function(Some(&ps_blit));

        if key.2.contains(Aspects::COLOR) {
            pipeline
                .color_attachments()
                .object_at(0)
                .unwrap()
                .set_pixel_format(key.1);
        }
        if key.2.contains(Aspects::DEPTH) {
            pipeline.set_depth_attachment_pixel_format(key.1);
        }
        if key.2.contains(Aspects::STENCIL) {
            pipeline.set_stencil_attachment_pixel_format(key.1);
        }

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
}


pub struct ServicePipes {
    pub library: metal::Library,
    pub sampler_states: SamplerStates,
    pub depth_stencil_states: DepthStencilStates,
    pub clears: ImageClearPipes,
    pub blits: ImageBlitPipes,
    copy_buffer: metal::ComputePipelineState,
    fill_buffer: metal::ComputePipelineState,
}

impl ServicePipes {
    pub fn new(device: &metal::DeviceRef) -> Self {
        let lib_path = Path::new(env!("OUT_DIR"))
            .join("gfx_shaders.metallib");
        let library = device.new_library_with_file(lib_path).unwrap();

        let copy_buffer = Self::create_copy_buffer(&library, device);
        let fill_buffer = Self::create_fill_buffer(&library, device);

        ServicePipes {
            sampler_states: SamplerStates::new(device),
            depth_stencil_states: DepthStencilStates::new(device),
            clears: ImageClearPipes {
                map: FastHashMap::default(),
            },
            blits: ImageBlitPipes {
                map: FastHashMap::default(),
            },
            library,
            copy_buffer,
            fill_buffer,
        }
    }

    pub fn get_copy_buffer(&self) -> &metal::ComputePipelineStateRef {
        &self.copy_buffer
    }

    fn create_copy_buffer(
        library: &metal::LibraryRef, device: &metal::DeviceRef
    ) -> metal::ComputePipelineState {
        let pipeline = metal::ComputePipelineDescriptor::new();

        let cs_copy_buffer = library.get_function("cs_copy_buffer", None).unwrap();
        pipeline.set_compute_function(Some(&cs_copy_buffer));
        pipeline.set_thread_group_size_is_multiple_of_thread_execution_width(true);

        if let Some(buffers) = pipeline.buffers() {
            buffers.object_at(0).unwrap().set_mutability(metal::MTLMutability::Mutable);
            buffers.object_at(1).unwrap().set_mutability(metal::MTLMutability::Immutable);
            buffers.object_at(2).unwrap().set_mutability(metal::MTLMutability::Immutable);
        }

        device.new_compute_pipeline_state(&pipeline).unwrap()
    }

    pub fn get_fill_buffer(&self) -> &metal::ComputePipelineStateRef {
        &self.fill_buffer
    }

    fn create_fill_buffer(
        library: &metal::LibraryRef, device: &metal::DeviceRef
    ) -> metal::ComputePipelineState {
        let pipeline = metal::ComputePipelineDescriptor::new();

        let cs_fill_buffer = library.get_function("cs_fill_buffer", None).unwrap();
        pipeline.set_compute_function(Some(&cs_fill_buffer));
        pipeline.set_thread_group_size_is_multiple_of_thread_execution_width(true);

        if let Some(buffers) = pipeline.buffers() {
            buffers.object_at(0).unwrap().set_mutability(metal::MTLMutability::Mutable);
            buffers.object_at(1).unwrap().set_mutability(metal::MTLMutability::Immutable);
        }

        device.new_compute_pipeline_state(&pipeline).unwrap()
    }
}
