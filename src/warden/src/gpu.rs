use failure::Error;
#[cfg(feature = "glsl-to-spirv")]
use glsl_to_spirv;

use std::collections::HashMap;
use std::io::Read;
use std::fs::File;
use std::path::PathBuf;
use std::{slice};

use hal::{self, buffer as b, command as c, format as f, image as i, memory, pso};
use hal::{Device, DescriptorPool, PhysicalDevice};

use raw;


const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: f::AspectFlags::COLOR,
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
        unsafe {
            slice::from_raw_parts(self.mapping.offset(offset), self.width)
        }
    }
}

impl<'a, B: hal::Backend> Drop for FetchGuard<'a, B> {
    fn drop(&mut self) {
        let buffer = self.buffer.take().unwrap();
        let memory = self.memory.take().unwrap();
        self.device.unmap_memory(&memory);
        self.device.destroy_buffer(buffer);
        self.device.free_memory(memory);
    }
}

pub struct Buffer<B: hal::Backend> {
    handle: B::Buffer,
    _memory: B::Memory,
    size: usize,
    stable_state: b::State,
}

impl<B: hal::Backend> Buffer<B> {
    fn barrier_to(&self, state: b::State) -> memory::Barrier<B> {
        memory::Barrier::Buffer {
            states: self.stable_state .. state,
            target: &self.handle,
        }
    }
    fn barrier_from(&self, state: b::State) -> memory::Barrier<B> {
        memory::Barrier::Buffer {
            states: state .. self.stable_state,
            target: &self.handle,
        }
    }
}

pub struct Image<B: hal::Backend> {
    handle: B::Image,
    _memory: B::Memory,
    kind: i::Kind,
    format: f::Format,
    stable_state: i::State,
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
    pub framebuffers: HashMap<String, (B::Framebuffer, hal::device::Extent)>,
    pub shaders: HashMap<String, B::ShaderModule>,
    pub desc_set_layouts: HashMap<String, (Vec<usize>, B::DescriptorSetLayout)>,
    pub desc_pools: HashMap<String, B::DescriptorPool>,
    pub desc_sets: HashMap<String, B::DescriptorSet>,
    pub pipeline_layouts: HashMap<String, B::PipelineLayout>,
    pub graphics_pipelines: HashMap<String, B::GraphicsPipeline>,
    pub compute_pipelines: HashMap<String, (String, B::ComputePipeline)>,
}

pub struct Job<B: hal::Backend, C> {
    submission: c::Submit<B, C, c::MultiShot, c::Primary>,
}

pub struct Scene<B: hal::Backend, C> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, Job<B, C>>,
    init_submit: c::Submit<B, C, c::MultiShot, c::Primary>,
    device: B::Device,
    queue_group: hal::QueueGroup<B, C>,
    command_pool: hal::CommandPool<B, C>,
    upload_buffers: HashMap<String, (B::Buffer, B::Memory)>,
    download_type: hal::MemoryTypeId,
    limits: hal::Limits,
}

fn align(x: usize, y: usize) -> usize {
    if x > 0 && y > 0 {
        ((x - 1) | (y - 1)) + 1
    } else {
        x
    }
}

