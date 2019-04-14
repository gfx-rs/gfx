use ash::extensions::khr;
use ash::version::DeviceV1_0;
use ash::vk;
use smallvec::SmallVec;

use hal;
use hal::error::HostExecutionError;
use hal::memory::Requirements;
use hal::pool::CommandPoolCreateFlags;
use hal::pso::VertexInputRate;
use hal::range::RangeArg;
use hal::{buffer, device as d, format, image, mapping, pass, pso, query, queue};
use hal::{Backbuffer, Features, MemoryTypeId, SwapchainConfig};

use std::borrow::Borrow;
use std::ffi::CString;
use std::ops::Range;
use std::sync::Arc;
use std::{mem, ptr};

use pool::RawCommandPool;
use {conv, native as n, result, window as w};
use {Backend as B, Device};

impl d::Device<B> for Device {
    unsafe fn allocate_memory(
        &self,
        mem_type: MemoryTypeId,
        size: u64,
    ) -> Result<n::Memory, d::AllocationError> {
        let info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
            p_next: ptr::null(),
            allocation_size: size,
            memory_type_index: mem_type.0 as _,
        };

        let result = self.raw.0.allocate_memory(&info, None);

        match result {
            Ok(memory) => Ok(n::Memory { raw: memory }),
            Err(vk::Result::ERROR_TOO_MANY_OBJECTS) => Err(d::AllocationError::TooManyObjects),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_command_pool(
        &self,
        family: queue::QueueFamilyId,
        create_flags: CommandPoolCreateFlags,
    ) -> Result<RawCommandPool, d::OutOfMemory> {
        let mut flags = vk::CommandPoolCreateFlags::empty();
        if create_flags.contains(CommandPoolCreateFlags::TRANSIENT) {
            flags |= vk::CommandPoolCreateFlags::TRANSIENT;
        }
        if create_flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            flags |= vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER;
        }

        let info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags,
            queue_family_index: family.0 as _,
        };

        let result = self.raw.0.create_command_pool(&info, None);

