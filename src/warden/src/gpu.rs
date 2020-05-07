#[cfg(feature = "glsl-to-spirv")]
use glsl_to_spirv;

use std::collections::hash_map::{Entry, HashMap};
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::PathBuf;
use std::{iter, mem, slice};

use hal::{
    self,
    adapter,
    buffer as b,
    command as c,
    format as f,
    image as i,
    memory,
    prelude::*,
    pso,
    query,
    queue,
};

use crate::raw;

const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: f::Aspects::COLOR,
    levels: 0 .. 1,
    layers: 0 .. 1,
};

pub struct FetchGuard<'a, B: hal::Backend> {
    device: &'a mut B::Device,
    buffer: Option<B::Buffer>,
    memory: Option<B::Memory>,
    mapping: *const u8,
    row_pitch: usize,
    width: usize,
}

impl<'a, B: hal::Backend> FetchGuard<'a, B> {
    pub fn row(&self, i: usize) -> &[u8] {
        let offset = (i * self.row_pitch) as isize;
        unsafe { slice::from_raw_parts(self.mapping.offset(offset), self.width) }
    }
}

impl<'a, B: hal::Backend> Drop for FetchGuard<'a, B> {
    fn drop(&mut self) {
        let buffer = self.buffer.take().unwrap();
        let memory = self.memory.take().unwrap();
        unsafe {
            self.device.unmap_memory(&memory);
            self.device.destroy_buffer(buffer);
            self.device.free_memory(memory);
        }
    }
}

pub struct Buffer<B: hal::Backend> {
    handle: B::Buffer,
    _memory: B::Memory,
    size: usize,
    stable_state: b::State,
}

impl<B: hal::Backend> Buffer<B> {
    fn _barrier_to(&self, access: b::Access) -> memory::Barrier<B> {
        memory::Barrier::whole_buffer(&self.handle, self.stable_state .. access)
    }
    fn barrier_from(&self, access: b::Access) -> memory::Barrier<B> {
        memory::Barrier::whole_buffer(&self.handle, access .. self.stable_state)
    }
    fn barrier<T>(
        &self,
        entry: Entry<T, b::State>,
        access: b::Access,
    ) -> Option<memory::Barrier<B>> {
        let from = mem::replace(entry.or_insert(self.stable_state), access);
        if from != access {
            Some(memory::Barrier::whole_buffer(&self.handle, from .. access))
        } else {
            None
        }
    }
}

pub struct Image<B: hal::Backend> {
    handle: B::Image,
    _memory: B::Memory,
    kind: i::Kind,
    format: f::Format,
    range: i::SubresourceRange,
    stable_state: i::State,
}

impl<B: hal::Backend> Image<B> {
    fn barrier_to(&self, access: i::Access, layout: i::Layout) -> memory::Barrier<B> {
        memory::Barrier::Image {
            states: self.stable_state .. (access, layout),
            target: &self.handle,
            families: None,
            range: self.range.clone(),
        }
    }
    fn barrier_from(&self, access: i::Access, layout: i::Layout) -> memory::Barrier<B> {
        memory::Barrier::Image {
            states: (access, layout) .. self.stable_state,
            target: &self.handle,
            families: None,
            range: self.range.clone(),
        }
    }
    fn barrier<T>(
        &self,
        entry: Entry<T, i::State>,
        access: i::Access,
        layout: i::Layout,
    ) -> Option<memory::Barrier<B>> {
        let from = mem::replace(entry.or_insert(self.stable_state), (access, layout));
        if from != (access, layout) {
            Some(memory::Barrier::Image {
                states: from .. (access, layout),
                target: &self.handle,
                families: None,
                range: self.range.clone(),
            })
        } else {
            None
        }
    }
}

pub struct RenderPass<B: hal::Backend> {
    pub handle: B::RenderPass,
    attachments: Vec<(String, Range<i::Layout>)>,
    subpasses: Vec<String>,
}

pub struct ImageView<B: hal::Backend> {
    pub handle: B::ImageView,
    image: String,
}

pub struct Framebuffer<B: hal::Backend> {
    pub handle: B::Framebuffer,
    views: Vec<(String, Range<i::Layout>)>,
    extent: i::Extent,
}

pub struct DescriptorSet<B: hal::Backend> {
    pub handle: B::DescriptorSet,
    views: Vec<(String, i::Layout)>,
}

pub struct Resources<B: hal::Backend> {
    pub buffers: HashMap<String, Buffer<B>>,
    pub images: HashMap<String, Image<B>>,
    pub image_views: HashMap<String, ImageView<B>>,
    pub samplers: HashMap<String, B::Sampler>,
    pub render_passes: HashMap<String, RenderPass<B>>,
    pub framebuffers: HashMap<String, Framebuffer<B>>,
    pub shaders: HashMap<String, B::ShaderModule>,
    pub desc_set_layouts:
        HashMap<String, (Vec<hal::pso::DescriptorBinding>, B::DescriptorSetLayout)>,
    pub desc_pools: HashMap<String, B::DescriptorPool>,
    pub desc_sets: HashMap<String, DescriptorSet<B>>,
    pub pipeline_layouts: HashMap<String, B::PipelineLayout>,
    pub graphics_pipelines: HashMap<String, B::GraphicsPipeline>,
    pub compute_pipelines: HashMap<String, (String, B::ComputePipeline)>,
}

pub struct Job<B: hal::Backend> {
    submission: B::CommandBuffer,
}

pub struct Scene<B: hal::Backend> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, Job<B>>,
    init_submit: B::CommandBuffer,
    finish_submit: B::CommandBuffer,
    device: B::Device,
    queue_group: queue::QueueGroup<B>,
    command_pool: Option<B::CommandPool>,
    query_pool: Option<B::QueryPool>,
    upload_buffers: HashMap<String, (B::Buffer, B::Memory)>,
    download_types: Vec<hal::MemoryTypeId>,
    limits: hal::Limits,
}

fn align(x: u64, y: u64) -> u64 {
    if x > 0 && y > 0 {
        ((x - 1) | (y - 1)) + 1
    } else {
        x
    }
}

