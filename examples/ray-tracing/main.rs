#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(not(any(
    feature = "vulkan",
    feature = "d offset: (), size: ()x11",
    feature = "dx12",
    feature = "metal",
    feature = "gl",
)))]
extern crate gfx_backend_empty as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

use cgmath::SquareMatrix;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main();
}

use hal::{
    acceleration_structure as accel, adapter, buffer, command, format, image, memory, pool,
    prelude::*, pso, window, IndexType, PhysicalDeviceProperties,
};

use std::{
    borrow::Borrow,
    io::Cursor,
    iter,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ops::{self, Deref},
    ptr,
};

#[cfg_attr(rustfmt, rustfmt_skip)]
const DIMS: window::Extent2D = window::Extent2D { width: 1024, height: 768 };

#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
struct Vertex {
    a_Pos: [f32; 3],
}

#[derive(Debug, Clone)]
struct CameraProperties {
    view_inverse: [[f32; 4]; 4],
    proj_inverse: [[f32; 4]; 4],
}

impl Default for CameraProperties {
    fn default() -> Self {
        use cgmath::{Matrix, Transform};

        CameraProperties {
            view_inverse: cgmath::conv::array4x4(
                cgmath::Matrix4::from_translation(cgmath::Vector3::unit_z() * -2.5)
                    .inverse_transform()
                    .unwrap(),
            ),
            proj_inverse: cgmath::conv::array4x4(
                cgmath::perspective(cgmath::Deg(60.0), 1024.0 / 768.0, 0.1, 512.0)
                    .inverse_transform()
                    .unwrap(),
            ),
        }
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Debug).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    #[cfg(not(any(
        feature = "vulkan",
        feature = "dx11",
        feature = "dx12",
        feature = "metal",
        feature = "gl",
    )))]
    eprintln!(
        "You are running the example with the empty backend, no graphical output is to be expected"
    );

    let event_loop = winit::event_loop::EventLoop::new();

    let wb = winit::window::WindowBuilder::new()
        .with_min_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
            64.0, 64.0,
        )))
        .with_inner_size(winit::dpi::Size::Physical(winit::dpi::PhysicalSize::new(
            DIMS.width,
            DIMS.height,
        )))
        .with_title("ray-tracing".to_string());

    // instantiate backend
    let window = wb.build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap()
        .append_child(&winit::platform::web::WindowExtWebSys::canvas(&window))
        .unwrap();

    let instance =
        back::Instance::create("gfx-rs ray-tracing", 1).expect("Failed to create an instance!");

    let surface = unsafe {
        instance
            .create_surface(&window)
            .expect("Failed to create a surface!")
    };

    let adapter = {
        let mut adapters = instance.enumerate_adapters();
        for adapter in &adapters {
            println!("{:?}", adapter.info);
        }
        adapters.remove(0)
    };

    let mut renderer = Renderer::new(instance, surface, adapter);

    renderer.render();

    // It is important that the closure move captures the Renderer,
    // otherwise it will not be dropped when the event loop exits.
    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit
                }
                winit::event::WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *control_flow = winit::event_loop::ControlFlow::Exit,
                winit::event::WindowEvent::Resized(dims) => {
                    println!("resized to {:?}", dims);
                    // renderer.dimensions = window::Extent2D {
                    //     width: dims.width,
                    //     height: dims.height,
                    // };
                    renderer.recreate_swapchain();
                }
                _ => {}
            },
            winit::event::Event::RedrawEventsCleared => {
                renderer.render();
            }
            _ => {}
        }
    });
}

struct Renderer<B: hal::Backend> {
    properties: PhysicalDeviceProperties,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    surface: ManuallyDrop<B::Surface>,
    format: hal::format::Format,
    dimensions: window::Extent2D,
    viewport: pso::Viewport,

    bottom_level_accel_struct: ManuallyDrop<AccelerationStructure<B>>,
    top_level_accel_struct: ManuallyDrop<AccelerationStructure<B>>,
    storage_image: ManuallyDrop<B::Image>,
    storage_image_view: ManuallyDrop<B::ImageView>,
    uniform_buffer: ManuallyDrop<B::Buffer>,
    uniform_buffer_memory: ManuallyDrop<B::Memory>,

