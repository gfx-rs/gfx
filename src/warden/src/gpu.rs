use std::collections::HashMap;
use std::io::Read;
use std::fs::File;
use std::slice;

use hal::{self, buffer, format as f, image as i, memory, pso};
use hal::{Device, DescriptorPool, PhysicalDevice, QueueFamily};

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
        self.device.release_mapping_raw(&buffer, None);
        self.device.destroy_buffer(buffer);
        self.device.free_memory(memory);
    }
}

pub struct Image<B: hal::Backend> {
    pub handle: B::Image,
    #[allow(dead_code)]
    memory: B::Memory,
    kind: i::Kind,
    format: hal::format::Format,
    stable_state: i::State,
}

pub struct RenderPass<B: hal::Backend> {
    pub handle: B::RenderPass,
    attachments: Vec<String>,
    subpasses: Vec<String>,
}

pub struct Resources<B: hal::Backend> {
    pub buffers: HashMap<String, (B::Buffer, B::Memory)>,
    pub images: HashMap<String, Image<B>>,
    pub image_views: HashMap<String, B::ImageView>,
    pub render_passes: HashMap<String, RenderPass<B>>,
    pub framebuffers: HashMap<String, (B::Framebuffer, hal::device::Extent)>,
    pub desc_set_layouts: HashMap<String, B::DescriptorSetLayout>,
    pub desc_pools: HashMap<String, B::DescriptorPool>,
    pub desc_sets: HashMap<String, B::DescriptorSet>,
    pub pipeline_layouts: HashMap<String, B::PipelineLayout>,
}

pub struct Scene<B: hal::Backend> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, hal::command::Submit<B, hal::queue::Graphics>>,
    init_submit: Option<hal::command::Submit<B, hal::queue::Graphics>>,
    device: B::Device,
    queue_group: hal::QueueGroup<B, hal::queue::Graphics>,
    command_pool: hal::CommandPool<B, hal::queue::Graphics>,
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

