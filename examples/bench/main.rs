#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "gl",
        feature = "dx11",
        feature = "dx12",
        feature = "metal",
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(not(any(
    feature = "vulkan",
    feature = "gl",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
)))]
extern crate gfx_backend_empty as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

use std::{iter, slice};

use hal::{command as com, image as i, prelude::*};

// AMD chokes on larger region counts...
const SIZE: u32 = 512;
// when 1, we use one-time-submit commands
const RUNS: usize = 2;
const FORMAT: hal::format::Format = hal::format::Format::Rgba8Unorm;

fn main() {
    env_logger::init();

    let instance =
        back::Instance::create("gfx-rs bench", 1).expect("Failed to create an instance!");

    let adapter = instance.enumerate_adapters().remove(0);
    println!("Running on {}", adapter.info.name);

    let memory_properties = adapter.physical_device.memory_properties();
    let limits = adapter.physical_device.limits();
    let family = adapter
        .queue_families
        .iter()
        .find(|family| family.queue_type().supports_compute())
        .unwrap();

    unsafe {
        let mut gpu = adapter
            .physical_device
            .open(&[(family, &[1.0])], hal::Features::empty())
            .unwrap();
        let device = &gpu.device;
        let queue_group = gpu.queue_groups.first_mut().unwrap();

        // source image

        let mut src_image = device
            .create_image(
                i::Kind::D2(1, 1, 1, 1),
                1,
                FORMAT,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_SRC | i::Usage::TRANSFER_DST,
                i::ViewCapabilities::empty(),
            )
            .unwrap();
        let src_image_requirements = device.get_image_requirements(&src_image);
        let src_image_type = memory_properties
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                src_image_requirements.type_mask & (1 << id) != 0
                    && memory_type
                        .properties
                        .contains(hal::memory::Properties::DEVICE_LOCAL)
            })
            .unwrap()
            .into();

        let src_memory_image = device
            .allocate_memory(src_image_type, src_image_requirements.size)
            .unwrap();
        device
            .bind_image_memory(&src_memory_image, 0, &mut src_image)
            .unwrap();

        // source buffer
        let bytes_per_texel = FORMAT.surface_desc().bits / 8;
        let buffer_size = (bytes_per_texel as u64).max(limits.non_coherent_atom_size as u64);

        let mut src_buffer = device
            .create_buffer(buffer_size, hal::buffer::Usage::TRANSFER_SRC)
            .unwrap();
        let src_buffer_requirements = device.get_buffer_requirements(&src_buffer);
        let src_buffer_type = memory_properties
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                src_buffer_requirements.type_mask & (1 << id) != 0
                    && memory_type
                        .properties
                        .contains(hal::memory::Properties::CPU_VISIBLE)
            })
            .unwrap()
            .into();

        let mut src_memory_buffer = device
            .allocate_memory(src_buffer_type, src_buffer_requirements.size)
            .unwrap();
        device
            .bind_buffer_memory(&src_memory_buffer, 0, &mut src_buffer)
            .unwrap();

        let ptr = device
            .map_memory(&mut src_memory_buffer, hal::memory::Segment::default())
            .unwrap();
        *(ptr as *mut u32) = 1;
        device
            .flush_mapped_memory_ranges(iter::once((
                &src_memory_buffer,
                hal::memory::Segment::default(),
            )))
            .unwrap();
        device.unmap_memory(&mut src_memory_buffer);

        // destination image

        let mut dst_image = device
            .create_image(
                i::Kind::D2(SIZE, SIZE, 1, 1),
                1,
                FORMAT,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_DST,
                i::ViewCapabilities::empty(),
            )
            .unwrap();
        let dst_requirements = device.get_image_requirements(&dst_image);
        let dst_type = memory_properties
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                dst_requirements.type_mask & (1 << id) != 0
                    && memory_type
                        .properties
                        .contains(hal::memory::Properties::DEVICE_LOCAL)
            })
            .unwrap()
            .into();

        let dst_memory = device
            .allocate_memory(dst_type, dst_requirements.size)
            .unwrap();
        device
            .bind_image_memory(&dst_memory, 0, &mut dst_image)
            .unwrap();

        // Initializing commands
        let subresource_layers = i::SubresourceLayers {
            aspects: hal::format::Aspects::COLOR,
            level: 0,
            layers: 0..1,
        };

        let mut command_pool = device
            .create_command_pool(family.id(), hal::pool::CommandPoolCreateFlags::empty())
            .expect("Can't create command pool");
        let mut fence = device.create_fence(false).unwrap();

        {
            let mut cmd_init = command_pool.allocate_one(com::Level::Primary);
            cmd_init.begin_primary(com::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_init.pipeline_barrier(
                hal::pso::PipelineStage::TOP_OF_PIPE..hal::pso::PipelineStage::TRANSFER,
                hal::memory::Dependencies::empty(),
                iter::once(hal::memory::Barrier::Image {
                    states: (i::Access::empty(), i::Layout::Undefined)
                        ..(i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                    families: None,
                    target: &src_image,
                    range: i::SubresourceRange {
                        aspects: hal::format::Aspects::COLOR,
                        ..i::SubresourceRange::default()
                    },
                })
                .chain(iter::once(hal::memory::Barrier::Buffer {
                    states: hal::buffer::Access::MEMORY_WRITE..hal::buffer::Access::TRANSFER_READ,
                    families: None,
                    target: &src_buffer,
                    range: hal::buffer::SubRange::default(),
                }))
                .chain(iter::once(hal::memory::Barrier::Image {
                    states: (i::Access::empty(), i::Layout::Undefined)
                        ..(i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                    families: None,
                    target: &dst_image,
                    range: i::SubresourceRange {
                        aspects: hal::format::Aspects::COLOR,
                        ..i::SubresourceRange::default()
                    },
                })),
            );
            cmd_init.copy_buffer_to_image(
                &src_buffer,
                &src_image,
                i::Layout::TransferDstOptimal,
                iter::once(com::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: 1,
                    buffer_height: 1,
                    image_layers: subresource_layers.clone(),
                    image_offset: i::Offset::ZERO,
                    image_extent: i::Extent {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                }),
            );
            cmd_init.pipeline_barrier(
                hal::pso::PipelineStage::TRANSFER..hal::pso::PipelineStage::TRANSFER,
                hal::memory::Dependencies::empty(),
                iter::once(hal::memory::Barrier::Image {
                    states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                        ..(i::Access::TRANSFER_READ, i::Layout::TransferSrcOptimal),
                    families: None,
                    target: &src_image,
                    range: i::SubresourceRange {
                        aspects: hal::format::Aspects::COLOR,
                        ..i::SubresourceRange::default()
                    },
                }),
            );
            cmd_init.finish();

            queue_group.queues[0].submit(
                iter::once(&cmd_init),
                iter::empty(),
                iter::empty(),
                Some(&mut fence),
            );
            device.wait_for_fence(&fence, !0).unwrap();
        }

        println!(
            "Pre-recording commands for {}x{} {:?}...",
            SIZE, SIZE, FORMAT
        );

        let num_queries = 3;
        let query_pool = device
            .create_query_pool(hal::query::Type::Timestamp, num_queries)
            .unwrap();
        let mut image_regions = Vec::new();
        let mut buffer_regions = Vec::new();
        for y in 0..SIZE {
            for x in 0..SIZE {
                image_regions.push(com::ImageCopy {
                    src_subresource: subresource_layers.clone(),
                    src_offset: i::Offset::ZERO,
                    dst_subresource: subresource_layers.clone(),
                    dst_offset: i::Offset {
                        x: x as i32,
                        y: y as i32,
                        z: 0,
                    },
                    extent: i::Extent {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                });
                buffer_regions.push(com::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: 1,
                    buffer_height: 1,
                    image_layers: subresource_layers.clone(),
                    image_offset: i::Offset {
                        x: x as i32,
                        y: y as i32,
                        z: 0,
                    },
                    image_extent: i::Extent {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                });
            }
        }

        let mut cmd_bench = command_pool.allocate_one(com::Level::Primary);
        cmd_bench.begin_primary(if RUNS == 1 {
            com::CommandBufferFlags::ONE_TIME_SUBMIT
        } else {
            com::CommandBufferFlags::empty()
        });
        cmd_bench.reset_query_pool(&query_pool, 0..num_queries);
        cmd_bench.write_timestamp(
            hal::pso::PipelineStage::TRANSFER,
            hal::query::Query {
                pool: &query_pool,
                id: 0,
            },
        );
        cmd_bench.copy_image(
            &src_image,
            i::Layout::TransferSrcOptimal,
            &dst_image,
            i::Layout::TransferDstOptimal,
            image_regions.into_iter(),
        );
        cmd_bench.write_timestamp(
            hal::pso::PipelineStage::TRANSFER,
            hal::query::Query {
                pool: &query_pool,
                id: 1,
            },
        );
        cmd_bench.copy_buffer_to_image(
            &src_buffer,
            &dst_image,
            i::Layout::TransferDstOptimal,
            buffer_regions.into_iter(),
        );
        cmd_bench.write_timestamp(
            hal::pso::PipelineStage::TRANSFER,
            hal::query::Query {
                pool: &query_pool,
                id: 2,
            },
        );
        cmd_bench.finish();

        println!("Benchmarking...");

        let period = queue_group.queues[0].timestamp_period() as f64 / 1_000_000.0;
        let mut timings = vec![0u8; num_queries as usize * 8];
        for i in 0..RUNS {
            device.reset_fence(&mut fence).unwrap();
            queue_group.queues[0].submit(
                iter::once(&cmd_bench),
                iter::empty(),
                iter::empty(),
                Some(&mut fence),
            );
            device.wait_for_fence(&fence, !0).unwrap();

            device
                .get_query_pool_results(
                    &query_pool,
                    0..num_queries,
                    &mut timings,
                    8,
                    hal::query::ResultFlags::BITS_64 | hal::query::ResultFlags::WAIT,
                )
                .unwrap();
            let ticks = slice::from_raw_parts(timings.as_ptr() as *const u64, num_queries as usize);
            let copy_image_time = ((ticks[1] - ticks[0]) as f64 * period) as u32;
            let copy_buffer_time = ((ticks[2] - ticks[1]) as f64 * period) as u32;
            println!(
                "\tRun[{}]: image->image({} ms), buffer->image({} ms)",
                i, copy_image_time, copy_buffer_time
            );
        }

        device.destroy_query_pool(query_pool);
        device.destroy_command_pool(command_pool);
        device.destroy_image(src_image);
        device.destroy_buffer(src_buffer);
        device.destroy_image(dst_image);
        device.destroy_fence(fence);
        device.free_memory(src_memory_image);
        device.free_memory(src_memory_buffer);
        device.free_memory(dst_memory);
    }
}
