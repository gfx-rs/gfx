use conversions as conv;

use metal;
use hal::pso;
use hal::backend::FastHashMap;
use hal::command::ClearColorRaw;
use hal::format::{Aspects, ChannelType};
use hal::image::Filter;

use std::mem;
use std::path::Path;
use std::sync::{Mutex, RwLock};


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
    map: FastHashMap<pso::DepthStencilDesc, metal::DepthStencilState>,
    write_none: pso::DepthStencilDesc,
    write_depth: pso::DepthStencilDesc,
    write_stencil: pso::DepthStencilDesc,
    write_all: pso::DepthStencilDesc,
}

impl DepthStencilStates {
    fn new(device: &metal::DeviceRef) -> Self {
        let write_none = pso::DepthStencilDesc {
            depth: pso::DepthTest::Off,
            depth_bounds: false,
            stencil: pso::StencilTest::Off,
        };
        let write_depth = pso::DepthStencilDesc {
            depth: pso::DepthTest::On {
                fun: pso::Comparison::Always,
                write: true,
            },
            depth_bounds: false,
            stencil: pso::StencilTest::Off,
        };
        let face = pso::StencilFace {
            fun: pso::Comparison::Always,
            mask_read: pso::State::Static(!0),
            mask_write: pso::State::Static(!0),
            op_fail: pso::StencilOp::Replace,
            op_depth_fail: pso::StencilOp::Replace,
            op_pass: pso::StencilOp::Replace,
            reference: pso::State::Dynamic, //irrelevant
        };
        let write_stencil = pso::DepthStencilDesc {
            depth: pso::DepthTest::Off,
            depth_bounds: false,
            stencil: pso::StencilTest::On {
                front: face,
                back: face,
            },
        };
        let write_all = pso::DepthStencilDesc {
            depth: pso::DepthTest::On {
                fun: pso::Comparison::Always,
                write: true,
            },
            depth_bounds: false,
            stencil: pso::StencilTest::On {
                front: face,
                back: face,
            },
        };

        let mut map = FastHashMap::default();
        for desc in &[&write_none, &write_depth, &write_stencil, &write_all] {
            let raw_desc = Self::create_desc(desc).unwrap();
            let raw = device.new_depth_stencil_state(&raw_desc);
            map.insert(**desc, raw);
        }

        DepthStencilStates {
            map,
            write_none,
            write_depth,
            write_stencil,
            write_all,
        }
    }

    pub fn get_write(&self, aspects: Aspects) -> &metal::DepthStencilStateRef {
        let key = if aspects.contains(Aspects::DEPTH | Aspects::STENCIL) {
            &self.write_all
        } else if aspects.contains(Aspects::DEPTH) {
            &self.write_depth
        } else if aspects.contains(Aspects::STENCIL) {
            &self.write_stencil
        } else {
            &self.write_none
        };
        self.map.get(key).unwrap()
    }

    pub fn prepare(&mut self, desc: &pso::DepthStencilDesc, device: &metal::DeviceRef) {
        use std::collections::hash_map::Entry;

        if let Entry::Vacant(e) = self.map.entry(*desc) {
            if let Some(raw_desc) = Self::create_desc(desc) {
                e.insert(device.new_depth_stencil_state(&raw_desc));
            }
        }
    }

    fn create_stencil(face: &pso::StencilFace) -> Option<metal::StencilDescriptor> {
        let desc = metal::StencilDescriptor::new();
        desc.set_stencil_compare_function(conv::map_compare_function(face.fun));
        desc.set_read_mask(match face.mask_read {
            pso::State::Static(value) => value,
            pso::State::Dynamic => return None,
        });
        desc.set_write_mask(match face.mask_write {
            pso::State::Static(value) => value,
            pso::State::Dynamic => return None,
        });
        desc.set_stencil_failure_operation(conv::map_stencil_op(face.op_fail));
        desc.set_depth_failure_operation(conv::map_stencil_op(face.op_depth_fail));
        desc.set_depth_stencil_pass_operation(conv::map_stencil_op(face.op_pass));
        Some(desc)
    }

    fn create_desc(desc: &pso::DepthStencilDesc) -> Option<metal::DepthStencilDescriptor> {
        let raw = metal::DepthStencilDescriptor::new();

        match desc.depth {
            pso::DepthTest::On { fun, write } => {
                raw.set_depth_compare_function(conv::map_compare_function(fun));
                raw.set_depth_write_enabled(write);
            }
            pso::DepthTest::Off => {}
        }
        match desc.stencil {
            pso::StencilTest::On { ref front, ref back } => {
                let front_desc = Self::create_stencil(front)?;
                raw.set_front_face_stencil(Some(&front_desc));
                let back_desc = if front == back {
                    front_desc
                } else {
                    Self::create_stencil(back)?
                };
                raw.set_back_face_stencil(Some(&back_desc));
            }
            pso::StencilTest::Off => {}
        }

        Some(raw)
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

    pub fn create(
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
    pub library: Mutex<metal::Library>,
    pub sampler_states: SamplerStates,
    pub depth_stencil_states: RwLock<DepthStencilStates>,
    pub clears: Mutex<ImageClearPipes>,
    pub blits: Mutex<ImageBlitPipes>,
    pub copy_buffer: metal::ComputePipelineState,
    pub fill_buffer: metal::ComputePipelineState,
}

impl ServicePipes {
    pub fn new(device: &metal::DeviceRef) -> Self {
        let lib_path = Path::new(env!("OUT_DIR"))
            .join("gfx_shaders.metallib");
        let library = device.new_library_with_file(lib_path).unwrap();

        let copy_buffer = Self::create_copy_buffer(&library, device);
        let fill_buffer = Self::create_fill_buffer(&library, device);

        ServicePipes {
            library: Mutex::new(library),
            sampler_states: SamplerStates::new(device),
            depth_stencil_states: RwLock::new(DepthStencilStates::new(device)),
            clears: Mutex::new(ImageClearPipes {
                map: FastHashMap::default(),
            }),
            blits: Mutex::new(ImageBlitPipes {
                map: FastHashMap::default(),
            }),
            copy_buffer,
            fill_buffer,
        }
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

    pub fn with_depth_stencil<T, F: FnOnce(&metal::DepthStencilStateRef) -> T>(
        &self,
        desc: pso::DepthStencilDesc,
        device: &Mutex<metal::Device>,
        fun: F,
    ) -> T {
        if let Some(state) = self.depth_stencil_states.read().unwrap().map.get(&desc) {
            return fun(state);
        }
        let raw_desc = DepthStencilStates::create_desc(&desc)
            .expect("Incomplete descriptor provided");
        let state = device
            .lock()
            .unwrap()
            .new_depth_stencil_state(&raw_desc);
        fun(self.depth_stencil_states
            .write()
            .unwrap()
            .map
            .entry(desc)
            .or_insert(state)
        )
    }
}
