use failure::Error;
#[cfg(feature = "glsl-to-spirv")]
use glsl_to_spirv;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{iter, slice};

use crate::hal::{self, buffer as b, command as c, format as f, image as i, memory, pso};
use crate::hal::{DescriptorPool, Device, PhysicalDevice};

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
    stable_state: b::Access,
}

impl<B: hal::Backend> Buffer<B> {
    fn barrier_to(&self, access: b::Access) -> memory::Barrier<B> {
        memory::Barrier::whole_buffer(&self.handle, self.stable_state, access)
    }
    fn barrier_from(&self, access: b::Access) -> memory::Barrier<B> {
        memory::Barrier::whole_buffer(&self.handle, access, self.stable_state)
    }
}

pub struct Image<B: hal::Backend> {
    handle: B::Image,
    _memory: B::Memory,
    kind: i::Kind,
    format: f::Format,
    range: i::SubresourceRange,
    stable_access: i::Access,
    stable_layout: i::Layout,
}

impl<B: hal::Backend> Image<B> {
    fn barrier_to(&self, access: i::Access, layout: i::Layout) -> memory::Barrier<B> {
        memory::Barrier::Image {
            src_access: self.stable_access,
            dst_access: access,
            src_layout: self.stable_layout,
            dst_layout: layout,
            target: &self.handle,
            families: None,
            range: self.range.clone(),
        }
    }
    fn barrier_from(&self, access: i::Access, layout: i::Layout) -> memory::Barrier<B> {
        memory::Barrier::Image {
            src_access: access,
            dst_access: self.stable_access,
            src_layout: layout,
            dst_layout: self.stable_layout,
            target: &self.handle,
            families: None,
            range: self.range.clone(),
        }
    }
}

pub struct RenderPass<B: hal::Backend> {
    pub handle: B::RenderPass,
    attachments: Vec<String>,
    subpasses: Vec<String>,
}

pub struct Resources<B: hal::Backend> {
    pub buffers: HashMap<String, Buffer<B>>,
    pub images: HashMap<String, Image<B>>,
    pub image_views: HashMap<String, B::ImageView>,
    pub render_passes: HashMap<String, RenderPass<B>>,
    pub framebuffers: HashMap<String, (B::Framebuffer, i::Extent)>,
    pub shaders: HashMap<String, B::ShaderModule>,
    pub desc_set_layouts:
        HashMap<String, (Vec<hal::pso::DescriptorBinding>, B::DescriptorSetLayout)>,
    pub desc_pools: HashMap<String, B::DescriptorPool>,
    pub desc_sets: HashMap<String, B::DescriptorSet>,
    pub pipeline_layouts: HashMap<String, B::PipelineLayout>,
    pub graphics_pipelines: HashMap<String, B::GraphicsPipeline>,
    pub compute_pipelines: HashMap<String, (String, B::ComputePipeline)>,
}

pub struct Job<B: hal::Backend, C> {
    submission: c::CommandBuffer<B, C, c::MultiShot, c::Primary>,
}

pub struct Scene<B: hal::Backend, C> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, Job<B, C>>,
    init_submit: c::CommandBuffer<B, C, c::MultiShot, c::Primary>,
    device: B::Device,
    queue_group: hal::QueueGroup<B, C>,
    command_pool: Option<hal::CommandPool<B, C>>,
    upload_buffers: HashMap<String, (B::Buffer, B::Memory)>,
    download_type: hal::MemoryTypeId,
    limits: hal::Limits,
}

fn align(x: u64, y: u64) -> u64 {
    if x > 0 && y > 0 {
        ((x - 1) | (y - 1)) + 1
    } else {
        x
    }
}

