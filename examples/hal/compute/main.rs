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

use hal::{
    Backend, Compute, Device, DescriptorPool, Instance, PhysicalDevice, QueueFamily,
};
use hal::{queue, pso, memory, buffer, pool, command};

#[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
fn main() {
    env_logger::init();

    // For now this just panics if you didn't pass numbers. Could add proper error handling.
    if std::env::args().len() == 1 { panic!("You must pass a list of positive integers!") }
    let numbers: Vec<u32> = std::env::args()
        .skip(1)
        .map(|s| u32::from_str(&s).expect("You must pass a list of positive integers!"))
        .collect();
    let stride = std::mem::size_of::<u32>() as u64;

    #[cfg(any(feature = "vulkan", feature = "dx12", feature = "metal"))]
    let instance = back::Instance::create("gfx-rs compute", 1);

    let adapter = instance.enumerate_adapters().into_iter()
        .find(|a| a.queue_families
            .iter()
            .any(|family| family.supports_compute())
        )
        .expect("Failed to find a GPU with compute support!");

    let memory_properties = adapter.physical_device.memory_properties();
    let (mut device, mut queue_group) = adapter
        .open_with::<_, Compute>(1, |_family| true)
        .unwrap();

    let shader = device.create_shader_module(include_bytes!("shader/collatz.spv")).unwrap();

    let (pipeline_layout, pipeline, set_layout, mut desc_pool) = {
        let set_layout = device.create_descriptor_set_layout(&[
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::StorageBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::COMPUTE,
                }
            ],
        );

        let pipeline_layout = device.create_pipeline_layout(Some(&set_layout), &[]);
        let entry_point = pso::EntryPoint { entry: "main", module: &shader, specialization: &[] };
        let pipeline = device
            .create_compute_pipeline(&pso::ComputePipelineDesc::new(entry_point, &pipeline_layout))
            .expect("Error creating compute pipeline!");

        let desc_pool = device.create_descriptor_pool(
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

    let (staging_memory, staging_buffer) = create_buffer::<back::Backend>(
        &mut device,
        &memory_properties.memory_types,
        memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT,
        buffer::Usage::TRANSFER_SRC | buffer::Usage::TRANSFER_DST,
        stride,
        numbers.len() as u64,
    );

    {
        let mut writer = device.acquire_mapping_writer::<u32>(&staging_memory, 0..stride * numbers.len() as u64).unwrap();
        writer.copy_from_slice(&numbers);
        device.release_mapping_writer(writer);
    }

    let (device_memory, device_buffer) = create_buffer::<back::Backend>(
        &mut device,
        &memory_properties.memory_types,
        memory::Properties::DEVICE_LOCAL,
        buffer::Usage::TRANSFER_SRC | buffer::Usage::TRANSFER_DST | buffer::Usage::STORAGE,
        stride,
        numbers.len() as u64,
    );

    let desc_set = desc_pool.allocate_set(&set_layout);
    device.write_descriptor_sets(Some(
        pso::DescriptorSetWrite {
            set: &desc_set,
            binding: 0,
            array_offset: 0,
            descriptors: Some(
                pso::Descriptor::Buffer(&device_buffer, None .. None)
            ),
        }
    ));

    let mut command_pool = device.create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty(), 16);
    let fence = device.create_fence(false);
    let submission = queue::Submission::new().submit(Some({
        let mut command_buffer = command_pool.acquire_command_buffer(false);
        command_buffer.copy_buffer(&staging_buffer, &device_buffer, &[command::BufferCopy { src: 0, dst: 0, size: stride * numbers.len() as u64}]);
        command_buffer.pipeline_barrier(
            pso::PipelineStage::TRANSFER .. pso::PipelineStage::COMPUTE_SHADER,
            memory::Dependencies::empty(),
            Some(memory::Barrier::Buffer {
                states: buffer::Access::TRANSFER_WRITE .. buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE,
                target: &device_buffer
            }),
        );
        command_buffer.bind_compute_pipeline(&pipeline);
        command_buffer.bind_compute_descriptor_sets(&pipeline_layout, 0, &[desc_set]);
        command_buffer.dispatch([numbers.len() as u32, 1, 1]);
        command_buffer.pipeline_barrier(
            pso::PipelineStage::COMPUTE_SHADER .. pso::PipelineStage::TRANSFER,
            memory::Dependencies::empty(),
            Some(memory::Barrier::Buffer {
                states: buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE .. buffer::Access::TRANSFER_READ,
                target: &device_buffer
            }),
        );
        command_buffer.copy_buffer(&device_buffer, &staging_buffer, &[command::BufferCopy { src: 0, dst: 0, size: stride * numbers.len() as u64}]);
        command_buffer.finish()
    }));
    queue_group.queues[0].submit(submission, Some(&fence));
    device.wait_for_fence(&fence, !0);

    {
        let reader = device.acquire_mapping_reader::<u32>(&staging_memory, 0..stride * numbers.len() as u64).unwrap();
        println!("Times: {:?}", reader.into_iter().map(|n| *n).collect::<Vec<u32>>());
        device.release_mapping_reader(reader);
    }

    device.destroy_command_pool(command_pool.downgrade());
    device.destroy_descriptor_pool(desc_pool);
    device.destroy_descriptor_set_layout(set_layout);
    device.destroy_shader_module(shader);
    device.destroy_buffer(device_buffer);
    device.destroy_buffer(staging_buffer);
    device.destroy_fence(fence);
    device.destroy_pipeline_layout(pipeline_layout);
    device.free_memory(device_memory);
    device.free_memory(staging_memory);
    device.destroy_compute_pipeline(pipeline);
}

fn create_buffer<B: Backend>(
    device: &mut B::Device,
    memory_types: &[hal::MemoryType],
    properties: memory::Properties,
    usage: buffer::Usage,
    stride: u64,
    len: u64,
) -> (B::Memory, B::Buffer) {
    let buffer = device.create_buffer(stride * len, usage).unwrap();
    let requirements = device.get_buffer_requirements(&buffer);

    let ty = memory_types
        .into_iter()
        .enumerate()
        .position(|(id, memory_type)| {
            requirements.type_mask & (1 << id) != 0 &&
            memory_type.properties.contains(properties)
        })
        .unwrap()
        .into();

    let memory = device.allocate_memory(ty, requirements.size).unwrap();
    let buffer = device.bind_buffer_memory(&memory, 0, buffer).unwrap();

    (memory, buffer)
}

#[cfg(not(any(feature = "vulkan", feature = "dx12", feature = "metal")))]
fn main() {
    println!("You need to enable one of the next-gen API feature (vulkan, dx12, metal) to run this example.");
}
