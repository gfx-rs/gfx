use arrayvec::ArrayVec;
use ash::{extensions::khr, version::DeviceV1_0, vk};
use inplace_it::inplace_or_alloc_from_iter;
use smallvec::SmallVec;

use hal::{
    memory,
    memory::{Requirements, Segment},
    pool::CommandPoolCreateFlags,
    pso::VertexInputRate,
    window::SwapchainConfig,
    {buffer, device as d, format, image, pass, pso, query, queue}, {Features, MemoryTypeId},
};

use std::{ffi::CString, marker::PhantomData, mem, ops::Range, ptr, sync::Arc};

use crate::{command as cmd, conv, native as n, pool::RawCommandPool, window as w, Backend as B};
use ash::vk::Handle;

#[derive(Debug, Default)]
struct GraphicsPipelineInfoBuf<'a> {
    // 10 is the max amount of dynamic states
    dynamic_states: ArrayVec<[vk::DynamicState; 10]>,

    // 5 is the amount of stages
    c_strings: ArrayVec<[CString; 5]>,
    stages: ArrayVec<[vk::PipelineShaderStageCreateInfo; 5]>,
    specializations: ArrayVec<[vk::SpecializationInfo; 5]>,
    specialization_entries: ArrayVec<[SmallVec<[vk::SpecializationMapEntry; 4]>; 5]>,

    vertex_bindings: Vec<vk::VertexInputBindingDescription>,
    vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    blend_states: Vec<vk::PipelineColorBlendAttachmentState>,

    sample_mask: [u32; 2],
    vertex_input_state: vk::PipelineVertexInputStateCreateInfo,
    input_assembly_state: vk::PipelineInputAssemblyStateCreateInfo,
    tessellation_state: Option<vk::PipelineTessellationStateCreateInfo>,
    viewport_state: vk::PipelineViewportStateCreateInfo,
    rasterization_state: vk::PipelineRasterizationStateCreateInfo,
    rasterization_conservative_state: vk::PipelineRasterizationConservativeStateCreateInfoEXT, // May be unused or may be pointed to by rasterization_state
    multisample_state: vk::PipelineMultisampleStateCreateInfo,
    depth_stencil_state: vk::PipelineDepthStencilStateCreateInfo,
    color_blend_state: vk::PipelineColorBlendStateCreateInfo,
    pipeline_dynamic_state: vk::PipelineDynamicStateCreateInfo,
    viewports: [vk::Viewport; 1],
    scissors: [vk::Rect2D; 1],

    lifetime: PhantomData<&'a vk::Pipeline>,
}
impl<'a> GraphicsPipelineInfoBuf<'a> {
    unsafe fn add_stage(&mut self, stage: vk::ShaderStageFlags, source: &pso::EntryPoint<'a, B>) {
        let string = CString::new(source.entry).unwrap();
        self.c_strings.push(string);
        let name = self.c_strings.last().unwrap().as_c_str();

        self.specialization_entries.push(
            source
                .specialization
                .constants
                .iter()
                .map(|c| vk::SpecializationMapEntry {
                    constant_id: c.id,
                    offset: c.range.start as _,
                    size: (c.range.end - c.range.start) as _,
                })
                .collect(),
        );
        let map_entries = self.specialization_entries.last().unwrap();

        self.specializations.push(vk::SpecializationInfo {
            map_entry_count: map_entries.len() as _,
            p_map_entries: map_entries.as_ptr(),
            data_size: source.specialization.data.len() as _,
            p_data: source.specialization.data.as_ptr() as _,
        });

        self.stages.push(
            vk::PipelineShaderStageCreateInfo::builder()
                .flags(vk::PipelineShaderStageCreateFlags::empty())
                .stage(stage)
                .module(source.module.raw)
                .name(name)
                .specialization_info(self.specializations.last().unwrap())
                .build(),
        )
    }

    unsafe fn new(desc: &pso::GraphicsPipelineDesc<'a, B>, device: &super::RawDevice) -> Self {
        let mut this = Self::default();

        match desc.primitive_assembler {
            pso::PrimitiveAssemblerDesc::Vertex {
                ref buffers,
                ref attributes,
                ref input_assembler,
                ref vertex,
                ref tessellation,
                ref geometry,
            } => {
                // Vertex stage
                // vertex shader is required
                this.add_stage(vk::ShaderStageFlags::VERTEX, vertex);

                // Geometry stage
                if let Some(ref entry) = geometry {
                    this.add_stage(vk::ShaderStageFlags::GEOMETRY, entry);
                }
                // Tessellation stage
                if let Some(ts) = tessellation {
                    this.add_stage(vk::ShaderStageFlags::TESSELLATION_CONTROL, &ts.0);
                    this.add_stage(vk::ShaderStageFlags::TESSELLATION_EVALUATION, &ts.1);
                }
                this.vertex_bindings = buffers.iter().map(|vbuf| {
                    vk::VertexInputBindingDescription {
                        binding: vbuf.binding,
                        stride: vbuf.stride as u32,
                        input_rate: match vbuf.rate {
                            VertexInputRate::Vertex => vk::VertexInputRate::VERTEX,
                            VertexInputRate::Instance(divisor) => {
                                debug_assert_eq!(divisor, 1, "Custom vertex rate divisors not supported in Vulkan backend without extension");
                                vk::VertexInputRate::INSTANCE
                            },
                        },
                    }
                }).collect();

                this.vertex_attributes = attributes
                    .iter()
                    .map(|attr| vk::VertexInputAttributeDescription {
                        location: attr.location as u32,
                        binding: attr.binding as u32,
                        format: conv::map_format(attr.element.format),
                        offset: attr.element.offset as u32,
                    })
                    .collect();

                this.vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
                    .flags(vk::PipelineVertexInputStateCreateFlags::empty())
                    .vertex_binding_descriptions(&this.vertex_bindings)
                    .vertex_attribute_descriptions(&this.vertex_attributes)
                    .build();

                this.input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
                    .flags(vk::PipelineInputAssemblyStateCreateFlags::empty())
                    .topology(conv::map_topology(&input_assembler))
                    .primitive_restart_enable(input_assembler.restart_index.is_some())
                    .build();
            }
            pso::PrimitiveAssemblerDesc::Mesh { ref task, ref mesh } => {
                this.vertex_bindings = Vec::new();
                this.vertex_attributes = Vec::new();
                this.vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default();
                this.input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default();

                // Task stage, optional
                if let Some(ref entry) = task {
                    this.add_stage(vk::ShaderStageFlags::TASK_NV, entry);
                }

                // Mesh stage
                this.add_stage(vk::ShaderStageFlags::MESH_NV, mesh);
            }
        };

        // Pixel stage
        if let Some(ref entry) = desc.fragment {
            this.add_stage(vk::ShaderStageFlags::FRAGMENT, entry);
        }

        let depth_bias = match desc.rasterizer.depth_bias {
            Some(pso::State::Static(db)) => db,
            Some(pso::State::Dynamic) => {
                this.dynamic_states.push(vk::DynamicState::DEPTH_BIAS);
                pso::DepthBias::default()
            }
            None => pso::DepthBias::default(),
        };

        let polygon_mode = match desc.rasterizer.polygon_mode {
            pso::PolygonMode::Point => vk::PolygonMode::POINT,
            pso::PolygonMode::Line => vk::PolygonMode::LINE,
            pso::PolygonMode::Fill => vk::PolygonMode::FILL,
        };

        let line_width = match desc.rasterizer.line_width {
            pso::State::Static(w) => w,
            pso::State::Dynamic => {
                this.dynamic_states.push(vk::DynamicState::LINE_WIDTH);
                1.0
            }
        };

        this.rasterization_conservative_state =
            vk::PipelineRasterizationConservativeStateCreateInfoEXT::builder()
                .conservative_rasterization_mode(match desc.rasterizer.conservative {
                    false => vk::ConservativeRasterizationModeEXT::DISABLED,
                    true => vk::ConservativeRasterizationModeEXT::OVERESTIMATE,
                })
                .build();

        this.rasterization_state = {
            let mut rasterization_state_builder =
                vk::PipelineRasterizationStateCreateInfo::builder()
                    .flags(vk::PipelineRasterizationStateCreateFlags::empty())
                    .depth_clamp_enable(if desc.rasterizer.depth_clamping {
                        if device.features.contains(Features::DEPTH_CLAMP) {
                            true
                        } else {
                            warn!("Depth clamping was requested on a device with disabled feature");
                            false
                        }
                    } else {
                        false
                    })
                    .rasterizer_discard_enable(
                        desc.fragment.is_none()
                            && desc.depth_stencil.depth.is_none()
                            && desc.depth_stencil.stencil.is_none(),
                    )
                    .polygon_mode(polygon_mode)
                    .cull_mode(conv::map_cull_face(desc.rasterizer.cull_face))
                    .front_face(conv::map_front_face(desc.rasterizer.front_face))
                    .depth_bias_enable(desc.rasterizer.depth_bias.is_some())
                    .depth_bias_constant_factor(depth_bias.const_factor)
                    .depth_bias_clamp(depth_bias.clamp)
                    .depth_bias_slope_factor(depth_bias.slope_factor)
                    .line_width(line_width);
            if desc.rasterizer.conservative {
                rasterization_state_builder = rasterization_state_builder
                    .push_next(&mut this.rasterization_conservative_state);
            }

            rasterization_state_builder.build()
        };

        this.tessellation_state = {
            if let pso::PrimitiveAssemblerDesc::Vertex {
                input_assembler, ..
            } = &desc.primitive_assembler
            {
                if let pso::Primitive::PatchList(patch_control_points) = input_assembler.primitive {
                    Some(
                        vk::PipelineTessellationStateCreateInfo::builder()
                            .flags(vk::PipelineTessellationStateCreateFlags::empty())
                            .patch_control_points(patch_control_points as _)
                            .build(),
                    )
                } else {
                    None
                }
            } else {
                None
            }
        };

        this.viewport_state = {
            //Note: without `multiViewport` feature, there has to be
            // the count of 1 for both viewports and scissors, even
            // though the actual pointers are ignored.
            match desc.baked_states.scissor {
                Some(ref rect) => {
                    this.scissors = [conv::map_rect(rect)];
                }
                None => {
                    this.dynamic_states.push(vk::DynamicState::SCISSOR);
                }
            }
            match desc.baked_states.viewport {
                Some(ref vp) => {
                    this.viewports = [device.map_viewport(vp)];
                }
                None => {
                    this.dynamic_states.push(vk::DynamicState::VIEWPORT);
                }
            }
            vk::PipelineViewportStateCreateInfo::builder()
                .flags(vk::PipelineViewportStateCreateFlags::empty())
                .scissors(&this.scissors)
                .viewports(&this.viewports)
                .build()
        };

        this.multisample_state = match desc.multisampling {
            Some(ref ms) => {
                this.sample_mask = [
                    (ms.sample_mask & 0xFFFFFFFF) as u32,
                    ((ms.sample_mask >> 32) & 0xFFFFFFFF) as u32,
                ];
                vk::PipelineMultisampleStateCreateInfo::builder()
                    .flags(vk::PipelineMultisampleStateCreateFlags::empty())
                    .rasterization_samples(conv::map_sample_count_flags(ms.rasterization_samples))
                    .sample_shading_enable(ms.sample_shading.is_some())
                    .min_sample_shading(ms.sample_shading.unwrap_or(0.0))
                    .sample_mask(&this.sample_mask)
                    .alpha_to_coverage_enable(ms.alpha_coverage)
                    .alpha_to_one_enable(ms.alpha_to_one)
                    .build()
            }
            None => vk::PipelineMultisampleStateCreateInfo::builder()
                .flags(vk::PipelineMultisampleStateCreateFlags::empty())
                .rasterization_samples(vk::SampleCountFlags::TYPE_1)
                .build(),
        };

        let depth_stencil = desc.depth_stencil;
        let (depth_test_enable, depth_write_enable, depth_compare_op) = match depth_stencil.depth {
            Some(ref depth) => (true, depth.write as _, conv::map_comparison(depth.fun)),
            None => (false, false, vk::CompareOp::NEVER),
        };
        let (stencil_test_enable, front, back) = match depth_stencil.stencil {
            Some(ref stencil) => {
                let mut front = conv::map_stencil_side(&stencil.faces.front);
                let mut back = conv::map_stencil_side(&stencil.faces.back);
                match stencil.read_masks {
                    pso::State::Static(ref sides) => {
                        front.compare_mask = sides.front;
                        back.compare_mask = sides.back;
                    }
                    pso::State::Dynamic => {
                        this.dynamic_states
                            .push(vk::DynamicState::STENCIL_COMPARE_MASK);
                    }
                }
                match stencil.write_masks {
                    pso::State::Static(ref sides) => {
                        front.write_mask = sides.front;
                        back.write_mask = sides.back;
                    }
                    pso::State::Dynamic => {
                        this.dynamic_states
                            .push(vk::DynamicState::STENCIL_WRITE_MASK);
                    }
                }
                match stencil.reference_values {
                    pso::State::Static(ref sides) => {
                        front.reference = sides.front;
                        back.reference = sides.back;
                    }
                    pso::State::Dynamic => {
                        this.dynamic_states
                            .push(vk::DynamicState::STENCIL_REFERENCE);
                    }
                }
                (true, front, back)
            }
            None => mem::zeroed(),
        };
        let (min_depth_bounds, max_depth_bounds) = match desc.baked_states.depth_bounds {
            Some(ref range) => (range.start, range.end),
            None => {
                this.dynamic_states.push(vk::DynamicState::DEPTH_BOUNDS);
                (0.0, 1.0)
            }
        };

        this.depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
            .flags(vk::PipelineDepthStencilStateCreateFlags::empty())
            .depth_test_enable(depth_test_enable)
            .depth_write_enable(depth_write_enable)
            .depth_compare_op(depth_compare_op)
            .depth_bounds_test_enable(depth_stencil.depth_bounds)
            .stencil_test_enable(stencil_test_enable)
            .front(front)
            .back(back)
            .min_depth_bounds(min_depth_bounds)
            .max_depth_bounds(max_depth_bounds)
            .build();

        this.blend_states = desc
            .blender
            .targets
            .iter()
            .map(|color_desc| {
                let color_write_mask =
                    vk::ColorComponentFlags::from_raw(color_desc.mask.bits() as _);
                match color_desc.blend {
                    Some(ref bs) => {
                        let (color_blend_op, src_color_blend_factor, dst_color_blend_factor) =
                            conv::map_blend_op(bs.color);
                        let (alpha_blend_op, src_alpha_blend_factor, dst_alpha_blend_factor) =
                            conv::map_blend_op(bs.alpha);
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
                    None => vk::PipelineColorBlendAttachmentState {
                        color_write_mask,
                        ..mem::zeroed()
                    },
                }
            })
            .collect();

        this.color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .flags(vk::PipelineColorBlendStateCreateFlags::empty())
            .logic_op_enable(false) // TODO
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&this.blend_states) // TODO:
            .blend_constants(match desc.baked_states.blend_constants {
                Some(value) => value,
                None => {
                    this.dynamic_states.push(vk::DynamicState::BLEND_CONSTANTS);
                    [0.0; 4]
                }
            })
            .build();

        this.pipeline_dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
            .flags(vk::PipelineDynamicStateCreateFlags::empty())
            .dynamic_states(&this.dynamic_states)
            .build();

        this
    }
}