impl<B: hal::Backend> Scene<B> {
    pub fn new(
        adapter: adapter::Adapter<B>,
        featues: hal::Features,
        raw: &raw::Scene,
        data_path: PathBuf,
    ) -> Result<Self, ()> {
        info!("creating Scene from {:?}", data_path);
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();

        // initialize graphics
        let mut gpu = unsafe {
            adapter
                .physical_device
                .open(&[(&adapter.queue_families[0], &[1.0])], featues)
                .unwrap()
        };
        let device = gpu.device;
        let queue_group = gpu.queue_groups.pop().unwrap();

        let upload_types: Vec<hal::MemoryTypeId> = memory_types
            .iter()
            .enumerate()
            .filter_map(|(i, mt)| {
                if mt.properties.contains(memory::Properties::CPU_VISIBLE) {
                    Some(i.into())
                } else {
                    None
                }
            })
            .collect();
        let download_types: Vec<hal::MemoryTypeId> = memory_types
            .iter()
            .enumerate()
            .filter_map(|(i, mt)| {
                if mt
                    .properties
                    .contains(memory::Properties::CPU_VISIBLE | memory::Properties::CPU_CACHED)
                {
                    Some(i.into())
                } else {
                    None
                }
            })
            .collect();
        info!("upload memory: {:?}", upload_types);
        info!("download memory: {:?}", &download_types);

        let mut command_pool = unsafe {
            device
                .create_command_pool(
                    queue_group.family,
                    hal::pool::CommandPoolCreateFlags::empty(),
                )
                .unwrap()
        };
        let query_pool = unsafe { device.create_query_pool(query::Type::Timestamp, 2) };

        // create resources
        let mut resources = Resources::<B> {
            buffers: HashMap::new(),
            images: HashMap::new(),
            image_views: HashMap::new(),
            samplers: HashMap::new(),
            render_passes: HashMap::new(),
            framebuffers: HashMap::new(),
            shaders: HashMap::new(),
            desc_set_layouts: HashMap::new(),
            desc_pools: HashMap::new(),
            desc_sets: HashMap::new(),
            pipeline_layouts: HashMap::new(),
            graphics_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
        };
        let mut upload_buffers = HashMap::new();
        let (mut finish_cmd, mut init_cmd);
        unsafe {
            finish_cmd = command_pool.allocate_one(c::Level::Primary);
            finish_cmd.begin_primary(c::CommandBufferFlags::empty());
            if let Ok(ref pool) = query_pool {
                finish_cmd.write_timestamp(
                    pso::PipelineStage::BOTTOM_OF_PIPE,
                    query::Query { pool, id: 1 },
                );
            }
            finish_cmd.insert_debug_marker("_done", 0x0000FF00);
            finish_cmd.finish();
        }
        unsafe {
            init_cmd = command_pool.allocate_one(c::Level::Primary);
            init_cmd.begin_primary(c::CommandBufferFlags::empty());
            init_cmd.begin_debug_marker("_init", 0x0000FF00);
            if let Ok(ref pool) = query_pool {
                init_cmd.reset_query_pool(pool, 0 .. 2);
                init_cmd.write_timestamp(
                    pso::PipelineStage::TOP_OF_PIPE,
                    query::Query { pool, id: 0 },
                );
            }
        }
        // Pass[1]: images, samplers, buffers, passes, descriptor set layouts/pools
        for (name, resource) in &raw.resources {
            match *resource {
                raw::Resource::Buffer {
                    size,
                    usage,
                    ref data,
                } => {
                    // allocate memory
                    let mut buffer = unsafe { device.create_buffer(size as _, usage) }.unwrap();
                    let requirements = unsafe { device.get_buffer_requirements(&buffer) };
                    let memory_type = memory_types
                        .iter()
                        .enumerate()
                        .position(|(id, mt)| {
                            requirements.type_mask & (1 << id) != 0
                                && mt.properties.contains(memory::Properties::DEVICE_LOCAL)
                        })
                        .unwrap()
                        .into();
                    let gpu_memory =
                        unsafe { device.allocate_memory(memory_type, requirements.size) }.unwrap();

                    unsafe {
                        device
                            .bind_buffer_memory(&gpu_memory, 0, &mut buffer)
                            .unwrap();
                    }

                    // process initial data for the buffer
                    let stable_state = if data.is_empty() {
                        let access = b::Access::SHADER_READ; //TODO
                        if false {
                            //TODO
                            let buffer_barrier = memory::Barrier::whole_buffer(
                                &buffer,
                                b::Access::empty() .. access,
                            );
                            unsafe {
                                init_cmd.pipeline_barrier(
                                    pso::PipelineStage::TOP_OF_PIPE
                                        .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                    memory::Dependencies::empty(),
                                    &[buffer_barrier],
                                );
                            }
                        }
                        access
                    } else {
                        // calculate required sizes
                        let upload_size =
                            align(size as _, limits.optimal_buffer_copy_pitch_alignment);
                        // create upload buffer
                        let mut upload_buffer =
                            unsafe { device.create_buffer(upload_size, b::Usage::TRANSFER_SRC) }
                                .unwrap();
                        let upload_req = unsafe { device.get_buffer_requirements(&upload_buffer) };
                        let upload_type = *upload_types
                            .iter()
                            .find(|i| upload_req.type_mask & (1 << i.0) != 0)
                            .unwrap();
                        let upload_memory =
                            unsafe { device.allocate_memory(upload_type, upload_req.size) }
                                .unwrap();

                        unsafe { device.bind_buffer_memory(&upload_memory, 0, &mut upload_buffer) }
                            .unwrap();
                        // write the data
                        unsafe {
                            let mapping = device
                                .map_memory(&upload_memory, memory::Segment::ALL)
                                .unwrap();
                            File::open(data_path.join(data))
                                .unwrap()
                                .read_exact(slice::from_raw_parts_mut(mapping, size))
                                .unwrap();
                            device.unmap_memory(&upload_memory);
                        }
                        // add init commands
                        let final_state = b::Access::SHADER_READ;
                        let pre_barrier = memory::Barrier::whole_buffer(
                            &buffer,
                            b::Access::empty() .. b::Access::TRANSFER_WRITE,
                        );
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                                memory::Dependencies::empty(),
                                &[pre_barrier],
                            );
                        }
                        let copy = c::BufferCopy {
                            src: 0,
                            dst: 0,
                            size: size as _,
                        };
                        unsafe {
                            init_cmd.copy_buffer(&upload_buffer, &buffer, &[copy]);
                        }
                        let post_barrier = memory::Barrier::whole_buffer(
                            &buffer,
                            b::Access::TRANSFER_WRITE .. final_state,
                        );
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                memory::Dependencies::empty(),
                                &[post_barrier],
                            );
                        }
                        // done
                        upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                        final_state
                    };

