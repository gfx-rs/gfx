#![cfg_attr(
    not(any(feature = "vulkan", feature = "dx12", feature = "metal")),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

extern crate env_logger;
extern crate gfx_hal as hal;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;

use std::str::FromStr;
use std::ops::Range;

use hal::{Backend, Compute, Gpu, Device, DescriptorPool, Instance, QueueFamily, QueueGroup};
use hal::{queue, pso, memory, buffer, pool, command, device};

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
fn main() {
    env_logger::init().unwrap();

    // For now this just panics if you didn't pass numbers. Could add proper error handling.
    if std::env::args().len() == 1 { panic!("You must pass a list of positive integers!") }
    let numbers: Vec<u32> = std::env::args()
        .skip(1)
        .map(|s| u32::from_str(&s).expect("You must pass a list of positive integers!"))
        .collect();
    let stride = std::mem::size_of::<u32>() as u64;

    #[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
    let instance = back::Instance::create("gfx-rs compute", 1);

    let mut gpu = instance.enumerate_adapters().into_iter()
        .find(|a| a.queue_families
            .iter()
            .any(|family| family.supports_compute())
        )
        .expect("Failed to find a GPU with compute support!")
        .open_with(|family| {
            if family.supports_compute() {
                Some(1)
            } else {
                None
            }
        });

    let shader = gpu.device.create_shader_module(include_bytes!("shader/collatz.spv")).unwrap();

    let (pipeline_layout, pipeline, set_layout, mut desc_pool) = {
        let set_layout = gpu.device.create_descriptor_set_layout(&[
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::StorageBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::COMPUTE,
                }
            ],
        );

        let pipeline_layout = gpu.device.create_pipeline_layout(&[&set_layout]);
        let entry_point = pso::EntryPoint { entry: "main", module: &shader };
        let pipeline = gpu.device
            .create_compute_pipelines(&[
                (entry_point, pso::ComputePipelineDesc::new(&pipeline_layout))
            ])
            .remove(0)
            .expect("Error creating compute pipeline!");

        let desc_pool = gpu.device.create_descriptor_pool(
            1,
            &[
                pso::DescriptorRangeDesc {
                    ty: pso::DescriptorType::StorageBuffer,
                    count: 1,
                },
            ],
        );
        (pipeline_layout, pipeline, set_layout, desc_pool)
    };

    let (staging_memory, staging_buffer) = create_buffer(
        &mut gpu,
        memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT,
        buffer::Usage::TRANSFER_SRC,
        stride,
        numbers.len() as u64,
    );

    {
        let mut writer = gpu.device.acquire_mapping_writer::<u32>(&staging_buffer, 0..stride * numbers.len() as u64).unwrap();
        writer.copy_from_slice(&numbers);
        gpu.device.release_mapping_writer(writer);
    }

    let (device_memory, device_buffer) = create_buffer(
        &mut gpu,
        memory::Properties::DEVICE_LOCAL,
        buffer::Usage::TRANSFER_DST,
        stride,
        numbers.len() as u64,
    );

    let desc_set = desc_pool.allocate_sets(&[&set_layout]).remove(0);
    gpu.device.update_descriptor_sets(&[
        pso::DescriptorSetWrite {
            set: &desc_set,
            binding: 0,
            array_offset: 0,
            write: pso::DescriptorWrite::StorageBuffer(vec![(&device_buffer, 0..stride * numbers.len() as u64)])
        }
    ]);

    let mut queue_group = QueueGroup::<_, Compute>::new(gpu.queue_groups.remove(0));
    let mut command_pool = gpu.device.create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty(), 16);
    let fence = gpu.device.create_fence(false);
    let submission = queue::Submission::new().submit(&[{
        let mut command_buffer = command_pool.acquire_command_buffer();
        command_buffer.copy_buffer(&staging_buffer, &device_buffer, &[command::BufferCopy { src: 0, dst: 0, size: stride * numbers.len() as u64}]);
        command_buffer.pipeline_barrier(
            Range { start: pso::PipelineStage::TRANSFER, end: pso::PipelineStage::COMPUTE_SHADER },
                &[memory::Barrier::Buffer {
                    states: Range {
                        start: buffer::Access::TRANSFER_WRITE,
                        end: buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE
                    },
                    target: &device_buffer
                }]);
        command_buffer.bind_compute_pipeline(&pipeline);
        command_buffer.bind_compute_descriptor_sets(&pipeline_layout, 0, &[&desc_set]);
        command_buffer.dispatch(numbers.len() as u32, 1, 1);
        command_buffer.pipeline_barrier(
            Range { start: pso::PipelineStage::COMPUTE_SHADER, end: pso::PipelineStage::TRANSFER },
                &[memory::Barrier::Buffer {
                    states: Range {
                        start: buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE,
                        end: buffer::Access::TRANSFER_READ
                    },
                    target: &device_buffer
                }]);
        command_buffer.copy_buffer(&device_buffer, &staging_buffer, &[command::BufferCopy { src: 0, dst: 0, size: stride * numbers.len() as u64}]);
        command_buffer.finish()
    }]);
    queue_group.queues[0].submit(submission, Some(&fence));
    gpu.device.wait_for_fences(&[&fence], device::WaitFor::All, !0);

    {
        let reader = gpu.device.acquire_mapping_reader::<u32>(&staging_buffer, 0..stride * numbers.len() as u64).unwrap();
        println!("Times: {:?}", reader.into_iter().map(|n| *n).collect::<Vec<u32>>());
        gpu.device.release_mapping_reader(reader);
    }

    gpu.device.destroy_descriptor_pool(desc_pool);
    gpu.device.destroy_descriptor_set_layout(set_layout);
    gpu.device.destroy_shader_module(shader);
    gpu.device.destroy_buffer(device_buffer);
    gpu.device.destroy_buffer(staging_buffer);
    gpu.device.destroy_fence(fence);
    gpu.device.destroy_pipeline_layout(pipeline_layout);
    gpu.device.free_memory(device_memory);
    gpu.device.free_memory(staging_memory);
    gpu.device.destroy_compute_pipeline(pipeline);
}

fn create_buffer<B: Backend>(gpu: &mut Gpu<B>, properties: memory::Properties, usage: buffer::Usage, stride: u64, len: u64) -> (B::Memory, B::Buffer) {
    let buffer = gpu.device.create_buffer(stride * len, stride, usage).unwrap();
    let requirements = gpu.device.get_buffer_requirements(&buffer);

    let ty = (&gpu.memory_types).into_iter().find(|memory_type| {
        requirements.type_mask & (1 << memory_type.id) != 0 &&
        memory_type.properties.contains(properties)
    }).unwrap();

    let memory = gpu.device.allocate_memory(&ty, requirements.size).unwrap();
    let buffer = gpu.device.bind_buffer_memory(&memory, 0, buffer).unwrap();

    (memory, buffer)
}

#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal")))]
fn main() {
    println!("You need to enable one of the next-gen API feature (vulkan, dx12, metal) to run this example.");
}