#[derive(Debug, Default)]
struct ComputePipelineInfoBuf<'a> {
    c_string: CString,
    specialization: vk::SpecializationInfo,
    entries: SmallVec<[vk::SpecializationMapEntry; 4]>,
    lifetime: PhantomData<&'a vk::Pipeline>,
}
impl<'a> ComputePipelineInfoBuf<'a> {
    unsafe fn new(desc: &pso::ComputePipelineDesc<'a, B>) -> Self {
        let mut this = Self::default();
        this.c_string = CString::new(desc.shader.entry).unwrap();
        this.entries = desc
            .shader
            .specialization
            .constants
            .iter()
            .map(|c| vk::SpecializationMapEntry {
                constant_id: c.id,
                offset: c.range.start as _,
                size: (c.range.end - c.range.start) as _,
            })
            .collect();
        this.specialization = vk::SpecializationInfo {
            map_entry_count: this.entries.len() as _,
            p_map_entries: this.entries.as_ptr(),
            data_size: desc.shader.specialization.data.len() as _,
            p_data: desc.shader.specialization.data.as_ptr() as _,
        };
        this
    }
}

#[derive(Debug, Default)]
struct RayTracingPipelineInfoBuf<'a> {
    shader_groups: Vec<ComputePipelineInfoBuf<'a>>,
}
impl<'a> RayTracingPipelineInfoBuf<'a> {
    unsafe fn new(desc: &pso::RayTracingPipelineDesc<'a, B>) -> Self {
        let mut this = Self::default();
        this.shader_groups = desc
            .stages
            .iter()
            .map(|stage_desc| {
                let mut buf = ComputePipelineInfoBuf::default();
                buf.c_string = CString::new(stage_desc.entry_point.entry).unwrap();
                buf.entries = stage_desc
                    .entry_point
                    .specialization
                    .constants
                    .iter()
                    .map(|c| vk::SpecializationMapEntry {
                        constant_id: c.id,
                        offset: c.range.start as _,
                        size: (c.range.end - c.range.start) as _,
                    })
                    .collect();
                buf.specialization = vk::SpecializationInfo {
                    map_entry_count: buf.entries.len() as _,
                    p_map_entries: buf.entries.as_ptr(),
                    data_size: stage_desc.entry_point.specialization.data.len() as _,
                    p_data: stage_desc.entry_point.specialization.data.as_ptr() as _,
                };
                buf
            })
            .collect();
        this
    }
}