impl<B: hal::Backend> Scene<B, hal::General> {
    pub fn new(
        adapter: hal::Adapter<B>, raw: &raw::Scene, data_path: PathBuf
    ) -> Result<Self, Error> {
        info!("creating Scene from {:?}", data_path);
        let memory_types = adapter
            .physical_device
            .memory_properties()
            .memory_types;
        let limits = adapter
            .physical_device
            .get_limits();

        // initialize graphics
        let (device, queue_group) = adapter.open_with(1, |_| true)?;

        let upload_type: hal::MemoryTypeId = memory_types
            .iter()
            .position(|mt| {
                mt.properties.contains(memory::Properties::CPU_VISIBLE)
                //&&!mt.properties.contains(memory::Properties::CPU_CACHED)
            })
            .unwrap()
            .into();
        let download_type = memory_types
            .iter()
            .position(|mt| {
                mt.properties.contains(memory::Properties::CPU_VISIBLE | memory::Properties::CPU_CACHED)
            })
            .unwrap()
            .into();
        info!("upload memory: {:?}", upload_type);
        info!("download memory: {:?}", &download_type);

        let mut command_pool = device.create_command_pool_typed(
            &queue_group,
            hal::pool::CommandPoolCreateFlags::empty(),
            1 + raw.jobs.len(),
        );

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
        let init_submit = {
            let mut init_cmd = command_pool.acquire_command_buffer(false);

            // Pass[1]: images, buffers, passes, descriptor set layouts/pools
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::Buffer { size, usage, ref data } => {
                        // allocate memory
                        let unbound = device.create_buffer(size as _, usage)
                            .unwrap();
                        let requirements = device.get_buffer_requirements(&unbound);
                        let memory_type = memory_types
                            .iter()
                            .enumerate()
                            .position(|(id, mt)| {
                                requirements.type_mask & (1 << id) != 0 &&
                                mt.properties.contains(memory::Properties::DEVICE_LOCAL)
                            })
                            .unwrap()
                            .into();
                        let memory = device.allocate_memory(memory_type, requirements.size)
                            .unwrap();
                        let buffer = device.bind_buffer_memory(&memory, 0, unbound)
                            .unwrap();

                        // process initial data for the buffer
                        let stable_state = if data.is_empty() {
                            let access = b::Access::SHADER_READ; //TODO
                            if false { //TODO
                                let buffer_barrier = memory::Barrier::Buffer {
                                    states: b::Access::empty() .. access,
                                    target: &buffer,
                                };
                                init_cmd.pipeline_barrier(
                                    pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                    &[buffer_barrier],
                                );
                            }
                            access
                        } else {
                            // calculate required sizes
                            let upload_size = align(size, limits.min_buffer_copy_pitch_alignment) as u64;
                            // create upload buffer
                            let unbound_buffer = device.create_buffer(upload_size, b::Usage::TRANSFER_SRC)
                                .unwrap();
                            let upload_req = device.get_buffer_requirements(&unbound_buffer);
                            assert_ne!(upload_req.type_mask & (1 << upload_type.0), 0);
                            let upload_memory = device.allocate_memory(upload_type, upload_req.size)
                                .unwrap();
                            let upload_buffer = device.bind_buffer_memory(&upload_memory, 0, unbound_buffer)
                                .unwrap();
                            // write the data
                            {
                                let mut mapping = device.acquire_mapping_writer::<u8>(&upload_memory, 0 .. size as _)
                                    .unwrap();
                                File::open(data_path.join(data))
                                    .unwrap()
                                    .read_exact(&mut mapping)
                                    .unwrap();
                                device.release_mapping_writer(mapping);
                            }
                            // add init commands
                            let final_state = b::Access::SHADER_READ;
                            let pre_barrier = memory::Barrier::Buffer {
                                states: b::Access::empty() .. b::Access::TRANSFER_WRITE,
                                target: &buffer,
                            };
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                                &[pre_barrier],
                            );
                            let copy = c::BufferCopy {
                                src: 0,
                                dst: 0,
                                size: size as _,
                            };
                            init_cmd.copy_buffer(
                                &upload_buffer,
                                &buffer,
                                &[copy],
                            );
                            let post_barrier = memory::Barrier::Buffer {
                                states: b::Access::TRANSFER_WRITE .. final_state,
                                target: &buffer,
                            };
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                &[post_barrier],
                            );
                            // done
                            upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                            final_state
                        };

                        resources.buffers.insert(name.clone(), Buffer {
                            handle: buffer,
                            _memory: memory,
                            size,
                            stable_state,
                        });
                    }
                    raw::Resource::Image { kind, num_levels, format, usage, ref data } => {
                        // allocate memory
                        let unbound = device.create_image(kind, num_levels, format, usage)
                            .unwrap();
                        let requirements = device.get_image_requirements(&unbound);
                        let memory_type = memory_types
                            .iter()
                            .enumerate()
                            .position(|(id, mt)| {
                                requirements.type_mask & (1 << id) != 0 &&
                                mt.properties.contains(memory::Properties::DEVICE_LOCAL)
                            })
                            .unwrap()
                            .into();
                        let memory = device.allocate_memory(memory_type, requirements.size)
                            .unwrap();
                        let image = device.bind_image_memory(&memory, 0, unbound)
                            .unwrap();

                        // process initial data for the image
                        let stable_state = if data.is_empty() {
                            let (aspects, access, layout) = if format.is_color() {
                                (f::AspectFlags::COLOR, i::Access::COLOR_ATTACHMENT_WRITE, i::ImageLayout::ColorAttachmentOptimal)
                            } else {
                                (f::AspectFlags::DEPTH | f::AspectFlags::STENCIL, i::Access::DEPTH_STENCIL_ATTACHMENT_WRITE, i::ImageLayout::DepthStencilAttachmentOptimal)
                            };
                            if false { //TODO
                                let image_barrier = memory::Barrier::Image {
                                    states: (i::Access::empty(), i::ImageLayout::Undefined) .. (access, layout),
                                    target: &image,
                                    range: i::SubresourceRange {
                                        aspects,
                                        .. COLOR_RANGE.clone()
                                    },
                                };
                                init_cmd.pipeline_barrier(pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::BOTTOM_OF_PIPE, &[image_barrier]);
                            }
                            (access, layout)
                        } else {
                            // calculate required sizes
                            let (w, h, d, aa) = kind.get_dimensions();
                            assert_eq!(aa, i::AaMode::Single);

                            let base_format = format.base_format();
                            let format_desc = base_format.0.desc();
                            let (block_width, block_height) = format_desc.dim;

                            // Width and height need to be multiple of the block dimensions.
                            let w = align(w as usize, block_width as usize);
                            let h = align(h as usize, block_height as usize);

                            let width_bytes = (format_desc.bits as usize * w as usize) / (8 * block_width as usize);
                            let row_pitch = align(width_bytes, limits.min_buffer_copy_pitch_alignment);
                            let upload_size = (row_pitch as u64 * h as u64 * d as u64) / block_height as u64;
                            // create upload buffer
                            let unbound_buffer = device.create_buffer(upload_size, b::Usage::TRANSFER_SRC)
                                .unwrap();
                            let upload_req = device.get_buffer_requirements(&unbound_buffer);
                            assert_ne!(upload_req.type_mask & (1 << upload_type.0), 0);
                            let upload_memory = device.allocate_memory(upload_type, upload_req.size)
                                .unwrap();
                            let upload_buffer = device.bind_buffer_memory(&upload_memory, 0, unbound_buffer)
                                .unwrap();
                            // write the data
                            {
                                let mut file = File::open(data_path.join(data))
                                    .unwrap();
                                let mut mapping = device.acquire_mapping_writer::<u8>(&upload_memory, 0..upload_size)
                                    .unwrap();
                                for y in 0 .. (h as usize * d as usize) {
                                    let dest_range = y as usize * row_pitch .. y as usize * row_pitch + width_bytes;
                                    file.read_exact(&mut mapping[dest_range])
                                        .unwrap();
                                }
                                device.release_mapping_writer(mapping);
                            }
                            // add init commands
                            let final_state = (i::Access::SHADER_READ, i::ImageLayout::ShaderReadOnlyOptimal);
                            let pre_barrier = memory::Barrier::Image {
                                states: (i::Access::empty(), i::ImageLayout::Undefined) ..
                                        (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                                target: &image,
                                range: COLOR_RANGE.clone(), //TODO
                            };
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                                &[pre_barrier],
                            );

                            let buffer_width = (row_pitch as u32 * 8) / format_desc.bits as u32;
                            let copy = c::BufferImageCopy {
                                buffer_offset: 0,
                                buffer_width,
                                buffer_height: h as u32,
                                image_layers: i::SubresourceLayers {
                                    aspects: f::AspectFlags::COLOR,
                                    level: 0,
                                    layers: 0 .. 1,
                                },
                                image_offset: c::Offset { x: 0, y: 0, z: 0 },
                                image_extent: hal::device::Extent {
                                    width: w as _,
                                    height: h as _,
                                    depth: d as _,
                                },
                            };
                            init_cmd.copy_buffer_to_image(
                                &upload_buffer,
                                &image,
                                i::ImageLayout::TransferDstOptimal,
                                &[copy],
                            );
                            let post_barrier = memory::Barrier::Image {
                                states: (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal) .. final_state,
                                target: &image,
                                range: COLOR_RANGE.clone(), //TODO
                            };
                            init_cmd.pipeline_barrier(
                                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                &[post_barrier],
                            );
                            // done
                            upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                            final_state
                        };

                        resources.images.insert(name.clone(), Image {
                            handle: image,
                            _memory: memory,
                            kind,
                            format,
                            stable_state,
                        });
                    }
                    raw::Resource::RenderPass { ref attachments, ref subpasses, ref dependencies } => {
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
                                let colors = sp.colors
                                    .iter()
                                    .map(&att_ref)
                                    .collect::<Vec<_>>();
                                let ds = sp.depth_stencil
                                    .as_ref()
                                    .map(&att_ref);
                                let inputs = sp.inputs
                                    .iter()
                                    .map(&att_ref)
                                    .collect::<Vec<_>>();
                                let preserves = sp.preserves
                                    .iter()
                                    .map(|sp| {
                                        attachments.keys().position(|s| s == sp).unwrap()
                                    })
                                    .collect::<Vec<_>>();
                                (colors, ds, inputs, preserves)
                            })
                            .collect::<Vec<_>>();
                        let raw_subs = temp
                            .iter()
                            .map(|t| hal::pass::SubpassDesc {
                                colors: &t.0,
                                depth_stencil: t.1.as_ref(),
                                inputs: &t.2,
                                preserves: &t.3,
                            })
                            .collect::<Vec<_>>();
                        let raw_deps = dependencies
                            .iter()
                            .map(|dep| hal::pass::SubpassDependency {
                                passes: subpass_ref(&dep.passes.start) .. subpass_ref(&dep.passes.end),
                                stages: dep.stages.clone(),
                                accesses: dep.accesses.clone(),
                            });

                        let rp = RenderPass {
                            handle: device.create_render_pass(raw_atts, raw_subs, raw_deps),
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
                        let base_file = File::open(&full_path)
                            .unwrap();
                        let mut file = match &*full_path
                            .extension()
                            .unwrap()
                            .to_string_lossy()
                        {
                            "spirv" => base_file,
                            #[cfg(feature = "glsl-to-spirv")]
                            "vert" => transpile(base_file, glsl_to_spirv::ShaderType::Vertex),
                            #[cfg(feature = "glsl-to-spirv")]
                            "frag" => transpile(base_file, glsl_to_spirv::ShaderType::Fragment),
                            #[cfg(feature = "glsl-to-spirv")]
                            "comp" => transpile(base_file, glsl_to_spirv::ShaderType::Compute),
                            other => panic!("Unknown shader extension: {}", other),
                        };
                        let mut spirv = Vec::new();
                        file.read_to_end(&mut spirv).unwrap();
                        let module = device.create_shader_module(&spirv)
                            .unwrap();
                        resources.shaders.insert(name.clone(), module);
                    }
                    raw::Resource::DescriptorSetLayout { ref bindings } => {
                        let layout = device.create_descriptor_set_layout(bindings);
                        let binding_starts = bindings.iter().map(|dsb| dsb.binding).collect();
                        resources.desc_set_layouts.insert(name.clone(), (binding_starts, layout));
                    }
                    raw::Resource::DescriptorPool { capacity, ref ranges } => {
                        let pool = device.create_descriptor_pool(capacity, ranges);
                        resources.desc_pools.insert(name.clone(), pool);
                    }
                    _ => {}
                }
            }

            // Pass[2]: image & buffer views, descriptor sets, pipeline layouts
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::ImageView { ref image, format, swizzle, ref range } => {
                        let image = &resources.images[image].handle;
                        let view = device.create_image_view(image, format, swizzle, range.clone())
                            .unwrap();
                        resources.image_views.insert(name.clone(), view);
                    }
                    raw::Resource::DescriptorSet { ref pool, ref layout, ref data } => {
                        // create a descriptor set
                        let (ref binding_starts, ref set_layout) = resources.desc_set_layouts[layout];
                        let desc_set = resources.desc_pools
                            .get_mut(pool)
                            .expect(&format!("Missing descriptor pool: {}", pool))
                            .allocate_set(set_layout);
                        resources.desc_sets.insert(name.clone(), desc_set);
                        // fill it up
                        let set = &resources.desc_sets[name];
                        let res_buffers = &resources.buffers;
                        let updates = binding_starts
                            .iter()
                            .zip(data)
                            .map(|(&binding, range)| hal::pso::DescriptorSetWrite {
                                set,
                                binding,
                                array_offset: 0,
                                write: match *range {
                                    raw::DescriptorRange::StorageBuffers(ref names) => {
                                        let buffers = names
                                            .iter()
                                            .map(|s| (&res_buffers[s].handle, ..))
                                            .collect();
                                        hal::pso::DescriptorWrite::StorageBuffer(buffers)
                                    }
                                },
                            })
                            .collect::<Vec<_>>();
                        device.update_descriptor_sets(&updates);
                    }
                    raw::Resource::PipelineLayout { ref set_layouts, ref push_constant_ranges } => {
                        let layout = {
                            let layouts = set_layouts
                                .iter()
                                .map(|sl| &resources.desc_set_layouts[sl].1);
                            device.create_pipeline_layout(layouts, push_constant_ranges)
                        };
                        resources.pipeline_layouts.insert(name.clone(), layout);
                    }
                    _ => {}
                }
            }

            // Pass[3]: framebuffers and pipelines
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::Framebuffer { ref pass, ref views, extent } => {
                        let rp = &resources.render_passes[pass];
                        let framebuffer = {
                            let image_views = rp.attachments
                                .iter()
                                .map(|s| {
                                    let entry = views
                                        .iter()
                                        .find(|entry| entry.0 == s)
                                        .unwrap();
                                    &resources.image_views[entry.1]
                                });
                            device.create_framebuffer(&rp.handle, image_views, extent)
                                .unwrap()
                        };
                        resources.framebuffers.insert(name.clone(), (framebuffer, extent));
                    }
                    raw::Resource::GraphicsPipeline {
                        ref shaders, ref rasterizer, ref vertex_buffers, ref attributes,
                        ref input_assembler, ref blender, depth_stencil, ref layout, ref subpass,
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
                                    specialization: &[],
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
                                    specialization: &[],
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
                            layout: &resources.pipeline_layouts[layout],
                            subpass: hal::pass::Subpass {
                                main_pass: &resources.render_passes[&subpass.parent].handle,
                                index: subpass.index,
                            },
                            flags: pso::PipelineCreationFlags::empty(),
                            parent: pso::BasePipeline::None,
                        };
                        let pso = device.create_graphics_pipelines(&[desc])
                            .swap_remove(0)
                            .unwrap();
                        resources.graphics_pipelines.insert(name.clone(), pso);
                    }
                    raw::Resource::ComputePipeline { ref shader, ref layout } => {
                        let desc = pso::ComputePipelineDesc {
                            shader: pso::EntryPoint {
                                entry: "main",
                                module: resources.shaders
                                    .get(shader)
                                    .expect(&format!("Missing compute shader: {}", shader)),
                                specialization: &[],
                            },
                            layout: resources.pipeline_layouts
                                .get(layout)
                                .expect(&format!("Missing pipeline layout: {}", layout)),
                            flags: pso::PipelineCreationFlags::empty(),
                            parent: pso::BasePipeline::None,
                        };
                        let pso = device.create_compute_pipelines(&[desc])
                            .swap_remove(0)
                            .unwrap();
                        resources.compute_pipelines.insert(name.clone(), (layout.clone(), pso));
                    }
                    _ => {}
                }
            }

            init_cmd.finish()
        };

        // fill up command buffers
        let mut jobs = HashMap::new();
        for (name, job) in &raw.jobs {
            let mut command_buf = command_pool.acquire_command_buffer(false);
            match *job {
                raw::Job::Transfer { ref commands } => {
                    use raw::TransferCommand as Tc;
                    for command in commands {
                        match *command {
                            Tc::CopyBuffer { ref src, ref dst, ref regions } => {
                                let sb = resources.buffers
                                    .get(src)
                                    .expect(&format!("Missing source buffer: {}", src));
                                let db = resources.buffers
                                    .get(dst)
                                    .expect(&format!("Missing destination buffer: {}", dst));
                                command_buf.pipeline_barrier(
                                    pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                                    vec![
                                        sb.barrier_to(b::State::TRANSFER_READ),
                                        db.barrier_to(b::State::TRANSFER_WRITE),
                                    ],
                                );
                                command_buf.copy_buffer(&sb.handle, &db.handle, regions);
                                command_buf.pipeline_barrier(
                                    pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                                    vec![
                                        sb.barrier_from(b::State::TRANSFER_READ),
                                        db.barrier_from(b::State::TRANSFER_WRITE),
                                    ],
                                );
                            }
                            Tc::CopyBufferToImage => unimplemented!(),
                            Tc::CopyImageToBuffer => unimplemented!(),
                        }
                    }
                }
                raw::Job::Graphics { ref framebuffer, ref pass, ref clear_values } => {
                    let (ref fb, extent) = resources.framebuffers[framebuffer];
                    let rp = &resources.render_passes[&pass.0];
                    let rect = c::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as _,
                        h: extent.height as _,
                    };
                    let mut encoder = command_buf.begin_renderpass_inline(&rp.handle, fb, rect, clear_values);
                    encoder.set_scissors(Some(rect));
                    encoder.set_viewports(Some(c::Viewport {
                        rect,
                        depth: 0.0 .. 1.0,
                    }));

                    for subpass in &rp.subpasses {
                        if Some(subpass) != rp.subpasses.first() {
                            encoder = encoder.next_subpass_inline();
                        }
                        for command in &pass.1[subpass].commands {
                            use raw::DrawCommand as Dc;
                            match *command {
                                Dc::BindIndexBuffer { ref buffer, offset, index_type } => {
                                    let view = b::IndexBufferView {
                                        buffer: &resources.buffers
                                            .get(buffer)
                                            .expect(&format!("Missing index buffer: {}", buffer))
                                            .handle,
                                        offset,
                                        index_type,
                                    };
                                    encoder.bind_index_buffer(view);
                                }
                                Dc::BindVertexBuffers(ref buffers) => {
                                    let buffers_raw = buffers
                                        .iter()
                                        .map(|&(ref name, offset)| {
                                            let buf = &resources.buffers
                                                .get(name)
                                                .expect(&format!("Missing vertex buffer: {}", name))
                                                .handle;
                                            (buf, offset)
                                        })
                                        .collect::<Vec<_>>();
                                    let set = pso::VertexBufferSet(buffers_raw);
                                    encoder.bind_vertex_buffers(set);
                                }
                                Dc::BindPipeline(ref name) => {
                                    let pso = resources.graphics_pipelines
                                        .get(name)
                                        .expect(&format!("Missing graphics pipeline: {}", name));
                                    encoder.bind_graphics_pipeline(pso);
                                }
                                Dc::BindDescriptorSets { ref layout, first, ref sets } => {
                                    encoder.bind_graphics_descriptor_sets(
                                        resources.pipeline_layouts
                                            .get(layout)
                                            .expect(&format!("Missing pipeline layout: {}", layout)),
                                        first,
                                        sets.iter().map(|name| {
                                            resources.desc_sets
                                                .get(name)
                                                .expect(&format!("Missing descriptor set: {}", name))
                                        }),
                                    );
                                }
                                Dc::Draw { ref vertices, ref instances } => {
                                    encoder.draw(vertices.clone(), instances.clone());
                                }
                                Dc::DrawIndexed { ref indices, base_vertex, ref instances } => {
                                    encoder.draw_indexed(indices.clone(), base_vertex, instances.clone());
                                }
                                Dc::SetViewports(ref viewports) => {
                                    encoder.set_viewports(viewports);
                                }
                                Dc::SetScissors(ref scissors) => {
                                    encoder.set_scissors(scissors);
                                }
                            }
                        }
                    }
                }
                raw::Job::Compute { ref pipeline, ref descriptor_sets, ref dispatch } => {
                    let (ref layout, ref pso) = resources.compute_pipelines[pipeline];
                    command_buf.bind_compute_pipeline(pso);
                    command_buf.bind_compute_descriptor_sets(
                        resources.pipeline_layouts
                            .get(layout)
                            .expect(&format!("Missing pipeline layout: {}", layout)),
                        0,
                        descriptor_sets.iter().map(|name| {
                            resources.desc_sets
                                .get(name)
                                .expect(&format!("Missing descriptor set: {}", name))
                        }),
                    );
                    command_buf.dispatch(dispatch.0, dispatch.1, dispatch.2);
                }
            }

            jobs.insert(name.clone(), Job {
                submission: command_buf.finish(),
            });
        }

        // done
        Ok(Scene {
            resources,
            jobs,
            init_submit,
            device,
            queue_group,
            command_pool,
            upload_buffers,
            download_type,
            limits,
        })
    }
}