    pipeline: ManuallyDrop<B::RayTracingPipeline>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    raygen_shader_binding_table: ManuallyDrop<B::Buffer>,
    raygen_shader_binding_table_memory: ManuallyDrop<B::Memory>,
    miss_shader_binding_table: ManuallyDrop<B::Buffer>,
    miss_shader_binding_table_memory: ManuallyDrop<B::Memory>,
    closest_hit_shader_binding_table: ManuallyDrop<B::Buffer>,
    closest_hit_shader_binding_table_memory: ManuallyDrop<B::Memory>,

    submission_complete_semaphores: Vec<B::Semaphore>,
    submission_complete_fences: Vec<B::Fence>,
    cmd_pools: Vec<B::CommandPool>,
    cmd_buffers: Vec<B::CommandBuffer>,
    desc_set: B::DescriptorSet,
    frames_in_flight: usize,
    frame: u64,
    // These members are dropped in the declaration order.
    device: B::Device,
    adapter: hal::adapter::Adapter<B>,
    queue_group: hal::queue::QueueGroup<B>,
    instance: B::Instance,
}

impl<B> Renderer<B>
where
    B: hal::Backend,
{
    fn new(
        instance: B::Instance,
        mut surface: B::Surface,
        adapter: hal::adapter::Adapter<B>,
    ) -> Renderer<B> {
        // Create device
        let required_features =
            hal::Features::ACCELERATION_STRUCTURE | hal::Features::RAY_TRACING_PIPELINE;

        // TODO search through all adapters in case the non-first one supports our required features?
        assert!(adapter
            .physical_device
            .features()
            .contains(required_features));

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let properties = adapter.physical_device.properties();

        // Build a new device and associated command queues
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                surface.supports_queue_family(family) && family.queue_type().supports_graphics()
            })
            .expect("No queue family supports presentation");
        let mut gpu = unsafe {
            adapter
                .physical_device
                .open(&[(family, &[1.0])], required_features)
                .unwrap()
        };
        let mut queue_group = gpu.queue_groups.pop().unwrap();
        let device = gpu.device;

        let caps = surface.capabilities(&adapter.physical_device);
        let format = {
            let formats = surface.supported_formats(&adapter.physical_device);
            formats.map_or(format::Format::Rgba8Srgb, |formats| {
                formats
                    .iter()
                    .find(|format| format.base_format().1 == format::ChannelType::Srgb)
                    .map(|format| *format)
                    .unwrap_or(formats[0])
            })
        };

        let swap_config = {
            let mut swap_config = window::SwapchainConfig::from_caps(&caps, format, DIMS);
            swap_config.image_usage |= image::Usage::TRANSFER_DST;
            swap_config
        };
        println!("{:?}", swap_config);
        let extent = swap_config.extent;
        // Define maximum number of frames we want to be able to be "in flight" (being computed simultaneously) at once
        let frames_in_flight = swap_config.image_count as usize;
        unsafe {
            surface
                .configure_swapchain(&device, swap_config)
                .expect("Can't configure swapchain");
        };

        let mut command_pool = unsafe {
            device.create_command_pool(queue_group.family, pool::CommandPoolCreateFlags::empty())
        }
        .expect("Can't create command pool");

        unsafe {
            // Create storage image
            let mut storage_image = device
                .create_image(
                    image::Kind::D2(extent.width, extent.height, 1, 1),
                    1,
                    format::Format::Bgra8Unorm,
                    image::Tiling::Optimal,
                    image::Usage::TRANSFER_SRC | image::Usage::STORAGE,
                    memory::SparseFlags::empty(),
                    image::ViewCapabilities::empty(),
                )
                .unwrap();

            let memory_requirements = device.get_image_requirements(&storage_image);
            let memory_type = memory_types
                .iter()
                .enumerate()
                .position(|(id, memory_type)| {
                    memory_requirements.type_mask & (1 << id) != 0
                        && memory_type
                            .properties
                            .contains(hal::memory::Properties::DEVICE_LOCAL)
                })
                .unwrap()
                .into();
            let storage_image_memory = device
                .allocate_memory(memory_type, memory_requirements.size)
                .unwrap();
            device
                .bind_image_memory(&storage_image_memory, 0, &mut storage_image)
                .unwrap();

            let storage_image_view = device
                .create_image_view(
                    &storage_image,
                    image::ViewKind::D2,
                    format::Format::Bgra8Unorm,
                    format::Swizzle::NO,
                    image::Usage::STORAGE,
                    image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
                .unwrap();

            let mut build_fence = device.create_fence(false).unwrap();
            let mut cmd_buffer = command_pool.allocate_one(command::Level::Primary);
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER..pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                iter::once(memory::Barrier::Image {
                    states: (image::Access::empty(), image::Layout::Undefined)
                        ..(image::Access::empty(), image::Layout::General),
                    target: &storage_image,
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                    families: None,
                }),
            );
            cmd_buffer.finish();
            queue_group.queues[0].submit(
                iter::once(&cmd_buffer),
                iter::empty(),
                iter::empty(),
                Some(&mut build_fence),
            );

            // Create uniform buffer
            let uniform_data: CameraProperties = Default::default();
            let (uniform_buffer, uniform_buffer_memory) = upload_to_buffer::<B, _>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::UNIFORM,
                &[uniform_data],
            );

            // Create blas
            let triangle_vertices = &[
                Vertex {
                    a_Pos: [1.0, 1.0, 0.0],
                },
                Vertex {
                    a_Pos: [-1.0, 1.0, 0.0],
                },
                Vertex {
                    a_Pos: [0.0, -1.0, 0.0],
                },
            ];

            let triangle_indices: &[u16] = &[0, 1, 2];
            let triangle_indices: &[u16] = &[0, 1, 2, 0, 2, 1]; // todo

            let vertex_buffer = upload_to_buffer::<B, _>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                triangle_vertices,
            );

            let index_buffer = upload_to_buffer::<B, _>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                triangle_indices,
            );

            let geometry_desc = accel::GeometryDesc {
                flags: accel::Flags::ALLOW_COMPACTION,
                ty: accel::Type::BottomLevel,
                geometries: &[&accel::Geometry {
                    flags: accel::GeometryFlags::OPAQUE,
                    geometry: accel::GeometryData::Triangles(accel::GeometryTriangles {
                        vertex_format: format::Format::Rgb32Sfloat,
                        vertex_buffer: &vertex_buffer.0,
                        vertex_buffer_offset: 0,
                        vertex_buffer_stride: std::mem::size_of::<Vertex>() as u32,
                        max_vertex: triangle_vertices.len() as u64,
                        index_buffer: Some((&index_buffer.0, 0, IndexType::U16)),
                        transform: None,
                    }),
                }],
            };

            let triangle_primitive_count = (triangle_indices.len() / 3) as u32;
            let triangle_blas_requirements = device.get_acceleration_structure_build_requirements(
                &geometry_desc,
                &[triangle_primitive_count],
            );

            let scratch_buffer = create_empty_buffer::<B>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_STORAGE
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                triangle_blas_requirements.build_scratch_size,
            );

            let accel_struct_bottom_buffer = create_empty_buffer::<B>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_STORAGE
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                triangle_blas_requirements.acceleration_structure_size,
            );

            let mut triangle_blas = AccelerationStructure::<B> {
                accel_struct: device
                    .create_acceleration_structure(&accel::CreateDesc {
                        buffer: &accel_struct_bottom_buffer.0,
                        buffer_offset: 0,
                        size: triangle_blas_requirements.acceleration_structure_size,
                        ty: accel::Type::BottomLevel,
                    })
                    .unwrap(),
                backing: accel_struct_bottom_buffer,
            };

            device.set_acceleration_structure_name(&mut triangle_blas.accel_struct, "triangle");

            let mut build_fence = device.create_fence(false).unwrap();
            let mut cmd_buffer = command_pool.allocate_one(command::Level::Primary);
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.build_acceleration_structure(
                &accel::BuildDesc {
                    src: None,
                    dst: &triangle_blas.accel_struct,
                    geometry: &geometry_desc,
                    scratch: &scratch_buffer.0,
                    scratch_offset: 0,
                },
                &[accel::BuildRangeDesc {
                    primitive_count: triangle_primitive_count,
                    primitive_offset: 0,
                    first_vertex: 0,
                    transform_offset: 0,
                }][..],
            );
            // cmd_buffer.pipeline_barrier(
            //     pso::PipelineStage::ACCELERATION_STRUCTURE_BUILD
            //         ..pso::PipelineStage::ACCELERATION_STRUCTURE_BUILD,
            //     memory::Dependencies::empty(),
            //     iter::once(memory::Barrier::AllBuffers(
            //         buffer::Access::ACCELERATION_STRUCTURE_WRITE
            //             ..buffer::Access::ACCELERATION_STRUCTURE_READ,
            //     )),
            // );
            cmd_buffer.finish();
            queue_group.queues[0].submit(
                iter::once(&cmd_buffer),
                iter::empty(),
                iter::empty(),
                Some(&mut build_fence),
            );
            device
                .wait_for_fence(&build_fence, !0)
                .expect("Can't wait for fence");
            device.free_memory(scratch_buffer.1);
            device.destroy_buffer(scratch_buffer.0);

            // Create tlas
            let instances = [accel::Instance::new(
                device.get_acceleration_structure_address(&triangle_blas.accel_struct),
            )];

            let instances_buffer = upload_to_buffer::<B, _>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                &instances,
            );

            let top_level_geometry_desc = accel::GeometryDesc {
                flags: accel::Flags::ALLOW_COMPACTION,
                ty: accel::Type::TopLevel,
                geometries: &[&accel::Geometry {
                    flags: accel::GeometryFlags::OPAQUE,
                    geometry: accel::GeometryData::Instances(accel::GeometryInstances {
                        buffer: &instances_buffer.0,
                        buffer_offset: 0,
                    }),
                }],
            };

            let tlas_requirements = device
                .get_acceleration_structure_build_requirements(&top_level_geometry_desc, &[1]);

            let tlas_scratch_buffer = create_empty_buffer::<B>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_STORAGE
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                tlas_requirements.build_scratch_size,
            );

            let tlas_buffer = create_empty_buffer::<B>(
                &device,
                properties.limits.non_coherent_atom_size as u64,
                &memory_types,
                buffer::Usage::ACCELERATION_STRUCTURE_STORAGE
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                tlas_requirements.acceleration_structure_size,
            );

            let mut tlas = AccelerationStructure::<B> {
                accel_struct: device
                    .create_acceleration_structure(&accel::CreateDesc {
                        buffer: &tlas_buffer.0,
                        buffer_offset: 0,
                        size: tlas_requirements.acceleration_structure_size,
                        ty: accel::Type::TopLevel,
                    })
                    .unwrap(),
                backing: tlas_buffer,
            };

            device.set_acceleration_structure_name(&mut tlas.accel_struct, "tlas");

            let mut build_fence = device.create_fence(false).unwrap();
            let mut cmd_buffer = command_pool.allocate_one(command::Level::Primary);
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.build_acceleration_structure(
                &accel::BuildDesc {
                    src: None,
                    dst: &tlas.accel_struct,
                    geometry: &top_level_geometry_desc,
                    scratch: &tlas_scratch_buffer.0,
                    scratch_offset: 0,
                },
                &[accel::BuildRangeDesc {
                    primitive_count: 1,
                    primitive_offset: 0,
                    first_vertex: 0,
                    transform_offset: 0,
                }][..],
            );
            // cmd_buffer.pipeline_barrier(
            //     pso::PipelineStage::ACCELERATION_STRUCTURE_BUILD
            //         ..pso::PipelineStage::ACCELERATION_STRUCTURE_BUILD,
            //     memory::Dependencies::empty(),
            //     iter::once(memory::Barrier::AllBuffers(
            //         buffer::Access::ACCELERATION_STRUCTURE_WRITE
            //             ..buffer::Access::ACCELERATION_STRUCTURE_READ,
            //     )),
            // );
            cmd_buffer.finish();
            queue_group.queues[0].submit(
                iter::once(&cmd_buffer),
                iter::empty(),
                iter::empty(),
                Some(&mut build_fence),
            );
            device
                .wait_for_fence(&build_fence, !0)
                .expect("Can't wait for fence");
            device.free_memory(tlas_scratch_buffer.1);
            device.destroy_buffer(tlas_scratch_buffer.0);

            // Create uniform buffer
            // TODO

            // Create rt pipeline
            let desc_set_layout = device
                .create_descriptor_set_layout(
                    vec![
                        pso::DescriptorSetLayoutBinding {
                            binding: 0,
                            ty: pso::DescriptorType::AccelerationStructure,
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::RAYGEN,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 1,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::RAYGEN,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 2,
                            ty: pso::DescriptorType::Buffer {
                                ty: pso::BufferDescriptorType::Uniform,
                                format: pso::BufferDescriptorFormat::Structured {
                                    dynamic_offset: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::RAYGEN,
                            immutable_samplers: false,
                        },
                    ]
                    .into_iter(),
                    iter::empty(),
                )
                .unwrap();

            let pipeline_layout = device
                .create_pipeline_layout(iter::once(&desc_set_layout), iter::empty())
                .unwrap();

            let raygen_module = device
                .create_shader_module(
                    &auxil::read_spirv(Cursor::new(&include_bytes!("./data/simple.rgen.spv")[..]))
                        .unwrap(),
                )
                .unwrap();

            let miss_module = device
                .create_shader_module(
                    &auxil::read_spirv(Cursor::new(&include_bytes!("./data/simple.rmiss.spv")[..]))
                        .unwrap(),
                )
                .unwrap();

            let closest_hit_module = device
                .create_shader_module(
                    &auxil::read_spirv(Cursor::new(&include_bytes!("./data/simple.rchit.spv")[..]))
                        .unwrap(),
                )
                .unwrap();

            let stages = vec![
                pso::ShaderStageDesc {
                    stage: pso::ShaderStageFlags::RAYGEN,
                    entry_point: pso::EntryPoint {
                        entry: "main",
                        module: &raygen_module,
                        specialization: pso::Specialization::EMPTY,
                    },
                },
                pso::ShaderStageDesc {
                    stage: pso::ShaderStageFlags::MISS,
                    entry_point: pso::EntryPoint {
                        entry: "main",
                        module: &miss_module,
                        specialization: pso::Specialization::EMPTY,
                    },
                },
                pso::ShaderStageDesc {
                    stage: pso::ShaderStageFlags::CLOSEST_HIT,
                    entry_point: pso::EntryPoint {
                        entry: "main",
                        module: &closest_hit_module,
                        specialization: pso::Specialization::EMPTY,
                    },
                },
            ];

            let groups = vec![
                pso::ShaderGroupDesc::General { general_shader: 0 },
                pso::ShaderGroupDesc::General { general_shader: 1 },
                pso::ShaderGroupDesc::TrianglesHitGroup {
                    closest_hit_shader: Some(2),
                    any_hit_shader: None,
                },
            ];

            let pipeline = device
                .create_ray_tracing_pipeline(
                    &pso::RayTracingPipelineDesc::new(&stages, &groups, 1, &pipeline_layout),
                    None,
                )
                .unwrap();

            // Create sbt
            // inline uint32_t aligned_size(uint32_t value, uint32_t alignment)
            // return (value + alignment - 1) & ~(alignment - 1);

            let handle_size = properties.ray_tracing_pipeline.shader_group_handle_size as usize;

            let shader_handle_data = device
                .get_ray_tracing_shader_group_handles(
                    &pipeline,
                    0,
                    groups.len() as u32,
                    groups.len() * handle_size,
                )
                .unwrap();

            let raygen_shader_binding_table = upload_to_buffer::<B, _>(
                &device,
                properties
                    .ray_tracing_pipeline
                    .shader_group_handle_alignment as u64,
                &memory_types,
                buffer::Usage::SHADER_BINDING_TABLE
                    | buffer::Usage::TRANSFER_SRC // todo needed?
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                &shader_handle_data[0..handle_size],
            );

            let miss_shader_binding_table = upload_to_buffer::<B, _>(
                &device,
                properties
                    .ray_tracing_pipeline
                    .shader_group_handle_alignment as u64,
                &memory_types,
                buffer::Usage::SHADER_BINDING_TABLE
                    | buffer::Usage::TRANSFER_SRC // todo needed?
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                &shader_handle_data[handle_size..handle_size * 2],
            );

            let closest_hit_shader_binding_table = upload_to_buffer::<B, _>(
                &device,
                properties
                    .ray_tracing_pipeline
                    .shader_group_handle_alignment as u64,
                &memory_types,
                buffer::Usage::SHADER_BINDING_TABLE
                    | buffer::Usage::TRANSFER_SRC // todo needed?
                    | buffer::Usage::SHADER_DEVICE_ADDRESS,
                &shader_handle_data[handle_size * 2..handle_size * 3],
            );

            // Create desc sets
            // TODO
            let mut desc_pool = device
                .create_descriptor_pool(
                    1,
                    vec![
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::AccelerationStructure,
                            count: 1,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                            count: 1,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Buffer {
                                ty: pso::BufferDescriptorType::Uniform,
                                format: pso::BufferDescriptorFormat::Structured {
                                    dynamic_offset: false,
                                },
                            },
                            count: 1,
                        },
                    ]
                    .into_iter(),
                    pso::DescriptorPoolCreateFlags::empty(),
                )
                .unwrap();

            let mut desc_set = desc_pool.allocate_one(&desc_set_layout).unwrap();

            device.write_descriptor_set(pso::DescriptorSetWrite {
                set: &mut desc_set,
                binding: 0,
                array_offset: 0,
                descriptors: vec![
                    pso::Descriptor::AccelerationStructure(&tlas.accel_struct),
                    pso::Descriptor::Image(&storage_image_view, image::Layout::General),
                    pso::Descriptor::Buffer(&uniform_buffer, buffer::SubRange::WHOLE),
                ]
                .into_iter(),
            });

            // Create cmd buffer

            // The number of the rest of the resources is based on the frames in flight.
            let mut submission_complete_semaphores = Vec::with_capacity(frames_in_flight);
            let mut submission_complete_fences = Vec::with_capacity(frames_in_flight);
            let mut cmd_pools = Vec::with_capacity(frames_in_flight);
            let mut cmd_buffers = Vec::with_capacity(frames_in_flight);

            cmd_pools.push(command_pool);
            for _ in 1..frames_in_flight {
                cmd_pools.push(
                    device
                        .create_command_pool(
                            queue_group.family,
                            pool::CommandPoolCreateFlags::empty(),
                        )
                        .expect("Can't create command pool"),
                );
            }

            for i in 0..frames_in_flight {
                submission_complete_semaphores.push(
                    device
                        .create_semaphore()
                        .expect("Could not create semaphore"),
                );
                submission_complete_fences
                    .push(device.create_fence(true).expect("Could not create fence"));
                cmd_buffers.push(cmd_pools[i].allocate_one(command::Level::Primary));
            }

            Self {
                properties,
                desc_pool: ManuallyDrop::new(desc_pool),
                surface: ManuallyDrop::new(surface),
                format,
                dimensions: extent,
                viewport: pso::Viewport {
                    rect: pso::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as _,
                        h: extent.height as _,
                    },
                    depth: 0.0..1.0,
                },

                bottom_level_accel_struct: ManuallyDrop::new(triangle_blas),
                top_level_accel_struct: ManuallyDrop::new(tlas),
                storage_image: ManuallyDrop::new(storage_image),
                storage_image_view: ManuallyDrop::new(storage_image_view),
                uniform_buffer: ManuallyDrop::new(uniform_buffer),
                uniform_buffer_memory: ManuallyDrop::new(uniform_buffer_memory),

                pipeline: ManuallyDrop::new(pipeline),
                pipeline_layout: ManuallyDrop::new(pipeline_layout),
                raygen_shader_binding_table: ManuallyDrop::new(raygen_shader_binding_table.0),
                raygen_shader_binding_table_memory: ManuallyDrop::new(
                    raygen_shader_binding_table.1,
                ),
                miss_shader_binding_table: ManuallyDrop::new(miss_shader_binding_table.0),
                miss_shader_binding_table_memory: ManuallyDrop::new(miss_shader_binding_table.1),
                closest_hit_shader_binding_table: ManuallyDrop::new(
                    closest_hit_shader_binding_table.0,
                ),
                closest_hit_shader_binding_table_memory: ManuallyDrop::new(
                    closest_hit_shader_binding_table.1,
                ),
                submission_complete_semaphores,
                submission_complete_fences,
                cmd_pools,
                cmd_buffers,
                desc_set,
                frames_in_flight,
                frame: 0,
                device,
                adapter,
                queue_group,
                instance,
            }
        }
    }

    fn recreate_swapchain(&mut self) {
        // let caps = self.surface.capabilities(&self.adapter.physical_device);
        // let swap_config = window::SwapchainConfig::from_caps(&caps, self.format, self.dimensions);
        // println!("{:?}", swap_config);

        // let extent = swap_config.extent.to_extent();
        // self.viewport.rect.w = extent.width as _;
        // self.viewport.rect.h = extent.height as _;

        // unsafe {
        //     self.device.wait_idle().unwrap();
        //     self.device
        //         .destroy_framebuffer(ManuallyDrop::into_inner(ptr::read(&self.framebuffer)));
        //     self.framebuffer = ManuallyDrop::new(
        //         self.device
        //             .create_framebuffer(
        //                 &self.render_pass,
        //                 iter::once(swap_config.framebuffer_attachment()),
        //                 extent,
        //             )
        //             .unwrap(),
        //     )
        // };

        // unsafe {
        //     self.surface
        //         .configure_swapchain(&self.device, swap_config)
        //         .expect("Can't create swapchain");
        // }
    }

    fn render(&mut self) {
        unsafe {
            let surface_image = match self.surface.acquire_image(!0) {
                Ok((image, _)) => image,
                Err(_) => {
                    self.recreate_swapchain();
                    return;
                }
            };

            // Compute index into our resource ring buffers based on the frame number
            // and number of frames in flight. Pay close attention to where this index is needed
            // versus when the swapchain image index we got from acquire_image is needed.
            let frame_idx = self.frame as usize % self.frames_in_flight;

            // Wait for the fence of the previous submission of this frame and reset it; ensures we are
            // submitting only up to maximum number of frames_in_flight if we are submitting faster than
            // the gpu can keep up with. This would also guarantee that any resources which need to be
            // updated with a CPU->GPU data copy are not in use by the GPU, so we can perform those updates.
            // In this case there are none to be done, however.
            let fence = &mut self.submission_complete_fences[frame_idx];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.cmd_pools[frame_idx].reset(false);

            // Rendering
            let cmd_buffer = &mut self.cmd_buffers[frame_idx];
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);

            // Trace the rays
            cmd_buffer.bind_ray_tracing_pipeline(&self.pipeline);
            cmd_buffer.bind_ray_tracing_descriptor_sets(
                &self.pipeline_layout,
                0,
                iter::once(&self.desc_set),
                iter::empty(),
            );
            let handle_size = self
                .properties
                .ray_tracing_pipeline
                .shader_group_handle_size;
            cmd_buffer.trace_rays(
                Some(pso::ShaderBindingTable {
                    buffer: &self.raygen_shader_binding_table,
                    offset: 0,
                    stride: handle_size,
                    size: handle_size as u64,
                }),
                Some(pso::ShaderBindingTable {
                    buffer: &self.miss_shader_binding_table,
                    offset: 0,
                    stride: handle_size,
                    size: handle_size as u64,
                }),
                Some(pso::ShaderBindingTable {
                    buffer: &self.closest_hit_shader_binding_table,
                    offset: 0,
                    stride: handle_size,
                    size: handle_size as u64,
                }),
                None,
                [self.dimensions.width, self.dimensions.height, 1],
            );

            // Copy storage image to output
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER..pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                iter::once(memory::Barrier::Image {
                    states: (image::Access::empty(), image::Layout::Undefined)
                        ..(
                            image::Access::TRANSFER_WRITE,
                            image::Layout::TransferDstOptimal,
                        ),
                    target: surface_image.borrow(),
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                    families: None,
                }),
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER..pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                iter::once(memory::Barrier::Image {
                    states: (image::Access::empty(), image::Layout::General)
                        ..(
                            image::Access::TRANSFER_READ,
                            image::Layout::TransferSrcOptimal,
                        ),
                    target: self.storage_image.deref(),
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                    families: None,
                }),
            );
            cmd_buffer.copy_image(
                &self.storage_image,
                image::Layout::TransferSrcOptimal,
                surface_image.borrow(),
                image::Layout::TransferDstOptimal,
                iter::once(command::ImageCopy {
                    src_subresource: image::SubresourceLayers {
                        aspects: format::Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    src_offset: image::Offset::ZERO,
                    dst_subresource: image::SubresourceLayers {
                        aspects: format::Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    dst_offset: image::Offset::ZERO,
                    extent: image::Extent {
                        width: self.dimensions.width,
                        height: self.dimensions.height,
                        depth: 1,
                    },
                }),
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER..pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                iter::once(memory::Barrier::Image {
                    states: (
                        image::Access::TRANSFER_WRITE,
                        image::Layout::TransferDstOptimal,
                    )..(image::Access::empty(), image::Layout::Present),
                    target: surface_image.borrow(),
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                    families: None,
                }),
            );
            cmd_buffer.pipeline_barrier(
                pso::PipelineStage::TRANSFER..pso::PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                iter::once(memory::Barrier::Image {
                    states: (
                        image::Access::TRANSFER_READ,
                        image::Layout::TransferSrcOptimal,
                    )..(image::Access::empty(), image::Layout::General),
                    target: self.storage_image.deref(),
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                    families: None,
                }),
            );

            cmd_buffer.finish();

            self.queue_group.queues[0].submit(
                iter::once(&*cmd_buffer),
                iter::empty(),
                iter::once(&self.submission_complete_semaphores[frame_idx]),
                Some(&mut self.submission_complete_fences[frame_idx]),
            );

            // present frame
            let result = self.queue_group.queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&mut self.submission_complete_semaphores[frame_idx]),
            );

            if result.is_err() {
                self.recreate_swapchain();
            }

            // Increment our frame
            self.frame += 1;
        }
    }
}