                    resources.buffers.insert(
                        name.clone(),
                        Buffer {
                            handle: buffer,
                            _memory: gpu_memory,
                            size,
                            stable_state,
                        },
                    );
                }
                raw::Resource::Image {
                    kind,
                    num_levels,
                    format,
                    usage,
                    ref data,
                } => {
                    // allocate memory
                    let mut image = unsafe {
                        device.create_image(
                            kind,
                            num_levels,
                            format,
                            i::Tiling::Optimal,
                            usage,
                            i::ViewCapabilities::empty(),
                        )
                    }
                    .unwrap();
                    let requirements = unsafe { device.get_image_requirements(&image) };
                    let memory_type = memory_types
                        .iter()
                        .enumerate()
                        .position(|(id, mt)| {
                            requirements.type_mask & (1 << id) != 0
                                && mt.properties.contains(memory::Properties::DEVICE_LOCAL)
                        })
                        .unwrap()
                        .into();
                    let gpu_memory =
                        unsafe { device.allocate_memory(memory_type, requirements.size) }.unwrap();
                    unsafe { device.bind_image_memory(&gpu_memory, 0, &mut image) }.unwrap();

                    // process initial data for the image
                    let stable_state = if data.is_empty() {
                        let (aspects, access, layout) = if format.is_color() {
                            (
                                f::Aspects::COLOR,
                                i::Access::COLOR_ATTACHMENT_WRITE,
                                i::Layout::ColorAttachmentOptimal,
                            )
                        } else {
                            (
                                f::Aspects::DEPTH | f::Aspects::STENCIL,
                                i::Access::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                i::Layout::DepthStencilAttachmentOptimal,
                            )
                        };
                        let image_barrier = memory::Barrier::Image {
                            states: (i::Access::empty(), i::Layout::Undefined) .. (access, layout),
                            target: &image,
                            families: None,
                            range: i::SubresourceRange {
                                aspects,
                                ..COLOR_RANGE.clone()
                            },
                        };
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::BOTTOM_OF_PIPE
                                    .. pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                                memory::Dependencies::empty(),
                                &[image_barrier],
                            );
                        }
                        (access, layout)
                    } else {
                        // calculate required sizes
                        let extent = kind.extent();
                        assert_eq!(kind.num_samples(), 1);

                        let base_format = format.base_format();
                        let format_desc = base_format.0.desc();
                        let (block_width, block_height) = format_desc.dim;

                        // Width and height need to be multiple of the block dimensions.
                        let w = align(extent.width as _, block_width as _);
                        let h = align(extent.height as _, block_height as _);
                        let d = extent.depth;

                        let width_bytes = (format_desc.bits as u64 * w) / (8 * block_width as u64);
                        let row_pitch =
                            align(width_bytes, limits.optimal_buffer_copy_pitch_alignment);
                        let upload_size =
                            (row_pitch as u64 * h as u64 * d as u64) / block_height as u64;
                        // create upload buffer
                        let mut upload_buffer =
                            unsafe { device.create_buffer(upload_size, b::Usage::TRANSFER_SRC) }
                                .unwrap();
                        let upload_req = unsafe { device.get_buffer_requirements(&upload_buffer) };
                        let upload_type = *upload_types
                            .iter()
                            .find(|i| upload_req.type_mask & (1 << i.0) != 0)
                            .unwrap();
                        let upload_memory =
                            unsafe { device.allocate_memory(upload_type, upload_req.size) }
                                .unwrap();
                        unsafe { device.bind_buffer_memory(&upload_memory, 0, &mut upload_buffer) }
                            .unwrap();
                        // write the data
                        unsafe {
                            let mut file = File::open(data_path.join(data)).unwrap();
                            let mapping = device
                                .map_memory(&upload_memory, memory::Segment::ALL)
                                .unwrap();
                            for y in 0 .. (h as usize * d as usize) {
                                let slice = slice::from_raw_parts_mut(
                                    mapping.offset(y as isize * row_pitch as isize),
                                    width_bytes as usize,
                                );
                                file.read_exact(slice).unwrap();
                            }
                            device.unmap_memory(&upload_memory);
                        }
                        // add init commands
                        let final_state =
                            (i::Access::SHADER_READ, i::Layout::ShaderReadOnlyOptimal);
                        let pre_barrier = memory::Barrier::Image {
                            states: (i::Access::empty(), i::Layout::Undefined)
                                .. (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                            families: None,
                            target: &image,
                            range: COLOR_RANGE.clone(), //TODO
                        };
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                                memory::Dependencies::empty(),
                                &[pre_barrier],
                            );
                        }

                        let buffer_width = (row_pitch as u32 * 8) / format_desc.bits as u32;
                        let copy = c::BufferImageCopy {
                            buffer_offset: 0,
                            buffer_width,
                            buffer_height: h as u32,
                            image_layers: i::SubresourceLayers {
                                aspects: f::Aspects::COLOR,
                                level: 0,
                                layers: 0 .. 1,
                            },
                            image_offset: i::Offset::ZERO,
                            image_extent: extent,
                        };
                        unsafe {
                            init_cmd.copy_buffer_to_image(
                                &upload_buffer,
                                &image,
                                i::Layout::TransferDstOptimal,
                                &[copy],
                            );
                        }
                        let post_barrier = memory::Barrier::Image {
                            states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                                .. final_state,
                            families: None,
                            target: &image,
                            range: COLOR_RANGE.clone(), //TODO
                        };
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                memory::Dependencies::empty(),
                                &[post_barrier],
                            );
                        }
                        // done
                        upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                        final_state
                    };

                    resources.images.insert(
                        name.clone(),
                        Image {
                            handle: image,
                            _memory: gpu_memory,
                            kind,
                            format,
                            range: COLOR_RANGE.clone(),
                            stable_state,
                        },
                    );
                }
                raw::Resource::Sampler { ref info } => {
                    let sampler = unsafe { device.create_sampler(info).unwrap() };
                    resources.samplers.insert(name.clone(), sampler);
                }
                raw::Resource::RenderPass {
                    ref attachments,
                    ref subpasses,
                    ref dependencies,
                } => {
                    let att_ref = |aref: &raw::AttachmentRef| {
                        let id = attachments.keys().position(|s| s == &aref.0).unwrap();
                        (id, aref.1)
                    };
                    let subpass_ref = |s: &String| {
                        if s.is_empty() {
                            None
                        } else {
                            subpasses
                                .keys()
                                .position(|sp| s == sp)
                                .map(|id| id as hal::pass::SubpassId)
                        }
                    };

                    let raw_atts = attachments.values().cloned();
                    let temp = subpasses
                        .values()
                        .map(|sp| {
                            let colors = sp.colors.iter().map(&att_ref).collect::<Vec<_>>();
                            let ds = sp.depth_stencil.as_ref().map(&att_ref);
                            let inputs = sp.inputs.iter().map(&att_ref).collect::<Vec<_>>();
                            let preserves = sp
                                .preserves
                                .iter()
                                .map(|sp| attachments.keys().position(|s| s == sp).unwrap())
                                .collect::<Vec<_>>();
                            let resolves = sp.resolves.iter().map(&att_ref).collect::<Vec<_>>();
                            (colors, ds, inputs, preserves, resolves)
                        })
                        .collect::<Vec<_>>();
                    let raw_subs = temp
                        .iter()
                        .map(|t| hal::pass::SubpassDesc {
                            colors: &t.0,
                            depth_stencil: t.1.as_ref(),
                            inputs: &t.2,
                            preserves: &t.3,
                            resolves: &t.4,
                        })
                        .collect::<Vec<_>>();
                    let raw_deps = dependencies.iter().map(|dep| hal::pass::SubpassDependency {
                        passes: subpass_ref(&dep.passes.start) .. subpass_ref(&dep.passes.end),
                        stages: dep.stages.clone(),
                        accesses: dep.accesses.clone(),
                        flags: memory::Dependencies::empty(),
                    });

                    let rp = RenderPass {
                        handle: unsafe { device.create_render_pass(raw_atts, raw_subs, raw_deps) }
                            .expect("Render pass creation failure"),
                        attachments: attachments
                            .iter()
                            .map(|(key, at)| (key.clone(), at.layouts.clone()))
                            .collect(),
                        subpasses: subpasses.keys().cloned().collect(),
                    };
                    resources.render_passes.insert(name.clone(), rp);
                }
                raw::Resource::Shader(ref local_path) => {
                    #[cfg(feature = "glsl-to-spirv")]
                    fn transpile(mut file: File, ty: glsl_to_spirv::ShaderType) -> File {
                        let mut code = String::new();
                        file.read_to_string(&mut code).unwrap();
                        glsl_to_spirv::compile(&code, ty).unwrap()
                    }
                    let full_path = data_path.join(local_path);
                    let base_file = File::open(&full_path).unwrap();
                    let file = match &*full_path.extension().unwrap().to_string_lossy() {
                        "spirv" => base_file,
                        #[cfg(feature = "glsl-to-spirv")]
                        "vert" => transpile(base_file, glsl_to_spirv::ShaderType::Vertex),
                        #[cfg(feature = "glsl-to-spirv")]
                        "frag" => transpile(base_file, glsl_to_spirv::ShaderType::Fragment),
                        #[cfg(feature = "glsl-to-spirv")]
                        "comp" => transpile(base_file, glsl_to_spirv::ShaderType::Compute),
                        other => panic!("Unknown shader extension: {}", other),
                    };
                    let spirv = pso::read_spirv(file).unwrap();
                    let module = unsafe { device.create_shader_module(&spirv) }.unwrap();
                    resources.shaders.insert(name.clone(), module);
                }
                raw::Resource::DescriptorSetLayout {
                    ref bindings,
                    ref immutable_samplers,
                } => {
                    assert!(immutable_samplers.is_empty()); //TODO! requires changing the order,
                    assert!(!bindings.is_empty());
                    // since samples are expect to be all read by this point
                    let layout = unsafe { device.create_descriptor_set_layout(bindings, &[]) }
                        .expect("Descriptor set layout creation failure!");
                    let binding_indices = bindings.iter().map(|dsb| dsb.binding).collect();
                    resources
                        .desc_set_layouts
                        .insert(name.clone(), (binding_indices, layout));
                }
                raw::Resource::DescriptorPool {
                    capacity,
                    ref ranges,
                } => {
                    assert!(!ranges.is_empty());
                    assert!(capacity > 0);
                    let pool = unsafe {
                        device.create_descriptor_pool(
                            capacity,
                            ranges,
                            pso::DescriptorPoolCreateFlags::empty(),
                        )
                    }
                    .expect("Descriptor pool creation failure!");
                    resources.desc_pools.insert(name.clone(), pool);
                }
                _ => {}
            }
        }

        // Pass[2]: image & buffer views, pipeline layouts
        for (name, resource) in &raw.resources {
            match *resource {
                raw::Resource::ImageView {
                    ref image,
                    kind,
                    format,
                    swizzle,
                    ref range,
                } => {
                    let img = &resources.images[image].handle;
                    let view = unsafe {
                        device.create_image_view(img, kind, format, swizzle, range.clone())
                    }
                    .unwrap();
                    resources.image_views.insert(
                        name.clone(),
                        ImageView {
                            handle: view,
                            image: image.clone(),
                        },
                    );
                }
                raw::Resource::PipelineLayout {
                    ref set_layouts,
                    ref push_constant_ranges,
                } => {
                    let layout = {
                        let layouts = set_layouts
                            .iter()
                            .map(|sl| &resources.desc_set_layouts[sl].1);
                        unsafe { device.create_pipeline_layout(layouts, push_constant_ranges) }
                            .unwrap()
                    };
                    resources.pipeline_layouts.insert(name.clone(), layout);
                }
                _ => {}
            }
        }

        // Pass[3]: descriptor sets, framebuffers and pipelines
        for (name, resource) in &raw.resources {
            match *resource {
                raw::Resource::DescriptorSet {
                    ref pool,
                    ref layout,
                    ref data,
                } => {
                    // create a descriptor set
                    let (ref binding_indices, ref set_layout) = resources.desc_set_layouts[layout];
                    let desc_set = unsafe {
                        resources
                            .desc_pools
                            .get_mut(pool)
                            .expect(&format!("Missing descriptor pool: {}", pool))
                            .allocate_set(set_layout)
                    }
                    .expect(&format!(
                        "Failed to allocate set with layout: {:?}",
                        set_layout
                    ));
                    // fill it up
                    let mut writes = Vec::new();
                    let mut views = Vec::new();
                    for (&binding, range) in binding_indices.iter().zip(data) {
                        writes.push(hal::pso::DescriptorSetWrite {
                            set: &desc_set,
                            binding,
                            array_offset: 0,
                            descriptors: match *range {
                                raw::DescriptorRange::Buffers(ref names) => names
                                    .iter()
                                    .map(|s| {
                                        let buf = resources
                                            .buffers
                                            .get(s)
                                            .expect(&format!("Missing buffer: {}", s));
                                        hal::pso::Descriptor::Buffer(
                                            &buf.handle,
                                            b::SubRange::WHOLE,
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                                raw::DescriptorRange::Images(ref names_and_layouts) => {
                                    views.extend_from_slice(names_and_layouts);
                                    names_and_layouts
                                        .iter()
                                        .map(|&(ref s, layout)| {
                                            let view = resources
                                                .image_views
                                                .get(s)
                                                .expect(&format!("Missing image view: {}", s));
                                            hal::pso::Descriptor::Image(&view.handle, layout)
                                        })
                                        .collect::<Vec<_>>()
                                }
                                raw::DescriptorRange::Samplers(ref names) => names
                                    .iter()
                                    .map(|s| {
                                        let sampler = resources
                                            .samplers
                                            .get(s)
                                            .expect(&format!("Missing sampler: {}", s));
                                        hal::pso::Descriptor::Sampler(sampler)
                                    })
                                    .collect::<Vec<_>>(),
                            },
                        });
                    }
                    unsafe {
                        device.write_descriptor_sets(writes);
                    }
                    resources.desc_sets.insert(
                        name.clone(),
                        DescriptorSet {
                            handle: desc_set,
                            views,
                        },
                    );
                }
                raw::Resource::Framebuffer {
                    ref pass,
                    ref views,
                    extent,
                } => {
                    let rp = resources
                        .render_passes
                        .get(pass)
                        .expect(&format!("Missing render pass: {}", pass));
                    let view_pairs = rp
                        .attachments
                        .iter()
                        .map(|at| {
                            let entry = views.iter().find(|entry| entry.0 == &at.0).unwrap();
                            (entry.1.clone(), at.1.clone())
                        })
                        .collect::<Vec<_>>();
                    let framebuffer = {
                        let image_views = view_pairs
                            .iter()
                            .map(|vp| &resources.image_views[&vp.0].handle);
                        unsafe { device.create_framebuffer(&rp.handle, image_views, extent) }
                            .unwrap()
                    };
                    resources.framebuffers.insert(
                        name.clone(),
                        Framebuffer {
                            handle: framebuffer,
                            views: view_pairs,
                            extent,
                        },
                    );
                }
                raw::Resource::GraphicsPipeline {
                    ref shaders,
                    ref rasterizer,
                    ref vertex_buffers,
                    ref attributes,
                    ref input_assembler,
                    ref blender,
                    depth_stencil,
                    ref layout,
                    ref subpass,
                } => {
                    let reshaders = &resources.shaders;
                    let entry = |shader: &String| -> Option<pso::EntryPoint<B>> {
                        if shader.is_empty() {
                            None
                        } else {
                            Some(pso::EntryPoint {
                                entry: "main",
                                module: reshaders
                                    .get(shader)
                                    .expect(&format!("Missing shader: {}", shader)),
                                specialization: pso::Specialization::default(),
                            })
                        }
                    };

                    let hs = entry(&shaders.hull);
                    let ds = entry(&shaders.domain);
                    let tessellation = if hs.is_some() && ds.is_some() {
                        Some((hs.unwrap(), ds.unwrap()))
                    } else {
                        None
                    };

                    let desc = pso::GraphicsPipelineDesc {
                        rasterizer: rasterizer.clone(),
                        primitive_assembler: pso::PrimitiveAssembler::Vertex {
                            buffers: vertex_buffers.clone(),
                            attributes: attributes.clone(),
                            input_assembler: input_assembler.clone(),
                            vertex: pso::EntryPoint {
                                entry: "main",
                                module: reshaders
                                    .get(&shaders.vertex)
                                    .expect(&format!("Missing vertex shader: {}", shaders.vertex)),
                                specialization: pso::Specialization::default(),
                            },
                            tessellation,
                            geometry: entry(&shaders.geometry),
                        },
                        fragment: entry(&shaders.fragment),
                        blender: blender.clone(),
                        depth_stencil: depth_stencil.clone(),
                        baked_states: pso::BakedStates::default(), //TODO
                        multisampling: None,                       // TODO
                        layout: &resources.pipeline_layouts[layout],
                        subpass: hal::pass::Subpass {
                            main_pass: &resources
                                .render_passes
                                .get(&subpass.parent)
                                .expect(&format!("Missing render pass: {}", subpass.parent))
                                .handle,
                            index: subpass.index,
                        },
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    };
                    let pso = unsafe { device.create_graphics_pipeline(&desc, None) }.unwrap();
                    resources.graphics_pipelines.insert(name.clone(), pso);
                }
                raw::Resource::ComputePipeline {
                    ref shader,
                    ref layout,
                } => {
                    let desc = pso::ComputePipelineDesc {
                        shader: pso::EntryPoint {
                            entry: "main",
                            module: resources
                                .shaders
                                .get(shader)
                                .expect(&format!("Missing compute shader: {}", shader)),
                            specialization: pso::Specialization::default(),
                        },
                        layout: resources
                            .pipeline_layouts
                            .get(layout)
                            .expect(&format!("Missing pipeline layout: {}", layout)),
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    };
                    let pso = unsafe { device.create_compute_pipeline(&desc, None) }.unwrap();
                    resources
                        .compute_pipelines
                        .insert(name.clone(), (layout.clone(), pso));
                }
                _ => {}
            }
        }

        unsafe {
            init_cmd.end_debug_marker();
            init_cmd.finish();
        }

        // fill up command buffers
        let mut jobs = HashMap::new();
        for (name, job) in &raw.jobs {
            use crate::raw::TransferCommand as Tc;
            let mut command_buf;
            unsafe {
                command_buf = command_pool.allocate_one(c::Level::Primary);
                command_buf.begin_primary(c::CommandBufferFlags::SIMULTANEOUS_USE);
                command_buf.begin_debug_marker(name, 0x00FF0000);
            }
            match *job {
                raw::Job::Transfer { ref commands } => {
                    let mut buffers = HashMap::new();
                    let mut images = HashMap::new();
                    let src_stage =
                        pso::PipelineStage::TRANSFER | pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT;
                    for command in commands {
                        match *command {
                            Tc::CopyBuffer {
                                ref src,
                                ref dst,
                                ref regions,
                            } => unsafe {
                                let sb = resources
                                    .buffers
                                    .get(src)
                                    .expect(&format!("Missing source buffer: {}", src));
                                let db = resources
                                    .buffers
                                    .get(dst)
                                    .expect(&format!("Missing destination buffer: {}", dst));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    sb.barrier(buffers.entry(src), b::State::TRANSFER_READ)
                                        .into_iter()
                                        .chain(
                                            db.barrier(
                                                buffers.entry(dst),
                                                b::State::TRANSFER_WRITE,
                                            ),
                                        ),
                                );
                                command_buf.copy_buffer(&sb.handle, &db.handle, regions);
                            },
                            Tc::CopyImage {
                                ref src,
                                ref dst,
                                ref regions,
                            } => unsafe {
                                let st = resources
                                    .images
                                    .get(src)
                                    .expect(&format!("Missing source image: {}", src));
                                let dt = resources
                                    .images
                                    .get(dst)
                                    .expect(&format!("Missing destination image: {}", dst));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    st.barrier(
                                        images.entry(src),
                                        i::Access::TRANSFER_READ,
                                        i::Layout::TransferSrcOptimal,
                                    )
                                    .into_iter()
                                    .chain(dt.barrier(
                                        images.entry(dst),
                                        i::Access::TRANSFER_WRITE,
                                        i::Layout::TransferDstOptimal,
                                    )),
                                );
                                command_buf.copy_image(
                                    &st.handle,
                                    i::Layout::TransferSrcOptimal,
                                    &dt.handle,
                                    i::Layout::TransferDstOptimal,
                                    regions,
                                );
                            },
                            Tc::CopyBufferToImage {
                                ref src,
                                ref dst,
                                ref regions,
                            } => unsafe {
                                let sb = resources
                                    .buffers
                                    .get(src)
                                    .expect(&format!("Missing source buffer: {}", src));
                                let dt = resources
                                    .images
                                    .get(dst)
                                    .expect(&format!("Missing destination image: {}", dst));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    sb.barrier(buffers.entry(src), b::State::TRANSFER_READ)
                                        .into_iter()
                                        .chain(dt.barrier(
                                            images.entry(dst),
                                            i::Access::TRANSFER_WRITE,
                                            i::Layout::TransferDstOptimal,
                                        )),
                                );
                                command_buf.copy_buffer_to_image(
                                    &sb.handle,
                                    &dt.handle,
                                    i::Layout::TransferDstOptimal,
                                    regions,
                                );
                            },
                            Tc::CopyImageToBuffer {
                                ref src,
                                ref dst,
                                ref regions,
                            } => unsafe {
                                let st = resources
                                    .images
                                    .get(src)
                                    .expect(&format!("Missing source image: {}", src));
                                let db = resources
                                    .buffers
                                    .get(dst)
                                    .expect(&format!("Missing destination buffer: {}", dst));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    st.barrier(
                                        images.entry(src),
                                        i::Access::TRANSFER_READ,
                                        i::Layout::TransferSrcOptimal,
                                    )
                                    .into_iter()
                                    .chain(
                                        db.barrier(buffers.entry(dst), b::State::TRANSFER_WRITE),
                                    ),
                                );
                                command_buf.copy_image_to_buffer(
                                    &st.handle,
                                    i::Layout::TransferSrcOptimal,
                                    &db.handle,
                                    regions,
                                );
                            },
                            Tc::ClearImage {
                                ref image,
                                ref value,
                                ref ranges,
                            } => unsafe {
                                let img = resources
                                    .images
                                    .get(image)
                                    .expect(&format!("Missing clear image: {}", image));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    img.barrier(
                                        images.entry(image),
                                        i::Access::TRANSFER_WRITE,
                                        i::Layout::TransferDstOptimal,
                                    ),
                                );
                                command_buf.clear_image(
                                    &img.handle,
                                    i::Layout::TransferDstOptimal,
                                    value.to_raw(),
                                    ranges,
                                );
                            },
                            Tc::BlitImage {
                                ref src,
                                ref dst,
                                filter,
                                ref regions,
                            } => unsafe {
                                let st = resources
                                    .images
                                    .get(src)
                                    .expect(&format!("Missing source image: {}", src));
                                let dt = resources
                                    .images
                                    .get(dst)
                                    .expect(&format!("Missing destination image: {}", dst));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    st.barrier(
                                        images.entry(src),
                                        i::Access::TRANSFER_READ,
                                        i::Layout::TransferSrcOptimal,
                                    )
                                    .into_iter()
                                    .chain(dt.barrier(
                                        images.entry(dst),
                                        i::Access::TRANSFER_WRITE,
                                        i::Layout::TransferDstOptimal,
                                    )),
                                );
                                command_buf.blit_image(
                                    &st.handle,
                                    i::Layout::TransferSrcOptimal,
                                    &dt.handle,
                                    i::Layout::TransferDstOptimal,
                                    filter,
                                    regions,
                                );
                            },
                            Tc::FillBuffer {
                                ref buffer,
                                offset,
                                size,
                                data,
                            } => unsafe {
                                let buf = resources
                                    .buffers
                                    .get(buffer)
                                    .expect(&format!("Missing buffer: {}", buffer));
                                command_buf.pipeline_barrier(
                                    src_stage .. pso::PipelineStage::TRANSFER,
                                    memory::Dependencies::empty(),
                                    buf.barrier(buffers.entry(buffer), b::State::TRANSFER_WRITE),
                                );
                                command_buf.fill_buffer(
                                    &buf.handle,
                                    b::SubRange { offset, size },
                                    data,
                                );
                            },
                        }
                    }

                    let buffer_cleanup = buffers.into_iter().map(|(name, state)| {
                        resources.buffers.get(name).unwrap().barrier_from(state)
                    });
                    let image_cleanup = images.into_iter().map(|(name, (access, layout))| {
                        resources
                            .images
                            .get(name)
                            .unwrap()
                            .barrier_from(access, layout)
                    });
                    let dst_stages = pso::PipelineStage::FRAGMENT_SHADER
                        | pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT;
                    unsafe {
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER .. dst_stages,
                            memory::Dependencies::empty(),
                            buffer_cleanup.chain(image_cleanup),
                        );
                    }
                }
                raw::Job::Graphics {
                    ref framebuffer,
                    ref pass,
                    ref clear_values,
                } => unsafe {
                    // collect all used image descriptors
                    let mut all_images = Vec::new();
                    for subpass in pass.1.iter() {
                        for com in subpass.1.commands.iter() {
                            if let raw::DrawCommand::BindDescriptorSets { ref sets, .. } = *com {
                                for set in sets {
                                    for pair in resources.desc_sets[set].views.iter() {
                                        let view = &resources.image_views[&pair.0];
                                        all_images.push((view.image.clone(), pair.1));
                                    }
                                }
                            }
                        }
                    }

                    let fb = resources
                        .framebuffers
                        .get(framebuffer)
                        .expect(&format!("Missing framebuffer: {}", framebuffer));
                    let rp = resources
                        .render_passes
                        .get(&pass.0)
                        .expect(&format!("Missing render pass: {}", pass.0));
                    let rect = pso::Rect {
                        x: 0,
                        y: 0,
                        w: fb.extent.width as _,
                        h: fb.extent.height as _,
                    };
                    command_buf.pipeline_barrier(
                        pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT
                            .. pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                        memory::Dependencies::empty(),
                        fb.views.iter().map(|v| {
                            let view = &resources.image_views[&v.0];
                            resources.images[&view.image]
                                .barrier_to(i::Access::COLOR_ATTACHMENT_WRITE, v.1.start)
                        }),
                    );
                    command_buf.pipeline_barrier(
                        pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT
                            .. pso::PipelineStage::VERTEX_SHADER
                                | pso::PipelineStage::FRAGMENT_SHADER,
                        memory::Dependencies::empty(),
                        all_images.iter().map(|&(ref name, layout)| {
                            resources.images[name].barrier_to(i::Access::SHADER_READ, layout)
                        }),
                    );
                    command_buf.begin_render_pass(
                        &rp.handle,
                        &fb.handle,
                        rect,
                        clear_values.iter().map(|cv| cv.to_raw()),
                        c::SubpassContents::Inline,
                    );
                    command_buf.set_scissors(0, Some(rect));
                    command_buf.set_viewports(
                        0,
                        Some(pso::Viewport {
                            rect,
                            depth: 0.0 .. 1.0,
                        }),
                    );

                    for subpass in &rp.subpasses {
                        if Some(subpass) != rp.subpasses.first() {
                            command_buf.next_subpass(c::SubpassContents::Inline);
                        }
                        for command in &pass.1[subpass].commands {
                            use crate::raw::DrawCommand as Dc;
                            match *command {
                                Dc::BindIndexBuffer {
                                    ref buffer,
                                    ref range,
                                    index_type,
                                } => {
                                    let view = b::IndexBufferView {
                                        buffer: &resources
                                            .buffers
                                            .get(buffer)
                                            .expect(&format!("Missing index buffer: {}", buffer))
                                            .handle,
                                        range: range.clone(),
                                        index_type,
                                    };
                                    command_buf.bind_index_buffer(view);
                                }
                                Dc::BindVertexBuffers(ref buffers) => {
                                    let buffers_raw = buffers.iter().map(|&(ref name, ref sub)| {
                                        let buf = &resources
                                            .buffers
                                            .get(name)
                                            .expect(&format!("Missing vertex buffer: {}", name))
                                            .handle;
                                        (buf, sub.clone())
                                    });
                                    command_buf.bind_vertex_buffers(0, buffers_raw);
                                }
                                Dc::BindPipeline(ref name) => {
                                    let pso = resources
                                        .graphics_pipelines
                                        .get(name)
                                        .expect(&format!("Missing graphics pipeline: {}", name));
                                    command_buf.bind_graphics_pipeline(pso);
                                }
                                Dc::BindDescriptorSets {
                                    ref layout,
                                    first,
                                    ref sets,
                                } => {
                                    command_buf.bind_graphics_descriptor_sets(
                                        resources.pipeline_layouts.get(layout).expect(&format!(
                                            "Missing pipeline layout: {}",
                                            layout
                                        )),
                                        first,
                                        sets.iter().map(|name| {
                                            &resources
                                                .desc_sets
                                                .get(name)
                                                .expect(&format!(
                                                    "Missing descriptor set: {}",
                                                    name
                                                ))
                                                .handle
                                        }),
                                        &[],
                                    );
                                }
                                Dc::Draw {
                                    ref vertices,
                                    ref instances,
                                } => {
                                    command_buf.draw(vertices.clone(), instances.clone());
                                }
                                Dc::DrawIndexed {
                                    ref indices,
                                    base_vertex,
                                    ref instances,
                                } => {
                                    command_buf.draw_indexed(
                                        indices.clone(),
                                        base_vertex,
                                        instances.clone(),
                                    );
                                }
                                Dc::SetViewports(ref viewports) => {
                                    command_buf.set_viewports(0, viewports);
                                }
                                Dc::SetScissors(ref scissors) => {
                                    command_buf.set_scissors(0, scissors);
                                }
                            }
                        }
                    }

                    command_buf.end_render_pass();
                    command_buf.pipeline_barrier(
                        pso::PipelineStage::VERTEX_SHADER | pso::PipelineStage::FRAGMENT_SHADER
                            .. pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                        memory::Dependencies::empty(),
                        all_images.iter().map(|&(ref name, layout)| {
                            resources.images[name].barrier_from(i::Access::SHADER_READ, layout)
                        }),
                    );
                    command_buf.pipeline_barrier(
                        pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT
                            .. pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                        memory::Dependencies::empty(),
                        fb.views.iter().map(|v| {
                            let view = &resources.image_views[&v.0];
                            resources.images[&view.image]
                                .barrier_from(i::Access::COLOR_ATTACHMENT_WRITE, v.1.end)
                        }),
                    );
                },
                raw::Job::Compute {
                    ref pipeline,
                    ref descriptor_sets,
                    dispatch,
                } => unsafe {
                    let (ref layout, ref pso) = resources.compute_pipelines[pipeline];
                    command_buf.bind_compute_pipeline(pso);
                    command_buf.bind_compute_descriptor_sets(
                        resources
                            .pipeline_layouts
                            .get(layout)
                            .expect(&format!("Missing pipeline layout: {}", layout)),
                        0,
                        descriptor_sets.iter().map(|name| {
                            &resources
                                .desc_sets
                                .get(name)
                                .expect(&format!("Missing descriptor set: {}", name))
                                .handle
                        }),
                        &[],
                    );
                    command_buf.dispatch(dispatch);
                },
            }

            unsafe {
                command_buf.end_debug_marker();
                command_buf.finish();
            }
            jobs.insert(
                name.clone(),
                Job {
                    submission: command_buf,
                },
            );
        }

        // done
        Ok(Scene {
            resources,
            jobs,
            init_submit: init_cmd,
            finish_submit: finish_cmd,
            device,
            queue_group,
            command_pool: Some(command_pool),
            query_pool: query_pool.ok(),
            upload_buffers,
            download_types,
            limits,
        })
    }
}