        match result {
            Ok(pool) => Ok(RawCommandPool {
                raw: pool,
                device: self.raw.clone(),
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn destroy_command_pool(&self, pool: RawCommandPool) {
        self.raw.0.destroy_command_pool(pool.raw, None);
    }

    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> Result<n::RenderPass, d::OutOfMemory>
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        let map_subpass_ref = |pass: pass::SubpassRef| match pass {
            pass::SubpassRef::External => vk::SUBPASS_EXTERNAL,
            pass::SubpassRef::Pass(id) => id as u32,
        };

        let attachments = attachments
            .into_iter()
            .map(|attachment| {
                let attachment = attachment.borrow();
                vk::AttachmentDescription {
                    flags: vk::AttachmentDescriptionFlags::empty(), // TODO: may even alias!
                    format: attachment
                        .format
                        .map_or(vk::Format::UNDEFINED, conv::map_format),
                    samples: vk::SampleCountFlags::from_raw(
                        (attachment.samples as u32) & vk::SampleCountFlags::all().as_raw(),
                    ),
                    load_op: conv::map_attachment_load_op(attachment.ops.load),
                    store_op: conv::map_attachment_store_op(attachment.ops.store),
                    stencil_load_op: conv::map_attachment_load_op(attachment.stencil_ops.load),
                    stencil_store_op: conv::map_attachment_store_op(attachment.stencil_ops.store),
                    initial_layout: conv::map_image_layout(attachment.layouts.start),
                    final_layout: conv::map_image_layout(attachment.layouts.end),
                }
            })
            .collect::<Vec<_>>();

        let clear_attachments_mask = attachments
            .iter()
            .enumerate()
            .filter_map(|(i, at)| {
                if at.load_op == vk::AttachmentLoadOp::CLEAR
                    || at.stencil_load_op == vk::AttachmentLoadOp::CLEAR
                {
                    Some(1 << i as u64)
                } else {
                    None
                }
            })
            .sum();

        let attachment_refs = subpasses
            .into_iter()
            .map(|subpass| {
                let subpass = subpass.borrow();
                fn make_ref(&(id, layout): &pass::AttachmentRef) -> vk::AttachmentReference {
                    vk::AttachmentReference {
                        attachment: id as _,
                        layout: conv::map_image_layout(layout),
                    }
                }
                let colors = subpass.colors.iter().map(make_ref).collect::<Box<[_]>>();
                let depth_stencil = subpass.depth_stencil.map(make_ref);
                let inputs = subpass.inputs.iter().map(make_ref).collect::<Box<[_]>>();
                let preserves = subpass
                    .preserves
                    .iter()
                    .map(|&id| id as u32)
                    .collect::<Box<[_]>>();

                (colors, depth_stencil, inputs, preserves)
            })
            .collect::<Box<[_]>>();

        let subpasses = attachment_refs
            .iter()
            .map(|(colors, depth_stencil, inputs, preserves)| {
                vk::SubpassDescription {
                    flags: vk::SubpassDescriptionFlags::empty(),
                    pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
                    input_attachment_count: inputs.len() as u32,
                    p_input_attachments: inputs.as_ptr(),
                    color_attachment_count: colors.len() as u32,
                    p_color_attachments: colors.as_ptr(),
                    p_resolve_attachments: ptr::null(), // TODO
                    p_depth_stencil_attachment: match depth_stencil {
                        Some(ref aref) => aref as *const _,
                        None => ptr::null(),
                    },
                    preserve_attachment_count: preserves.len() as u32,
                    p_preserve_attachments: preserves.as_ptr(),
                }
            })
            .collect::<Box<[_]>>();

        let dependencies = dependencies
            .into_iter()
            .map(|dependency| {
                let dependency = dependency.borrow();
                // TODO: checks
                vk::SubpassDependency {
                    src_subpass: map_subpass_ref(dependency.passes.start),
                    dst_subpass: map_subpass_ref(dependency.passes.end),
                    src_stage_mask: conv::map_pipeline_stage(dependency.stages.start),
                    dst_stage_mask: conv::map_pipeline_stage(dependency.stages.end),
                    src_access_mask: conv::map_image_access(dependency.accesses.start),
                    dst_access_mask: conv::map_image_access(dependency.accesses.end),
                    dependency_flags: vk::DependencyFlags::empty(), // TODO
                }
            })
            .collect::<Vec<_>>();

        let info = vk::RenderPassCreateInfo {
            s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::RenderPassCreateFlags::empty(),
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            subpass_count: subpasses.len() as u32,
            p_subpasses: subpasses.as_ptr(),
            dependency_count: dependencies.len() as u32,
            p_dependencies: dependencies.as_ptr(),
        };

        let result = self.raw.0.create_render_pass(&info, None);

        match result {
            Ok(renderpass) => Ok(n::RenderPass {
                raw: renderpass,
                clear_attachments_mask,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn create_pipeline_layout<IS, IR>(
        &self,
        sets: IS,
        push_constant_ranges: IR,
    ) -> Result<n::PipelineLayout, d::OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        let set_layouts = sets
            .into_iter()
            .map(|set| set.borrow().raw)
            .collect::<Vec<_>>();

        debug!("create_pipeline_layout {:?}", set_layouts);

        let push_constant_ranges = push_constant_ranges
            .into_iter()
            .map(|range| {
                let &(s, ref r) = range.borrow();
                vk::PushConstantRange {
                    stage_flags: conv::map_stage_flags(s),
                    offset: r.start * 4,
                    size: (r.end - r.start) * 4,
                }
            })
            .collect::<Vec<_>>();

        let info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineLayoutCreateFlags::empty(),
            set_layout_count: set_layouts.len() as u32,
            p_set_layouts: set_layouts.as_ptr(),
            push_constant_range_count: push_constant_ranges.len() as u32,
            p_push_constant_ranges: push_constant_ranges.as_ptr(),
        };

        let result = self.raw.0.create_pipeline_layout(&info, None);

        match result {
            Ok(raw) => Ok(n::PipelineLayout { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn create_pipeline_cache(
        &self,
        data: Option<&[u8]>,
    ) -> Result<n::PipelineCache, d::OutOfMemory> {
        let (data_len, data) = if let Some(d) = data {
            (d.len(), d.as_ptr())
        } else {
            (0_usize, ptr::null())
        };

        let info = vk::PipelineCacheCreateInfo {
            s_type: vk::StructureType::PIPELINE_CACHE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineCacheCreateFlags::empty(),
            initial_data_size: data_len,
            p_initial_data: data as _,
        };

        let result = self.raw.0.create_pipeline_cache(&info, None);

        match result {
            Ok(raw) => Ok(n::PipelineCache { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn get_pipeline_cache_data(
        &self,
        cache: &n::PipelineCache,
    ) -> Result<Vec<u8>, d::OutOfMemory> {
        let result = self.raw.0.get_pipeline_cache_data(cache.raw);

        match result {
            Ok(data) => Ok(data),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn destroy_pipeline_cache(&self, cache: n::PipelineCache) {
        self.raw.0.destroy_pipeline_cache(cache.raw, None);
    }

    unsafe fn merge_pipeline_caches<I>(
        &self,
        target: &n::PipelineCache,
        sources: I,
    ) -> Result<(), d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<n::PipelineCache>,
    {
        let caches = sources
            .into_iter()
            .map(|s| s.borrow().raw)
            .collect::<Vec<_>>();
        let result = self.raw.0.fp_v1_0().merge_pipeline_caches(
            self.raw.0.handle(),
            target.raw,
            caches.len() as u32,
            caches.as_ptr(),
        );

        match result {
            vk::Result::SUCCESS => Ok(()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::OutOfHostMemory),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn create_graphics_pipelines<'a, T>(
        &self,
        descs: T,
        cache: Option<&n::PipelineCache>,
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>>
    where
        T: IntoIterator,
        T::Item: Borrow<pso::GraphicsPipelineDesc<'a, B>>,
    {
        let descs = descs.into_iter().collect::<Vec<_>>();
        debug!(
            "create_graphics_pipelines {:?}",
            descs.iter().map(Borrow::borrow).collect::<Vec<_>>()
        );
        const NUM_STAGES: usize = 5;
        const MAX_DYNAMIC_STATES: usize = 10;

        // Store pipeline parameters to avoid stack usage
        let mut info_stages = Vec::with_capacity(descs.len());
        let mut info_vertex_descs = Vec::with_capacity(descs.len());
        let mut info_vertex_input_states = Vec::with_capacity(descs.len());
        let mut info_input_assembly_states = Vec::with_capacity(descs.len());
        let mut info_tessellation_states = Vec::with_capacity(descs.len());
        let mut info_viewport_states = Vec::with_capacity(descs.len());
        let mut info_rasterization_states = Vec::with_capacity(descs.len());
        let mut info_multisample_states = Vec::with_capacity(descs.len());
        let mut info_depth_stencil_states = Vec::with_capacity(descs.len());
        let mut info_color_blend_states = Vec::with_capacity(descs.len());
        let mut info_dynamic_states = Vec::with_capacity(descs.len());
        let mut color_attachments = Vec::with_capacity(descs.len());
        let mut info_specializations = Vec::with_capacity(descs.len() * NUM_STAGES);
        let mut specialization_entries = Vec::with_capacity(descs.len() * NUM_STAGES);
        let mut dynamic_states = Vec::with_capacity(descs.len() * MAX_DYNAMIC_STATES);
        let mut viewports = Vec::with_capacity(descs.len());
        let mut scissors = Vec::with_capacity(descs.len());
        let mut sample_masks = Vec::with_capacity(descs.len());

        let mut c_strings = Vec::new(); // hold the C strings temporarily
        let mut make_stage = |stage, source: &pso::EntryPoint<'a, B>| {
            let string = CString::new(source.entry).unwrap();
            let p_name = string.as_ptr();
            c_strings.push(string);

            let map_entries = source
                .specialization
                .constants
                .iter()
                .map(|c| vk::SpecializationMapEntry {
                    constant_id: c.id,
                    offset: c.range.start as _,
                    size: (c.range.end - c.range.start) as _,
                })
                .collect::<SmallVec<[_; 4]>>();

            specialization_entries.push(map_entries);
            let map_entries = specialization_entries.last().unwrap();

            info_specializations.push(vk::SpecializationInfo {
                map_entry_count: map_entries.len() as _,
                p_map_entries: map_entries.as_ptr(),
                data_size: source.specialization.data.len() as _,
                p_data: source.specialization.data.as_ptr() as _,
            });
            let info = info_specializations.last().unwrap();

            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                stage,
                module: source.module.raw,
                p_name,
                p_specialization_info: info,
            }
        };

        let infos = descs
            .iter()
            .map(|desc| {
                let desc = desc.borrow();
                let mut stages = Vec::new();
                // Vertex stage
                if true {
                    //vertex shader is required
                    stages.push(make_stage(
                        vk::ShaderStageFlags::VERTEX,
                        &desc.shaders.vertex,
                    ));
                }
                // Pixel stage
                if let Some(ref entry) = desc.shaders.fragment {
                    stages.push(make_stage(vk::ShaderStageFlags::FRAGMENT, entry));
                }
                // Geometry stage
                if let Some(ref entry) = desc.shaders.geometry {
                    stages.push(make_stage(vk::ShaderStageFlags::GEOMETRY, entry));
                }
                // Domain stage
                if let Some(ref entry) = desc.shaders.domain {
                    stages.push(make_stage(
                        vk::ShaderStageFlags::TESSELLATION_EVALUATION,
                        entry,
                    ));
                }
                // Hull stage
                if let Some(ref entry) = desc.shaders.hull {
                    stages.push(make_stage(
                        vk::ShaderStageFlags::TESSELLATION_CONTROL,
                        entry,
                    ));
                }

                let (polygon_mode, line_width) =
                    conv::map_polygon_mode(desc.rasterizer.polygon_mode);
                info_stages.push(stages);

                {
                    let mut vertex_bindings = Vec::new();
                    for vbuf in &desc.vertex_buffers {
                        vertex_bindings.push(vk::VertexInputBindingDescription {
                            binding: vbuf.binding,
                            stride: vbuf.stride as u32,
                            input_rate: match vbuf.rate {
                                VertexInputRate::Vertex => vk::VertexInputRate::VERTEX,
                                VertexInputRate::Instance(divisor) => {
                                    debug_assert_eq!(divisor, 1, "Custom vertex rate divisors not supported in Vulkan backend without extension");
                                    vk::VertexInputRate::INSTANCE
                                },
                            },
                        });
                    }
                    let mut vertex_attributes = Vec::new();
                    for attr in desc.attributes.iter() {
                        vertex_attributes.push(vk::VertexInputAttributeDescription {
                            location: attr.location as u32,
                            binding: attr.binding as u32,
                            format: conv::map_format(attr.element.format),
                            offset: attr.element.offset as u32,
                        });
                    }

                    info_vertex_descs.push((vertex_bindings, vertex_attributes));
                }

                let &(ref vertex_bindings, ref vertex_attributes) =
                    info_vertex_descs.last().unwrap();

                info_vertex_input_states.push(vk::PipelineVertexInputStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineVertexInputStateCreateFlags::empty(),
                    vertex_binding_description_count: vertex_bindings.len() as u32,
                    p_vertex_binding_descriptions: vertex_bindings.as_ptr(),
                    vertex_attribute_description_count: vertex_attributes.len() as u32,
                    p_vertex_attribute_descriptions: vertex_attributes.as_ptr(),
                });

                info_input_assembly_states.push(vk::PipelineInputAssemblyStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
                    topology: conv::map_topology(desc.input_assembler.primitive),
                    primitive_restart_enable: vk::FALSE,
                });
                let depth_bias = match desc.rasterizer.depth_bias {
                    Some(pso::State::Static(db)) => db,
                    Some(pso::State::Dynamic) => {
                        dynamic_states.push(vk::DynamicState::DEPTH_BIAS);
                        pso::DepthBias::default()
                    }
                    None => pso::DepthBias::default(),
                };

                info_rasterization_states.push(vk::PipelineRasterizationStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineRasterizationStateCreateFlags::empty(),
                    depth_clamp_enable: if desc.rasterizer.depth_clamping {
                        if self.raw.1.contains(Features::DEPTH_CLAMP) {
                            vk::TRUE
                        } else {
                            warn!("Depth clamping was requested on a device with disabled feature");
                            vk::FALSE
                        }
                    } else {
                        vk::FALSE
                    },
                    rasterizer_discard_enable: match (&desc.shaders.fragment, &desc.depth_stencil.depth, &desc.depth_stencil.stencil) {
                        (None, pso::DepthTest::Off, pso::StencilTest::Off) => vk::TRUE,
                                                                         _ => vk::FALSE,
                    },
                    polygon_mode,
                    cull_mode: conv::map_cull_face(desc.rasterizer.cull_face),
                    front_face: conv::map_front_face(desc.rasterizer.front_face),
                    depth_bias_enable: if desc.rasterizer.depth_bias.is_some() {
                        vk::TRUE
                    } else {
                        vk::FALSE
                    },
                    depth_bias_constant_factor: depth_bias.const_factor,
                    depth_bias_clamp: depth_bias.clamp,
                    depth_bias_slope_factor: depth_bias.slope_factor,
                    line_width,
                });

                use hal::Primitive::PatchList;
                if let PatchList(patch_control_points) = desc.input_assembler.primitive {
                    info_tessellation_states.push(vk::PipelineTessellationStateCreateInfo {
                        s_type: vk::StructureType::PIPELINE_TESSELLATION_STATE_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::PipelineTessellationStateCreateFlags::empty(),
                        patch_control_points: patch_control_points as u32,
                    });
                }

                let dynamic_state_base = dynamic_states.len();

                info_viewport_states.push(vk::PipelineViewportStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineViewportStateCreateFlags::empty(),
                    scissor_count: 1, // TODO
                    p_scissors: match desc.baked_states.scissor {
                        Some(ref rect) => {
                            scissors.push(conv::map_rect(rect));
                            scissors.last().unwrap()
                        }
                        None => {
                            dynamic_states.push(vk::DynamicState::SCISSOR);
                            ptr::null()
                        }
                    },
                    viewport_count: 1, // TODO
                    p_viewports: match desc.baked_states.viewport {
                        Some(ref vp) => {
                            viewports.push(conv::map_viewport(vp));
                            viewports.last().unwrap()
                        }
                        None => {
                            dynamic_states.push(vk::DynamicState::VIEWPORT);
                            ptr::null()
                        }
                    },
                });

                let multisampling_state = match desc.multisampling {
                    Some(ref ms) => {
                        let sample_mask = [
                            (ms.sample_mask & 0xFFFFFFFF) as u32,
                            ((ms.sample_mask >> 32) & 0xFFFFFFFF) as u32,
                        ];
                        sample_masks.push(sample_mask);

                        vk::PipelineMultisampleStateCreateInfo {
                            s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::PipelineMultisampleStateCreateFlags::empty(),
                            rasterization_samples: vk::SampleCountFlags::from_raw(
                                (ms.rasterization_samples as u32)
                                    & vk::SampleCountFlags::all().as_raw(),
                            ),
                            sample_shading_enable: ms.sample_shading.is_some() as _,
                            min_sample_shading: ms.sample_shading.unwrap_or(0.0),
                            p_sample_mask: sample_masks.last().unwrap().as_ptr(),
                            alpha_to_coverage_enable: ms.alpha_coverage as _,
                            alpha_to_one_enable: ms.alpha_to_one as _,
                        }
                    }
                    None => vk::PipelineMultisampleStateCreateInfo {
                        s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::PipelineMultisampleStateCreateFlags::empty(),
                        rasterization_samples: vk::SampleCountFlags::TYPE_1,
                        sample_shading_enable: vk::FALSE,
                        min_sample_shading: 0.0,
                        p_sample_mask: ptr::null(),
                        alpha_to_coverage_enable: vk::FALSE,
                        alpha_to_one_enable: vk::FALSE,
                    },
                };
                info_multisample_states.push(multisampling_state);

                let depth_stencil = desc.depth_stencil;
                let (depth_test_enable, depth_write_enable, depth_compare_op) =
                    match depth_stencil.depth {
                        pso::DepthTest::On { fun, write } => {
                            (vk::TRUE, write as _, conv::map_comparison(fun))
                        }
                        pso::DepthTest::Off => (vk::FALSE, vk::FALSE, vk::CompareOp::NEVER),
                    };
                let (stencil_test_enable, front, back) = match depth_stencil.stencil {
                    pso::StencilTest::On {
                        ref front,
                        ref back,
                    } => (
                        vk::TRUE,
                        conv::map_stencil_side(front),
                        conv::map_stencil_side(back),
                    ),
                    pso::StencilTest::Off => mem::zeroed(),
                };
                let (min_depth_bounds, max_depth_bounds) = match desc.baked_states.depth_bounds {
                    Some(ref range) => (range.start, range.end),
                    None => {
                        dynamic_states.push(vk::DynamicState::DEPTH_BOUNDS);
                        (0.0, 1.0)
                    }
                };

                info_depth_stencil_states.push(vk::PipelineDepthStencilStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
                    depth_test_enable,
                    depth_write_enable,
                    depth_compare_op,
                    depth_bounds_test_enable: depth_stencil.depth_bounds as _,
                    stencil_test_enable,
                    front,
                    back,
                    min_depth_bounds,
                    max_depth_bounds,
                });

                // Build blend states for color attachments
                let blend_states = desc
                    .blender
                    .targets
                    .iter()
                    .map(|&pso::ColorBlendDesc(mask, ref blend)| {
                        let color_write_mask = vk::ColorComponentFlags::from_raw(mask.bits() as _);
                        match *blend {
                            pso::BlendState::On { color, alpha } => {
                                let (
                                    color_blend_op,
                                    src_color_blend_factor,
                                    dst_color_blend_factor,
                                ) = conv::map_blend_op(color);
                                let (
                                    alpha_blend_op,
                                    src_alpha_blend_factor,
                                    dst_alpha_blend_factor,
                                ) = conv::map_blend_op(alpha);
                                vk::PipelineColorBlendAttachmentState {
                                    color_write_mask,
                                    blend_enable: vk::TRUE,
                                    src_color_blend_factor,
                                    dst_color_blend_factor,
                                    color_blend_op,
                                    src_alpha_blend_factor,
                                    dst_alpha_blend_factor,
                                    alpha_blend_op,
                                }
                            }
                            pso::BlendState::Off => vk::PipelineColorBlendAttachmentState {
                                color_write_mask,
                                ..mem::zeroed()
                            },
                        }
                    })
                    .collect::<Vec<_>>();
                color_attachments.push(blend_states);

                info_color_blend_states.push(vk::PipelineColorBlendStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineColorBlendStateCreateFlags::empty(),
                    logic_op_enable: vk::FALSE, // TODO
                    logic_op: vk::LogicOp::CLEAR,
                    attachment_count: color_attachments.last().unwrap().len() as _,
                    p_attachments: color_attachments.last().unwrap().as_ptr(), // TODO:
                    blend_constants: match desc.baked_states.blend_color {
                        Some(value) => value,
                        None => {
                            dynamic_states.push(vk::DynamicState::BLEND_CONSTANTS);
                            [0.0; 4]
                        }
                    },
                });

                info_dynamic_states.push(vk::PipelineDynamicStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_DYNAMIC_STATE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineDynamicStateCreateFlags::empty(),
                    dynamic_state_count: (dynamic_states.len() - dynamic_state_base) as _,
                    p_dynamic_states: dynamic_states.as_ptr().offset(dynamic_state_base as _),
                });

                let (base_handle, base_index) = match desc.parent {
                    pso::BasePipeline::Pipeline(pipeline) => (pipeline.0, -1),
                    pso::BasePipeline::Index(index) => (vk::Pipeline::null(), index as _),
                    pso::BasePipeline::None => (vk::Pipeline::null(), -1),
                };

                let mut flags = vk::PipelineCreateFlags::empty();
                match desc.parent {
                    pso::BasePipeline::None => (),
                    _ => {
                        flags |= vk::PipelineCreateFlags::DERIVATIVE;
                    }
                }
                if desc
                    .flags
                    .contains(pso::PipelineCreationFlags::DISABLE_OPTIMIZATION)
                {
                    flags |= vk::PipelineCreateFlags::DISABLE_OPTIMIZATION;
                }
                if desc
                    .flags
                    .contains(pso::PipelineCreationFlags::ALLOW_DERIVATIVES)
                {
                    flags |= vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
                }

                Ok(vk::GraphicsPipelineCreateInfo {
                    s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags,
                    stage_count: info_stages.last().unwrap().len() as _,
                    p_stages: info_stages.last().unwrap().as_ptr(),
                    p_vertex_input_state: info_vertex_input_states.last().unwrap(),
                    p_input_assembly_state: info_input_assembly_states.last().unwrap(),
                    p_rasterization_state: info_rasterization_states.last().unwrap(),
                    p_tessellation_state: match desc.input_assembler.primitive {
                        PatchList(_) => info_tessellation_states.last().unwrap(),
                        _            => ptr::null(),
                    },
                    p_viewport_state: info_viewport_states.last().unwrap(),
                    p_multisample_state: info_multisample_states.last().unwrap(),
                    p_depth_stencil_state: info_depth_stencil_states.last().unwrap(),
                    p_color_blend_state: info_color_blend_states.last().unwrap(),
                    p_dynamic_state: info_dynamic_states.last().unwrap(),
                    layout: desc.layout.raw,
                    render_pass: desc.subpass.main_pass.raw,
                    subpass: desc.subpass.index as _,
                    base_pipeline_handle: base_handle,
                    base_pipeline_index: base_index,
                })
            })
            .collect::<Vec<_>>();

        let valid_infos = infos
            .iter()
            .filter_map(|info| info.clone().ok())
            .collect::<Vec<_>>();
        let result = if valid_infos.is_empty() {
            Ok(Vec::new())
        } else {
            self.raw.0.create_graphics_pipelines(
                match cache {
                    Some(cache) => cache.raw,
                    None => vk::PipelineCache::null(),
                },
                &valid_infos,
                None,
            )
        };

        let (pipelines, error) = match result {
            Ok(pipelines) => (pipelines, None),
            Err((pipelines, error)) => (pipelines, Some(error)),
        };

        let mut psos = pipelines.into_iter();
        infos
            .into_iter()
            .map(|result| {
                result.and_then(|_| {
                    let pso = psos.next().unwrap();
                    if pso == vk::Pipeline::null() {
                        match error {
                            Some(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                                Err(d::OutOfMemory::OutOfHostMemory.into())
                            }
                            Some(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                                Err(d::OutOfMemory::OutOfDeviceMemory.into())
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        Ok(n::GraphicsPipeline(pso))
                    }
                })
            })
            .collect()
    }

    unsafe fn create_compute_pipelines<'a, T>(
        &self,
        descs: T,
        cache: Option<&n::PipelineCache>,
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>>
    where
        T: IntoIterator,
        T::Item: Borrow<pso::ComputePipelineDesc<'a, B>>,
    {
        let descs = descs.into_iter().collect::<Vec<_>>();
        let mut c_strings = Vec::new(); // hold the C strings temporarily
        let mut info_specializations = Vec::with_capacity(descs.len());
        let mut specialization_entries = Vec::with_capacity(descs.len());

        let infos = descs
            .iter()
            .map(|desc| {
                let desc = desc.borrow();
                let string = CString::new(desc.shader.entry).unwrap();
                let p_name = string.as_ptr();
                c_strings.push(string);

                let map_entries = desc
                    .shader
                    .specialization
                    .constants
                    .iter()
                    .map(|c| vk::SpecializationMapEntry {
                        constant_id: c.id,
                        offset: c.range.start as _,
                        size: (c.range.end - c.range.start) as _,
                    })
                    .collect::<SmallVec<[_; 4]>>();

                specialization_entries.push(map_entries);
                let map_entries = specialization_entries.last().unwrap();

                info_specializations.push(vk::SpecializationInfo {
                    map_entry_count: map_entries.len() as _,
                    p_map_entries: map_entries.as_ptr(),
                    data_size: desc.shader.specialization.data.len() as _,
                    p_data: desc.shader.specialization.data.as_ptr() as _,
                });
                let info = info_specializations.last().unwrap();

                let stage = vk::PipelineShaderStageCreateInfo {
                    s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::PipelineShaderStageCreateFlags::empty(),
                    stage: vk::ShaderStageFlags::COMPUTE,
                    module: desc.shader.module.raw,
                    p_name,
                    p_specialization_info: info,
                };

                let (base_handle, base_index) = match desc.parent {
                    pso::BasePipeline::Pipeline(pipeline) => (pipeline.0, -1),
                    pso::BasePipeline::Index(index) => (vk::Pipeline::null(), index as _),
                    pso::BasePipeline::None => (vk::Pipeline::null(), -1),
                };

                let mut flags = vk::PipelineCreateFlags::empty();
                match desc.parent {
                    pso::BasePipeline::None => (),
                    _ => {
                        flags |= vk::PipelineCreateFlags::DERIVATIVE;
                    }
                }
                if desc
                    .flags
                    .contains(pso::PipelineCreationFlags::DISABLE_OPTIMIZATION)
                {
                    flags |= vk::PipelineCreateFlags::DISABLE_OPTIMIZATION;
                }
                if desc
                    .flags
                    .contains(pso::PipelineCreationFlags::ALLOW_DERIVATIVES)
                {
                    flags |= vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
                }

                Ok(vk::ComputePipelineCreateInfo {
                    s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags,
                    stage,
                    layout: desc.layout.raw,
                    base_pipeline_handle: base_handle,
                    base_pipeline_index: base_index,
                })
            })
            .collect::<Vec<_>>();

        let valid_infos = infos
            .iter()
            .filter_map(|info| info.clone().ok())
            .collect::<Vec<_>>();
        let result = if valid_infos.is_empty() {
            Ok(Vec::new())
        } else {
            self.raw.0.create_compute_pipelines(
                match cache {
                    Some(cache) => cache.raw,
                    None => vk::PipelineCache::null(),
                },
                &valid_infos,
                None,
            )
        };

        let (pipelines, error) = match result {
            Ok(pipelines) => (pipelines, None),
            Err((pipelines, error)) => (pipelines, Some(error)),
        };

        let mut psos = pipelines.into_iter();
        infos
            .into_iter()
            .map(|result| {
                result.and_then(|_| {
                    let pso = psos.next().unwrap();
                    if pso == vk::Pipeline::null() {
                        match error {
                            Some(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                                Err(d::OutOfMemory::OutOfHostMemory.into())
                            }
                            Some(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                                Err(d::OutOfMemory::OutOfDeviceMemory.into())
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        Ok(n::ComputePipeline(pso))
                    }
                })
            })
            .collect()
    }

    unsafe fn create_framebuffer<T>(
        &self,
        renderpass: &n::RenderPass,
        attachments: T,
        extent: image::Extent,
    ) -> Result<n::Framebuffer, d::OutOfMemory>
    where
        T: IntoIterator,
        T::Item: Borrow<n::ImageView>,
    {
        let attachments_raw = attachments
            .into_iter()
            .map(|attachment| attachment.borrow().view)
            .collect::<SmallVec<[_; 4]>>();

        let info = vk::FramebufferCreateInfo {
            s_type: vk::StructureType::FRAMEBUFFER_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::FramebufferCreateFlags::empty(),
            render_pass: renderpass.raw,
            attachment_count: attachments_raw.len() as u32,
            p_attachments: attachments_raw.as_ptr(),
            width: extent.width,
            height: extent.height,
            layers: extent.depth,
        };

        let result = self.raw.0.create_framebuffer(&info, None);

        match result {
            Ok(raw) => Ok(n::Framebuffer { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::OutOfDeviceMemory),
            _ => unreachable!(),
        }
    }

    unsafe fn create_shader_module(
        &self,
        spirv_data: &[u8],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(spirv_data.len() & 3, 0);

        let info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: spirv_data.len(),
            p_code: spirv_data as *const _ as *const u32,
        };

        let module = self.raw.0.create_shader_module(&info, None);

        match module {
            Ok(raw) => Ok(n::ShaderModule { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            Err(_) => {
                Err(d::ShaderError::CompilationFailed(String::new())) // TODO
            }
        }
    }

    unsafe fn create_sampler(
        &self,
        sampler_info: image::SamplerInfo,
    ) -> Result<n::Sampler, d::AllocationError> {
        use hal::pso::Comparison;

        let (anisotropy_enable, max_anisotropy) = match sampler_info.anisotropic {
            image::Anisotropic::Off => (vk::FALSE, 1.0),
            image::Anisotropic::On(aniso) => {
                if self.raw.1.contains(Features::SAMPLER_ANISOTROPY) {
                    (vk::TRUE, aniso as f32)
                } else {
                    warn!(
                        "Anisotropy({}) was requested on a device with disabled feature",
                        aniso
                    );
                    (vk::FALSE, 1.0)
                }
            }
        };
        let info = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SAMPLER_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::SamplerCreateFlags::empty(),
            mag_filter: conv::map_filter(sampler_info.mag_filter),
            min_filter: conv::map_filter(sampler_info.min_filter),
            mipmap_mode: conv::map_mip_filter(sampler_info.mip_filter),
            address_mode_u: conv::map_wrap(sampler_info.wrap_mode.0),
            address_mode_v: conv::map_wrap(sampler_info.wrap_mode.1),
            address_mode_w: conv::map_wrap(sampler_info.wrap_mode.2),
            mip_lod_bias: sampler_info.lod_bias.into(),
            anisotropy_enable,
            max_anisotropy,
            compare_enable: if sampler_info.comparison.is_some() {
                vk::TRUE
            } else {
                vk::FALSE
            },
            compare_op: conv::map_comparison(sampler_info.comparison.unwrap_or(Comparison::Never)),
            min_lod: sampler_info.lod_range.start.into(),
            max_lod: sampler_info.lod_range.end.into(),
            border_color: match conv::map_border_color(sampler_info.border) {
                Some(bc) => bc,
                None => {
                    error!("Unsupported border color {:x}", sampler_info.border.0);
                    vk::BorderColor::FLOAT_TRANSPARENT_BLACK
                }
            },
            unnormalized_coordinates: vk::FALSE,
        };

        let result = self.raw.0.create_sampler(&info, None);

        match result {
            Ok(sampler) => Ok(n::Sampler(sampler)),
            Err(vk::Result::ERROR_TOO_MANY_OBJECTS) => Err(d::AllocationError::TooManyObjects),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    ///
    unsafe fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
    ) -> Result<n::Buffer, buffer::CreationError> {
        let info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::BufferCreateFlags::empty(), // TODO:
            size,
            usage: conv::map_buffer_usage(usage),
            sharing_mode: vk::SharingMode::EXCLUSIVE, // TODO:
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
        };

        let result = self.raw.0.create_buffer(&info, None);

        match result {
            Ok(raw) => Ok(n::Buffer { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn get_buffer_requirements(&self, buffer: &n::Buffer) -> Requirements {
        let req = self.raw.0.get_buffer_memory_requirements(buffer.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: req.memory_type_bits as _,
        }
    }

    unsafe fn bind_buffer_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        buffer: &mut n::Buffer,
    ) -> Result<(), d::BindError> {
        let result = self
            .raw
            .0
            .bind_buffer_memory(buffer.raw, memory.raw, offset);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        buffer: &n::Buffer,
        format: Option<format::Format>,
        range: R,
    ) -> Result<n::BufferView, buffer::ViewCreationError> {
        let (offset, size) = conv::map_range_arg(&range);
        let info = vk::BufferViewCreateInfo {
            s_type: vk::StructureType::BUFFER_VIEW_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::BufferViewCreateFlags::empty(),
            buffer: buffer.raw,
            format: format.map_or(vk::Format::UNDEFINED, conv::map_format),
            offset,
            range: size,
        };

        let result = self.raw.0.create_buffer_view(&info, None);

        match result {
            Ok(raw) => Ok(n::BufferView { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Result<n::Image, image::CreationError> {
        let flags = conv::map_view_capabilities(view_caps);
        let extent = conv::map_extent(kind.extent());
        let array_layers = kind.num_layers();
        let samples = kind.num_samples() as u32;
        let image_type = match kind {
            image::Kind::D1(..) => vk::ImageType::TYPE_1D,
            image::Kind::D2(..) => vk::ImageType::TYPE_2D,
            image::Kind::D3(..) => vk::ImageType::TYPE_3D,
        };

        let info = vk::ImageCreateInfo {
            s_type: vk::StructureType::IMAGE_CREATE_INFO,
            p_next: ptr::null(),
            flags,
            image_type,
            format: conv::map_format(format),
            extent: extent.clone(),
            mip_levels: mip_levels as u32,
            array_layers: array_layers as u32,
            samples: vk::SampleCountFlags::from_raw(samples & vk::SampleCountFlags::all().as_raw()),
            tiling: conv::map_tiling(tiling),
            usage: conv::map_image_usage(usage),
            sharing_mode: vk::SharingMode::EXCLUSIVE, // TODO:
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            initial_layout: vk::ImageLayout::UNDEFINED,
        };

        let result = self.raw.0.create_image(&info, None);

        match result {
            Ok(raw) => Ok(n::Image {
                raw,
                ty: image_type,
                flags,
                extent,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn get_image_requirements(&self, image: &n::Image) -> Requirements {
        let req = self.raw.0.get_image_memory_requirements(image.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: req.memory_type_bits as _,
        }
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        image: &n::Image,
        subresource: image::Subresource,
    ) -> image::SubresourceFootprint {
        let sub = conv::map_subresource(&subresource);
        let layout = self.raw.0.get_image_subresource_layout(image.raw, sub);

        image::SubresourceFootprint {
            slice: layout.offset..layout.offset + layout.size,
            row_pitch: layout.row_pitch,
            array_pitch: layout.array_pitch,
            depth_pitch: layout.depth_pitch,
        }
    }

    unsafe fn bind_image_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        image: &mut n::Image,
    ) -> Result<(), d::BindError> {
        // TODO: error handling
        // TODO: check required type
        let result = self.raw.0.bind_image_memory(image.raw, memory.raw, offset);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_image_view(
        &self,
        image: &n::Image,
        kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        let is_cube = image
            .flags
            .intersects(vk::ImageCreateFlags::CUBE_COMPATIBLE);
        let info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::ImageViewCreateFlags::empty(),
            image: image.raw,
            view_type: match conv::map_view_kind(kind, image.ty, is_cube) {
                Some(ty) => ty,
                None => return Err(image::ViewError::BadKind(kind)),
            },
            format: conv::map_format(format),
            components: conv::map_swizzle(swizzle),
            subresource_range: conv::map_subresource_range(&range),
        };

        let result = self.raw.0.create_image_view(&info, None);

        match result {
            Ok(view) => Ok(n::ImageView {
                image: image.raw,
                view,
                range,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_descriptor_pool<T>(
        &self,
        max_sets: usize,
        descriptor_pools: T,
        flags: pso::DescriptorPoolCreateFlags,
    ) -> Result<n::DescriptorPool, d::OutOfMemory>
    where
        T: IntoIterator,
        T::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        let pools = descriptor_pools
            .into_iter()
            .map(|pool| {
                let pool = pool.borrow();
                vk::DescriptorPoolSize {
                    ty: conv::map_descriptor_type(pool.ty),
                    descriptor_count: pool.count as u32,
                }
            })
            .collect::<Vec<_>>();

        let info = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: conv::map_descriptor_pool_create_flags(flags),
            max_sets: max_sets as u32,
            pool_size_count: pools.len() as u32,
            p_pool_sizes: pools.as_ptr(),
        };

        let result = self.raw.0.create_descriptor_pool(&info, None);

        match result {
            Ok(pool) => Ok(n::DescriptorPool {
                raw: pool,
                device: self.raw.clone(),
                set_free_vec: Vec::new(),
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        binding_iter: I,
        immutable_sampler_iter: J,
    ) -> Result<n::DescriptorSetLayout, d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<n::Sampler>,
    {
        let immutable_samplers = immutable_sampler_iter
            .into_iter()
            .map(|is| is.borrow().0)
            .collect::<Vec<_>>();
        let mut sampler_offset = 0;

        let bindings = Arc::new(
            binding_iter
                .into_iter()
                .map(|b| b.borrow().clone())
                .collect::<Vec<_>>(),
        );

        let raw_bindings = bindings
            .iter()
            .map(|b| vk::DescriptorSetLayoutBinding {
                binding: b.binding,
                descriptor_type: conv::map_descriptor_type(b.ty),
                descriptor_count: b.count as _,
                stage_flags: conv::map_stage_flags(b.stage_flags),
                p_immutable_samplers: if b.immutable_samplers {
                    let slice = &immutable_samplers[sampler_offset..];
                    sampler_offset += b.count;
                    slice.as_ptr()
                } else {
                    ptr::null()
                },
            })
            .collect::<Vec<_>>();

        debug!("create_descriptor_set_layout {:?}", raw_bindings);

        let info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            binding_count: raw_bindings.len() as _,
            p_bindings: raw_bindings.as_ptr(),
        };

        let result = self.raw.0.create_descriptor_set_layout(&info, None);

        match result {
            Ok(layout) => Ok(n::DescriptorSetLayout {
                raw: layout,
                bindings,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, B, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, B>>,
    {
        let mut raw_writes = Vec::new();
        let mut image_infos = Vec::new();
        let mut buffer_infos = Vec::new();
        let mut texel_buffer_views = Vec::new();

        for sw in write_iter {
            let layout = sw
                .set
                .bindings
                .iter()
                .find(|lb| lb.binding == sw.binding)
                .expect("Descriptor set writes don't match the set layout!");
            let mut raw = vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                p_next: ptr::null(),
                dst_set: sw.set.raw,
                dst_binding: sw.binding,
                dst_array_element: sw.array_offset as _,
                descriptor_count: 0,
                descriptor_type: conv::map_descriptor_type(layout.ty),
                p_image_info: ptr::null(),
                p_buffer_info: ptr::null(),
                p_texel_buffer_view: ptr::null(),
            };

            for descriptor in sw.descriptors {
                raw.descriptor_count += 1;
                match *descriptor.borrow() {
                    pso::Descriptor::Sampler(sampler) => {
                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: sampler.0,
                            image_view: vk::ImageView::null(),
                            image_layout: vk::ImageLayout::GENERAL,
                        });
                    }
                    pso::Descriptor::Image(view, layout) => {
                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: vk::Sampler::null(),
                            image_view: view.view,
                            image_layout: conv::map_image_layout(layout),
                        });
                    }
                    pso::Descriptor::CombinedImageSampler(view, layout, sampler) => {
                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: sampler.0,
                            image_view: view.view,
                            image_layout: conv::map_image_layout(layout),
                        });
                    }
                    pso::Descriptor::Buffer(buffer, ref range) => {
                        let offset = range.start.unwrap_or(0);
                        buffer_infos.push(vk::DescriptorBufferInfo {
                            buffer: buffer.raw,
                            offset,
                            range: match range.end {
                                Some(end) => end - offset,
                                None => vk::WHOLE_SIZE,
                            },
                        });
                    }
                    pso::Descriptor::UniformTexelBuffer(view)
                    | pso::Descriptor::StorageTexelBuffer(view) => {
                        texel_buffer_views.push(view.raw);
                    }
                }
            }

            raw.p_image_info = image_infos.len() as _;
            raw.p_buffer_info = buffer_infos.len() as _;
            raw.p_texel_buffer_view = texel_buffer_views.len() as _;
            raw_writes.push(raw);
        }

        // Patch the pointers now that we have all the storage allocated
        for raw in &mut raw_writes {
            use vk::DescriptorType as Dt;
            match raw.descriptor_type {
                Dt::SAMPLER
                | Dt::SAMPLED_IMAGE
                | Dt::STORAGE_IMAGE
                | Dt::COMBINED_IMAGE_SAMPLER
                | Dt::INPUT_ATTACHMENT => {
                    raw.p_buffer_info = ptr::null();
                    raw.p_texel_buffer_view = ptr::null();
                    let base = raw.p_image_info as usize - raw.descriptor_count as usize;
                    raw.p_image_info = image_infos[base..].as_ptr();
                }
                Dt::UNIFORM_TEXEL_BUFFER | Dt::STORAGE_TEXEL_BUFFER => {
                    raw.p_buffer_info = ptr::null();
                    raw.p_image_info = ptr::null();
                    let base = raw.p_texel_buffer_view as usize - raw.descriptor_count as usize;
                    raw.p_texel_buffer_view = texel_buffer_views[base..].as_ptr();
                }
                Dt::UNIFORM_BUFFER
                | Dt::STORAGE_BUFFER
                | Dt::STORAGE_BUFFER_DYNAMIC
                | Dt::UNIFORM_BUFFER_DYNAMIC => {
                    raw.p_image_info = ptr::null();
                    raw.p_texel_buffer_view = ptr::null();
                    let base = raw.p_buffer_info as usize - raw.descriptor_count as usize;
                    raw.p_buffer_info = buffer_infos[base..].as_ptr();
                }
                _ => panic!("unknown descriptor type"),
            }
        }

        self.raw.0.update_descriptor_sets(&raw_writes, &[]);
    }

    unsafe fn copy_descriptor_sets<'a, I>(&self, copies: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>,
    {
        let copies = copies
            .into_iter()
            .map(|copy| {
                let c = copy.borrow();
                vk::CopyDescriptorSet {
                    s_type: vk::StructureType::COPY_DESCRIPTOR_SET,
                    p_next: ptr::null(),
                    src_set: c.src_set.raw,
                    src_binding: c.src_binding as u32,
                    src_array_element: c.src_array_offset as u32,
                    dst_set: c.dst_set.raw,
                    dst_binding: c.dst_binding as u32,
                    dst_array_element: c.dst_array_offset as u32,
                    descriptor_count: c.count as u32,
                }
            })
            .collect::<Vec<_>>();

        self.raw.0.update_descriptor_sets(&[], &copies);
    }

    unsafe fn map_memory<R>(&self, memory: &n::Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        let (offset, size) = conv::map_range_arg(&range);
        let result = self
            .raw
            .0
            .map_memory(memory.raw, offset, size, vk::MemoryMapFlags::empty());

        match result {
            Ok(ptr) => Ok(ptr as *mut _),
            Err(vk::Result::ERROR_MEMORY_MAP_FAILED) => Err(mapping::Error::MappingFailed),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn unmap_memory(&self, memory: &n::Memory) {
        self.raw.0.unmap_memory(memory.raw)
    }

    unsafe fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I) -> Result<(), d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let ranges = conv::map_memory_ranges(ranges);
        let result = self.raw.0.flush_mapped_memory_ranges(&ranges);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I, R>(
        &self,
        ranges: I,
    ) -> Result<(), d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let ranges = conv::map_memory_ranges(ranges);
        let result = self.raw.0.invalidate_mapped_memory_ranges(&ranges);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    fn create_semaphore(&self) -> Result<n::Semaphore, d::OutOfMemory> {
        let info = vk::SemaphoreCreateInfo {
            s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::SemaphoreCreateFlags::empty(),
        };

        let result = unsafe { self.raw.0.create_semaphore(&info, None) };

        match result {
            Ok(semaphore) => Ok(n::Semaphore(semaphore)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    fn create_fence(&self, signaled: bool) -> Result<n::Fence, d::OutOfMemory> {
        let info = vk::FenceCreateInfo {
            s_type: vk::StructureType::FENCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: if signaled {
                vk::FenceCreateFlags::SIGNALED
            } else {
                vk::FenceCreateFlags::empty()
            },
        };

        let result = unsafe { self.raw.0.create_fence(&info, None) };

        match result {
            Ok(fence) => Ok(n::Fence(fence)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn reset_fences<I>(&self, fences: I) -> Result<(), d::OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let fences = fences
            .into_iter()
            .map(|fence| fence.borrow().0)
            .collect::<Vec<_>>();
        let result = self.raw.0.reset_fences(&fences);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn wait_for_fences<I>(
        &self,
        fences: I,
        wait: d::WaitFor,
        timeout_ns: u64,
    ) -> Result<bool, d::OomOrDeviceLost>
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let fences = fences
            .into_iter()
            .map(|fence| fence.borrow().0)
            .collect::<Vec<_>>();
        let all = match wait {
            d::WaitFor::Any => false,
            d::WaitFor::All => true,
        };
        let result = self.raw.0.wait_for_fences(&fences, all, timeout_ns);
        match result {
            Ok(()) => Ok(true),
            Err(vk::Result::TIMEOUT) => Ok(false),
            Err(vk::Result::ERROR_DEVICE_LOST) => Err(d::DeviceLost.into()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn get_fence_status(&self, fence: &n::Fence) -> Result<bool, d::DeviceLost> {
        let result = self.raw.0.get_fence_status(fence.0);
        match result {
            Ok(()) => Ok(true),
            Err(vk::Result::NOT_READY) => Ok(false),
            Err(vk::Result::ERROR_DEVICE_LOST) => Err(d::DeviceLost),
            _ => unreachable!(),
        }
    }

    unsafe fn free_memory(&self, memory: n::Memory) {
        self.raw.0.free_memory(memory.raw, None);
    }

    unsafe fn create_query_pool(
        &self,
        ty: query::Type,
        query_count: query::Id,
    ) -> Result<n::QueryPool, query::CreationError> {
        let (query_type, pipeline_statistics) = match ty {
            query::Type::Occlusion => (
                vk::QueryType::OCCLUSION,
                vk::QueryPipelineStatisticFlags::empty(),
            ),
            query::Type::PipelineStatistics(statistics) => (
                vk::QueryType::PIPELINE_STATISTICS,
                conv::map_pipeline_statistics(statistics),
            ),
            query::Type::Timestamp => (
                vk::QueryType::TIMESTAMP,
                vk::QueryPipelineStatisticFlags::empty(),
            ),
        };

        let info = vk::QueryPoolCreateInfo {
            s_type: vk::StructureType::QUERY_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::QueryPoolCreateFlags::empty(),
            query_type,
            query_count,
            pipeline_statistics,
        };

        let result = self.raw.0.create_query_pool(&info, None);

        match result {
            Ok(pool) => Ok(n::QueryPool(pool)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                Err(d::OutOfMemory::OutOfHostMemory.into())
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                Err(d::OutOfMemory::OutOfDeviceMemory.into())
            }
            _ => unreachable!(),
        }
    }

    unsafe fn get_query_pool_results(
        &self,
        pool: &n::QueryPool,
        queries: Range<query::Id>,
        data: &mut [u8],
        stride: buffer::Offset,
        flags: query::ResultFlags,
    ) -> Result<bool, d::OomOrDeviceLost> {
        let result = self.raw.0.fp_v1_0().get_query_pool_results(
            self.raw.0.handle(),
            pool.0,
            queries.start,
            queries.end - queries.start,
            data.len(),
            data.as_mut_ptr() as *mut _,
            stride,
            conv::map_query_result_flags(flags),
        );

        match result {
            vk::Result::SUCCESS => Ok(true),
            vk::Result::NOT_READY => Ok(false),
            vk::Result::ERROR_DEVICE_LOST => Err(d::DeviceLost.into()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::OutOfHostMemory.into()),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::OutOfDeviceMemory.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn create_swapchain(
        &self,
        surface: &mut w::Surface,
        config: SwapchainConfig,
        provided_old_swapchain: Option<w::Swapchain>,
    ) -> Result<(w::Swapchain, Backbuffer<B>), hal::window::CreationError> {
        let functor = khr::Swapchain::new(&surface.raw.instance.0, &self.raw.0);

        let old_swapchain = match provided_old_swapchain {
            Some(osc) => osc.raw,
            None => vk::SwapchainKHR::null(),
        };

        surface.width = config.extent.width;
        surface.height = config.extent.height;

        let info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: surface.raw.handle,
            min_image_count: config.image_count,
            image_format: conv::map_format(config.format),
            image_color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            image_extent: vk::Extent2D {
                width: surface.width,
                height: surface.height,
            },
            image_array_layers: 1,
            image_usage: conv::map_image_usage(config.image_usage),
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: vk::SurfaceTransformFlagsKHR::IDENTITY,
            composite_alpha: conv::map_composite_alpha(config.composite_alpha),
            present_mode: conv::map_present_mode(config.present_mode),
            clipped: 1,
            old_swapchain,
        };

        let result = functor.create_swapchain(&info, None);

        if old_swapchain != vk::SwapchainKHR::null() {
            functor.destroy_swapchain(old_swapchain, None)
        }

        let swapchain_raw = match result {
            Ok(swapchain_raw) => swapchain_raw,
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                return Err(d::OutOfMemory::OutOfHostMemory.into());
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                return Err(d::OutOfMemory::OutOfDeviceMemory.into());
            }
            Err(vk::Result::ERROR_DEVICE_LOST) => return Err(d::DeviceLost.into()),
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => return Err(d::SurfaceLost.into()),
            Err(vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR) => return Err(d::WindowInUse.into()),
            _ => unreachable!("Unexpected result - driver bug? {:?}", result),
        };

        let result = functor.get_swapchain_images(swapchain_raw);

        let backbuffer_images = match result {
            Ok(backbuffer_images) => backbuffer_images,
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                return Err(d::OutOfMemory::OutOfHostMemory.into());
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                return Err(d::OutOfMemory::OutOfDeviceMemory.into());
            }
            _ => unreachable!(),
        };

        let swapchain = w::Swapchain {
            raw: swapchain_raw,
            functor,
        };

        let images = backbuffer_images
            .into_iter()
            .map(|image| n::Image {
                raw: image,
                ty: vk::ImageType::TYPE_2D,
                flags: vk::ImageCreateFlags::empty(),
                extent: vk::Extent3D {
                    width: surface.width,
                    height: surface.height,
                    depth: 1,
                },
            })
            .collect();

        Ok((swapchain, Backbuffer::Images(images)))
    }

    unsafe fn destroy_swapchain(&self, swapchain: w::Swapchain) {
        swapchain.functor.destroy_swapchain(swapchain.raw, None);
    }

    unsafe fn destroy_query_pool(&self, pool: n::QueryPool) {
        self.raw.0.destroy_query_pool(pool.0, None);
    }

    unsafe fn destroy_shader_module(&self, module: n::ShaderModule) {
        self.raw.0.destroy_shader_module(module.raw, None);
    }

    unsafe fn destroy_render_pass(&self, rp: n::RenderPass) {
        self.raw.0.destroy_render_pass(rp.raw, None);
    }

    unsafe fn destroy_pipeline_layout(&self, pl: n::PipelineLayout) {
        self.raw.0.destroy_pipeline_layout(pl.raw, None);
    }

    unsafe fn destroy_graphics_pipeline(&self, pipeline: n::GraphicsPipeline) {
        self.raw.0.destroy_pipeline(pipeline.0, None);
    }

    unsafe fn destroy_compute_pipeline(&self, pipeline: n::ComputePipeline) {
        self.raw.0.destroy_pipeline(pipeline.0, None);
    }

    unsafe fn destroy_framebuffer(&self, fb: n::Framebuffer) {
        self.raw.0.destroy_framebuffer(fb.raw, None);
    }

    unsafe fn destroy_buffer(&self, buffer: n::Buffer) {
        self.raw.0.destroy_buffer(buffer.raw, None);
    }

    unsafe fn destroy_buffer_view(&self, view: n::BufferView) {
        self.raw.0.destroy_buffer_view(view.raw, None);
    }

    unsafe fn destroy_image(&self, image: n::Image) {
        self.raw.0.destroy_image(image.raw, None);
    }

    unsafe fn destroy_image_view(&self, view: n::ImageView) {
        self.raw.0.destroy_image_view(view.view, None);
    }

    unsafe fn destroy_sampler(&self, sampler: n::Sampler) {
        self.raw.0.destroy_sampler(sampler.0, None);
    }

    unsafe fn destroy_descriptor_pool(&self, pool: n::DescriptorPool) {
        self.raw.0.destroy_descriptor_pool(pool.raw, None);
    }

    unsafe fn destroy_descriptor_set_layout(&self, layout: n::DescriptorSetLayout) {
        self.raw.0.destroy_descriptor_set_layout(layout.raw, None);
    }

    unsafe fn destroy_fence(&self, fence: n::Fence) {
        self.raw.0.destroy_fence(fence.0, None);
    }

    unsafe fn destroy_semaphore(&self, semaphore: n::Semaphore) {
        self.raw.0.destroy_semaphore(semaphore.0, None);
    }

    fn wait_idle(&self) -> Result<(), HostExecutionError> {
        unsafe {
            self.raw
                .0
                .device_wait_idle()
                .map_err(From::from)
                .map_err(From::<result::Error>::from)
        }
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send + Sync>() {}
    foo::<Device>()
}