impl<B: hal::Backend> Scene<B> {
    pub fn new(adapter: hal::Adapter<B>, raw: &raw::Scene, data_path: &str) -> Self {
        info!("creating Scene from {}", data_path);
        let memory_types = adapter
            .physical_device
            .memory_properties()
            .memory_types;
        let limits = adapter
            .physical_device
            .get_limits();

        // initialize graphics
        let hal::Gpu { device, mut queue_groups } =
            adapter.open_with(|family| {
                if family.supports_graphics() {
                    Some(1)
                } else { None }
            });

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

        let queue_group = hal::QueueGroup::<_, hal::Graphics>::new(queue_groups.remove(0));
        let mut command_pool = device.create_command_pool_typed(
            &queue_group,
            hal::pool::CommandPoolCreateFlags::empty(),
            1 + raw.jobs.len(),
        );

        // create resources
        let mut resources = Resources {
            buffers: HashMap::new(),
            images: HashMap::new(),
            image_views: HashMap::new(),
            render_passes: HashMap::new(),
            framebuffers: HashMap::new(),
            desc_set_layouts: HashMap::new(),
            desc_pools: HashMap::new(),
            desc_sets: HashMap::new(),
            pipeline_layouts: HashMap::new(),
        };
        let mut upload_buffers = HashMap::new();
        let init_submit = {
            let mut init_cmd = command_pool.acquire_command_buffer();

            // Pass[1]: images, buffers, passes, descriptor set layouts/pools
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::Buffer => {
                    }
                    raw::Resource::Image { kind, num_levels, format, usage, ref data } => {
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
                            let unbound_buffer = device.create_buffer(upload_size, buffer::Usage::TRANSFER_SRC)
                                .unwrap();
                            let upload_req = device.get_buffer_requirements(&unbound_buffer);
                            assert_ne!(upload_req.type_mask & (1 << upload_type.0), 0);
                            let upload_memory = device.allocate_memory(upload_type, upload_req.size)
                                .unwrap();
                            let upload_buffer = device.bind_buffer_memory(&upload_memory, 0, unbound_buffer)
                                .unwrap();
                            // write the data
                            {
                                let mut file = File::open(&format!("{}/{}", data_path, data))
                                    .unwrap();
                                let mut mapping = device.acquire_mapping_writer::<u8>(&upload_buffer, 0..upload_size)
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
                            let image_barrier = memory::Barrier::Image {
                                states: (i::Access::empty(), i::ImageLayout::Undefined) ..
                                        (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                                target: &image,
                                range: COLOR_RANGE.clone(), //TODO
                            };
                            init_cmd.pipeline_barrier(pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER, &[image_barrier]);

                            let buffer_width = (row_pitch as u32 * 8) / format_desc.bits as u32;
                            init_cmd.copy_buffer_to_image(
                                &upload_buffer,
                                &image,
                                i::ImageLayout::TransferDstOptimal,
                                &[hal::command::BufferImageCopy {
                                    buffer_offset: 0,
                                    buffer_width,
                                    buffer_height: h as u32,
                                    image_layers: i::SubresourceLayers {
                                        aspects: f::AspectFlags::COLOR,
                                        level: 0,
                                        layers: 0 .. 1,
                                    },
                                    image_offset: hal::command::Offset { x: 0, y: 0, z: 0 },
                                    image_extent: hal::device::Extent {
                                        width: w as _,
                                        height: h as _,
                                        depth: d as _,
                                    },
                                }]);
                            let image_barrier = memory::Barrier::Image {
                                states: (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal) .. final_state,
                                target: &image,
                                range: COLOR_RANGE.clone(), //TODO
                            };
                            init_cmd.pipeline_barrier(pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE, &[image_barrier]);
                            // done
                            upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                            final_state
                        };

                        resources.images.insert(name.clone(), Image {
                            handle: image,
                            memory,
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
                        let subpass_ref = |name: &String| {
                            if name.is_empty() {
                                hal::pass::SubpassRef::External
                            } else {
                                let id = subpasses.keys().position(|s| s == name).unwrap();
                                hal::pass::SubpassRef::Pass(id)
                            }
                        };

                        let raw_atts = attachments
                            .values()
                            .cloned()
                            .collect::<Vec<_>>();
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
                                    .map(|name| {
                                        attachments.keys().position(|s| s == name).unwrap()
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
                            })
                            .collect::<Vec<_>>();

                        let rp = RenderPass {
                            handle: device.create_render_pass(&raw_atts, &raw_subs, &raw_deps),
                            attachments: attachments.keys().cloned().collect(),
                            subpasses: subpasses.keys().cloned().collect(),
                        };
                        resources.render_passes.insert(name.clone(), rp);
                    }
                    raw::Resource::DescriptorSetLayout { ref bindings } => {
                        let layout = device.create_descriptor_set_layout(bindings);
                        resources.desc_set_layouts.insert(name.clone(), layout);
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
                    raw::Resource::DescriptorSet { ref pool, ref layout } => {
                        let set_layout = &resources.desc_set_layouts[layout];
                        let dest_pool: &mut B::DescriptorPool = resources.desc_pools
                            .get_mut(pool)
                            .unwrap();
                        let set = dest_pool
                            .allocate_sets(&[set_layout])
                            .pop()
                            .unwrap();
                        resources.desc_sets.insert(name.clone(), set);
                    }
                    raw::Resource::PipelineLayout { ref set_layouts, ref push_constant_ranges } => {
                        let layout = {
                            let layouts = set_layouts
                                .iter()
                                .map(|sl| &resources.desc_set_layouts[sl])
                                .collect::<Vec<_>>();
                            device.create_pipeline_layout(&layouts, &push_constant_ranges)
                        };
                        resources.pipeline_layouts.insert(name.clone(), layout);
                    }
                    _ => {}
                }
            }

            // Pass[3]: framebuffers
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::Framebuffer { ref pass, ref views, extent } => {
                        let rp = &resources.render_passes[pass];
                        let framebuffer = {
                            let image_views = rp.attachments
                                .iter()
                                .map(|name| {
                                    let entry = views
                                        .iter()
                                        .find(|entry| entry.0 == name)
                                        .unwrap();
                                    &resources.image_views[entry.1]
                                })
                                .collect::<Vec<_>>();
                            device.create_framebuffer(&rp.handle, &image_views, extent)
                                .unwrap()
                        };
                        resources.framebuffers.insert(name.clone(), (framebuffer, extent));
                    }
                    _ => {}
                }
            }

            Some(init_cmd.finish())
        };

        // fill up command buffers
        let mut jobs = HashMap::new();
        for (name, job) in &raw.jobs {
            let mut command_buf = command_pool.acquire_command_buffer();
            match *job {
                raw::Job::Transfer { ref commands } => {
                    use raw::TransferCommand as Tc;
                    for command in commands {
                        match *command {
                            //TODO
                            Tc::CopyBufferToImage => {}
                        }
                    }
                }
                raw::Job::Graphics { ref descriptors, ref framebuffer, ref pass, ref clear_values } => {
                    let _ = descriptors; //TODO
                    let (ref fb, extent) = resources.framebuffers[framebuffer];
                    let rp = &resources.render_passes[&pass.0];
                    let rect = hal::command::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as _,
                        h: extent.height as _,
                    };
                    let mut encoder = command_buf.begin_renderpass_inline(&rp.handle, fb, rect, clear_values);
                    for subpass in &rp.subpasses {
                        if Some(subpass) != rp.subpasses.first() {
                            encoder = encoder.next_subpass_inline();
                        }
                        for command in &pass.1[subpass].commands {
                            use raw::DrawCommand as Dc;
                            match *command {
                                Dc::BindIndexBuffer { ref buffer, offset, index_type } => {
                                    let view = buffer::IndexBufferView {
                                        buffer: &resources.buffers[buffer].0,
                                        offset,
                                        index_type,
                                    };
                                    encoder.bind_index_buffer(view);
                                }
                                Dc::BindVertexBuffers(ref buffers) => {
                                    let buffers_raw = buffers
                                        .iter()
                                        .map(|&(ref name, offset)| {
                                            (&resources.buffers[name].0, offset)
                                        })
                                        .collect::<Vec<_>>();
                                    let set = pso::VertexBufferSet(buffers_raw);
                                    encoder.bind_vertex_buffers(set);
                                }
                                Dc::BindPipeline(_) => {
                                    unimplemented!()
                                }
                                Dc::BindDescriptorSets { .. } => { //ref layout, first, ref sets
                                    unimplemented!()
                                }
                                Dc::Draw { ref vertices, ref instances } => {
                                    encoder.draw(vertices.clone(), instances.clone());
                                }
                                Dc::DrawIndexed { ref indices, base_vertex, ref instances } => {
                                    encoder.draw_indexed(indices.clone(), base_vertex, instances.clone());
                                }
                            }
                        }
                    }
                }
            }
            jobs.insert(name.clone(), command_buf.finish());
        }