impl d::Device<B> for super::Device {
    unsafe fn allocate_memory(
        &self,
        mem_type: MemoryTypeId,
        size: u64,
    ) -> Result<n::Memory, d::AllocationError> {
        let mut flags_info = vk::MemoryAllocateFlagsInfo::builder().flags(
            // TODO needs Vulkan 1.2? Also either expose in hal or infer from usage?
            vk::MemoryAllocateFlags::DEVICE_ADDRESS,
        );

        let info = vk::MemoryAllocateInfo::builder()
            .allocation_size(size)
            .memory_type_index(self.get_ash_memory_type_index(mem_type))
            .push_next(&mut flags_info);

        let result = self.shared.raw.allocate_memory(&info, None);

        match result {
            Ok(memory) => Ok(n::Memory { raw: memory }),
            Err(vk::Result::ERROR_TOO_MANY_OBJECTS) => Err(d::AllocationError::TooManyObjects),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
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

        let info = vk::CommandPoolCreateInfo::builder()
            .flags(flags)
            .queue_family_index(family.0 as _);

        let result = self.shared.raw.create_command_pool(&info, None);

        match result {
            Ok(pool) => Ok(RawCommandPool {
                raw: pool,
                device: self.shared.clone(),
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn destroy_command_pool(&self, pool: RawCommandPool) {
        self.shared.raw.destroy_command_pool(pool.raw, None);
    }

    unsafe fn create_render_pass<'a, Ia, Is, Id>(
        &self,
        attachments: Ia,
        subpasses: Is,
        dependencies: Id,
    ) -> Result<n::RenderPass, d::OutOfMemory>
    where
        Ia: Iterator<Item = pass::Attachment>,
        Is: Iterator<Item = pass::SubpassDesc<'a>>,
        Id: Iterator<Item = pass::SubpassDependency>,
    {
        let attachments_iter = attachments.map(|attachment| vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(), // TODO: may even alias!
            format: attachment
                .format
                .map_or(vk::Format::UNDEFINED, conv::map_format),
            samples: conv::map_sample_count_flags(attachment.samples),
            load_op: conv::map_attachment_load_op(attachment.ops.load),
            store_op: conv::map_attachment_store_op(attachment.ops.store),
            stencil_load_op: conv::map_attachment_load_op(attachment.stencil_ops.load),
            stencil_store_op: conv::map_attachment_store_op(attachment.stencil_ops.store),
            initial_layout: conv::map_image_layout(attachment.layouts.start),
            final_layout: conv::map_image_layout(attachment.layouts.end),
        });

        let dependencies_iter = dependencies.map(|sdep|
            // TODO: checks
            vk::SubpassDependency {
                src_subpass: sdep
                    .passes
                    .start
                    .map_or(vk::SUBPASS_EXTERNAL, |id| id as u32),
                dst_subpass: sdep.passes.end.map_or(vk::SUBPASS_EXTERNAL, |id| id as u32),
                src_stage_mask: conv::map_pipeline_stage(sdep.stages.start),
                dst_stage_mask: conv::map_pipeline_stage(sdep.stages.end),
                src_access_mask: conv::map_image_access(sdep.accesses.start),
                dst_access_mask: conv::map_image_access(sdep.accesses.end),
                dependency_flags: mem::transmute(sdep.flags),
            });

        let result = inplace_or_alloc_from_iter(attachments_iter, |attachments| {
            let attachment_refs = subpasses
                .map(|subpass| {
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
                    let resolves = subpass.resolves.iter().map(make_ref).collect::<Box<[_]>>();

                    (colors, depth_stencil, inputs, preserves, resolves)
                })
                .collect::<Box<[_]>>();

            let subpasses = attachment_refs
                .iter()
                .map(|(colors, depth_stencil, inputs, preserves, resolves)| {
                    vk::SubpassDescription {
                        flags: vk::SubpassDescriptionFlags::empty(),
                        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
                        input_attachment_count: inputs.len() as u32,
                        p_input_attachments: inputs.as_ptr(),
                        color_attachment_count: colors.len() as u32,
                        p_color_attachments: colors.as_ptr(),
                        p_resolve_attachments: if resolves.is_empty() {
                            ptr::null()
                        } else {
                            resolves.as_ptr()
                        },
                        p_depth_stencil_attachment: match depth_stencil {
                            Some(ref aref) => aref as *const _,
                            None => ptr::null(),
                        },
                        preserve_attachment_count: preserves.len() as u32,
                        p_preserve_attachments: preserves.as_ptr(),
                    }
                })
                .collect::<Box<[_]>>();

            inplace_or_alloc_from_iter(dependencies_iter, |dependencies| {
                let info = vk::RenderPassCreateInfo::builder()
                    .flags(vk::RenderPassCreateFlags::empty())
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&dependencies);

                self.shared
                    .raw
                    .create_render_pass(&info, None)
                    .map(|raw| n::RenderPass {
                        raw,
                        attachment_count: attachments.len(),
                    })
            })
        });

        match result {
            Ok(renderpass) => Ok(renderpass),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn create_pipeline_layout<'a, Is, Ic>(
        &self,
        set_layouts: Is,
        push_constant_ranges: Ic,
    ) -> Result<n::PipelineLayout, d::OutOfMemory>
    where
        Is: Iterator<Item = &'a n::DescriptorSetLayout>,
        Ic: Iterator<Item = (pso::ShaderStageFlags, Range<u32>)>,
    {
        let vk_set_layouts_iter = set_layouts.map(|set| set.raw);

        let push_constant_ranges_iter =
            push_constant_ranges.map(|(s, ref r)| vk::PushConstantRange {
                stage_flags: conv::map_stage_flags(s),
                offset: r.start,
                size: r.end - r.start,
            });

        let result = inplace_or_alloc_from_iter(vk_set_layouts_iter, |set_layouts| {
            // TODO: set_layouts doesnt implement fmt::Debug, submit PR?
            // debug!("create_pipeline_layout {:?}", set_layouts);

            inplace_or_alloc_from_iter(push_constant_ranges_iter, |push_constant_ranges| {
                let info = vk::PipelineLayoutCreateInfo::builder()
                    .flags(vk::PipelineLayoutCreateFlags::empty())
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&push_constant_ranges);

                self.shared.raw.create_pipeline_layout(&info, None)
            })
        });

        match result {
            Ok(raw) => Ok(n::PipelineLayout { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn create_pipeline_cache(
        &self,
        data: Option<&[u8]>,
    ) -> Result<n::PipelineCache, d::OutOfMemory> {
        let info =
            vk::PipelineCacheCreateInfo::builder().flags(vk::PipelineCacheCreateFlags::empty());
        let info = if let Some(d) = data {
            info.initial_data(d)
        } else {
            info
        };

        let result = self.shared.raw.create_pipeline_cache(&info, None);

        match result {
            Ok(raw) => Ok(n::PipelineCache { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn get_pipeline_cache_data(
        &self,
        cache: &n::PipelineCache,
    ) -> Result<Vec<u8>, d::OutOfMemory> {
        let result = self.shared.raw.get_pipeline_cache_data(cache.raw);

        match result {
            Ok(data) => Ok(data),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn destroy_pipeline_cache(&self, cache: n::PipelineCache) {
        self.shared.raw.destroy_pipeline_cache(cache.raw, None);
    }

    unsafe fn merge_pipeline_caches<'a, I>(
        &self,
        target: &mut n::PipelineCache,
        sources: I,
    ) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = &'a n::PipelineCache>,
    {
        let caches_iter = sources.map(|s| s.raw);

        let result = inplace_or_alloc_from_iter(caches_iter, |caches| {
            //TODO: https://github.com/MaikKlein/ash/issues/357
            self.shared.raw.fp_v1_0().merge_pipeline_caches(
                self.shared.raw.handle(),
                target.raw,
                caches.len() as u32,
                caches.as_ptr(),
            )
        });

        match result {
            vk::Result::SUCCESS => Ok(()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::Host),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        cache: Option<&n::PipelineCache>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        debug!("create_graphics_pipeline {:?}", desc);
        let buf = GraphicsPipelineInfoBuf::new(desc, &self.shared);

        let info = {
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

            let builder = vk::GraphicsPipelineCreateInfo::builder()
                .flags(flags)
                .stages(&buf.stages)
                .vertex_input_state(&buf.vertex_input_state)
                .input_assembly_state(&buf.input_assembly_state)
                .rasterization_state(&buf.rasterization_state);
            let builder = match buf.tessellation_state.as_ref() {
                Some(t) => builder.tessellation_state(t),
                None => builder,
            };
            builder
                .viewport_state(&buf.viewport_state)
                .multisample_state(&buf.multisample_state)
                .depth_stencil_state(&buf.depth_stencil_state)
                .color_blend_state(&buf.color_blend_state)
                .dynamic_state(&buf.pipeline_dynamic_state)
                .layout(desc.layout.raw)
                .render_pass(desc.subpass.main_pass.raw)
                .subpass(desc.subpass.index as _)
                .base_pipeline_handle(base_handle)
                .base_pipeline_index(base_index)
        };

        let mut pipeline = vk::Pipeline::null();

        match self.shared.raw.fp_v1_0().create_graphics_pipelines(
            self.shared.raw.handle(),
            cache.map_or(vk::PipelineCache::null(), |cache| cache.raw),
            1,
            &*info,
            ptr::null(),
            &mut pipeline,
        ) {
            vk::Result::SUCCESS => Ok(n::GraphicsPipeline(pipeline)),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::Host.into()),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::Device.into()),
            _ => Err(pso::CreationError::Other),
        }
    }

    unsafe fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        cache: Option<&n::PipelineCache>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        debug!("create_graphics_pipeline {:?}", desc);
        let buf = ComputePipelineInfoBuf::new(desc);

        let info = {
            let stage = vk::PipelineShaderStageCreateInfo::builder()
                .flags(vk::PipelineShaderStageCreateFlags::empty())
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(desc.shader.module.raw)
                .name(buf.c_string.as_c_str())
                .specialization_info(&buf.specialization);

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

            vk::ComputePipelineCreateInfo::builder()
                .flags(flags)
                .stage(*stage)
                .layout(desc.layout.raw)
                .base_pipeline_handle(base_handle)
                .base_pipeline_index(base_index)
                .build()
        };

        let mut pipeline = vk::Pipeline::null();

        match self.shared.raw.fp_v1_0().create_compute_pipelines(
            self.shared.raw.handle(),
            cache.map_or(vk::PipelineCache::null(), |cache| cache.raw),
            1,
            &info,
            ptr::null(),
            &mut pipeline,
        ) {
            vk::Result::SUCCESS => {
                if let Some(name) = desc.label {
                    self.shared
                        .set_object_name(vk::ObjectType::PIPELINE, pipeline, name);
                }
                Ok(n::ComputePipeline(pipeline))
            }
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::Host.into()),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::Device.into()),
            _ => Err(pso::CreationError::Other),
        }
    }

    unsafe fn create_ray_tracing_pipeline<'a>(
        &self,
        desc: &pso::RayTracingPipelineDesc<'a, B>,
        cache: Option<&n::PipelineCache>,
    ) -> Result<n::RayTracingPipeline, pso::CreationError> {
        let buf = RayTracingPipelineInfoBuf::new(desc);

        let stages = desc
            .stages
            .iter()
            .zip(&buf.shader_groups)
            .map(|(stage_desc, buf)| {
                vk::PipelineShaderStageCreateInfo::builder()
                    .flags(vk::PipelineShaderStageCreateFlags::empty())
                    .stage(conv::map_shader_stage(stage_desc.stage))
                    .module(stage_desc.entry_point.module.raw)
                    .name(buf.c_string.as_c_str())
                    .specialization_info(&buf.specialization)
                    .build()
            })
            .collect::<Vec<_>>();
        let groups = desc
            .groups
            .iter()
            .map(conv::map_shader_group_desc)
            .collect::<Vec<_>>();

        let info = {
            let (base_handle, base_index) = match desc.parent {
                pso::BasePipeline::Pipeline(pipeline) => (pipeline.0, -1),
                pso::BasePipeline::Index(index) => (vk::Pipeline::null(), index as _),
                pso::BasePipeline::None => (vk::Pipeline::null(), -1),
            };

            vk::RayTracingPipelineCreateInfoKHR::builder()
                .flags(conv::map_pipeline_create_flags(desc.flags, &desc.parent))
                .stages(&stages)
                .groups(&groups)
                .max_pipeline_ray_recursion_depth(desc.max_pipeline_ray_recursion_depth)
                // .library_info()
                // .library_interface()
                // .dynamic_state()
                .layout(desc.layout.raw)
                .base_pipeline_handle(base_handle)
                .base_pipeline_index(base_index)
        };

        // TODO create_ray_tracing_pipelines also returns VK_OPERATION_DEFERRED_KHR, VK_OPERATION_NOT_DEFERRED_KHR, VK_PIPELINE_COMPILE_REQUIRED_EXT on success, but ash does not support this.
        match self
            .shared
            .extension_fns
            .ray_tracing_pipeline
            .as_ref()
            .expect(
                "Feature RAY_TRACING_PIPELINE must be enabled to call create_ray_tracing_pipeline",
            )
            .unwrap_extension()
            .create_ray_tracing_pipelines(
                vk::DeferredOperationKHR::null(),
                cache.map_or(vk::PipelineCache::null(), |cache| cache.raw),
                &[info.build()],
                None,
            ) {
            Ok(pipelines) => {
                // if let Some(name) = desc.label {
                //     self.shared
                //         .set_object_name(vk::ObjectType::PIPELINE, pipeline, name);
                // }
                Ok(n::RayTracingPipeline(pipelines[0]))
            }
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            Err(vk::Result::ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS) => {
                todo!()
            }
            _ => Err(pso::CreationError::Other),
        }
    }

    unsafe fn create_framebuffer<T>(
        &self,
        renderpass: &n::RenderPass,
        attachments: T,
        extent: image::Extent,
    ) -> Result<n::Framebuffer, d::OutOfMemory>
    where
        T: Iterator<Item = image::FramebufferAttachment>,
    {
        if !self.shared.imageless_framebuffers {
            return Ok(n::Framebuffer::Legacy {
                name: String::new(),
                map: Default::default(),
                extent,
            });
        }

        // guarantee that we don't reallocate it
        let mut view_formats =
            SmallVec::<[vk::Format; 5]>::with_capacity(renderpass.attachment_count);
        let attachment_infos = attachments
            .map(|fat| {
                let mut info = vk::FramebufferAttachmentImageInfo::builder()
                    .usage(conv::map_image_usage(fat.usage))
                    .flags(conv::map_view_capabilities(fat.view_caps))
                    .width(extent.width)
                    .height(extent.height)
                    .layer_count(extent.depth)
                    .build();
                info.view_format_count = 1;
                info.p_view_formats = view_formats.as_ptr().add(view_formats.len());
                view_formats.push(conv::map_format(fat.format));
                info
            })
            .collect::<SmallVec<[_; 5]>>();

        let mut attachments_info = vk::FramebufferAttachmentsCreateInfo::builder()
            .attachment_image_infos(&attachment_infos)
            .build();

        let mut info = vk::FramebufferCreateInfo::builder()
            .flags(vk::FramebufferCreateFlags::IMAGELESS_KHR)
            .render_pass(renderpass.raw)
            .width(extent.width)
            .height(extent.height)
            .layers(extent.depth)
            .push_next(&mut attachments_info);
        info.attachment_count = renderpass.attachment_count as u32;
        info.p_attachments = ptr::null();

        let result = self.shared.raw.create_framebuffer(&info, None);

        match result {
            Ok(raw) => Ok(n::Framebuffer::ImageLess(raw)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn create_shader_module(
        &self,
        spirv_data: &[u32],
    ) -> Result<n::ShaderModule, d::ShaderError> {
        let info = vk::ShaderModuleCreateInfo::builder()
            .flags(vk::ShaderModuleCreateFlags::empty())
            .code(spirv_data);

        let module = self.shared.raw.create_shader_module(&info, None);

        match module {
            Ok(raw) => Ok(n::ShaderModule { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            Err(_) => {
                Err(d::ShaderError::CompilationFailed(String::new())) // TODO
            }
        }
    }

    #[cfg(feature = "naga")]
    unsafe fn create_shader_module_from_naga(
        &self,
        shader: d::NagaShader,
    ) -> Result<n::ShaderModule, (d::ShaderError, d::NagaShader)> {
        match naga::back::spv::write_vec(&shader.module, &shader.info, &self.naga_options) {
            Ok(spv) => self.create_shader_module(&spv).map_err(|e| (e, shader)),
            Err(e) => return Err((d::ShaderError::CompilationFailed(format!("{}", e)), shader)),
        }
    }

    unsafe fn create_sampler(
        &self,
        desc: &image::SamplerDesc,
    ) -> Result<n::Sampler, d::AllocationError> {
        use hal::pso::Comparison;

        let (anisotropy_enable, max_anisotropy) =
            desc.anisotropy_clamp.map_or((false, 1.0), |aniso| {
                if self.shared.features.contains(Features::SAMPLER_ANISOTROPY) {
                    (true, aniso as f32)
                } else {
                    warn!(
                        "Anisotropy({}) was requested on a device with disabled feature",
                        aniso
                    );
                    (false, 1.0)
                }
            });

        let mut reduction_info;
        let mut info = vk::SamplerCreateInfo::builder()
            .flags(vk::SamplerCreateFlags::empty())
            .mag_filter(conv::map_filter(desc.mag_filter))
            .min_filter(conv::map_filter(desc.min_filter))
            .mipmap_mode(conv::map_mip_filter(desc.mip_filter))
            .address_mode_u(conv::map_wrap(desc.wrap_mode.0))
            .address_mode_v(conv::map_wrap(desc.wrap_mode.1))
            .address_mode_w(conv::map_wrap(desc.wrap_mode.2))
            .mip_lod_bias(desc.lod_bias.0)
            .anisotropy_enable(anisotropy_enable)
            .max_anisotropy(max_anisotropy)
            .compare_enable(desc.comparison.is_some())
            .compare_op(conv::map_comparison(
                desc.comparison.unwrap_or(Comparison::Never),
            ))
            .min_lod(desc.lod_range.start.0)
            .max_lod(desc.lod_range.end.0)
            .border_color(conv::map_border_color(desc.border))
            .unnormalized_coordinates(!desc.normalized);

        if self.shared.features.contains(Features::SAMPLER_REDUCTION) {
            reduction_info = vk::SamplerReductionModeCreateInfo::builder()
                .reduction_mode(conv::map_reduction(desc.reduction_mode))
                .build();
            info = info.push_next(&mut reduction_info);
        }

        let result = self.shared.raw.create_sampler(&info, None);

        match result {
            Ok(sampler) => Ok(n::Sampler(sampler)),
            Err(vk::Result::ERROR_TOO_MANY_OBJECTS) => Err(d::AllocationError::TooManyObjects),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    ///
    unsafe fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
        sparse: memory::SparseFlags,
    ) -> Result<n::Buffer, buffer::CreationError> {
        let info = vk::BufferCreateInfo::builder()
            .flags(conv::map_buffer_create_flags(sparse))
            .size(size)
            .usage(conv::map_buffer_usage(usage))
            .sharing_mode(vk::SharingMode::EXCLUSIVE); // TODO:

        let result = self.shared.raw.create_buffer(&info, None);

        match result {
            Ok(raw) => Ok(n::Buffer { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn get_buffer_requirements(&self, buffer: &n::Buffer) -> Requirements {
        let req = self.shared.raw.get_buffer_memory_requirements(buffer.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: self.filter_memory_requirements(req.memory_type_bits),
        }
    }

    unsafe fn bind_buffer_memory(
        &self,
        memory: &n::Memory,
        offset: u64,
        buffer: &mut n::Buffer,
    ) -> Result<(), d::BindError> {
        let result = self
            .shared
            .raw
            .bind_buffer_memory(buffer.raw, memory.raw, offset);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn create_buffer_view(
        &self,
        buffer: &n::Buffer,
        format: Option<format::Format>,
        range: buffer::SubRange,
    ) -> Result<n::BufferView, buffer::ViewCreationError> {
        let info = vk::BufferViewCreateInfo::builder()
            .flags(vk::BufferViewCreateFlags::empty())
            .buffer(buffer.raw)
            .format(format.map_or(vk::Format::UNDEFINED, conv::map_format))
            .offset(range.offset)
            .range(range.size.unwrap_or(vk::WHOLE_SIZE));

        let result = self.shared.raw.create_buffer_view(&info, None);

        match result {
            Ok(raw) => Ok(n::BufferView { raw }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
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
        sparse: memory::SparseFlags,
        view_caps: image::ViewCapabilities,
    ) -> Result<n::Image, image::CreationError> {
        let flags = conv::map_view_capabilities_sparse(sparse, view_caps);
        let extent = conv::map_extent(kind.extent());
        let array_layers = kind.num_layers();
        let samples = kind.num_samples();
        let image_type = match kind {
            image::Kind::D1(..) => vk::ImageType::TYPE_1D,
            image::Kind::D2(..) => vk::ImageType::TYPE_2D,
            image::Kind::D3(..) => vk::ImageType::TYPE_3D,
        };

        //Note: this is a hack, we should expose this in the API instead
        let layout = match tiling {
            image::Tiling::Linear => vk::ImageLayout::PREINITIALIZED,
            image::Tiling::Optimal => vk::ImageLayout::UNDEFINED,
        };

        let info = vk::ImageCreateInfo::builder()
            .flags(flags)
            .image_type(image_type)
            .format(conv::map_format(format))
            .extent(extent.clone())
            .mip_levels(mip_levels as u32)
            .array_layers(array_layers as u32)
            .samples(conv::map_sample_count_flags(samples))
            .tiling(conv::map_tiling(tiling))
            .usage(conv::map_image_usage(usage))
            .sharing_mode(vk::SharingMode::EXCLUSIVE) // TODO:
            .initial_layout(layout);

        let result = self.shared.raw.create_image(&info, None);

        match result {
            Ok(raw) => Ok(n::Image {
                raw,
                ty: image_type,
                flags,
                extent,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn get_image_requirements(&self, image: &n::Image) -> Requirements {
        let req = self.shared.raw.get_image_memory_requirements(image.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: self.filter_memory_requirements(req.memory_type_bits),
        }
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        image: &n::Image,
        subresource: image::Subresource,
    ) -> image::SubresourceFootprint {
        let sub = conv::map_subresource(&subresource);
        let layout = self.shared.raw.get_image_subresource_layout(image.raw, sub);

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
        let result = self
            .shared
            .raw
            .bind_image_memory(image.raw, memory.raw, offset);

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn create_image_view(
        &self,
        image: &n::Image,
        kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        usage: image::Usage,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewCreationError> {
        let is_cube = image
            .flags
            .intersects(vk::ImageCreateFlags::CUBE_COMPATIBLE);
        let view_type = match conv::map_view_kind(kind, image.ty, is_cube) {
            Some(ty) => ty,
            None => return Err(image::ViewCreationError::BadKind(kind)),
        };

        self.image_view_from_raw(image.raw, view_type, format, swizzle, usage, range)
    }

    unsafe fn create_descriptor_pool<T>(
        &self,
        max_sets: usize,
        descriptor_ranges: T,
        flags: pso::DescriptorPoolCreateFlags,
    ) -> Result<n::DescriptorPool, d::OutOfMemory>
    where
        T: Iterator<Item = pso::DescriptorRangeDesc>,
    {
        let pools_iter = descriptor_ranges.map(|pool| vk::DescriptorPoolSize {
            ty: conv::map_descriptor_type(pool.ty),
            descriptor_count: pool.count as u32,
        });

        let result = inplace_or_alloc_from_iter(pools_iter, |pools| {
            let info = vk::DescriptorPoolCreateInfo::builder()
                .flags(conv::map_descriptor_pool_create_flags(flags))
                .max_sets(max_sets as u32)
                .pool_sizes(&pools);

            self.shared.raw.create_descriptor_pool(&info, None)
        });

        match result {
            Ok(pool) => Ok(n::DescriptorPool::new(pool, &self.shared)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn create_descriptor_set_layout<'a, I, J>(
        &self,
        binding_iter: I,
        immutable_samplers: J,
    ) -> Result<n::DescriptorSetLayout, d::OutOfMemory>
    where
        I: Iterator<Item = pso::DescriptorSetLayoutBinding>,
        J: Iterator<Item = &'a n::Sampler>,
    {
        let vk_immutable_samplers_iter = immutable_samplers.map(|is| is.0);
        let mut sampler_offset = 0;

        let mut bindings = binding_iter.collect::<Vec<_>>();
        // Sorting will come handy in `write_descriptor_sets`.
        bindings.sort_by_key(|b| b.binding);

        let result = inplace_or_alloc_from_iter(vk_immutable_samplers_iter, |immutable_samplers| {
            let raw_bindings_iter = bindings.iter().map(|b| vk::DescriptorSetLayoutBinding {
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
            });

            inplace_or_alloc_from_iter(raw_bindings_iter, |raw_bindings| {
                // TODO raw_bindings doesnt implement fmt::Debug
                // debug!("create_descriptor_set_layout {:?}", raw_bindings);

                let info = vk::DescriptorSetLayoutCreateInfo::builder()
                    .flags(vk::DescriptorSetLayoutCreateFlags::empty())
                    .bindings(&raw_bindings);

                self.shared.raw.create_descriptor_set_layout(&info, None)
            })
        });

        match result {
            Ok(layout) => Ok(n::DescriptorSetLayout {
                raw: layout,
                bindings: Arc::new(bindings),
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn write_descriptor_set<'a, I>(&self, op: pso::DescriptorSetWrite<'a, B, I>)
    where
        I: Iterator<Item = pso::Descriptor<'a, B>>,
    {
        /// Stores the
        struct RawWriteOffsets {
            image_info: usize,
            buffer_info: usize,
            texel_buffer_view: usize,
            accel_struct: usize,
        }

        let descriptors = op.descriptors;
        let mut raw_writes =
            Vec::<vk::WriteDescriptorSet>::with_capacity(descriptors.size_hint().0);
        // Parallel array to `raw_writes`.
        let mut raw_writes_indices = Vec::<RawWriteOffsets>::new();
        let mut image_infos = Vec::new();
        let mut buffer_infos = Vec::new();
        let mut texel_buffer_views = Vec::new();
        let mut accel_structs = Vec::new();

        // gfx-hal allows the type and stages to be different between the descriptor
        // in a single write, while Vulkan requires them to be the same.
        let mut last_type = vk::DescriptorType::SAMPLER;
        let mut last_stages = pso::ShaderStageFlags::empty();

        let mut binding_pos = op
            .set
            .bindings
            .binary_search_by_key(&op.binding, |b| b.binding)
            .expect("Descriptor set writes don't match the set layout!");
        let mut array_offset = op.array_offset;

        for descriptor in descriptors {
            let layout_binding = &op.set.bindings[binding_pos];
            array_offset += 1;
            if array_offset == layout_binding.count {
                array_offset = 0;
                binding_pos += 1;
            }

            let descriptor_type = conv::map_descriptor_type(layout_binding.ty);
            if descriptor_type == last_type && layout_binding.stage_flags == last_stages {
                raw_writes.last_mut().unwrap().descriptor_count += 1;
            } else {
                last_type = descriptor_type;
                last_stages = layout_binding.stage_flags;
                raw_writes.push(vk::WriteDescriptorSet {
                    s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                    p_next: ptr::null(),
                    dst_set: op.set.raw,
                    dst_binding: layout_binding.binding,
                    dst_array_element: if layout_binding.binding == op.binding {
                        op.array_offset as _
                    } else {
                        0
                    },
                    descriptor_count: 1,
                    descriptor_type,
                    p_image_info: ptr::null(),
                    p_buffer_info: ptr::null(),
                    p_texel_buffer_view: ptr::null(),
                });
                raw_writes_indices.push(RawWriteOffsets {
                    image_info: image_infos.len(),
                    buffer_info: buffer_infos.len(),
                    texel_buffer_view: texel_buffer_views.len(),
                    accel_struct: accel_structs.len(),
                });
            }

            match descriptor {
                pso::Descriptor::Sampler(sampler) => {
                    image_infos.push(
                        vk::DescriptorImageInfo::builder()
                            .sampler(sampler.0)
                            .image_view(vk::ImageView::null())
                            .image_layout(vk::ImageLayout::GENERAL)
                            .build(),
                    );
                }
                pso::Descriptor::Image(view, layout) => {
                    image_infos.push(
                        vk::DescriptorImageInfo::builder()
                            .sampler(vk::Sampler::null())
                            .image_view(view.raw)
                            .image_layout(conv::map_image_layout(layout))
                            .build(),
                    );
                }
                pso::Descriptor::CombinedImageSampler(view, layout, sampler) => {
                    image_infos.push(
                        vk::DescriptorImageInfo::builder()
                            .sampler(sampler.0)
                            .image_view(view.raw)
                            .image_layout(conv::map_image_layout(layout))
                            .build(),
                    );
                }
                pso::Descriptor::Buffer(buffer, ref sub) => {
                    buffer_infos.push(
                        vk::DescriptorBufferInfo::builder()
                            .buffer(buffer.raw)
                            .offset(sub.offset)
                            .range(sub.size.unwrap_or(vk::WHOLE_SIZE))
                            .build(),
                    );
                }
                pso::Descriptor::TexelBuffer(view) => {
                    texel_buffer_views.push(view.raw);
                }
                pso::Descriptor::AccelerationStructure(accel_struct) => {
                    accel_structs.push(accel_struct.0);
                }
            }
        }

        let mut accel_structure_writes =
            Vec::<vk::WriteDescriptorSetAccelerationStructureKHR>::new();

        // Patch the pointers now that we have all the storage allocated.
        debug_assert_eq!(raw_writes.len(), raw_writes_indices.len());
        for (raw, offsets) in raw_writes.iter_mut().zip(raw_writes_indices) {
            use crate::vk::DescriptorType as Dt;
            match raw.descriptor_type {
                Dt::SAMPLER
                | Dt::SAMPLED_IMAGE
                | Dt::STORAGE_IMAGE
                | Dt::COMBINED_IMAGE_SAMPLER
                | Dt::INPUT_ATTACHMENT => {
                    raw.p_image_info = image_infos[offsets.image_info..].as_ptr();
                }
                Dt::UNIFORM_TEXEL_BUFFER | Dt::STORAGE_TEXEL_BUFFER => {
                    raw.p_texel_buffer_view =
                        texel_buffer_views[offsets.texel_buffer_view..].as_ptr();
                }
                Dt::UNIFORM_BUFFER
                | Dt::STORAGE_BUFFER
                | Dt::STORAGE_BUFFER_DYNAMIC
                | Dt::UNIFORM_BUFFER_DYNAMIC => {
                    raw.p_buffer_info = buffer_infos[offsets.buffer_info..].as_ptr();
                }
                Dt::ACCELERATION_STRUCTURE_KHR => {
                    accel_structure_writes.push(vk::WriteDescriptorSetAccelerationStructureKHR {
                        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET_ACCELERATION_STRUCTURE_KHR,
                        p_next: ptr::null(),
                        acceleration_structure_count: 1,
                        p_acceleration_structures: accel_structs[offsets.accel_struct..].as_ptr(),
                    });

                    raw.p_next = accel_structure_writes.last_mut().unwrap() as *mut _ as *mut _;
                }
                _ => panic!("unknown descriptor type"),
            }
        }

        self.shared.raw.update_descriptor_sets(&raw_writes, &[]);
    }

    unsafe fn copy_descriptor_set<'a>(&self, op: pso::DescriptorSetCopy<'a, B>) {
        let copy = vk::CopyDescriptorSet::builder()
            .src_set(op.src_set.raw)
            .src_binding(op.src_binding as u32)
            .src_array_element(op.src_array_offset as u32)
            .dst_set(op.dst_set.raw)
            .dst_binding(op.dst_binding as u32)
            .dst_array_element(op.dst_array_offset as u32)
            .descriptor_count(op.count as u32)
            .build();

        self.shared.raw.update_descriptor_sets(&[], &[copy]);
    }

    unsafe fn map_memory(
        &self,
        memory: &mut n::Memory,
        segment: Segment,
    ) -> Result<*mut u8, d::MapError> {
        let result = self.shared.raw.map_memory(
            memory.raw,
            segment.offset,
            segment.size.unwrap_or(vk::WHOLE_SIZE),
            vk::MemoryMapFlags::empty(),
        );

        match result {
            Ok(ptr) => Ok(ptr as *mut _),
            Err(vk::Result::ERROR_MEMORY_MAP_FAILED) => Err(d::MapError::MappingFailed),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn unmap_memory(&self, memory: &mut n::Memory) {
        self.shared.raw.unmap_memory(memory.raw)
    }

    unsafe fn flush_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = (&'a n::Memory, Segment)>,
    {
        let vk_ranges_iter = ranges.map(conv::map_memory_range);
        let result = inplace_or_alloc_from_iter(vk_ranges_iter, |ranges| {
            self.shared.raw.flush_mapped_memory_ranges(&ranges)
        });

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), d::OutOfMemory>
    where
        I: Iterator<Item = (&'a n::Memory, Segment)>,
    {
        let vk_ranges_iter = ranges.map(conv::map_memory_range);
        let result = inplace_or_alloc_from_iter(vk_ranges_iter, |ranges| {
            self.shared.raw.invalidate_mapped_memory_ranges(&ranges)
        });

        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    fn create_semaphore(&self) -> Result<n::Semaphore, d::OutOfMemory> {
        let info = vk::SemaphoreCreateInfo::builder().flags(vk::SemaphoreCreateFlags::empty());

        let result = unsafe { self.shared.raw.create_semaphore(&info, None) };

        match result {
            Ok(semaphore) => Ok(n::Semaphore(semaphore)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    fn create_fence(&self, signaled: bool) -> Result<n::Fence, d::OutOfMemory> {
        let info = vk::FenceCreateInfo::builder().flags(if signaled {
            vk::FenceCreateFlags::SIGNALED
        } else {
            vk::FenceCreateFlags::empty()
        });

        let result = unsafe { self.shared.raw.create_fence(&info, None) };

        match result {
            Ok(fence) => Ok(n::Fence(fence)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn reset_fence(&self, fence: &mut n::Fence) -> Result<(), d::OutOfMemory> {
        match self.shared.raw.reset_fences(&[fence.0]) {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn wait_for_fences<'a, I>(
        &self,
        fences_iter: I,
        wait: d::WaitFor,
        timeout_ns: u64,
    ) -> Result<bool, d::WaitError>
    where
        I: Iterator<Item = &'a n::Fence>,
    {
        let vk_fences_iter = fences_iter.map(|fence| fence.0);

        let all = match wait {
            d::WaitFor::Any => false,
            d::WaitFor::All => true,
        };

        let result = inplace_or_alloc_from_iter(vk_fences_iter, |fences| {
            self.shared.raw.wait_for_fences(fences, all, timeout_ns)
        });

        match result {
            Ok(()) => Ok(true),
            Err(vk::Result::TIMEOUT) => Ok(false),
            Err(vk::Result::ERROR_DEVICE_LOST) => Err(d::DeviceLost.into()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn get_fence_status(&self, fence: &n::Fence) -> Result<bool, d::DeviceLost> {
        let result = self.shared.raw.get_fence_status(fence.0);
        match result {
            Ok(ok) => Ok(ok),
            Err(vk::Result::NOT_READY) => Ok(false), //TODO: shouldn't be needed
            Err(vk::Result::ERROR_DEVICE_LOST) => Err(d::DeviceLost),
            _ => unreachable!(),
        }
    }

    fn create_event(&self) -> Result<n::Event, d::OutOfMemory> {
        let info = vk::EventCreateInfo::builder().flags(vk::EventCreateFlags::empty());

        let result = unsafe { self.shared.raw.create_event(&info, None) };
        match result {
            Ok(e) => Ok(n::Event(e)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn get_event_status(&self, event: &n::Event) -> Result<bool, d::WaitError> {
        let result = self.shared.raw.get_event_status(event.0);
        match result {
            Ok(b) => Ok(b),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            Err(vk::Result::ERROR_DEVICE_LOST) => Err(d::DeviceLost.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn set_event(&self, event: &mut n::Event) -> Result<(), d::OutOfMemory> {
        let result = self.shared.raw.set_event(event.0);
        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn reset_event(&self, event: &mut n::Event) -> Result<(), d::OutOfMemory> {
        let result = self.shared.raw.reset_event(event.0);
        match result {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn free_memory(&self, memory: n::Memory) {
        self.shared.raw.free_memory(memory.raw, None);
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
            query::Type::AccelerationStructureCompactedSize => (
                vk::QueryType::ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR,
                vk::QueryPipelineStatisticFlags::empty(),
            ),
            query::Type::AccelerationStructureSerializationSize => (
                vk::QueryType::ACCELERATION_STRUCTURE_SERIALIZATION_SIZE_KHR,
                vk::QueryPipelineStatisticFlags::empty(),
            ),
        };

        let info = vk::QueryPoolCreateInfo::builder()
            .flags(vk::QueryPoolCreateFlags::empty())
            .query_type(query_type)
            .query_count(query_count)
            .pipeline_statistics(pipeline_statistics);

        let result = self.shared.raw.create_query_pool(&info, None);

        match result {
            Ok(pool) => Ok(n::QueryPool(pool)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn get_query_pool_results(
        &self,
        pool: &n::QueryPool,
        queries: Range<query::Id>,
        data: &mut [u8],
        stride: buffer::Stride,
        flags: query::ResultFlags,
    ) -> Result<bool, d::WaitError> {
        let result = self.shared.raw.fp_v1_0().get_query_pool_results(
            self.shared.raw.handle(),
            pool.0,
            queries.start,
            queries.end - queries.start,
            data.len(),
            data.as_mut_ptr() as *mut _,
            stride as vk::DeviceSize,
            conv::map_query_result_flags(flags),
        );

        match result {
            vk::Result::SUCCESS => Ok(true),
            vk::Result::NOT_READY => Ok(false),
            vk::Result::ERROR_DEVICE_LOST => Err(d::DeviceLost.into()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(d::OutOfMemory::Host.into()),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }

    unsafe fn create_acceleration_structure(
        &self,
        desc: &hal::acceleration_structure::CreateDesc<B>,
    ) -> Result<n::AccelerationStructure, d::OutOfMemory> {
        let result = self
            .shared
            .extension_fns
            .acceleration_structure
            .as_ref()
            .expect("Feature ACCELERATION_STRUCTURE must be enabled to call create_acceleration_structure").unwrap_extension()
            .create_acceleration_structure(
                &vk::AccelerationStructureCreateInfoKHR::builder()
                    .buffer(desc.buffer.raw)
                    .offset(desc.buffer_offset)
                    .size(desc.size)
                    .ty(match desc.ty {
                        hal::acceleration_structure::Type::TopLevel => {
                            vk::AccelerationStructureTypeKHR::TOP_LEVEL
                        }
                        hal::acceleration_structure::Type::BottomLevel => {
                            vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL
                        }
                        hal::acceleration_structure::Type::Generic => {
                            vk::AccelerationStructureTypeKHR::GENERIC
                        }
                    })
                    // TODO(capture-replay)
                    // .create_flags(vk::AccelerationStructureCreateFlagsKHR::empty())
                    // .device_address()
                    .build(),
                None,
            );

        match result {
            Ok(acceleration_structure) => Ok(n::AccelerationStructure(acceleration_structure)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            // TODO(capture-replay)
            Err(vk::Result::ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS) => todo!(),
            _ => unreachable!(),
        }
    }

    unsafe fn get_acceleration_structure_build_requirements(
        &self,
        desc: &hal::acceleration_structure::GeometryDesc<B>,
        max_primitive_counts: &[u32],
    ) -> hal::acceleration_structure::SizeRequirements {
        let geometries = desc
            .geometries
            .iter()
            .map(|&geometry| conv::map_geometry(&self.shared, geometry))
            .collect::<Vec<_>>();

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(conv::map_acceleration_structure_type(desc.ty))
            .flags(conv::map_acceleration_structure_flags(desc.flags))
            .geometries(geometries.as_slice());

        let build_size_info = self
            .shared
            .extension_fns
            .acceleration_structure
            .as_ref()
            .expect(
                // TODO: this string prevents rustfmt from running?
                "Feature ACCELERATION_STRUCTURE must be enabled to call get_acceleration_structure_build_requirements",
            ).unwrap_extension()
            .get_acceleration_structure_build_sizes(
                vk::AccelerationStructureBuildTypeKHR::DEVICE,
                &build_info,
                max_primitive_counts,
            );

        hal::acceleration_structure::SizeRequirements {
            acceleration_structure_size: build_size_info.acceleration_structure_size,
            update_scratch_size: build_size_info.update_scratch_size,
            build_scratch_size: build_size_info.build_scratch_size,
        }
    }

    unsafe fn get_acceleration_structure_address(
        &self,
        accel_struct: &n::AccelerationStructure,
    ) -> hal::acceleration_structure::DeviceAddress {
        hal::acceleration_structure::DeviceAddress(
            self.shared
                .extension_fns
                .acceleration_structure
                .as_ref()
                .expect("Feature ACCELERATION_STRUCTURE must be enabled to call get_acceleration_structure_address").unwrap_extension()
                .get_acceleration_structure_device_address(
                    &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
                        .acceleration_structure(accel_struct.0)
                        .build(),
                ),
        )
    }

    unsafe fn get_device_acceleration_structure_compatibility(
        &self,
        serialized_accel_struct: &[u8; 32],
    ) -> hal::acceleration_structure::Compatibility {
        match self
            .shared
            .extension_fns
            .acceleration_structure
            .as_ref()
            .expect("Feature ACCELERATION_STRUCTURE must be enabled to call get_device_acceleration_structure_compatibility").unwrap_extension()
            .get_device_acceleration_structure_compatibility(
                &vk::AccelerationStructureVersionInfoKHR::builder()
                    .version_data(serialized_accel_struct),
            ) {
            vk::AccelerationStructureCompatibilityKHR::COMPATIBLE => {
                hal::acceleration_structure::Compatibility::Compatible
            }
            vk::AccelerationStructureCompatibilityKHR::INCOMPATIBLE => {
                hal::acceleration_structure::Compatibility::Incompatible
            }
            _ => unreachable!(),
        }
    }

    unsafe fn get_ray_tracing_shader_group_handles<'a>(
        &self,
        pipeline: &'a n::RayTracingPipeline,
        first_group: u32,
        group_count: u32,
        data_size: usize,
    ) -> Result<Vec<u8>, d::OutOfMemory> {
        let result = self
            .shared
            .extension_fns
            .ray_tracing_pipeline
            .as_ref()
            .expect("Feature RAY_TRACING_PIPELINE must be enabled to call get_ray_tracing_shader_group_handles").unwrap_extension()
            .get_ray_tracing_shader_group_handles(pipeline.0, first_group, group_count, data_size);

        match result {
            Ok(data) => Ok(data),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            _ => unreachable!(),
        }
    }

    unsafe fn get_ray_tracing_shader_group_stack_size<'a>(
        &self,
        pipeline: &'a n::RayTracingPipeline,
        group: u32,
        group_shader: pso::GroupShader,
    ) -> u64 {
        self.shared
            .extension_fns
            .ray_tracing_pipeline
            .as_ref()
            .expect("Feature RAY_TRACING_PIPELINE must be enabled to call get_ray_tracing_shader_group_stack_size")
            .unwrap_extension()
            .get_ray_tracing_shader_group_stack_size(
                pipeline.0,
                group,
                conv::map_group_shader(group_shader),
            )
    }

    unsafe fn destroy_query_pool(&self, pool: n::QueryPool) {
        self.shared.raw.destroy_query_pool(pool.0, None);
    }

    unsafe fn destroy_shader_module(&self, module: n::ShaderModule) {
        self.shared.raw.destroy_shader_module(module.raw, None);
    }

    unsafe fn destroy_render_pass(&self, rp: n::RenderPass) {
        self.shared.raw.destroy_render_pass(rp.raw, None);
    }

    unsafe fn destroy_pipeline_layout(&self, pl: n::PipelineLayout) {
        self.shared.raw.destroy_pipeline_layout(pl.raw, None);
    }

    unsafe fn destroy_graphics_pipeline(&self, pipeline: n::GraphicsPipeline) {
        self.shared.raw.destroy_pipeline(pipeline.0, None);
    }

    unsafe fn destroy_compute_pipeline(&self, pipeline: n::ComputePipeline) {
        self.shared.raw.destroy_pipeline(pipeline.0, None);
    }

    unsafe fn destroy_ray_tracing_pipeline(&self, pipeline: n::RayTracingPipeline) {
        self.shared.raw.destroy_pipeline(pipeline.0, None);
    }

    unsafe fn destroy_framebuffer(&self, fb: n::Framebuffer) {
        match fb {
            n::Framebuffer::ImageLess(raw) => {
                self.shared.raw.destroy_framebuffer(raw, None);
            }
            n::Framebuffer::Legacy { map, .. } => {
                for (_, raw) in map.into_inner() {
                    self.shared.raw.destroy_framebuffer(raw, None);
                }
            }
        }
    }

    unsafe fn destroy_buffer(&self, buffer: n::Buffer) {
        self.shared.raw.destroy_buffer(buffer.raw, None);
    }

    unsafe fn destroy_buffer_view(&self, view: n::BufferView) {
        self.shared.raw.destroy_buffer_view(view.raw, None);
    }

    unsafe fn destroy_image(&self, image: n::Image) {
        self.shared.raw.destroy_image(image.raw, None);
    }

    unsafe fn destroy_image_view(&self, view: n::ImageView) {
        self.shared.raw.destroy_image_view(view.raw, None);
    }

    unsafe fn destroy_sampler(&self, sampler: n::Sampler) {
        self.shared.raw.destroy_sampler(sampler.0, None);
    }

    unsafe fn destroy_descriptor_pool(&self, pool: n::DescriptorPool) {
        self.shared.raw.destroy_descriptor_pool(pool.finish(), None);
    }

    unsafe fn destroy_descriptor_set_layout(&self, layout: n::DescriptorSetLayout) {
        self.shared
            .raw
            .destroy_descriptor_set_layout(layout.raw, None);
    }

    unsafe fn destroy_fence(&self, fence: n::Fence) {
        self.shared.raw.destroy_fence(fence.0, None);
    }

    unsafe fn destroy_semaphore(&self, semaphore: n::Semaphore) {
        self.shared.raw.destroy_semaphore(semaphore.0, None);
    }

    unsafe fn destroy_event(&self, event: n::Event) {
        self.shared.raw.destroy_event(event.0, None);
    }

    unsafe fn destroy_acceleration_structure(&self, accel_struct: n::AccelerationStructure) {
        self.shared
            .extension_fns
            .acceleration_structure
            .as_ref()
            .expect("Feature ACCELERATION_STRUCTURE must be enabled to call destroy_acceleration_structure").unwrap_extension()
            .destroy_acceleration_structure(accel_struct.0, None);
    }

    fn wait_idle(&self) -> Result<(), d::OutOfMemory> {
        match unsafe { self.shared.raw.device_wait_idle() } {
            Ok(()) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device),
            _ => unreachable!(),
        }
    }

    unsafe fn set_image_name(&self, image: &mut n::Image, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::IMAGE, image.raw, name)
    }

    unsafe fn set_buffer_name(&self, buffer: &mut n::Buffer, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::BUFFER, buffer.raw, name)
    }

    unsafe fn set_command_buffer_name(&self, command_buffer: &mut cmd::CommandBuffer, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::COMMAND_BUFFER, command_buffer.raw, name)
    }

    unsafe fn set_semaphore_name(&self, semaphore: &mut n::Semaphore, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::SEMAPHORE, semaphore.0, name)
    }

    unsafe fn set_fence_name(&self, fence: &mut n::Fence, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::FENCE, fence.0, name)
    }

    unsafe fn set_framebuffer_name(&self, framebuffer: &mut n::Framebuffer, name: &str) {
        match *framebuffer {
            n::Framebuffer::ImageLess(raw) => {
                self.shared
                    .set_object_name(vk::ObjectType::FRAMEBUFFER, raw, name);
            }
            n::Framebuffer::Legacy {
                name: ref mut old_name,
                ref mut map,
                extent: _,
            } => {
                old_name.clear();
                old_name.push_str(name);
                for &raw in map.get_mut().values() {
                    self.shared
                        .set_object_name(vk::ObjectType::FRAMEBUFFER, raw, name);
                }
            }
        }
    }

    unsafe fn set_render_pass_name(&self, render_pass: &mut n::RenderPass, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::RENDER_PASS, render_pass.raw, name)
    }

    unsafe fn set_descriptor_set_name(&self, descriptor_set: &mut n::DescriptorSet, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::DESCRIPTOR_SET, descriptor_set.raw, name)
    }

    unsafe fn set_descriptor_set_layout_name(
        &self,
        descriptor_set_layout: &mut n::DescriptorSetLayout,
        name: &str,
    ) {
        self.shared.set_object_name(
            vk::ObjectType::DESCRIPTOR_SET_LAYOUT,
            descriptor_set_layout.raw,
            name,
        )
    }

    unsafe fn set_pipeline_layout_name(&self, pipeline_layout: &mut n::PipelineLayout, name: &str) {
        self.shared
            .set_object_name(vk::ObjectType::PIPELINE_LAYOUT, pipeline_layout.raw, name)
    }

    unsafe fn set_acceleration_structure_name(
        &self,
        accel_struct: &mut n::AccelerationStructure,
        name: &str,
    ) {
        self.shared.set_object_name(
            vk::ObjectType::ACCELERATION_STRUCTURE_KHR,
            accel_struct.0,
            name,
        )
    }

    unsafe fn set_display_power_state(
        &self,
        display: &hal::display::Display<B>,
        power_state: &hal::display::control::PowerState,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        let display_control_extension = match self.shared.extension_fns.display_control {
            Some(ref display_control_extension) => {
                display_control_extension
            }
            _ => return Err(hal::display::control::DisplayControlError::UnsupportedFeature),
        };

        let vk_power_state = match power_state {
            hal::display::control::PowerState::Off => vk::DisplayPowerStateEXT::OFF,
            hal::display::control::PowerState::Suspend => vk::DisplayPowerStateEXT::SUSPEND,
            hal::display::control::PowerState::On => vk::DisplayPowerStateEXT::ON,
        };

        let vk_power_info = vk::DisplayPowerInfoEXT::builder()
            .power_state(vk_power_state)
            .build();

        match display_control_extension.display_power_control_ext(
            self.shared.raw.handle(),
            display.handle.0,
            &vk_power_info,
        ) {
            vk::Result::SUCCESS => Ok(()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                Err(hal::display::control::DisplayControlError::OutOfHostMemory)
            }
            _ => unreachable!(),
        }
    }

    unsafe fn register_device_event(
        &self,
        device_event: &hal::display::control::DeviceEvent,
        fence: &mut <B as hal::Backend>::Fence,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        let display_control_extension = match self.shared.extension_fns.display_control {
            Some(ref display_control_extension) => {
                display_control_extension
            }
            _ => return Err(hal::display::control::DisplayControlError::UnsupportedFeature),
        };

        let vk_device_event = match device_event {
            hal::display::control::DeviceEvent::DisplayHotplug => vk::DeviceEventTypeEXT::DISPLAY_HOTPLUG,
        };

        let vk_device_event_info = vk::DeviceEventInfoEXT::builder()
            .device_event(vk_device_event)
            .build();

        match display_control_extension.register_device_event_ext(
            self.shared.raw.handle(),
            &vk_device_event_info,
            std::ptr::null(),
            &mut fence.0,
        ) {
            vk::Result::SUCCESS => Ok(()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                Err(hal::display::control::DisplayControlError::OutOfHostMemory)
            }
            vk::Result::ERROR_FEATURE_NOT_PRESENT => {
                Err(hal::display::control::DisplayControlError::UnsupportedFeature)
            } // This is a non-standard error returned on Linux Intel Haswell
            err => {
                error!("Unexpected error: {:#?}", err);
                Err(hal::display::control::DisplayControlError::UnsupportedFeature)
            }
        }
    }

    unsafe fn register_display_event(
        &self,
        display: &hal::display::Display<B>,
        display_event: &hal::display::control::DisplayEvent,
        fence: &mut <B as hal::Backend>::Fence,
    ) -> Result<(), hal::display::control::DisplayControlError> {
        let display_control_extension = match self.shared.extension_fns.display_control {
            Some(ref display_control_extension) => {
                display_control_extension
            }
            _ => return Err(hal::display::control::DisplayControlError::UnsupportedFeature),
        };

        let vk_display_event = match display_event {
            hal::display::control::DisplayEvent::FirstPixelOut => vk::DisplayEventTypeEXT::FIRST_PIXEL_OUT,
        };

        let vk_display_event_info = vk::DisplayEventInfoEXT::builder()
            .display_event(vk_display_event)
            .build();

        match display_control_extension.register_display_event_ext(
            self.shared.raw.handle(),
            display.handle.0,
            &vk_display_event_info,
            std::ptr::null(),
            &mut fence.0,
        ) {
            vk::Result::SUCCESS => Ok(()),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                Err(hal::display::control::DisplayControlError::OutOfHostMemory)
            }
            _ => unreachable!(),
        }
    }

    fn start_capture(&self) {
        unsafe {
            self.render_doc
                .start_frame_capture(self.shared.raw.handle().as_raw() as *mut _, ptr::null_mut())
        }
    }

    fn stop_capture(&self) {
        unsafe {
            self.render_doc
                .end_frame_capture(self.shared.raw.handle().as_raw() as *mut _, ptr::null_mut())
        }
    }
}

impl super::Device {
    /// We only work with a subset of Ash-exposed memory types that we know.
    /// This function filters an ash mask into our mask.
    fn filter_memory_requirements(&self, ash_mask: u32) -> u32 {
        let mut hal_index = 0;
        let mut mask = 0;
        for ash_index in 0..32 {
            if self.valid_ash_memory_types & (1 << ash_index) != 0 {
                if ash_mask & (1 << ash_index) != 0 {
                    mask |= 1 << hal_index;
                }
                hal_index += 1;
            }
        }
        mask
    }

    fn get_ash_memory_type_index(&self, hal_type: MemoryTypeId) -> u32 {
        let mut hal_count = hal_type.0;
        for ash_index in 0..32 {
            if self.valid_ash_memory_types & (1 << ash_index) != 0 {
                if hal_count == 0 {
                    return ash_index;
                }
                hal_count -= 1;
            }
        }
        panic!("Unable to get Ash memory type for {:?}", hal_type);
    }

    pub(crate) unsafe fn create_swapchain(
        &self,
        surface: &mut w::Surface,
        config: SwapchainConfig,
        provided_old_swapchain: Option<w::Swapchain>,
    ) -> Result<(w::Swapchain, Vec<n::Image>), hal::window::SwapchainError> {
        let functor = khr::Swapchain::new(&surface.raw.instance.inner, &self.shared.raw);

        let old_swapchain = match provided_old_swapchain {
            Some(osc) => osc.raw,
            None => vk::SwapchainKHR::null(),
        };

        let info = vk::SwapchainCreateInfoKHR::builder()
            .flags(vk::SwapchainCreateFlagsKHR::empty())
            .surface(surface.raw.handle)
            .min_image_count(config.image_count)
            .image_format(conv::map_format(config.format))
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(vk::Extent2D {
                width: config.extent.width,
                height: config.extent.height,
            })
            .image_array_layers(1)
            .image_usage(conv::map_image_usage(config.image_usage))
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
            .composite_alpha(conv::map_composite_alpha_mode(config.composite_alpha_mode))
            .present_mode(conv::map_present_mode(config.present_mode))
            .clipped(true)
            .old_swapchain(old_swapchain);

        let result = functor.create_swapchain(&info, None);

        if old_swapchain != vk::SwapchainKHR::null() {
            functor.destroy_swapchain(old_swapchain, None)
        }

        let swapchain_raw = match result {
            Ok(swapchain_raw) => swapchain_raw,
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                return Err(d::OutOfMemory::Host.into());
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                return Err(d::OutOfMemory::Device.into());
            }
            Err(vk::Result::ERROR_DEVICE_LOST) => return Err(d::DeviceLost.into()),
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => return Err(hal::window::SurfaceLost.into()),
            Err(vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR) => {
                return Err(hal::window::SwapchainError::WindowInUse)
            }
            Err(other) => {
                error!("Unexpected result - driver bug? {:?}", other);
                return Err(hal::window::SwapchainError::Unknown);
            }
        };

        let result = functor.get_swapchain_images(swapchain_raw);

        let backbuffer_images = match result {
            Ok(backbuffer_images) => backbuffer_images,
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => {
                return Err(d::OutOfMemory::Host.into());
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => {
                return Err(d::OutOfMemory::Device.into());
            }
            _ => unreachable!(),
        };

        let extent = vk::Extent3D {
            width: config.extent.width,
            height: config.extent.height,
            depth: 1,
        };
        let swapchain = w::Swapchain {
            raw: swapchain_raw,
            functor,
            vendor_id: self.vendor_id,
            extent,
        };

        let images = backbuffer_images
            .into_iter()
            .map(|image| n::Image {
                raw: image,
                ty: vk::ImageType::TYPE_2D,
                flags: vk::ImageCreateFlags::empty(),
                extent,
            })
            .collect();

        Ok((swapchain, images))
    }

    pub unsafe fn image_view_from_raw(
        &self,
        raw_image: vk::Image,
        view_type: vk::ImageViewType,
        format: format::Format,
        swizzle: format::Swizzle,
        usage: image::Usage,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewCreationError> {
        let mut image_view_info;
        let mut info = vk::ImageViewCreateInfo::builder()
            .flags(vk::ImageViewCreateFlags::empty())
            .image(raw_image)
            .view_type(view_type)
            .format(conv::map_format(format))
            .components(conv::map_swizzle(swizzle))
            .subresource_range(conv::map_subresource_range(&range));

        if self.shared.image_view_usage {
            image_view_info = vk::ImageViewUsageCreateInfo::builder()
                .usage(conv::map_image_usage(usage))
                .build();
            info = info.push_next(&mut image_view_info);
        }

        let result = self.shared.raw.create_image_view(&info, None);

        match result {
            Ok(raw) => Ok(n::ImageView {
                image: raw_image,
                raw,
                range,
            }),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(d::OutOfMemory::Host.into()),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(d::OutOfMemory::Device.into()),
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send + Sync>() {}
    foo::<super::Device>()
}
