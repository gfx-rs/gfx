use std::collections::HashMap;
use std::io::Read;
use std::fs::File;

use hal::{self, image as i};
use hal::{Adapter, Device};

use raw;


const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: i::ASPECT_COLOR,
    levels: 0 .. 1,
    layers: 0 .. 1,
};

pub struct Resources<B: hal::Backend> {
    pub buffers: HashMap<String, (B::Buffer, B::Memory)>,
    pub images: HashMap<String, (B::Image, B::Memory)>,
    pub image_views: HashMap<String, B::ImageView>,
    pub render_passes: HashMap<String, (B::RenderPass, Vec<String>)>,
    pub framebuffers: HashMap<String, B::Framebuffer>,
}

pub struct Scene<B: hal::Backend> {
    pub resources: Resources<B>,
    pub jobs: HashMap<String, B::CommandBuffer>,
    init_submission: hal::command::Submit<B, hal::queue::Graphics>,
    device: B::Device,
    queue: hal::CommandQueue<B, hal::queue::Graphics>,
    command_pool: hal::CommandPool<B, hal::queue::Graphics>,
    upload_buffers: HashMap<String, (B::Buffer, B::Memory)>,
}

impl<B: hal::Backend> Scene<B> {
    pub fn new(adapter: &B::Adapter, raw: &raw::Scene) -> Self {
        fn align(x: usize, y: usize) -> usize {
            if x > 0 && y > 0 {
                ((x - 1) | (y - 1)) + 1
            } else {
                x
            }
        }

        // initialize graphics
        let hal::Gpu { mut device, mut graphics_queues, memory_types, .. } = {
            let (ref family, queue_type) = adapter.get_queue_families()[0];
            assert!(queue_type.supports_graphics());
            adapter.open(&[(family, hal::QueueType::Graphics, 1)])
        };
        let upload_type = memory_types
            .iter()
            .find(|mt| {
                mt.properties.contains(hal::memory::CPU_VISIBLE)
                //&&!mt.properties.contains(hal::memory::CPU_CACHED)
            })
            .unwrap();
        let limits = device.get_limits().clone();
        let queue = graphics_queues.remove(0);
        let mut command_pool = queue.create_graphics_pool(
            1 + raw.jobs.len(),
            hal::pool::CommandPoolCreateFlags::empty(),
        );

        // create resources
        let mut resources = Resources {
            buffers: HashMap::new(),
            images: HashMap::new(),
            image_views: HashMap::new(),
            render_passes: HashMap::new(),
            framebuffers: HashMap::new(),
        };
        let mut upload_buffers = HashMap::new();
        let init_submission = {
            let mut init_cmd = command_pool.acquire_command_buffer();

            // Pass[1]: images, buffers, and passes
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
                            .find(|mt| {
                                requirements.type_mask & (1 << mt.id) != 0 &&
                                mt.properties.contains(hal::memory::DEVICE_LOCAL)
                            })
                            .unwrap();
                        let memory = device.allocate_memory(memory_type, requirements.size)
                            .unwrap();
                        let image = device.bind_image_memory(&memory, 0, unbound)
                            .unwrap();

                        // process initial data for the image
                        if !data.is_empty() {
                            // calculate required sizes
                            let (w, h, d, aa) = kind.get_dimensions();
                            assert_eq!(aa, i::AaMode::Single);
                            let bpp = format.0.describe_bits().total as usize;
                            let width_bytes = bpp * w as usize / 8;
                            let row_pitch = align(width_bytes, limits.min_buffer_copy_pitch_alignment);
                            let upload_size = row_pitch as u64 * h as u64 * d as u64;
                            // create upload buffer
                            let unbound_buffer = device.create_buffer(upload_size, bpp as _, hal::buffer::TRANSFER_SRC)
                                .unwrap();
                            let upload_req = device.get_buffer_requirements(&unbound_buffer);
                            let upload_memory = device.allocate_memory(upload_type, upload_req.size)
                                .unwrap();
                            let upload_buffer = device.bind_buffer_memory(&upload_memory, 0, unbound_buffer)
                                .unwrap();
                            // write the data
                            {
                                let mut file = File::open(&format!("../../reftests/data/{}", data))
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
                            let image_barrier = hal::memory::Barrier::Image {
                                states: (i::Access::empty(), i::ImageLayout::Undefined) ..
                                        (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal),
                                target: &image,
                                range: COLOR_RANGE.clone(),
                            };
                            init_cmd.pipeline_barrier(hal::pso::TOP_OF_PIPE .. hal::pso::TRANSFER, &[image_barrier]);
                            init_cmd.copy_buffer_to_image(
                                &upload_buffer,
                                &image,
                                i::ImageLayout::TransferDstOptimal,
                                &[hal::command::BufferImageCopy {
                                    buffer_offset: 0,
                                    buffer_row_pitch: row_pitch as u32,
                                    buffer_slice_pitch: row_pitch as u32 * h as u32,
                                    image_layers: i::SubresourceLayers {
                                        aspects: i::ASPECT_COLOR,
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
                            let image_barrier = hal::memory::Barrier::Image {
                                states: (i::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal) ..
                                        (i::SHADER_READ, i::ImageLayout::ShaderReadOnlyOptimal),
                                target: &image,
                                range: COLOR_RANGE.clone(),
                            };
                            init_cmd.pipeline_barrier(hal::pso::TRANSFER .. hal::pso::BOTTOM_OF_PIPE, &[image_barrier]);
                            // done
                            upload_buffers.insert(name.clone(), (upload_buffer, upload_memory));
                        }

                        resources.images.insert(name.clone(), (image, memory));
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

                        let rp = device.create_render_pass(&raw_atts, &raw_subs, &raw_deps);
                        let att_names = attachments.keys().cloned().collect();
                        resources.render_passes.insert(name.clone(), (rp, att_names));
                    }
                    _ => {}
                }
            }

            // Pass[2]: image & buffer views
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::ImageView { ref image, format, swizzle, ref range } => {
                        let image = &resources.images[image].0;
                        let view = device.create_image_view(image, format, swizzle, range.clone())
                            .unwrap();
                        resources.image_views.insert(name.clone(), view);
                    }
                    _ => {}
                }
            }

            // Pass[3]: framebuffers
            for (name, resource) in &raw.resources {
                match *resource {
                    raw::Resource::Framebuffer { ref pass, ref views, extent } => {
                        let (ref render_pass, ref att_names) = resources.render_passes[pass];
                        let framebuffer = {
                            let image_views = att_names
                                .iter()
                                .map(|name| {
                                    let entry = views
                                        .iter()
                                        .find(|entry| entry.0 == name)
                                        .unwrap();
                                    &resources.image_views[entry.1]
                                })
                                .collect::<Vec<_>>();
                            device.create_framebuffer(render_pass, &image_views, extent)
                                .unwrap()
                        };
                        resources.framebuffers.insert(name.clone(), framebuffer);
                    }
                    _ => {}
                }
            }

            init_cmd.finish()
        };

        // fill up command buffers
        let mut jobs = HashMap::new();
        Scene {
            resources,
            jobs,
            init_submission,
            device,
            queue,
            command_pool,
            upload_buffers,
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
        let _ = &self.queue;
        let _ = &self.command_pool;
        //queue.destroy_command_pool(command_pool);
    }
}