impl<B: hal::Backend> Scene<B, hal::General> {
    pub fn new(
        adapter: hal::Adapter<B>,
        raw: &raw::Scene,
        data_path: PathBuf,
    ) -> Result<Self, Error> {
        info!("creating Scene from {:?}", data_path);
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();

        // initialize graphics
        let (device, queue_group) = adapter.open_with(1, |_| true)?;

        let upload_type: hal::MemoryTypeId = memory_types
            .iter()
            .position(|mt| {
                mt.properties
                    .contains(memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT)
                //&&!mt.properties.contains(memory::Properties::CPU_CACHED)
            })
            .unwrap()
            .into();
        let download_type = memory_types
            .iter()
            .position(|mt| {
                mt.properties
                    .contains(memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT)
                //&&!mt.properties.contains(memory::Properties::CPU_CACHED)
            })
            .unwrap()
            .into();
        info!("upload memory: {:?}", upload_type);
        info!("download memory: {:?}", &download_type);

        let mut command_pool = unsafe {
            device
                .create_command_pool_typed(&queue_group, hal::pool::CommandPoolCreateFlags::empty())
        }?;

        // create resources
        let mut resources = Resources::<B> {
            buffers: HashMap::new(),
            images: HashMap::new(),
            image_views: HashMap::new(),
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
        let mut init_cmd = command_pool.acquire_command_buffer::<c::MultiShot>();
        unsafe {
            init_cmd.begin(false);
        }
        // Pass[1]: images, buffers, passes, descriptor set layouts/pools
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
                                b::Access::empty(),
                                access,
                            );
                            unsafe {
                                init_cmd.pipeline_barrier(
                                    pso::PipelineStage::TOP_OF_PIPE,
                                    pso::PipelineStage::BOTTOM_OF_PIPE,
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
                        assert_ne!(upload_req.type_mask & (1 << upload_type.0), 0);
                        let upload_memory =
                            unsafe { device.allocate_memory(upload_type, upload_req.size) }
                                .unwrap();

                        unsafe { device.bind_buffer_memory(&upload_memory, 0, &mut upload_buffer) }
                            .unwrap();
                        // write the data
                        unsafe {
                            let mut mapping = device
                                .acquire_mapping_writer::<u8>(&upload_memory, 0 .. size as _)
                                .unwrap();
                            File::open(data_path.join(data))
                                .unwrap()
                                .read_exact(&mut mapping)
                                .unwrap();
                            device.release_mapping_writer(mapping).unwrap();
                        }
                        // add init commands
                        let final_state = b::Access::SHADER_READ;
                        let pre_barrier = memory::Barrier::whole_buffer(
                            &buffer,
                            b::Access::empty(),
                            b::Access::TRANSFER_WRITE,
                        );
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE,
                                pso::PipelineStage::TRANSFER,
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
                            b::Access::TRANSFER_WRITE,
                            final_state,
                        );
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER,
                                pso::PipelineStage::BOTTOM_OF_PIPE,
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
                    let (stable_access, stable_layout) = if data.is_empty() {
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
                        if false {
                            //TODO
                            let image_barrier = memory::Barrier::Image {
                                src_access: i::Access::empty(),
                                dst_access: access,
                                src_layout: i::Layout::Undefined,
                                dst_layout: layout,
                                target: &image,
                                families: None,
                                range: i::SubresourceRange {
                                    aspects,
                                    ..COLOR_RANGE.clone()
                                },
                            };
                            unsafe {
                                init_cmd.pipeline_barrier(
                                    pso::PipelineStage::TOP_OF_PIPE,
                                    pso::PipelineStage::BOTTOM_OF_PIPE,
                                    memory::Dependencies::empty(),
                                    &[image_barrier],
                                );
                            }
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
                        assert_ne!(upload_req.type_mask & (1 << upload_type.0), 0);
                        let upload_memory =
                            unsafe { device.allocate_memory(upload_type, upload_req.size) }
                                .unwrap();
                        unsafe { device.bind_buffer_memory(&upload_memory, 0, &mut upload_buffer) }
                            .unwrap();
                        // write the data
                        {
                            let mut file = File::open(data_path.join(data)).unwrap();
                            let mut mapping = unsafe {
                                device
                                    .acquire_mapping_writer::<u8>(&upload_memory, 0 .. upload_size)
                            }
                            .unwrap();
                            for y in 0 .. (h as usize * d as usize) {
                                let dest_range = y * row_pitch as usize
                                    .. y * row_pitch as usize + width_bytes as usize;
                                file.read_exact(&mut mapping[dest_range]).unwrap();
                            }
                            unsafe {
                                device.release_mapping_writer(mapping).unwrap();
                            }
                        }
                        // add init commands
                        let final_access = i::Access::SHADER_READ;
                        let final_layout = i::Layout::ShaderReadOnlyOptimal;
                        let pre_barrier = memory::Barrier::Image {
                            src_access: i::Access::empty(),
                            dst_access: i::Access::TRANSFER_WRITE,
                            src_layout: i::Layout::Undefined,
                            dst_layout: i::Layout::TransferDstOptimal,
                            families: None,
                            target: &image,
                            range: COLOR_RANGE.clone(), //TODO
                        };
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE,
                                pso::PipelineStage::TRANSFER,
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
                            src_access: i::Access::TRANSFER_WRITE,
                            dst_access: final_access,
                            src_layout: i::Layout::TransferDstOptimal,
                            dst_layout: final_layout,
                            families: None,
                            target: &image,
                            range: COLOR_RANGE.clone(), //TODO
                        };
                        unsafe {
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER,
                                pso::PipelineStage::BOTTOM_OF_PIPE,
                                memory::Dependencies::empty(),
                                &[post_barrier],
                            );
                        }
                        // done
                        upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                        (final_access, final_layout)
                    };

                    resources.images.insert(
                        name.clone(),
                        Image {
                            handle: image,
                            _memory: gpu_memory,
                            kind,
                            format,
                            range: COLOR_RANGE.clone(),
                            stable_access,
                            stable_layout,
                        },
                    );
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
                            hal::pass::SubpassRef::External
                        } else {
                            let id = subpasses.keys().position(|sp| s == sp).unwrap();
                            hal::pass::SubpassRef::Pass(id)
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
                        src_subpass: subpass_ref(&dep.src_subpass),
                        dst_subpass: subpass_ref(&dep.dst_subpass),
                        src_stage: dep.src_stage.clone(),
                        dst_stage: dep.dst_stage.clone(),
                        src_access: dep.src_access.clone(),
                        dst_access: dep.dst_access.clone(),
                    });

                    let rp = RenderPass {
                        handle: unsafe { device.create_render_pass(raw_atts, raw_subs, raw_deps) }
                            .expect("Render pass creation failure"),
                        attachments: attachments.keys().cloned().collect(),
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
                    let spirv = hal::read_spirv(file).unwrap();
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

        // Pass[2]: image & buffer views, descriptor sets, pipeline layouts
        for (name, resource) in &raw.resources {
            match *resource {
                raw::Resource::ImageView {
                    ref image,
                    kind,
                    format,
                    swizzle,
                    ref range,
                } => {
                    let image = &resources.images[image].handle;
                    let view = unsafe {
                        device.create_image_view(image, kind, format, swizzle, range.clone())
                    }
                    .unwrap();
                    resources.image_views.insert(name.clone(), view);
                }
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
                    resources.desc_sets.insert(name.clone(), desc_set);
                    // fill it up
                    let set = &resources.desc_sets[name];
                    let writes = binding_indices.iter().zip(data).map(|(&binding, range)| {
                        hal::pso::DescriptorSetWrite {
                            set,
                            binding,
                            array_offset: 0,
                            descriptors: match *range {
                                raw::DescriptorRange::Buffers(ref names) => names.iter().map(|s| {
                                    let buf = resources
                                        .buffers
                                        .get(s)
                                        .expect(&format!("Missing buffer: {}", s));
                                    hal::pso::Descriptor::Buffer(&buf.handle, None .. None)
                                }),
                                raw::DescriptorRange::Images(_) => unimplemented!(),
                            },
                        }
                    });
                    unsafe {
                        device.write_descriptor_sets(writes);
                    }
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

        // Pass[3]: framebuffers and pipelines
        for (name, resource) in &raw.resources {
            match *resource {
                raw::Resource::Framebuffer {
                    ref pass,
                    ref views,
                    extent,
                } => {
                    let rp = &resources.render_passes[pass];
                    let framebuffer = {
                        let image_views = rp.attachments.iter().map(|s| {
                            let entry = views.iter().find(|entry| entry.0 == s).unwrap();
                            &resources.image_views[entry.1]
                        });
                        unsafe { device.create_framebuffer(&rp.handle, image_views, extent) }
                            .unwrap()
                    };
                    resources
                        .framebuffers
                        .insert(name.clone(), (framebuffer, extent));
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
                    let desc = pso::GraphicsPipelineDesc {
                        shaders: pso::GraphicsShaderSet {
                            vertex: pso::EntryPoint {
                                entry: "main",
                                module: reshaders
                                    .get(&shaders.vertex)
                                    .expect(&format!("Missing vertex shader: {}", shaders.vertex)),
                                specialization: pso::Specialization::default(),
                            },
                            hull: entry(&shaders.hull),
                            domain: entry(&shaders.domain),
                            geometry: entry(&shaders.geometry),
                            fragment: entry(&shaders.fragment),
                        },
                        rasterizer: rasterizer.clone(),
                        vertex_buffers: vertex_buffers.clone(),
                        attributes: attributes.clone(),
                        input_assembler: input_assembler.clone(),
                        blender: blender.clone(),
                        depth_stencil: depth_stencil.clone(),
                        baked_states: pso::BakedStates::default(), //TODO
                        multisampling: None,                       // TODO
                        layout: &resources.pipeline_layouts[layout],
                        subpass: hal::pass::Subpass {
                            main_pass: &resources.render_passes[&subpass.parent].handle,
                            index: subpass.index,
                        },
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    };
                    let pso = unsafe { device.create_graphics_pipelines(&[desc], None) }
                        .remove(0)
                        .unwrap();
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
                    let pso = unsafe { device.create_compute_pipelines(&[desc], None) }
                        .remove(0)
                        .unwrap();
                    resources
                        .compute_pipelines
                        .insert(name.clone(), (layout.clone(), pso));
                }
                _ => {}
            }
        }

        unsafe {
            init_cmd.finish();
        }

        // fill up command buffers
        let mut jobs = HashMap::new();
        for (name, job) in &raw.jobs {
            use crate::raw::TransferCommand as Tc;
            let mut command_buf = command_pool.acquire_command_buffer::<c::MultiShot>();
            unsafe {
                command_buf.begin(false);
            }
            match *job {
                raw::Job::Transfer(ref command) => match *command {
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
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![
                                sb.barrier_to(b::Access::TRANSFER_READ),
                                db.barrier_to(b::Access::TRANSFER_WRITE),
                            ],
                        );
                        command_buf.copy_buffer(&sb.handle, &db.handle, regions);
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![
                                sb.barrier_from(b::Access::TRANSFER_READ),
                                db.barrier_from(b::Access::TRANSFER_WRITE),
                            ],
                        );
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
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_to(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                dt.barrier_to(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
                        );
                        command_buf.copy_image(
                            &st.handle,
                            i::Layout::TransferSrcOptimal,
                            &dt.handle,
                            i::Layout::TransferDstOptimal,
                            regions,
                        );
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_from(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                dt.barrier_from(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
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
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![
                                sb.barrier_to(b::Access::TRANSFER_READ),
                                dt.barrier_to(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
                        );
                        command_buf.copy_buffer_to_image(
                            &sb.handle,
                            &dt.handle,
                            i::Layout::TransferDstOptimal,
                            regions,
                        );
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![
                                sb.barrier_from(b::Access::TRANSFER_READ),
                                dt.barrier_from(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
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
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_to(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                db.barrier_to(b::Access::TRANSFER_WRITE),
                            ],
                        );
                        command_buf.copy_image_to_buffer(
                            &st.handle,
                            i::Layout::TransferSrcOptimal,
                            &db.handle,
                            regions,
                        );
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_from(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                db.barrier_from(b::Access::TRANSFER_WRITE),
                            ],
                        );
                    },
                    Tc::ClearImage {
                        ref image,
                        color,
                        depth_stencil,
                        ref ranges,
                    } => unsafe {
                        let img = resources
                            .images
                            .get(image)
                            .expect(&format!("Missing clear image: {}", image));
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![img.barrier_to(
                                i::Access::TRANSFER_WRITE,
                                i::Layout::TransferDstOptimal,
                            )],
                        );
                        command_buf.clear_image(
                            &img.handle,
                            i::Layout::TransferDstOptimal,
                            color,
                            depth_stencil,
                            ranges,
                        );
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![img.barrier_from(
                                i::Access::TRANSFER_WRITE,
                                i::Layout::TransferDstOptimal,
                            )],
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
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_to(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                dt.barrier_to(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
                        );
                        command_buf.blit_image(
                            &st.handle,
                            i::Layout::TransferSrcOptimal,
                            &dt.handle,
                            i::Layout::TransferDstOptimal,
                            filter,
                            regions,
                        );
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![
                                st.barrier_from(
                                    i::Access::TRANSFER_READ,
                                    i::Layout::TransferSrcOptimal,
                                ),
                                dt.barrier_from(
                                    i::Access::TRANSFER_WRITE,
                                    i::Layout::TransferDstOptimal,
                                ),
                            ],
                        );
                    },
                    Tc::FillBuffer {
                        ref buffer,
                        start,
                        end,
                        data,
                    } => unsafe {
                        let buf = resources
                            .buffers
                            .get(buffer)
                            .expect(&format!("Missing buffer: {}", buffer));
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TOP_OF_PIPE,
                            pso::PipelineStage::TRANSFER,
                            memory::Dependencies::empty(),
                            vec![buf.barrier_to(b::Access::TRANSFER_WRITE)],
                        );
                        command_buf.fill_buffer(&buf.handle, (start, end), data);
                        command_buf.pipeline_barrier(
                            pso::PipelineStage::TRANSFER,
                            pso::PipelineStage::BOTTOM_OF_PIPE,
                            memory::Dependencies::empty(),
                            vec![buf.barrier_from(b::Access::TRANSFER_WRITE)],
                        );
                    },
                },
                raw::Job::Graphics {
                    ref framebuffer,
                    ref pass,
                    ref clear_values,
                } => unsafe {
                    let (ref fb, extent) = resources.framebuffers[framebuffer];
                    let rp = &resources.render_passes[&pass.0];
                    let rect = pso::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as _,
                        h: extent.height as _,
                    };
                    let mut encoder =
                        command_buf.begin_render_pass_inline(&rp.handle, fb, rect, clear_values);
                    encoder.set_scissors(0, Some(rect));
                    encoder.set_viewports(
                        0,
                        Some(pso::Viewport {
                            rect,
                            depth: 0.0 .. 1.0,
                        }),
                    );

                    for subpass in &rp.subpasses {
                        if Some(subpass) != rp.subpasses.first() {
                            encoder = encoder.next_subpass_inline();
                        }
                        for command in &pass.1[subpass].commands {
                            use crate::raw::DrawCommand as Dc;
                            match *command {
                                Dc::BindIndexBuffer {
                                    ref buffer,
                                    offset,
                                    index_type,
                                } => {
                                    let view = b::IndexBufferView {
                                        buffer: &resources
                                            .buffers
                                            .get(buffer)
                                            .expect(&format!("Missing index buffer: {}", buffer))
                                            .handle,
                                        offset,
                                        index_type,
                                    };
                                    encoder.bind_index_buffer(view);
                                }
                                Dc::BindVertexBuffers(ref buffers) => {
                                    let buffers_raw = buffers.iter().map(|&(ref name, offset)| {
                                        let buf = &resources
                                            .buffers
                                            .get(name)
                                            .expect(&format!("Missing vertex buffer: {}", name))
                                            .handle;
                                        (buf, offset)
                                    });
                                    encoder.bind_vertex_buffers(0, buffers_raw);
                                }
                                Dc::BindPipeline(ref name) => {
                                    let pso = resources
                                        .graphics_pipelines
                                        .get(name)
                                        .expect(&format!("Missing graphics pipeline: {}", name));
                                    encoder.bind_graphics_pipeline(pso);
                                }
                                Dc::BindDescriptorSets {
                                    ref layout,
                                    first,
                                    ref sets,
                                } => {
                                    encoder.bind_graphics_descriptor_sets(
                                        resources.pipeline_layouts.get(layout).expect(&format!(
                                            "Missing pipeline layout: {}",
                                            layout
                                        )),
                                        first,
                                        sets.iter().map(|name| {
                                            resources.desc_sets.get(name).expect(&format!(
                                                "Missing descriptor set: {}",
                                                name
                                            ))
                                        }),
                                        &[],
                                    );
                                }
                                Dc::Draw {
                                    ref vertices,
                                    ref instances,
                                } => {
                                    encoder.draw(vertices.clone(), instances.clone());
                                }
                                Dc::DrawIndexed {
                                    ref indices,
                                    base_vertex,
                                    ref instances,
                                } => {
                                    encoder.draw_indexed(
                                        indices.clone(),
                                        base_vertex,
                                        instances.clone(),
                                    );
                                }
                                Dc::SetViewports(ref viewports) => {
                                    encoder.set_viewports(0, viewports);
                                }
                                Dc::SetScissors(ref scissors) => {
                                    encoder.set_scissors(0, scissors);
                                }
                            }
                        }
                    }
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
                            resources
                                .desc_sets
                                .get(name)
                                .expect(&format!("Missing descriptor set: {}", name))
                        }),
                        &[],
                    );
                    command_buf.dispatch(dispatch);
                },
            }

            unsafe {
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
            device,
            queue_group,
            command_pool: Some(command_pool),
            upload_buffers,
            download_type,
            limits,
        })
    }
}

