use ash::vk;
use ash::extensions as ext;
use ash::version::DeviceV1_0;
use smallvec::SmallVec;

use hal::{buffer, device as d, format, image, mapping, pass, pso, query, queue};
use hal::{Backbuffer, MemoryTypeId, SwapchainConfig};
use hal::error::HostExecutionError;
use hal::memory::Requirements;
use hal::pool::CommandPoolCreateFlags;
use hal::range::RangeArg;

use std::{mem, ptr};
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::ffi::CString;
use std::ops::Range;

use {Backend as B, Device};
use {conv, native as n, result, window as w};
use pool::RawCommandPool;


#[derive(Debug)]
pub struct UnboundBuffer(n::Buffer);

#[derive(Debug)]
pub struct UnboundImage(n::Image);

impl Device {
    #[cfg(feature = "glsl-to-spirv")]
    pub fn create_shader_module_from_glsl(
        &self,
        code: &str,
        stage: pso::Stage,
    ) -> Result<n::ShaderModule, d::ShaderError> {
        use self::d::Device;
        use std::io::Read;
        use glsl_to_spirv::{compile, ShaderType};

        let ty = match stage {
            pso::Stage::Vertex => ShaderType::Vertex,
            pso::Stage::Fragment => ShaderType::Fragment,
            pso::Stage::Geometry => ShaderType::Geometry,
            pso::Stage::Hull => ShaderType::TessellationControl,
            pso::Stage::Domain => ShaderType::TessellationEvaluation,
            pso::Stage::Compute => ShaderType::Compute,
        };

        match compile(code, ty) {
            Ok(mut file) => {
                let mut data = Vec::new();
                file.read_to_end(&mut data).unwrap();
                self.create_shader_module(&data)
            },
            Err(string) => Err(d::ShaderError::CompilationFailed(string)),
        }
    }
}

impl d::Device<B> for Device {
    fn allocate_memory(&self, mem_type: MemoryTypeId, size: u64) -> Result<n::Memory, d::OutOfMemory> {
        let info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MemoryAllocateInfo,
            p_next: ptr::null(),
            allocation_size: size,
            memory_type_index: mem_type.0 as _,
        };

        let memory = unsafe {
            self.raw.0.allocate_memory(&info, None)
        }.expect("Error on memory allocation"); // TODO: error handling