        // done
        Scene {
            resources,
            jobs,
            init_submit,
            device,
            queue_group,
            command_pool,
            upload_buffers,
            download_type,
            limits,
        }
    }
}

impl<B: hal::Backend> Scene<B> {
    pub fn run<'a, I>(&mut self, jobs: I)
    where
        I: IntoIterator<Item = &'a str>
    {
        //TODO: re-use submits!
        let values = jobs.into_iter()
            .map(|name| self.jobs.remove(name).unwrap())
            .collect::<Vec<_>>();
        let submission = hal::queue::Submission::new()
            .submit(&[self.init_submit.take().unwrap()])
            .submit(&values);
        self.queue_group.queues[0].submit(submission, None);
    }

    pub fn fetch_image(&mut self, name: &str) -> FetchGuard<B> {
        let image = &self.resources.images[name];
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

        let unbound_buffer = self.device.create_buffer(down_size, buffer::Usage::TRANSFER_DST)
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
            let mut cmd_buffer = command_pool.acquire_command_buffer();
            let image_barrier = memory::Barrier::Image {
                states: image.stable_state .. (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal),
                target: &image.handle,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(pso::PipelineStage::TOP_OF_PIPE .. pso::PipelineStage::TRANSFER, &[image_barrier]);

            let buffer_width = (row_pitch as u32 * 8) / format_desc.bits as u32;
            cmd_buffer.copy_image_to_buffer(
                &image.handle,
                i::ImageLayout::TransferSrcOptimal,
                &down_buffer,
                &[hal::command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width,
                    buffer_height: height as u32,
                    image_layers: i::SubresourceLayers {
                        aspects: f::AspectFlags::COLOR,
                        level: 0,
                        layers: 0 .. 1,
                    },
                    image_offset: hal::command::Offset { x: 0, y: 0, z: 0 },
                    image_extent: hal::device::Extent {
                        width: width as _,
                        height: height as _,
                        depth: depth as _,
                    },
                }]);
            let image_barrier = memory::Barrier::Image {
                states: (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal) .. image.stable_state,
                target: &image.handle,
                range: COLOR_RANGE.clone(), //TODO
            };
            cmd_buffer.pipeline_barrier(pso::PipelineStage::TRANSFER .. pso::PipelineStage::BOTTOM_OF_PIPE, &[image_barrier]);
            cmd_buffer.finish()
        };

        let copy_fence = self.device.create_fence(false);
        let submission = hal::queue::Submission::new()
            .submit(&[copy_submit]);
        self.queue_group.queues[0].submit(submission, Some(&copy_fence));
        //queue.destroy_command_pool(command_pool);
        self.device.wait_for_fences(&[&copy_fence], hal::device::WaitFor::Any, !0);
        self.device.destroy_fence(copy_fence);

        let mapping = self.device.acquire_mapping_raw(&down_buffer, Some(0 .. down_size))
            .unwrap() as *const _;

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

impl<B: hal::Backend> Drop for Scene<B> {
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
