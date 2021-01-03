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
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

use std::{fs, ptr, slice, str::FromStr};

use hal::{adapter::MemoryType, buffer, command, memory, pool, prelude::*, pso};

#[cfg(any(
    feature = "vulkan",
    feature = "gl",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
))]
fn main() {
    env_logger::init();

    // For now this just panics if you didn't pass numbers. Could add proper error handling.
    if std::env::args().len() == 1 {
        panic!("You must pass a list of positive integers!")
    }
    let numbers: Vec<u32> = std::env::args()
        .skip(1)
        .map(|s| u32::from_str(&s).expect("You must pass a list of positive integers!"))
        .collect();
    let stride = std::mem::size_of::<u32>() as buffer::Stride;

    let instance =
        back::Instance::create("gfx-rs compute", 1).expect("Failed to create an instance!");

    let adapter = instance
        .enumerate_adapters()
        .into_iter()
        .find(|a| {
            a.queue_families
                .iter()
                .any(|family| family.queue_type().supports_compute())
        })
        .expect("Failed to find a GPU with compute support!");

    let memory_properties = adapter.physical_device.memory_properties();
    let family = adapter
        .queue_families
        .iter()
        .find(|family| family.queue_type().supports_compute())
        .unwrap();
    let mut gpu = unsafe {
        adapter
            .physical_device
            .open(&[(family, &[1.0])], hal::Features::empty())
            .unwrap()
    };
    let device = &gpu.device;
    let queue_group = gpu.queue_groups.first_mut().unwrap();

    let glsl = fs::read_to_string("compute/shader/collatz.comp").unwrap();
    let file = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Compute).unwrap();
    let spirv: Vec<u32> = auxil::read_spirv(file).unwrap();
    let shader = unsafe { device.create_shader_module(&spirv) }.unwrap();

    let (pipeline_layout, pipeline, set_layout, mut desc_pool) = {
        let set_layout = unsafe {
            device.create_descriptor_set_layout(
                &[pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::Buffer {
                        ty: pso::BufferDescriptorType::Storage { read_only: false },
                        format: pso::BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                    },
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::COMPUTE,
                    immutable_samplers: false,
                }],
                &[],
            )
        }
        .expect("Can't create descriptor set layout");

        let pipeline_layout = unsafe { device.create_pipeline_layout(Some(&set_layout), &[]) }
            .expect("Can't create pipeline layout");
        let entry_point = pso::EntryPoint {
            entry: "main",
            module: &shader,
            specialization: pso::Specialization::default(),
        };
        let pipeline = unsafe {
            device.create_compute_pipeline(
                &pso::ComputePipelineDesc::new(entry_point, &pipeline_layout),
                None,
            )
        }
        .expect("Error creating compute pipeline!");

        let desc_pool = unsafe {
            device.create_descriptor_pool(
                1,
                &[pso::DescriptorRangeDesc {
                    ty: pso::DescriptorType::Buffer {
                        ty: pso::BufferDescriptorType::Storage { read_only: false },
                        format: pso::BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                    },
                    count: 1,
                }],
                pso::DescriptorPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create descriptor pool");
        (pipeline_layout, pipeline, set_layout, desc_pool)
    };

    let (mut staging_memory, staging_buffer, _staging_size) = unsafe {
        create_buffer::<back::Backend>(
            &device,
            &memory_properties.memory_types,
            memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT,
            buffer::Usage::TRANSFER_SRC | buffer::Usage::TRANSFER_DST,
            stride,
            numbers.len() as u64,
        )
    };

    unsafe {
        let mapping = device
            .map_memory(&mut staging_memory, memory::Segment::ALL)
            .unwrap();
        ptr::copy_nonoverlapping(
            numbers.as_ptr() as *const u8,
            mapping,
            numbers.len() * stride as usize,
        );
        device.unmap_memory(&mut staging_memory);
    }

    let (device_memory, device_buffer, _device_buffer_size) = unsafe {
        create_buffer::<back::Backend>(
            &device,
            &memory_properties.memory_types,
            memory::Properties::DEVICE_LOCAL,
            buffer::Usage::TRANSFER_SRC | buffer::Usage::TRANSFER_DST | buffer::Usage::STORAGE,
            stride,
            numbers.len() as u64,
        )
    };

    let desc_set = unsafe {
        let mut desc_set = desc_pool.allocate_set(&set_layout).unwrap();
        device.write_descriptor_set(pso::DescriptorSetWrite {
            set: &mut desc_set,
            binding: 0,
            array_offset: 0,
            descriptors: Some(pso::Descriptor::Buffer(
                &device_buffer,
                buffer::SubRange::WHOLE,
            )),
        });
        desc_set
    };

    let mut command_pool =
        unsafe { device.create_command_pool(family.id(), pool::CommandPoolCreateFlags::empty()) }
            .expect("Can't create command pool");
    let mut fence = device.create_fence(false).unwrap();
    unsafe {
        let mut command_buffer = command_pool.allocate_one(command::Level::Primary);
        command_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);
        command_buffer.copy_buffer(
            &staging_buffer,
            &device_buffer,
            &[command::BufferCopy {
                src: 0,
                dst: 0,
                size: stride as u64 * numbers.len() as u64,
            }],
        );
        command_buffer.pipeline_barrier(
            pso::PipelineStage::TRANSFER..pso::PipelineStage::COMPUTE_SHADER,
            memory::Dependencies::empty(),
            Some(memory::Barrier::Buffer {
                states: buffer::Access::TRANSFER_WRITE
                    ..buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE,
                families: None,
                target: &device_buffer,
                range: buffer::SubRange::WHOLE,
            }),
        );
        command_buffer.bind_compute_pipeline(&pipeline);
        command_buffer.bind_compute_descriptor_sets(&pipeline_layout, 0, &[desc_set], &[]);
        command_buffer.dispatch([numbers.len() as u32, 1, 1]);
        command_buffer.pipeline_barrier(
            pso::PipelineStage::COMPUTE_SHADER..pso::PipelineStage::TRANSFER,
            memory::Dependencies::empty(),
            Some(memory::Barrier::Buffer {
                states: buffer::Access::SHADER_READ | buffer::Access::SHADER_WRITE
                    ..buffer::Access::TRANSFER_READ,
                families: None,
                target: &device_buffer,
                range: buffer::SubRange::WHOLE,
            }),
        );
        command_buffer.copy_buffer(
            &device_buffer,
            &staging_buffer,
            &[command::BufferCopy {
                src: 0,
                dst: 0,
                size: stride as u64 * numbers.len() as u64,
            }],
        );
        command_buffer.finish();

        queue_group.queues[0].submit_without_semaphores(Some(&command_buffer), Some(&mut fence));

        device.wait_for_fence(&fence, !0).unwrap();
        command_pool.free(Some(command_buffer));
    }

    unsafe {
        let mapping = device
            .map_memory(&mut staging_memory, memory::Segment::ALL)
            .unwrap();
        println!(
            "Times: {:?}",
            slice::from_raw_parts::<u32>(mapping as *const u8 as *const u32, numbers.len()),
        );
        device.unmap_memory(&mut staging_memory);
    }

    unsafe {
        device.destroy_command_pool(command_pool);
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
}

unsafe fn create_buffer<B: hal::Backend>(
    device: &B::Device,
    memory_types: &[MemoryType],
    properties: memory::Properties,
    usage: buffer::Usage,
    stride: buffer::Stride,
    len: u64,
) -> (B::Memory, B::Buffer, u64) {
    let mut buffer = device.create_buffer(stride as u64 * len, usage).unwrap();
    let requirements = device.get_buffer_requirements(&buffer);

    let ty = memory_types
        .into_iter()
        .enumerate()
        .position(|(id, memory_type)| {
            requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(properties)
        })
        .unwrap()
        .into();

    let memory = device.allocate_memory(ty, requirements.size).unwrap();
    device.bind_buffer_memory(&memory, 0, &mut buffer).unwrap();

    (memory, buffer, requirements.size)
}

#[cfg(not(any(
    feature = "vulkan",
    feature = "gl",
    feature = "dx11",
    feature = "dx12",
    feature = "metal"
)))]
fn main() {
    println!("You need to enable one of the next-gen API feature (vulkan, dx12, metal) to run this example.");
}