        Ok(n::Memory { raw: memory })
    }

    fn create_command_pool(
        &self, family: queue::QueueFamilyId, create_flags: CommandPoolCreateFlags
    ) -> RawCommandPool {
        let mut flags = vk::CommandPoolCreateFlags::empty();
        if create_flags.contains(CommandPoolCreateFlags::TRANSIENT) {
            flags |= vk::COMMAND_POOL_CREATE_TRANSIENT_BIT;
        }
        if create_flags.contains(CommandPoolCreateFlags::RESET_INDIVIDUAL) {
            flags |= vk::COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
        }

        let info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::CommandPoolCreateInfo,
            p_next: ptr::null(),
            flags,
            queue_family_index: family.0 as _,
        };

        let command_pool_raw = unsafe {
            self.raw.0
                .create_command_pool(&info, None)
        }.expect("Error on command pool creation"); // TODO: better error handling

        RawCommandPool {
            raw: command_pool_raw,
            device: self.raw.clone(),
        }
    }

    fn destroy_command_pool(&self, pool: RawCommandPool) {
        unsafe {
            self.raw.0
                .destroy_command_pool(pool.raw, None)
        };
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self, attachments: IA, subpasses: IS, dependencies: ID
    ) -> n::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        let map_subpass_ref = |pass: pass::SubpassRef| {
            match pass {
                pass::SubpassRef::External => vk::VK_SUBPASS_EXTERNAL,
                pass::SubpassRef::Pass(id) => id as u32,
            }
        };

        let attachments = attachments.into_iter().map(|attachment| {
            let attachment = attachment.borrow();
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(), // TODO: may even alias!
                format: attachment.format.map_or(vk::Format::Undefined, conv::map_format),
                samples: vk::SAMPLE_COUNT_1_BIT, // TODO: multisampling
                load_op: conv::map_attachment_load_op(attachment.ops.load),
                store_op: conv::map_attachment_store_op(attachment.ops.store),
                stencil_load_op: conv::map_attachment_load_op(attachment.stencil_ops.load),
                stencil_store_op: conv::map_attachment_store_op(attachment.stencil_ops.store),
                initial_layout: conv::map_image_layout(attachment.layouts.start),
                final_layout: conv::map_image_layout(attachment.layouts.end),
            }
        }).collect::<Vec<_>>();

        let mut attachment_refs = Vec::new();

        let subpasses = subpasses.into_iter().map(|subpass| {
            let subpass = subpass.borrow();
            {
                fn make_ref(&(id, layout): &pass::AttachmentRef) -> vk::AttachmentReference {
                    vk::AttachmentReference {
                        attachment: id as _,
                        layout: conv::map_image_layout(layout),
                    }
                }
                let colors = subpass.colors.iter()
                    .map(make_ref)
                    .collect::<Vec<_>>();
                let depth_stencil = subpass.depth_stencil
                    .map(make_ref);
                let inputs = subpass.inputs.iter()
                    .map(make_ref)
                    .collect::<Vec<_>>();
                let preserves = subpass.preserves.iter()
                    .map(|&id| id as u32)
                    .collect::<Vec<_>>();

                attachment_refs.push((colors, depth_stencil, inputs, preserves));
            }

            let &(ref color_attachments, ref depth_stencil, ref input_attachments, ref preserve_attachments) =
                attachment_refs.last().unwrap();

            vk::SubpassDescription {
                flags: vk::SubpassDescriptionFlags::empty(),
                pipeline_bind_point: vk::PipelineBindPoint::Graphics,
                input_attachment_count: input_attachments.len() as u32,
                p_input_attachments: input_attachments.as_ptr(),
                color_attachment_count: color_attachments.len() as u32,
                p_color_attachments: color_attachments.as_ptr(),
                p_resolve_attachments: ptr::null(), // TODO
                p_depth_stencil_attachment: match *depth_stencil {
                    Some(ref aref) => aref as *const _,
                    None => ptr::null(),
                },
                preserve_attachment_count: preserve_attachments.len() as u32,
                p_preserve_attachments: preserve_attachments.as_ptr(),
            }
        }).collect::<Vec<_>>();

        let dependencies = dependencies.into_iter().map(|dependency| {
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
        }).collect::<Vec<_>>();

        let info = vk::RenderPassCreateInfo {
            s_type: vk::StructureType::RenderPassCreateInfo,
            p_next: ptr::null(),
            flags: vk::RenderPassCreateFlags::empty(),
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            subpass_count: subpasses.len() as u32,
            p_subpasses: subpasses.as_ptr(),
            dependency_count: dependencies.len() as u32,
            p_dependencies: dependencies.as_ptr(),
        };

        let renderpass = unsafe {
            self.raw.0.create_render_pass(&info, None)
                .expect("Error on render pass creation") // TODO: handle this better
        };

        n::RenderPass { raw: renderpass }
    }

    fn create_pipeline_layout<IS, IR>(&self, sets: IS, push_constant_ranges: IR) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
    {
        let set_layouts = sets
            .into_iter()
            .map(|set| {
                set.borrow().raw
            }).collect::<Vec<_>>();

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
            }).collect::<Vec<_>>();

        let info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PipelineLayoutCreateInfo,
            p_next: ptr::null(),
            flags: vk::PipelineLayoutCreateFlags::empty(),
            set_layout_count: set_layouts.len() as u32,
            p_set_layouts: set_layouts.as_ptr(),
            push_constant_range_count: push_constant_ranges.len() as u32,
            p_push_constant_ranges: push_constant_ranges.as_ptr(),
        };

        let raw = unsafe {
            self.raw.0.create_pipeline_layout(&info, None)
                .expect("Error on pipeline signature creation") // TODO: handle this better
        };

        n::PipelineLayout { raw }
    }

    fn create_graphics_pipelines<'a, T>(
        &self, descs: T
    ) -> Vec<Result<n::GraphicsPipeline, pso::CreationError>>
    where
        T: IntoIterator,
        T::Item: Borrow<pso::GraphicsPipelineDesc<'a, B>>,
    {
        let descs = descs.into_iter().collect::<Vec<_>>();
        debug!("create_graphics_pipelines {:?}", descs.iter().map(Borrow::borrow).collect::<Vec<_>>());
        const NUM_STAGES: usize = 5;

        // Store pipeline parameters to avoid stack usage
        let mut info_stages                = Vec::with_capacity(descs.len());
        let mut info_vertex_descs          = Vec::with_capacity(descs.len());
        let mut info_vertex_input_states   = Vec::with_capacity(descs.len());
        let mut info_input_assembly_states = Vec::with_capacity(descs.len());
        let mut info_tessellation_states   = Vec::with_capacity(descs.len());
        let mut info_viewport_states       = Vec::with_capacity(descs.len());
        let mut info_rasterization_states  = Vec::with_capacity(descs.len());
        let mut info_multisample_states    = Vec::with_capacity(descs.len());
        let mut info_depth_stencil_states  = Vec::with_capacity(descs.len());
        let mut info_color_blend_states    = Vec::with_capacity(descs.len());
        let mut info_dynamic_states        = Vec::with_capacity(descs.len());
        let mut color_attachments          = Vec::with_capacity(descs.len());
        let mut info_specializations       = Vec::with_capacity(descs.len() * NUM_STAGES);
        let mut specialization_data        = Vec::with_capacity(descs.len() * NUM_STAGES);

        let dynamic_states = [vk::DynamicState::Viewport, vk::DynamicState::Scissor];
        let mut c_strings = Vec::new(); // hold the C strings temporarily
        let mut make_stage = |stage, source: &pso::EntryPoint<'a, B>| {
            let string = CString::new(source.entry).unwrap();
            let p_name = string.as_ptr();
            c_strings.push(string);

            let mut data = SmallVec::<[u8; 64]>::new();
            let map_entries = conv::map_specialization_constants(
                &source.specialization,
                &mut data,
            ).unwrap();

            specialization_data.push((data, map_entries));
            let &(ref data, ref map_entries) = specialization_data.last().unwrap();

            info_specializations.push(vk::SpecializationInfo {
                map_entry_count: map_entries.len() as _,
                p_map_entries: map_entries.as_ptr(),
                data_size: data.len() as _,
                p_data: data.as_ptr() as _,
            });
            let info = info_specializations.last().unwrap();

            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                stage,
                module: source.module.raw,
                p_name,
                p_specialization_info: info,
            }
        };

        let infos = descs.iter().map(|desc| {
            let desc = desc.borrow();
            let mut stages = Vec::new();
            // Vertex stage
            if true { //vertex shader is required
                stages.push(make_stage(vk::SHADER_STAGE_VERTEX_BIT, &desc.shaders.vertex));
            }
            // Pixel stage
            if let Some(ref entry) = desc.shaders.fragment {
                stages.push(make_stage(vk::SHADER_STAGE_FRAGMENT_BIT, entry));
            }
            // Geometry stage
            if let Some(ref entry) = desc.shaders.geometry {
                stages.push(make_stage(vk::SHADER_STAGE_GEOMETRY_BIT, entry));
            }
            // Domain stage
            if let Some(ref entry) = desc.shaders.domain {
                stages.push(make_stage(vk::SHADER_STAGE_TESSELLATION_EVALUATION_BIT, entry));
            }
            // Hull stage
            if let Some(ref entry) = desc.shaders.hull {
                stages.push(make_stage(vk::SHADER_STAGE_TESSELLATION_CONTROL_BIT, entry));
            }

            let (polygon_mode, line_width) = conv::map_polygon_mode(desc.rasterizer.polygon_mode);
            info_stages.push(stages);

            {
                let mut vertex_bindings = Vec::new();
                for (i, vbuf) in desc.vertex_buffers.iter().enumerate() {
                    vertex_bindings.push(vk::VertexInputBindingDescription {
                        binding: i as u32,
                        stride: vbuf.stride as u32,
                        input_rate: if vbuf.rate == 0 {
                            vk::VertexInputRate::Vertex
                        } else {
                            vk::VertexInputRate::Instance
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

            let &(ref vertex_bindings, ref vertex_attributes) = info_vertex_descs.last().unwrap();

            info_vertex_input_states.push(vk::PipelineVertexInputStateCreateInfo {
                s_type: vk::StructureType::PipelineVertexInputStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineVertexInputStateCreateFlags::empty(),
                vertex_binding_description_count: vertex_bindings.len() as u32,
                p_vertex_binding_descriptions: vertex_bindings.as_ptr(),
                vertex_attribute_description_count: vertex_attributes.len() as u32,
                p_vertex_attribute_descriptions: vertex_attributes.as_ptr(),
            });

            info_input_assembly_states.push(vk::PipelineInputAssemblyStateCreateInfo {
                s_type: vk::StructureType::PipelineInputAssemblyStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
                topology: conv::map_topology(desc.input_assembler.primitive),
                primitive_restart_enable: vk::VK_FALSE,
            });

            info_rasterization_states.push(vk::PipelineRasterizationStateCreateInfo {
                s_type: vk::StructureType::PipelineRasterizationStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineRasterizationStateCreateFlags::empty(),
                depth_clamp_enable: if desc.rasterizer.depth_clamping { vk::VK_TRUE } else { vk::VK_FALSE },
                rasterizer_discard_enable: if desc.shaders.fragment.is_none() { vk::VK_TRUE } else { vk::VK_FALSE },
                polygon_mode: polygon_mode,
                cull_mode: desc.rasterizer.cull_face.map(conv::map_cull_face).unwrap_or(vk::CULL_MODE_NONE),
                front_face: conv::map_front_face(desc.rasterizer.front_face),
                depth_bias_enable: if desc.rasterizer.depth_bias.is_some() { vk::VK_TRUE } else { vk::VK_FALSE },
                depth_bias_constant_factor: desc.rasterizer.depth_bias.map_or(0.0, |off| off.const_factor),
                depth_bias_clamp: desc.rasterizer.depth_bias.map_or(0.0, |off| off.clamp),
                depth_bias_slope_factor: desc.rasterizer.depth_bias.map_or(0.0, |off| off.slope_factor),
                line_width: line_width,
            });

            let is_tessellated = desc.shaders.hull.is_some() && desc.shaders.domain.is_some();
            if is_tessellated {
                info_tessellation_states.push(vk::PipelineTessellationStateCreateInfo {
                    s_type: vk::StructureType::PipelineTessellationStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineTessellationStateCreateFlags::empty(),
                    patch_control_points: 1 // TODO: 0 < control_points <= VkPhysicalDeviceLimits::maxTessellationPatchSize
                });
            }

            info_viewport_states.push(vk::PipelineViewportStateCreateInfo {
                s_type: vk::StructureType::PipelineViewportStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineViewportStateCreateFlags::empty(),
                scissor_count: 1, // TODO:
                p_scissors: ptr::null(), // dynamic
                viewport_count: 1, // TODO:
                p_viewports: ptr::null(), // dynamic
            });

            info_multisample_states.push(vk::PipelineMultisampleStateCreateInfo {
                s_type: vk::StructureType::PipelineMultisampleStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineMultisampleStateCreateFlags::empty(),
                rasterization_samples: vk::SAMPLE_COUNT_1_BIT, // TODO
                sample_shading_enable: vk::VK_FALSE, // TODO
                min_sample_shading: 0.0,  // TODO
                p_sample_mask: ptr::null(), // TODO
                alpha_to_coverage_enable: vk::VK_FALSE, // TODO
                alpha_to_one_enable: vk::VK_FALSE, // TODO
            });

            let depth_stencil = desc.depth_stencil.unwrap_or_default();
            let (depth_test_enable, depth_write_enable, depth_compare_op) = match depth_stencil.depth {
                pso::DepthTest::On { fun, write } => (vk::VK_TRUE, write as _, conv::map_comparison(fun)),
                pso::DepthTest::Off => (vk::VK_FALSE, vk::VK_FALSE, vk::CompareOp::Never),
            };
            let (stencil_test_enable, front, back) = match depth_stencil.stencil {
                pso::StencilTest::On { ref front, ref back } => (
                    vk::VK_TRUE,
                    conv::map_stencil_side(front),
                    conv::map_stencil_side(back),
                ),
                pso::StencilTest::Off => unsafe { mem::zeroed() },
            };

            info_depth_stencil_states.push(vk::PipelineDepthStencilStateCreateInfo {
                s_type: vk::StructureType::PipelineDepthStencilStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
                depth_test_enable,
                depth_write_enable,
                depth_compare_op,
                depth_bounds_test_enable: depth_stencil.depth_bounds as _,
                stencil_test_enable,
                front,
                back,
                min_depth_bounds: 0.0,
                max_depth_bounds: 1.0,
            });

            // Build blend states for color attachments
            let blend_states = desc.blender.targets
                .iter()
                .map(|&pso::ColorBlendDesc(mask, ref blend)| {
                    let color_write_mask = vk::ColorComponentFlags::from_flags(mask.bits() as _).unwrap();
                    match *blend {
                        pso::BlendState::On { color, alpha } => {
                            let (color_blend_op, src_color_blend_factor, dst_color_blend_factor) = conv::map_blend_op(color);
                            let (alpha_blend_op, src_alpha_blend_factor, dst_alpha_blend_factor) = conv::map_blend_op(alpha);
                            vk::PipelineColorBlendAttachmentState {
                                color_write_mask,
                                blend_enable: vk::VK_TRUE,
                                src_color_blend_factor,
                                dst_color_blend_factor,
                                color_blend_op,
                                src_alpha_blend_factor,
                                dst_alpha_blend_factor,
                                alpha_blend_op,
                            }
                        },
                        pso::BlendState::Off => vk::PipelineColorBlendAttachmentState {
                            color_write_mask,
                            .. unsafe { mem::zeroed() }
                        },
                    }
                })
                .collect::<Vec<_>>();
            color_attachments.push(blend_states);

            info_color_blend_states.push(vk::PipelineColorBlendStateCreateInfo {
                s_type: vk::StructureType::PipelineColorBlendStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineColorBlendStateCreateFlags::empty(),
                logic_op_enable: vk::VK_FALSE, // TODO
                logic_op: vk::LogicOp::Clear,
                attachment_count: color_attachments.last().unwrap().len() as _,
                p_attachments: color_attachments.last().unwrap().as_ptr(), // TODO:
                blend_constants: [0.0; 4], // TODO:
            });

            info_dynamic_states.push(vk::PipelineDynamicStateCreateInfo {
                s_type: vk::StructureType::PipelineDynamicStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineDynamicStateCreateFlags::empty(),
                dynamic_state_count: dynamic_states.len() as _,
                p_dynamic_states: dynamic_states.as_ptr(),
            });

            let (base_handle, base_index) = match desc.parent {
                pso::BasePipeline::Pipeline(pipeline) => (pipeline.0, -1),
                pso::BasePipeline::Index(index) => (vk::Pipeline::null(), index as _),
                pso::BasePipeline::None => (vk::Pipeline::null(), -1),
            };

            let mut flags = vk::PipelineCreateFlags::empty();
            match desc.parent {
                pso::BasePipeline::None => (),
                _ => { flags |= vk::PIPELINE_CREATE_DERIVATIVE_BIT; }
            }
            if desc.flags.contains(pso::PipelineCreationFlags::DISABLE_OPTIMIZATION) {
                flags |= vk::PIPELINE_CREATE_DISABLE_OPTIMIZATION_BIT;
            }
            if desc.flags.contains(pso::PipelineCreationFlags::ALLOW_DERIVATIVES) {
                flags |= vk::PIPELINE_CREATE_ALLOW_DERIVATIVES_BIT;
            }

            Ok(vk::GraphicsPipelineCreateInfo {
                s_type: vk::StructureType::GraphicsPipelineCreateInfo,
                p_next: ptr::null(),
                flags,
                stage_count: info_stages.last().unwrap().len() as _,
                p_stages: info_stages.last().unwrap().as_ptr(),
                p_vertex_input_state: info_vertex_input_states.last().unwrap(),
                p_input_assembly_state: info_input_assembly_states.last().unwrap(),
                p_rasterization_state: info_rasterization_states.last().unwrap(),
                p_tessellation_state: if is_tessellated { info_tessellation_states.last().unwrap() } else { ptr::null() },
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
        }).collect::<Vec<_>>();

        let valid_infos = infos.iter().filter_map(|info| info.clone().ok()).collect::<Vec<_>>();
        let result = if valid_infos.is_empty() {
            Ok(Vec::new())
        } else {
            unsafe {
                self.raw.0.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &valid_infos,
                    None,
                )
            }
        };

        match result {
            Ok(pipelines) |
            Err((pipelines, _))=> {
                let mut psos = pipelines.into_iter();
                infos
                    .into_iter()
                    .map(|result| result.and_then(|_| {
                        let pso = psos.next().unwrap();
                        if pso == vk::Pipeline::null() {
                            Err(pso::CreationError::Other)
                        } else {
                            Ok(n::GraphicsPipeline(pso))
                        }
                    }))
                    .collect()
            }
        }
    }

    fn create_compute_pipelines<'a, T>(
        &self, descs: T
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>>
    where
        T: IntoIterator,
        T::Item: Borrow<pso::ComputePipelineDesc<'a, B>>,
    {
        let descs = descs.into_iter().collect::<Vec<_>>();
        let mut c_strings = Vec::new(); // hold the C strings temporarily
        let mut info_specializations = Vec::with_capacity(descs.len());
        let mut specialization_data = Vec::with_capacity(descs.len());

        let infos = descs.iter().map(|desc| {
            let desc = desc.borrow();
            let string = CString::new(desc.shader.entry).unwrap();
            let p_name = string.as_ptr();
            c_strings.push(string);

            let mut data = SmallVec::<[u8; 64]>::new();
            let map_entries = conv::map_specialization_constants(
                &desc.shader.specialization,
                &mut data,
            ).unwrap();

            specialization_data.push((data, map_entries));
            let &(ref data, ref map_entries) = specialization_data.last().unwrap();

            info_specializations.push(vk::SpecializationInfo {
                map_entry_count: map_entries.len() as _,
                p_map_entries: map_entries.as_ptr(),
                data_size: data.len() as _,
                p_data: data.as_ptr() as _,
            });
            let info = info_specializations.last().unwrap();

            let stage = vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                stage: vk::SHADER_STAGE_COMPUTE_BIT,
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
                _ => { flags |= vk::PIPELINE_CREATE_DERIVATIVE_BIT; }
            }
            if desc.flags.contains(pso::PipelineCreationFlags::DISABLE_OPTIMIZATION) {
                flags |= vk::PIPELINE_CREATE_DISABLE_OPTIMIZATION_BIT;
            }
            if desc.flags.contains(pso::PipelineCreationFlags::ALLOW_DERIVATIVES) {
                flags |= vk::PIPELINE_CREATE_ALLOW_DERIVATIVES_BIT;
            }

            Ok(vk::ComputePipelineCreateInfo {
                s_type: vk::StructureType::ComputePipelineCreateInfo,
                p_next: ptr::null(),
                flags,
                stage,
                layout: desc.layout.raw,
                base_pipeline_handle: base_handle,
                base_pipeline_index: base_index,
            })
        }).collect::<Vec<_>>();

        let valid_infos = infos.iter().filter_map(|info| info.clone().ok()).collect::<Vec<_>>();
        let result = if valid_infos.is_empty() {
            Ok(Vec::new())
        } else {
            unsafe {
                self.raw.0.create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &valid_infos,
                    None,
                )
            }
        };

        match result {
            Ok(pipelines) |
            Err((pipelines, _))=> {
                let mut psos = pipelines.into_iter();
                infos
                    .into_iter()
                    .map(|result| result.and_then(|_| {
                        let pso = psos.next().unwrap();
                        if pso == vk::Pipeline::null() {
                            Err(pso::CreationError::Other)
                        } else {
                            Ok(n::ComputePipeline(pso))
                        }
                    }))
                    .collect()
            }
        }
    }

    fn create_framebuffer<T>(
        &self,
        renderpass: &n::RenderPass,
        attachments: T,
        extent: d::Extent,
    ) -> Result<n::FrameBuffer, d::FramebufferError>
    where
        T: IntoIterator,
        T::Item: Borrow<n::ImageView>,
    {
        let attachments_raw = attachments
            .into_iter()
            .map(|attachment| attachment.borrow().view)
            .collect::<SmallVec<[_; 4]>>();

        let info = vk::FramebufferCreateInfo {
            s_type: vk::StructureType::FramebufferCreateInfo,
            p_next: ptr::null(),
            flags: vk::FramebufferCreateFlags::empty(),
            render_pass: renderpass.raw,
            attachment_count: attachments_raw.len() as u32,
            p_attachments: attachments_raw.as_ptr(),
            width: extent.width,
            height: extent.height,
            layers: extent.depth,
        };

        let framebuffer = unsafe {
            self.raw.0.create_framebuffer(&info, None)
        }.expect("error on framebuffer creation");

        Ok(n::FrameBuffer { raw: framebuffer })
    }

    fn create_shader_module(&self, spirv_data: &[u8]) -> Result<n::ShaderModule, d::ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(spirv_data.len() & 3, 0);

        let info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::ShaderModuleCreateInfo,
            p_next: ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: spirv_data.len(),
            p_code: spirv_data as *const _ as *const u32,
        };

        let module = unsafe {
            self.raw.0.create_shader_module(&info, None)
        };

        match module {
            Ok(raw) => Ok(n::ShaderModule { raw }),
            Err(e) => {
                error!("Shader module error {:?}", e);
                Err(d::ShaderError::CompilationFailed(String::new())) // TODO
            }
        }
    }

    fn create_sampler(&self, sampler_info: image::SamplerInfo) -> n::Sampler {
        use hal::pso::Comparison;

        let (min_filter, mag_filter, mipmap_mode, aniso) = conv::map_filter(sampler_info.filter);
        let info = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SamplerCreateInfo,
            p_next: ptr::null(),
            flags: vk::SamplerCreateFlags::empty(),
            mag_filter,
            min_filter,
            mipmap_mode,
            address_mode_u: conv::map_wrap(sampler_info.wrap_mode.0),
            address_mode_v: conv::map_wrap(sampler_info.wrap_mode.1),
            address_mode_w: conv::map_wrap(sampler_info.wrap_mode.2),
            mip_lod_bias: sampler_info.lod_bias.into(),
            anisotropy_enable: if aniso > 1.0 { vk::VK_TRUE } else { vk::VK_FALSE },
            max_anisotropy: aniso,
            compare_enable: if sampler_info.comparison.is_some() { vk::VK_TRUE } else { vk::VK_FALSE },
            compare_op: conv::map_comparison(sampler_info.comparison.unwrap_or(Comparison::Never)),
            min_lod: sampler_info.lod_range.start.into(),
            max_lod: sampler_info.lod_range.end.into(),
            border_color: match conv::map_border_color(sampler_info.border) {
                Some(bc) => bc,
                None => {
                    error!("Unsupported border color {:x}", sampler_info.border.0);
                    vk::BorderColor::FloatTransparentBlack
                }
            },
            unnormalized_coordinates: vk::VK_FALSE,
        };

        let sampler = unsafe {
            self.raw.0.create_sampler(&info, None)
                        .expect("error on sampler creation")
        };

        n::Sampler(sampler)
    }

    ///
    fn create_buffer(&self, size: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        let info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BufferCreateInfo,
            p_next: ptr::null(),
            flags: vk::BufferCreateFlags::empty(), // TODO:
            size,
            usage: conv::map_buffer_usage(usage),
            sharing_mode: vk::SharingMode::Exclusive, // TODO:
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
        };

        let buffer = unsafe {
            self.raw.0.create_buffer(&info, None)
                .expect("Error on buffer creation") // TODO: error handling
        };

        Ok(UnboundBuffer(n::Buffer { raw: buffer }))
    }

    fn get_buffer_requirements(&self, buffer: &UnboundBuffer) -> Requirements {
        let req = self.raw.0.get_buffer_memory_requirements((buffer.0).raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: req.memory_type_bits as _,
        }
    }

    fn bind_buffer_memory(&self, memory: &n::Memory, offset: u64, buffer: UnboundBuffer) -> Result<n::Buffer, d::BindError> {
        assert_eq!(Ok(()), unsafe {
            self.raw.0.bind_buffer_memory((buffer.0).raw, memory.raw, offset)
        });

        let buffer = n::Buffer {
            raw: buffer.0.raw,
        };

        Ok(buffer)
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self, buffer: &n::Buffer, format: Option<format::Format>, range: R
    ) -> Result<n::BufferView, buffer::ViewError> {
        let (offset, size) = conv::map_range_arg(&range);
        let info = vk::BufferViewCreateInfo {
            s_type: vk::StructureType::BufferViewCreateInfo,
            p_next: ptr::null(),
            flags: vk::BufferViewCreateFlags::empty(),
            buffer: buffer.raw,
            format: format.map_or(vk::Format::Undefined, conv::map_format),
            offset,
            range: size,
        };

        let view = unsafe {
            self.raw.0.create_buffer_view(&info, None)
        }.expect("Error on buffer view creation"); //TODO: Proper error handling

        Ok(n::BufferView { raw: view })
    }

    fn create_image(&self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError>
    {
        use hal::image::Kind::*;

        let flags = match kind {
            Cube(_) => vk::IMAGE_CREATE_CUBE_COMPATIBLE_BIT,
            CubeArray(_, _) => vk::IMAGE_CREATE_CUBE_COMPATIBLE_BIT,
            _ => vk::ImageCreateFlags::empty(),
        };

        let (image_type, extent, array_layers, aa_mode) = match kind {
            D1(width) => (
                vk::ImageType::Type1d,
                vk::Extent3D { width: width as u32, height: 1, depth: 1 },
                1,
                image::AaMode::Single,
            ),
            D1Array(width, layers) => (
                vk::ImageType::Type1d,
                vk::Extent3D { width: width as u32, height: 1, depth: 1 },
                layers,
                image::AaMode::Single,
            ),
            D2(width, height, aa_mode) => (
                vk::ImageType::Type2d,
                vk::Extent3D { width: width as u32, height: height as u32, depth: 1 },
                1,
                aa_mode,
            ),
            D2Array(width, height, layers, aa_mode) => (
                vk::ImageType::Type2d,
                vk::Extent3D { width: width as u32, height: height as u32, depth: 1 },
                layers,
                aa_mode,
            ),
            D3(width, height, depth) => (
                vk::ImageType::Type3d,
                vk::Extent3D { width: width as u32, height: height as u32, depth: depth as u32 },
                1,
                image::AaMode::Single,
            ),
            Cube(size) => (
                vk::ImageType::Type2d,
                vk::Extent3D { width: size as u32, height: size as u32, depth: 1 },
                6,
                image::AaMode::Single,
            ),
            CubeArray(size, layers) => (
                vk::ImageType::Type2d,
                vk::Extent3D { width: size as u32, height: size as u32, depth: 1 },
                6 * layers,
                image::AaMode::Single,
            ),
        };

        let samples = match aa_mode {
            image::AaMode::Single => vk::SAMPLE_COUNT_1_BIT,
            _ => unimplemented!(),
        };

        let info = vk::ImageCreateInfo {
            s_type: vk::StructureType::ImageCreateInfo,
            p_next: ptr::null(),
            flags,
            image_type,
            format: conv::map_format(format),
            extent: extent.clone(),
            mip_levels: mip_levels as u32,
            array_layers: array_layers as u32,
            samples,
            tiling: vk::ImageTiling::Optimal, // TODO: read back?
            usage: conv::map_image_usage(usage),
            sharing_mode: vk::SharingMode::Exclusive, // TODO:
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            initial_layout: vk::ImageLayout::Undefined,
        };

        let raw = unsafe {
            self.raw.0.create_image(&info, None)
                .expect("Error on image creation") // TODO: error handling
        };

        Ok(UnboundImage(n::Image{ raw, extent }))
    }

    fn get_image_requirements(&self, image: &UnboundImage) -> Requirements {
        let req = self.raw.0.get_image_memory_requirements(image.0.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
            type_mask: req.memory_type_bits as _,
        }
    }

    fn bind_image_memory(&self, memory: &n::Memory, offset: u64, image: UnboundImage) -> Result<n::Image, d::BindError> {
        // TODO: error handling
        // TODO: check required type
        assert_eq!(Ok(()), unsafe {
            self.raw.0.bind_image_memory(image.0.raw, memory.raw, offset)
        });

        Ok(image.0)
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        let subresource_range = vk::ImageSubresourceRange {
            aspect_mask: conv::map_image_aspects(range.aspects),
            base_mip_level: range.levels.start as _,
            level_count: (range.levels.end - range.levels.start) as _,
            base_array_layer: range.layers.start as _,
            layer_count: (range.layers.end - range.layers.start) as _,
        };

        let info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::ImageViewCreateInfo,
            p_next: ptr::null(),
            flags: vk::ImageViewCreateFlags::empty(), // TODO
            image: image.raw,
            view_type: vk::ImageViewType::Type2d, // TODO
            format: conv::map_format(format),
            components: conv::map_swizzle(swizzle),
            subresource_range,
        };

        let view = unsafe {
            self.raw.0.create_image_view(&info, None)
        }.expect("Error on image view creation"); // TODO

        Ok(n::ImageView {
            image: image.raw,
            view,
            range,
        })
    }

    fn create_descriptor_pool<T>(&self, max_sets: usize, descriptor_pools: T) -> n::DescriptorPool
    where
        T: IntoIterator,
        T::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        let pools = descriptor_pools.into_iter().map(|pool| {
            let pool = pool.borrow();
            vk::DescriptorPoolSize {
                typ: conv::map_descriptor_type(pool.ty),
                descriptor_count: pool.count as u32,
            }
        }).collect::<Vec<_>>();

        let info = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DescriptorPoolCreateInfo,
            p_next: ptr::null(),
            flags: vk::DescriptorPoolCreateFlags::empty(), // disallow individual freeing
            max_sets: max_sets as u32,
            pool_size_count: pools.len() as u32,
            p_pool_sizes: pools.as_ptr(),
        };

        let pool = unsafe {
            self.raw.0.create_descriptor_pool(&info, None)
                        .expect("Error on descriptor set pool creation") // TODO
        };

        n::DescriptorPool {
            raw: pool,
            device: self.raw.clone(),
        }
    }

    fn create_descriptor_set_layout<T>(&self, bindings: T)-> n::DescriptorSetLayout
    where
        T: IntoIterator,
        T::Item: Borrow<pso::DescriptorSetLayoutBinding>,
    {
        let bindings = bindings.into_iter().map(|binding| {
            let binding = binding.borrow();
            vk::DescriptorSetLayoutBinding {
                binding: binding.binding as u32,
                descriptor_type: conv::map_descriptor_type(binding.ty),
                descriptor_count: binding.count as u32,
                stage_flags: conv::map_stage_flags(binding.stage_flags),
                p_immutable_samplers: ptr::null(), // TODO
            }
        }).collect::<Vec<_>>();

        debug!("create_descriptor_set_layout {:?}", bindings);

        let info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DescriptorSetLayoutCreateInfo,
            p_next: ptr::null(),
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            binding_count: bindings.len() as u32,
            p_bindings: bindings.as_ptr(),
        };

        let layout = unsafe {
            self.raw.0.create_descriptor_set_layout(&info, None)
        }.expect("Error on descriptor set layout creation"); // TODO

        n::DescriptorSetLayout {
            raw: layout,
        }
    }

    fn update_descriptor_sets<'a, I, R>(&self, writes: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetWrite<'a, 'a, B, R>>,
        R: RangeArg<u64>
    {
        let writes = writes.into_iter().collect::<Vec<_>>();

        let mut image_infos = Vec::new();
        let mut buffer_infos = Vec::new();
        let mut texel_buffer_views = Vec::new();

        for write in &writes {
            let write = write.borrow();
            match write.write {
                pso::DescriptorWrite::Sampler(ref samplers) => {
                    for sampler in samplers {
                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: sampler.0,
                            image_view: vk::ImageView::null(),
                            image_layout: vk::ImageLayout::General
                        });
                    }
                }
                pso::DescriptorWrite::SampledImage(ref images) |
                pso::DescriptorWrite::StorageImage(ref images) |
                pso::DescriptorWrite::InputAttachment(ref images) => {
                    for &(view, layout) in images {
                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: vk::Sampler::null(),
                            image_view: view.view,
                            image_layout: conv::map_image_layout(layout),
                        });
                    }
                }
                pso::DescriptorWrite::UniformBuffer(ref buffers) |
                pso::DescriptorWrite::StorageBuffer(ref buffers) => {
                    for &(buffer, ref range) in buffers {
                        let (offset, size) = conv::map_range_arg(range);
                        buffer_infos.push(vk::DescriptorBufferInfo {
                            buffer: buffer.raw,
                            offset,
                            range: size,
                        });
                    }
                }
                pso::DescriptorWrite::UniformTexelBuffer(ref views) |
                pso::DescriptorWrite::StorageTexelBuffer(ref views) => {
                    for view in views {
                        texel_buffer_views.push(view.raw)
                    }
                }
            };
        }

        // Track current subslice for each write
        let mut cur_image_index = 0;
        let mut cur_buffer_index = 0;
        let mut cur_view_index = 0;

        let writes = writes.iter().map(|write| {
            let write = write.borrow();
            let ty = match write.write {
                pso::DescriptorWrite::Sampler(_) => vk::DescriptorType::Sampler,
                pso::DescriptorWrite::SampledImage(_) => vk::DescriptorType::SampledImage,
                pso::DescriptorWrite::StorageImage(_) => vk::DescriptorType::StorageImage,
                pso::DescriptorWrite::InputAttachment(_) => vk::DescriptorType::InputAttachment,
                pso::DescriptorWrite::UniformBuffer(_) => vk::DescriptorType::UniformBuffer,
                pso::DescriptorWrite::StorageBuffer(_) => vk::DescriptorType::StorageBuffer,
                pso::DescriptorWrite::UniformTexelBuffer(_) => vk::DescriptorType::UniformTexelBuffer,
                pso::DescriptorWrite::StorageTexelBuffer(_) => vk::DescriptorType::StorageTexelBuffer,
            };

            let (count, image_info, buffer_info, texel_buffer_view) = match write.write {
                pso::DescriptorWrite::Sampler(ref samplers) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += samplers.len();
                    (samplers.len(), info_ptr, ptr::null(), ptr::null())
                }
                pso::DescriptorWrite::SampledImage(ref images) |
                pso::DescriptorWrite::StorageImage(ref images) |
                pso::DescriptorWrite::InputAttachment(ref images) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += images.len();
                    (images.len(), info_ptr, ptr::null(), ptr::null())
                }
                pso::DescriptorWrite::UniformBuffer(ref buffers) |
                pso::DescriptorWrite::StorageBuffer(ref buffers) => {
                    let info_ptr = &buffer_infos[cur_buffer_index] as *const _;
                    cur_buffer_index += buffers.len();
                    (buffers.len(), ptr::null(), info_ptr, ptr::null())
                }
                pso::DescriptorWrite::UniformTexelBuffer(ref views) |
                pso::DescriptorWrite::StorageTexelBuffer(ref views) => {
                    let info_ptr = &texel_buffer_views[cur_view_index] as *const _;
                    cur_view_index += views.len();
                    (views.len(), ptr::null(), ptr::null(), info_ptr)
                }
            };

            vk::WriteDescriptorSet {
                s_type: vk::StructureType::WriteDescriptorSet,
                p_next: ptr::null(),
                dst_set: write.set.raw,
                dst_binding: write.binding as u32,
                dst_array_element: write.array_offset as u32,
                descriptor_count: count as u32,
                descriptor_type: ty,
                p_image_info: image_info,
                p_buffer_info: buffer_info,
                p_texel_buffer_view: texel_buffer_view,
            }
        }).collect::<Vec<_>>();

        unsafe {
            self.raw.0.update_descriptor_sets(&writes, &[]);
        }
    }

    fn map_memory<R>(&self, memory: &n::Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>,
    {
        let (offset, size) = conv::map_range_arg(&range);
        let ptr = unsafe {
            self.raw.0.map_memory(
                memory.raw,
                offset,
                size,
                vk::MemoryMapFlags::empty(),
            ).expect("Error on memory mapping") // TODO
        };

        Ok(ptr as *mut _)
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        unsafe { self.raw.0.unmap_memory(memory.raw) }
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let ranges = conv::map_memory_ranges(ranges);
        unsafe {
            self.raw.0
                .flush_mapped_memory_ranges(&ranges)
                .expect("Memory flush failed"); // TODO
        }
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let ranges = conv::map_memory_ranges(ranges);
        unsafe {
            self.raw.0
                .invalidate_mapped_memory_ranges(&ranges)
                .expect("Memory invalidation failed"); // TODO
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        let info = vk::SemaphoreCreateInfo {
            s_type: vk::StructureType::SemaphoreCreateInfo,
            p_next: ptr::null(),
            flags: vk::SemaphoreCreateFlags::empty(),
        };

        let semaphore = unsafe {
            self.raw.0.create_semaphore(&info, None)
                        .expect("Error on semaphore creation") // TODO: error handling
        };

        n::Semaphore(semaphore)
    }

    fn create_fence(&self, signaled: bool) -> n::Fence {
        let info = vk::FenceCreateInfo {
            s_type: vk::StructureType::FenceCreateInfo,
            p_next: ptr::null(),
            flags: if signaled {
                vk::FENCE_CREATE_SIGNALED_BIT
            } else {
                vk::FenceCreateFlags::empty()
            },
        };

        let fence = unsafe {
            self.raw.0.create_fence(&info, None)
                        .expect("Error on fence creation") // TODO: error handling
        };

        n::Fence(fence)
    }

    fn reset_fences<I>(&self, fences: I)
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let fences = fences.into_iter().map(|fence| fence.borrow().0).collect::<Vec<_>>();
        assert_eq!(Ok(()), unsafe {
            self.raw.0.reset_fences(&fences)
        });
    }

    fn wait_for_fences<I>(&self, fences: I, wait: d::WaitFor, timeout_ms: u32) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<n::Fence>,
    {
        let fences = fences.into_iter().map(|fence| fence.borrow().0).collect::<Vec<_>>();
        let all = match wait {
            d::WaitFor::Any => false,
            d::WaitFor::All => true,
        };
        let result = unsafe {
            self.raw.0.wait_for_fences(&fences, all, timeout_ms as u64 * 1000)
        };
        match result {
            Ok(()) | Err(vk::Result::Success) => true,
            Err(vk::Result::Timeout) => false,
            _ => panic!("Unexpected wait result {:?}", result),
        }
    }

    fn get_fence_status(&self, fence: &n::Fence) -> bool {
        let result = unsafe {
            self.raw.0.get_fence_status(fence.0)
        };
        match result {
            Ok(()) | Err(vk::Result::Success) => true,
            Err(vk::Result::NotReady) => false,
            _ => panic!("Unexpected get_fence_status result {:?}", result),
        }
    }

    fn free_memory(&self, memory: n::Memory) {
        unsafe { self.raw.0.free_memory(memory.raw, None); }
    }

    fn create_query_pool(&self, ty: query::QueryType, query_count: u32) -> n::QueryPool {
        let (query_type, pipeline_statistics) = match ty {
            query::QueryType::Occlusion =>
                (vk::QueryType::Occlusion, vk::QueryPipelineStatisticFlags::empty()),
            query::QueryType::PipelineStatistics(statistics) =>
                (vk::QueryType::PipelineStatistics, conv::map_pipeline_statistics(statistics)),
            query::QueryType::Timestamp =>
                (vk::QueryType::Timestamp, vk::QueryPipelineStatisticFlags::empty()),
        };

        let info = vk::QueryPoolCreateInfo {
            s_type: vk::StructureType::QueryPoolCreateInfo,
            p_next: ptr::null(),
            flags: vk::QueryPoolCreateFlags::empty(),
            query_type,
            query_count,
            pipeline_statistics
        };

        let pool = unsafe {
            self.raw.0.create_query_pool(&info, None)
                        .expect("Error on query pool creation") // TODO: error handling
        };

        n::QueryPool(pool)
    }

    fn create_swapchain(
        &self,
        surface: &mut w::Surface,
        config: SwapchainConfig,
    ) -> (w::Swapchain, Backbuffer<B>) {
        let functor = ext::Swapchain::new(&surface.raw.instance.0, &self.raw.0)
            .expect("Unable to query swapchain function");

        // TODO: check for better ones if available
        let present_mode = vk::PresentModeKHR::Fifo; // required to be supported

        // TODO: handle depth stencil
        let format = config.color_format;

        let info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SwapchainCreateInfoKhr,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: surface.raw.handle,
            min_image_count: config.image_count,
            image_format: conv::map_format(format),
            image_color_space: vk::ColorSpaceKHR::SrgbNonlinear,
            image_extent: vk::Extent2D {
                width: surface.width,
                height: surface.height,
            },
            image_array_layers: 1,
            image_usage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            image_sharing_mode: vk::SharingMode::Exclusive,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
            composite_alpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
            present_mode: present_mode,
            clipped: 1,
            old_swapchain: vk::SwapchainKHR::null(),
        };

        let swapchain_raw = unsafe { functor.create_swapchain_khr(&info, None) }
            .expect("Unable to create a swapchain");

        let backbuffer_images = functor.get_swapchain_images_khr(swapchain_raw)
            .expect("Unable to get swapchain images");

        let swapchain = w::Swapchain {
            raw: swapchain_raw,
            functor,
            frame_queue: VecDeque::new(),
        };

        let images = backbuffer_images
            .into_iter()
            .map(|image| {
                n::Image {
                    raw: image,
                    extent: vk::Extent3D {
                        width: surface.width,
                        height: surface.height,
                        depth: 1,
                    },
                }
            })
            .collect();

        (swapchain, Backbuffer::Images(images))
    }

    fn destroy_query_pool(&self, pool: n::QueryPool) {
        unsafe { self.raw.0.destroy_query_pool(pool.0, None); }
    }

    fn destroy_shader_module(&self, module: n::ShaderModule) {
        unsafe { self.raw.0.destroy_shader_module(module.raw, None); }
    }

    fn destroy_renderpass(&self, rp: n::RenderPass) {
        unsafe { self.raw.0.destroy_render_pass(rp.raw, None); }
    }

    fn destroy_pipeline_layout(&self, pl: n::PipelineLayout) {
        unsafe { self.raw.0.destroy_pipeline_layout(pl.raw, None); }
    }

    fn destroy_graphics_pipeline(&self, pipeline: n::GraphicsPipeline) {
        unsafe { self.raw.0.destroy_pipeline(pipeline.0, None); }
    }

    fn destroy_compute_pipeline(&self, pipeline: n::ComputePipeline) {
        unsafe { self.raw.0.destroy_pipeline(pipeline.0, None); }
    }

    fn destroy_framebuffer(&self, fb: n::FrameBuffer) {
        unsafe { self.raw.0.destroy_framebuffer(fb.raw, None); }
    }

    fn destroy_buffer(&self, buffer: n::Buffer) {
        unsafe { self.raw.0.destroy_buffer(buffer.raw, None); }
    }

    fn destroy_buffer_view(&self, _view: n::BufferView) {
        unimplemented!()
    }

    fn destroy_image(&self, image: n::Image) {
        unsafe { self.raw.0.destroy_image(image.raw, None); }
    }

    fn destroy_image_view(&self, view: n::ImageView) {
        unsafe { self.raw.0.destroy_image_view(view.view, None); }
    }

    fn destroy_sampler(&self, sampler: n::Sampler) {
        unsafe { self.raw.0.destroy_sampler(sampler.0, None); }
    }

    fn destroy_descriptor_pool(&self, pool: n::DescriptorPool) {
        unsafe { self.raw.0.destroy_descriptor_pool(pool.raw, None); }
    }

    fn destroy_descriptor_set_layout(&self, layout: n::DescriptorSetLayout) {
        unsafe { self.raw.0.destroy_descriptor_set_layout(layout.raw, None); }
    }

    fn destroy_fence(&self, fence: n::Fence) {
        unsafe { self.raw.0.destroy_fence(fence.0, None); }
    }

    fn destroy_semaphore(&self, semaphore: n::Semaphore) {
        unsafe { self.raw.0.destroy_semaphore(semaphore.0, None); }
    }

    fn wait_idle(&self) -> Result<(), HostExecutionError> {
        self.raw
            .0
            .device_wait_idle()
            .map_err(From::from)
            .map_err(From::<result::Error>::from)
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send+Sync>() {}
    foo::<Device>()
}