impl<B: hal::Backend> Scene<B> {
    pub fn run<I>(&mut self, job_names: I)
    where
        I: Iterator,
        I::Item: AsRef<str>,
    {
        let jobs = &self.jobs;
        let submits = job_names.map(|name| {
            &jobs
                .get(name.as_ref())
                .expect(&format!("Missing job: {}", name.as_ref()))
                .submission
        });

        let command_buffers = iter::once(&self.init_submit)
            .chain(submits)
            .chain(iter::once(&self.finish_submit));
        unsafe {
            self.queue_group.queues[0].submit_without_semaphores(command_buffers, None);
        }
    }

    pub fn fetch_buffer(&mut self, name: &str) -> FetchGuard<B> {
        let buffer = self
            .resources
            .buffers
            .get(name)
            .expect(&format!("Unable to find buffer to fetch: {}", name));
        let limits = &self.limits;

        let down_size = align(
            buffer.size as u64,
            limits.optimal_buffer_copy_pitch_alignment,
        );

        let mut down_buffer =
            unsafe { self.device.create_buffer(down_size, b::Usage::TRANSFER_DST) }.unwrap();
        let down_req = unsafe { self.device.get_buffer_requirements(&down_buffer) };
        let download_type = *self
            .download_types
            .iter()
            .find(|i| down_req.type_mask & (1 << i.0) != 0)
            .unwrap();
        let down_memory =
            unsafe { self.device.allocate_memory(download_type, down_req.size) }.unwrap();

        unsafe {
            self.device
                .bind_buffer_memory(&down_memory, 0, &mut down_buffer)
        }
        .unwrap();

        let mut command_pool = unsafe {
            self.device.create_command_pool(
                self.queue_group.family,
                hal::pool::CommandPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create command pool");
        let mut cmd_buffer;
        unsafe {
            cmd_buffer = command_pool.allocate_one(c::Level::Primary);
            cmd_buffer.begin_primary(c::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.begin_debug_marker("_fetch_buffer", 0x0000FF00);
            let pre_barrier = memory::Barrier::whole_buffer(
                &buffer.handle,
                buffer.stable_state .. b::Access::TRANSFER_READ,
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                &[pre_barrier],
            );

            let copy = c::BufferCopy {
                src: 0,
                dst: 0,
                size: buffer.size as _,
            };
            cmd_buffer.copy_buffer(&buffer.handle, &down_buffer, &[copy]);

            let post_barrier = memory::Barrier::whole_buffer(
                &buffer.handle,
                b::Access::TRANSFER_READ .. buffer.stable_state,
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                memory::Dependencies::empty(),
                &[post_barrier],
            );
            cmd_buffer.end_debug_marker();
            cmd_buffer.finish()
        }

        let copy_fence = self
            .device
            .create_fence(false)
            .expect("Can't create copy-fence");
        unsafe {
            self.queue_group.queues[0]
                .submit_without_semaphores(iter::once(&cmd_buffer), Some(&copy_fence));
            self.device.wait_for_fence(&copy_fence, !0).unwrap();
            self.device.destroy_fence(copy_fence);
            self.device.destroy_command_pool(command_pool);
        }

        let mapping =
            unsafe { self.device.map_memory(&down_memory, memory::Segment::ALL) }.unwrap();

        FetchGuard {
            device: &mut self.device,
            buffer: Some(down_buffer),
            memory: Some(down_memory),
            mapping,
            row_pitch: down_size as _,
            width: buffer.size,
        }
    }

    pub fn fetch_image(&mut self, name: &str) -> FetchGuard<B> {
        let image = self
            .resources
            .images
            .get(name)
            .expect(&format!("Unable to find image to fetch: {}", name));
        let limits = &self.limits;

        let i::Extent {
            width,
            height,
            depth,
        } = image.kind.extent();
        assert_eq!(image.kind.num_samples(), 1);

        // TODO:
        let base_format = image.format.base_format();
        let format_desc = base_format.0.desc();
        let (block_width, block_height) = format_desc.dim;

        // Width and height need to be multiple of the block dimensions.
        let width = align(width as _, block_width as _);
        let height = align(height as _, block_height as _);

        let width_bytes = (format_desc.bits as u64 * width as u64) / (8 * block_width as u64);
        let row_pitch = align(width_bytes, limits.optimal_buffer_copy_pitch_alignment);
        let down_size = (row_pitch * height * depth as u64) / block_height as u64;

        let mut down_buffer =
            unsafe { self.device.create_buffer(down_size, b::Usage::TRANSFER_DST) }.unwrap();
        let down_req = unsafe { self.device.get_buffer_requirements(&down_buffer) };
        let download_type = *self
            .download_types
            .iter()
            .find(|i| down_req.type_mask & (1 << i.0) != 0)
            .unwrap();
        let down_memory =
            unsafe { self.device.allocate_memory(download_type, down_req.size) }.unwrap();
        unsafe {
            self.device
                .bind_buffer_memory(&down_memory, 0, &mut down_buffer)
        }
        .unwrap();

        let mut command_pool = unsafe {
            self.device.create_command_pool(
                self.queue_group.family,
                hal::pool::CommandPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create command pool");
        let mut cmd_buffer;
        unsafe {
            cmd_buffer = command_pool.allocate_one(c::Level::Primary);
            cmd_buffer.begin_primary(c::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.begin_debug_marker("_fetch_image", 0x0000FF00);
            let pre_barrier = memory::Barrier::Image {
                states: image.stable_state
                    .. (i::Access::TRANSFER_READ, i::Layout::TransferSrcOptimal),
                target: &image.handle,
                families: None,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                &[pre_barrier],
            );

            let copy = c::BufferImageCopy {
                buffer_offset: 0,
                buffer_width: (row_pitch as u32 * 8) / format_desc.bits as u32,
                buffer_height: height as u32,
                image_layers: i::SubresourceLayers {
                    aspects: f::Aspects::COLOR,
                    level: 0,
                    layers: 0 .. 1,
                },
                image_offset: i::Offset { x: 0, y: 0, z: 0 },
                image_extent: i::Extent {
                    width: width as _,
                    height: height as _,
                    depth: depth as _,
                },
            };
            cmd_buffer.copy_image_to_buffer(
                &image.handle,
                i::Layout::TransferSrcOptimal,
                &down_buffer,
                &[copy],
            );

            let post_barrier = memory::Barrier::Image {
                states: (i::Access::TRANSFER_READ, i::Layout::TransferSrcOptimal)
                    .. image.stable_state,
                target: &image.handle,
                families: None,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                memory::Dependencies::empty(),
                &[post_barrier],
            );
            cmd_buffer.end_debug_marker();
            cmd_buffer.finish();
        }

        let copy_fence = self
            .device
            .create_fence(false)
            .expect("Can't create copy-fence");
        unsafe {
            self.queue_group.queues[0]
                .submit_without_semaphores(iter::once(&cmd_buffer), Some(&copy_fence));
            self.device.wait_for_fence(&copy_fence, !0).unwrap();
            self.device.destroy_fence(copy_fence);
            self.device.destroy_command_pool(command_pool);
        }

        let mapping =
            unsafe { self.device.map_memory(&down_memory, memory::Segment::ALL) }.unwrap();

        FetchGuard {
            device: &mut self.device,
            buffer: Some(down_buffer),
            memory: Some(down_memory),
            mapping,
            row_pitch: row_pitch as _,
            width: width_bytes as _,
        }
    }

    pub fn measure_time(&self) -> u32 {
        let mut results = vec![0u32; 2];
        if let Some(ref pool) = self.query_pool {
            unsafe {
                self.device.wait_idle().unwrap();
                let raw_data = slice::from_raw_parts_mut(results.as_mut_ptr() as *mut u8, 4 * 2);
                self.device
                    .get_query_pool_results(pool, 0 .. 2, raw_data, 4, query::ResultFlags::empty())
                    .unwrap();
            }
        }
        results[1] - results[0]
    }
}

impl<B: hal::Backend> Drop for Scene<B> {
    fn drop(&mut self) {
        unsafe {
            for (_, (buffer, memory)) in self.upload_buffers.drain() {
                self.device.destroy_buffer(buffer);
                self.device.free_memory(memory);
            }
            //TODO: free those properly
            let _ = &self.queue_group;
            self.device
                .destroy_command_pool(self.command_pool.take().unwrap());
            if let Some(pool) = self.query_pool.take() {
                self.device.destroy_query_pool(pool);
            }
        }
    }
}