impl<B: hal::Backend> Scene<B, hal::General> {
    pub fn run<'a, I>(&mut self, job_names: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let jobs = &self.jobs;
        let submits = job_names.into_iter().map(|name| {
            &jobs
                .get(name)
                .expect(&format!("Missing job: {}", name))
                .submission
        });

        let command_buffers = iter::once(&self.init_submit).chain(submits);
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
        assert_ne!(down_req.type_mask & (1 << self.download_type.0), 0);
        let down_memory = unsafe {
            self.device
                .allocate_memory(self.download_type, down_req.size)
        }
        .unwrap();

        unsafe {
            self.device
                .bind_buffer_memory(&down_memory, 0, &mut down_buffer)
        }
        .unwrap();

        let mut command_pool = unsafe {
            self.device.create_command_pool_typed(
                &self.queue_group,
                hal::pool::CommandPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create command pool");
        let mut cmd_buffer = command_pool.acquire_command_buffer::<c::OneShot>();
        unsafe {
            cmd_buffer.begin();
            let pre_barrier = memory::Barrier::whole_buffer(
                &buffer.handle,
                buffer.stable_state,
                b::Access::TRANSFER_READ,
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE,
                pso::PipelineStage::TRANSFER,
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
                b::Access::TRANSFER_READ,
                buffer.stable_state,
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER,
                pso::PipelineStage::BOTTOM_OF_PIPE,
                memory::Dependencies::empty(),
                &[post_barrier],
            );
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
            self.device.destroy_command_pool(command_pool.into_raw());
        }

        let mapping = unsafe { self.device.map_memory(&down_memory, 0 .. down_size) }.unwrap();

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
        assert_ne!(down_req.type_mask & (1 << self.download_type.0), 0);
        let down_memory = unsafe {
            self.device
                .allocate_memory(self.download_type, down_req.size)
        }
        .unwrap();
        unsafe {
            self.device
                .bind_buffer_memory(&down_memory, 0, &mut down_buffer)
        }
        .unwrap();

        let mut command_pool = unsafe {
            self.device.create_command_pool_typed(
                &self.queue_group,
                hal::pool::CommandPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create command pool");
        let mut cmd_buffer = command_pool.acquire_command_buffer::<c::OneShot>();
        unsafe {
            cmd_buffer.begin();
            let pre_barrier = memory::Barrier::Image {
                src_access: image.stable_access,
                dst_access: i::Access::TRANSFER_READ,
                src_layout: image.stable_layout,
                dst_layout: i::Layout::TransferSrcOptimal,
                target: &image.handle,
                families: None,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE,
                pso::PipelineStage::TRANSFER,
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
                src_access: i::Access::TRANSFER_READ,
                dst_access: image.stable_access,
                src_layout: i::Layout::TransferSrcOptimal,
                dst_layout: image.stable_layout,
                target: &image.handle,
                families: None,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER,
                pso::PipelineStage::BOTTOM_OF_PIPE,
                memory::Dependencies::empty(),
                &[post_barrier],
            );
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
            self.device.destroy_command_pool(command_pool.into_raw());
        }

        let mapping = unsafe { self.device.map_memory(&down_memory, 0 .. down_size) }.unwrap();

        FetchGuard {
            device: &mut self.device,
            buffer: Some(down_buffer),
            memory: Some(down_memory),
            mapping,
            row_pitch: row_pitch as _,
            width: width_bytes as _,
        }
    }
}

impl<B: hal::Backend, C> Drop for Scene<B, C> {
    fn drop(&mut self) {
        unsafe {
            for (_, (buffer, memory)) in self.upload_buffers.drain() {
                self.device.destroy_buffer(buffer);
                self.device.free_memory(memory);
            }
            //TODO: free those properly
            let _ = &self.queue_group;
            self.device
                .destroy_command_pool(self.command_pool.take().unwrap().into_raw());
        }
    }
}