impl<B> Drop for Renderer<B>
where
    B: hal::Backend,
{
    fn drop(&mut self) {
        unsafe {
            // let _ = self.desc_set.take();
            self.device
                .destroy_descriptor_pool(ManuallyDrop::take(&mut self.desc_pool));
            // self.device
            //     .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
            //         &self.set_layout,
            //     )));

            for p in self.cmd_pools.drain(..) {
                self.device.destroy_command_pool(p);
            }
            for s in self.submission_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for f in self.submission_complete_fences.drain(..) {
                self.device.destroy_fence(f);
            }

            self.device
                .destroy_ray_tracing_pipeline(ManuallyDrop::take(&mut self.pipeline));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::take(&mut self.pipeline_layout));
            self.instance
                .destroy_surface(ManuallyDrop::take(&mut self.surface));
        }
    }
}

#[derive(Debug)]
struct AccelerationStructure<B: hal::Backend> {
    pub accel_struct: B::AccelerationStructure,
    pub backing: (B::Buffer, B::Memory),
}

fn create_empty_buffer<B: hal::Backend>(
    device: &B::Device,
    alignment: u64,
    memory_types: &[adapter::MemoryType],
    usage: buffer::Usage,
    size: u64,
) -> (B::Buffer, B::Memory) {
    assert_ne!(size, 0);
    let padded_buffer_len = ((size + alignment - 1) / alignment) * alignment;

    let mut buffer =
        unsafe { device.create_buffer(padded_buffer_len, usage, memory::SparseFlags::empty()) }
            .unwrap();

    let buffer_req = unsafe { device.get_buffer_requirements(&buffer) };

    let upload_type = memory_types
        .iter()
        .enumerate()
        .position(|(id, mem_type)| {
            // type_mask is a bit field where each bit represents a memory type. If the bit is set
            // to 1 it means we can use that type for our buffer. So this code finds the first
            // memory type that has a `1` (or, is allowed), and is visible to the CPU.
            buffer_req.type_mask & (1 << id) != 0
                && mem_type
                    .properties
                    .contains(memory::Properties::CPU_VISIBLE)
        })
        .unwrap()
        .into();

    // TODO: check transitions: read/write mapping and buffer read
    let buffer_memory = unsafe {
        let memory = device
            .allocate_memory(upload_type, buffer_req.size)
            .unwrap();
        device.bind_buffer_memory(&memory, 0, &mut buffer).unwrap();
        memory
    };

    (buffer, buffer_memory)
}

fn upload_to_buffer<B: hal::Backend, T>(
    device: &B::Device,
    alignment: u64,
    memory_types: &[adapter::MemoryType],
    usage: buffer::Usage,
    data: &[T],
) -> (B::Buffer, B::Memory) {
    let buffer_stride = mem::size_of::<T>() as u64;
    let buffer_len = data.len() as u64 * buffer_stride;

    let (buffer, mut buffer_memory) =
        create_empty_buffer::<B>(device, alignment, memory_types, usage, buffer_len);

    unsafe {
        let mapping = device
            .map_memory(&mut buffer_memory, memory::Segment::ALL)
            .unwrap();
        ptr::copy_nonoverlapping(data.as_ptr() as *const u8, mapping, buffer_len as usize);
        device
            .flush_mapped_memory_ranges(iter::once((&buffer_memory, memory::Segment::ALL)))
            .unwrap();
        device.unmap_memory(&mut buffer_memory);
    }

    (buffer, buffer_memory)
}