impl<B: hal::Backend> Scene<B, hal::General> {
    pub fn run<'a, I>(&mut self, job_names: I)
    where
        I: IntoIterator<Item = &'a str>
    {
        let jobs = &self.jobs;
        let submits = job_names
            .into_iter()
            .map(|name| {
                &jobs
                    .get(name)
                    .expect(&format!("Missing job: {}", name))
                    .submission
            });

        let submission = hal::queue::Submission::new()
            .submit(Some(&self.init_submit))
            .submit(submits);
        self.queue_group.queues[0].submit(submission, None);
    }

    pub fn fetch_buffer(&mut self, name: &str) -> FetchGuard<B> {
        let buffer = self.resources.buffers
            .get(name)
            .expect(&format!("Unable to find buffer to fetch: {}", name));
        let limits = &self.limits;

        let down_size = align(buffer.size, limits.min_buffer_copy_pitch_alignment) as u64;

        let unbound_buffer = self.device.create_buffer(down_size, b::Usage::TRANSFER_DST)
            .unwrap();
        let down_req = self.device.get_buffer_requirements(&unbound_buffer);
        assert_ne!(down_req.type_mask & (1<<self.download_type.0), 0);
        let down_memory = self.device.allocate_memory(self.download_type, down_req.size)
            .unwrap();
        let down_buffer = self.device.bind_buffer_memory(&down_memory, 0, unbound_buffer)
            .unwrap();

        let mut command_pool = self.device.create_command_pool_typed(
            &self.queue_group,
            hal::pool::CommandPoolCreateFlags::empty(),
            1,
        );
        let copy_submit = {
            let mut cmd_buffer = command_pool.acquire_command_buffer(false);
            let pre_barrier = memory::Barrier::Buffer {
                states: buffer.stable_state .. b::Access::TRANSFER_READ,
                target: &buffer.handle,
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                &[pre_barrier],
            );

            let copy = c::BufferCopy {
                src: 0,
                dst: 0,
                size: buffer.size as _,
            };
            cmd_buffer.copy_buffer(
                &buffer.handle,
                &down_buffer,
                &[copy],
            );

            let post_barrier = memory::Barrier::Buffer {
                states: b::Access::TRANSFER_READ .. buffer.stable_state,
                target: &buffer.handle,
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                &[post_barrier],
            );
            cmd_buffer.finish()
        };

        let copy_fence = self.device.create_fence(false);
        let submission = hal::queue::Submission::new()
            .submit(Some(copy_submit));
        self.queue_group.queues[0].submit(submission, Some(&copy_fence));
        //queue.destroy_command_pool(command_pool);
        self.device.wait_for_fence(&copy_fence, !0);
        self.device.destroy_fence(copy_fence);

        let mapping = self
            .device
            .map_memory(&down_memory, 0 .. down_size)
            .unwrap();

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
        let image = self.resources.images
            .get(name)
            .expect(&format!("Unable to find image to fetch: {}", name));
        let limits = &self.limits;

        let (width, height, depth, aa) = image.kind.get_dimensions();
        assert_eq!(aa, i::AaMode::Single);

        // TODO:
        let base_format = image.format.base_format();
        let format_desc = base_format.0.desc();
        let (block_width, block_height) = format_desc.dim;

        // Width and height need to be multiple of the block dimensions.
        let width = align(width as usize, block_width as usize);
        let height = align(height as usize, block_height as usize);

        let width_bytes = (format_desc.bits as usize * width as usize) / (8 * block_width as usize);
        let row_pitch = align(width_bytes, limits.min_buffer_copy_pitch_alignment);
        let down_size = (row_pitch as u64 * height as u64 * depth as u64) / block_height as u64;

        let unbound_buffer = self.device.create_buffer(down_size, b::Usage::TRANSFER_DST)
            .unwrap();
        let down_req = self.device.get_buffer_requirements(&unbound_buffer);
        assert_ne!(down_req.type_mask & (1<<self.download_type.0), 0);
        let down_memory = self.device.allocate_memory(self.download_type, down_req.size)
            .unwrap();
        let down_buffer = self.device.bind_buffer_memory(&down_memory, 0, unbound_buffer)
            .unwrap();

        let mut command_pool = self.device.create_command_pool_typed(
            &self.queue_group,
            hal::pool::CommandPoolCreateFlags::empty(),
            1,
        );
        let copy_submit = {
            let mut cmd_buffer = command_pool.acquire_command_buffer(false);
            let pre_barrier = memory::Barrier::Image {
                states: image.stable_state .. (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal),
                target: &image.handle,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER,
                &[pre_barrier],
            );

            let copy = c::BufferImageCopy {
                buffer_offset: 0,
                buffer_width: (row_pitch as u32 * 8) / format_desc.bits as u32,
                buffer_height: height as u32,
                image_layers: i::SubresourceLayers {
                    aspects: f::AspectFlags::COLOR,
                    level: 0,
                    layers: 0 .. 1,
                },
                image_offset: c::Offset { x: 0, y: 0, z: 0 },
                image_extent: hal::device::Extent {
                    width: width as _,
                    height: height as _,
                    depth: depth as _,
                },
            };
            cmd_buffer.copy_image_to_buffer(
                &image.handle,
                i::ImageLayout::TransferSrcOptimal,
                &down_buffer,
                &[copy],
            );

            let post_barrier = memory::Barrier::Image {
                states: (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal) .. image.stable_state,
                target: &image.handle,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE,
                &[post_barrier],
            );
            cmd_buffer.finish()
        };

        let copy_fence = self.device.create_fence(false);
        let submission = hal::queue::Submission::new()
            .submit(Some(copy_submit));
        self.queue_group.queues[0].submit(submission, Some(&copy_fence));
        //queue.destroy_command_pool(command_pool);
        self.device.wait_for_fence(&copy_fence, !0);
        self.device.destroy_fence(copy_fence);

        let mapping = self
            .device
            .map_memory(&down_memory, 0 .. down_size)
            .unwrap();

        FetchGuard {
            device: &mut self.device,
            buffer: Some(down_buffer),
            memory: Some(down_memory),
            mapping,
            row_pitch,
            width: width_bytes,
        }
    }
}

impl<B: hal::Backend, C> Drop for Scene<B, C> {
    fn drop(&mut self) {
        for (_, (buffer, memory)) in self.upload_buffers.drain() {
            self.device.destroy_buffer(buffer);
            self.device.free_memory(memory);
        }
        //TODO: free those properly
        let _ = &self.queue_group;
        let _ = &self.command_pool;
        //self.device.destroy_command_pool(self.command_pool.downgrade())
    }
}
