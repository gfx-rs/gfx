use ash::vk;
use ash::version::DeviceV1_0;
use core::{buffer, device as d, format, image, mapping, shade, pass, pso};
use core::{Features, Limits, HeapType, SubPass};
use core::memory::Requirements;
use native as n;
use std::{mem, ptr, slice};
use std::collections::BTreeMap;
use std::sync::Arc;

use {Backend as B, Device, RawDevice};
use conversions as conv;


#[derive(Debug)]
pub struct UnboundBuffer(n::Buffer);

#[derive(Debug)]
pub struct UnboundImage(n::Image);

#[derive(Debug)]
pub struct Mapping {
    device: Arc<RawDevice>,
    memory: vk::DeviceMemory,
}
unsafe impl Sync for Mapping {}
unsafe impl Send for Mapping {}
impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe { self.device.0.unmap_memory(self.memory) }
    }
}

impl Device {
    fn map_buffer(&self,
        buf: &n::Buffer,
        offset: u64,
        size: u64,
    ) -> Result<(*mut vk::types::c_void, Mapping), mapping::Error>
    {
        let result = unsafe {
            self.raw.0.map_memory(buf.memory, offset, size, vk::MemoryMapFlags::empty())
        };

        match result {
            Ok(data) =>Ok((data, Mapping {
                device: self.raw.clone(),
                memory: buf.memory,
            })),
            Err(error) => {
                error!("Mapping failed with {:?}", error);
                Err(mapping::Error::AccessOverlap) //TODO
            }
        }
    }

    pub fn create_shader_library(&mut self,
        shaders: &[(pso::EntryPoint, &[u8])],
    ) -> Result<n::ShaderLib, shade::CreateShaderError>
    {
        let mut shader_map = BTreeMap::new();
        // TODO: handle entry points with the same name
        for &(entry_point, byte_code) in shaders {
            // TODO
            // TODO: check code size length (multiple of 4)
            let info = vk::ShaderModuleCreateInfo {
                s_type: vk::StructureType::ShaderModuleCreateInfo,
                p_next: ptr::null(),
                flags: vk::ShaderModuleCreateFlags::empty(),
                code_size: byte_code.len(),
                p_code: byte_code as *const _ as *const u32,
            };

            let module = unsafe {
                self.raw.0.create_shader_module(&info, None)
                            .expect("Error on shader module creation") // TODO: error handling
            };

            shader_map.insert(entry_point, module);
        }
        Ok(n::ShaderLib { shaders: shader_map })
    }

    fn create_image_view(&mut self, image: &n::Image, format: format::Format) -> vk::ImageView {
        // TODO
        let components = vk::ComponentMapping {
            r: vk::ComponentSwizzle::Identity,
            g: vk::ComponentSwizzle::Identity,
            b: vk::ComponentSwizzle::Identity,
            a: vk::ComponentSwizzle::Identity,
        };

        // TODO
        let subresource_range = vk::ImageSubresourceRange {
            aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: vk::VK_REMAINING_ARRAY_LAYERS,
        };

        let info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::ImageViewCreateInfo,
            p_next: ptr::null(),
            flags: vk::ImageViewCreateFlags::empty(), // TODO
            image: image.raw,
            view_type: vk::ImageViewType::Type2d, // TODO
            format: conv::map_format(format.0, format.1).unwrap(), // TODO
            components: components,
            subresource_range: subresource_range,
        };

        unsafe {
            self.raw.0.create_image_view(&info, None)
                        .expect("Error on image view creation") // TODO
        }
    }
}

impl d::Device<B> for Device {
    fn get_features(&self) -> &Features { &self.features }
    fn get_limits(&self) -> &Limits { &self.limits }

    fn create_heap(&mut self, heap_type: &HeapType, _resource_type: d::ResourceHeapType, size: u64) -> Result<n::Heap, d::ResourceHeapError> {
        let info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MemoryAllocateInfo,
            p_next: ptr::null(),
            allocation_size: size,
            memory_type_index: heap_type.id as u32,
        };

        let memory = unsafe {
            self.raw.0.allocate_memory(&info, None)
                        .expect("Error on heap creation") // TODO: error handling
        };

        Ok(n::Heap(memory))
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> n::RenderPass
    {
        let map_subpass_ref = |pass: pass::SubpassRef| {
            match pass {
                pass::SubpassRef::External => vk::VK_SUBPASS_EXTERNAL,
                pass::SubpassRef::Pass(id) => id as u32,
            }
        };

        // TODO: basic implementation only, needs lot of tweaking!
        let attachments = attachments.iter().map(|attachment| {
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(), // TODO: may even alias!
                format: conv::map_format(attachment.format.0, attachment.format.1).unwrap(), // TODO: error handling
                samples: vk::SAMPLE_COUNT_1_BIT, // TODO: multisampling
                load_op: conv::map_attachment_load_op(attachment.load_op),
                store_op: conv::map_attachment_store_op(attachment.store_op),
                stencil_load_op: conv::map_attachment_load_op(attachment.stencil_load_op),
                stencil_store_op: conv::map_attachment_store_op(attachment.stencil_store_op),
                initial_layout: conv::map_image_layout(attachment.src_layout),
                final_layout: conv::map_image_layout(attachment.dst_layout),
            }
        }).collect::<Vec<_>>();

        let mut attachment_refs = Vec::new();

        let subpasses = subpasses.iter().map(|subpass| {
            {
                let color_attachments = subpass.color_attachments.iter()
                .map(|&(id, layout)| vk::AttachmentReference { attachment: id as u32, layout: conv::map_image_layout(layout) })
                .collect::<Vec<_>>();

                attachment_refs.push(color_attachments);
            }

            let color_attachments = attachment_refs.last().unwrap();

            vk::SubpassDescription {
                flags: vk::SubpassDescriptionFlags::empty(),
                pipeline_bind_point: vk::PipelineBindPoint::Graphics,
                input_attachment_count: 0, // TODO
                p_input_attachments: ptr::null(), // TODO
                color_attachment_count: color_attachments.len() as u32, // TODO
                p_color_attachments: color_attachments.as_ptr(), // TODO
                p_resolve_attachments: ptr::null(), // TODO
                p_depth_stencil_attachment: ptr::null(), // TODO
                preserve_attachment_count: 0, // TODO
                p_preserve_attachments: ptr::null(), // TODO
            }
        }).collect::<Vec<_>>();

        let dependencies = dependencies.iter().map(|dependency| {
            // TODO: checks
            vk::SubpassDependency {
                src_subpass: map_subpass_ref(dependency.src_pass),
                dst_subpass: map_subpass_ref(dependency.dst_pass),
                src_stage_mask: conv::map_pipeline_stage(dependency.src_stage),
                dst_stage_mask: conv::map_pipeline_stage(dependency.dst_stage),
                src_access_mask: conv::map_image_access(dependency.src_access),
                dst_access_mask: conv::map_image_access(dependency.dst_access),
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

    fn create_pipeline_layout(&mut self, sets: &[&n::DescriptorSetLayout]) -> n::PipelineLayout {
        // TODO:

        let set_layouts = sets.iter().map(|set| {
            set.raw
        }).collect::<Vec<_>>();

        let info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PipelineLayoutCreateInfo,
            p_next: ptr::null(),
            flags: vk::PipelineLayoutCreateFlags::empty(),
            set_layout_count: set_layouts.len() as u32,
            p_set_layouts: set_layouts.as_ptr(),
            push_constant_range_count: 0, // TODO
            p_push_constant_ranges: ptr::null(), // TODO
        };

        let raw = unsafe {
            self.raw.0.create_pipeline_layout(&info, None)
                .expect("Error on pipeline signature creation") // TODO: handle this better
        };

        n::PipelineLayout { raw }
    }

    fn create_graphics_pipelines<'a>(&mut self, descs: &[(&n::ShaderLib, &n::PipelineLayout, SubPass<'a, B>, &pso::GraphicsPipelineDesc)])
        -> Vec<Result<n::GraphicsPipeline, pso::CreationError>>
    {
        use core::state as s;

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

        let dynamic_states = [vk::DynamicState::Viewport, vk::DynamicState::Scissor];

        let infos = descs.iter().map(|&(shader_lib, signature, ref subpass, desc)| {
            let stages = {
                let mut stages = Vec::new();

                // Vertex stage
                let vs_module = if let Some(module) = shader_lib.shaders.get(&desc.shader_entries.vertex_shader)
                    { module } else { return Err(pso::CreationError) };
                stages.push(vk::PipelineShaderStageCreateInfo {
                    s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineShaderStageCreateFlags::empty(),
                    stage: vk::SHADER_STAGE_VERTEX_BIT,
                    module: *vs_module,
                    p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language // desc.shader_entries.vertex_shader.as_bytes().as_ptr() as *const i8,
                    p_specialization_info: ptr::null(),
                });

                // Pixel stage
                if let Some(pixel_shader) = desc.shader_entries.pixel_shader {
                    let ps_module = if let Some(module) = shader_lib.shaders.get(&pixel_shader)
                        { module } else { return Err(pso::CreationError) };
                    stages.push(vk::PipelineShaderStageCreateInfo {
                        s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                        p_next: ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        stage: vk::SHADER_STAGE_FRAGMENT_BIT,
                        module: *ps_module,
                        p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language // pixel_shader.as_bytes().as_ptr() as *const i8,
                        p_specialization_info: ptr::null(),
                    });
                }

                // Geometry stage
                if let Some(geometry_shader) = desc.shader_entries.geometry_shader {
                    let gs_module = if let Some(module) = shader_lib.shaders.get(&geometry_shader)
                        { module } else { return Err(pso::CreationError) };
                    stages.push(vk::PipelineShaderStageCreateInfo {
                        s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                        p_next: ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        stage: vk::SHADER_STAGE_GEOMETRY_BIT,
                        module: *gs_module,
                        p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language // geometry_shader.as_bytes().as_ptr() as *const i8,
                        p_specialization_info: ptr::null(),
                    });
                }

                // Domain stage
                if let Some(domain_shader) = desc.shader_entries.domain_shader {
                    let ds_module = if let Some(module) = shader_lib.shaders.get(&domain_shader)
                        { module } else { return Err(pso::CreationError) };
                    stages.push(vk::PipelineShaderStageCreateInfo {
                        s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                        p_next: ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        stage: vk::SHADER_STAGE_TESSELLATION_EVALUATION_BIT,
                        module: *ds_module,
                        p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language // domain_shader.as_bytes().as_ptr() as *const i8,
                        p_specialization_info: ptr::null(),
                    });
                }

                // Hull stage
                if let Some(hull_shader) = desc.shader_entries.hull_shader {
                    let hs_module = if let Some(module) = shader_lib.shaders.get(&hull_shader)
                        { module } else { return Err(pso::CreationError) };
                    stages.push(vk::PipelineShaderStageCreateInfo {
                        s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                        p_next: ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        stage: vk::SHADER_STAGE_TESSELLATION_CONTROL_BIT,
                        module: *hs_module,
                        p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language // hull_shader.as_bytes().as_ptr() as *const i8,
                        p_specialization_info: ptr::null(),
                    });
                }

                stages
            };

            let (polygon_mode, line_width) = conv::map_polygon_mode(desc.rasterizer.polgyon_mode);

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
                for (i, attr) in desc.attributes.iter().enumerate() {
                    vertex_attributes.push(vk::VertexInputAttributeDescription {
                        location: i as u32,
                        binding: attr.binding as u32,
                        format: match conv::map_format(attr.element.format.0, attr.element.format.1) {
                            Some(fm) => fm,
                            None => return Err(pso::CreationError),
                        },
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
                depth_clamp_enable: vk::VK_TRUE, // TODO
                rasterizer_discard_enable: vk::VK_FALSE, // TODO
                polygon_mode: polygon_mode,
                cull_mode: conv::map_cull_mode(desc.rasterizer.cull_mode),
                front_face: conv::map_front_face(desc.rasterizer.front_face),
                depth_bias_enable: if desc.rasterizer.depth_bias.is_some() { vk::VK_TRUE } else { vk::VK_FALSE },
                depth_bias_constant_factor: desc.rasterizer.depth_bias.map_or(0.0, |off| off.const_factor),
                depth_bias_clamp: desc.rasterizer.depth_bias.map_or(0.0, |off| off.clamp),
                depth_bias_slope_factor: desc.rasterizer.depth_bias.map_or(0.0, |off| off.slope_factor),
                line_width: line_width,
            });

            if desc.shader_entries.hull_shader.is_some() &&
               desc.shader_entries.domain_shader.is_some()
            {
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

            info_depth_stencil_states.push(vk::PipelineDepthStencilStateCreateInfo {
                s_type: vk::StructureType::PipelineDepthStencilStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
                depth_test_enable: if let Some((_, pso::DepthStencilInfo { depth: Some(_), .. })) =
                    desc.depth_stencil { vk::VK_TRUE } else { vk::VK_FALSE },
                depth_write_enable: if let Some((_, pso::DepthStencilInfo { depth: Some(s::Depth { write: true, .. }), .. })) =
                    desc.depth_stencil { vk::VK_TRUE } else { vk::VK_FALSE },
                depth_compare_op: if let Some((_, pso::DepthStencilInfo { depth: Some(s::Depth { fun, .. }), ..})) =
                    desc.depth_stencil { conv::map_comparison(fun) } else { vk::CompareOp::Never },
                depth_bounds_test_enable: vk::VK_FALSE, // TODO
                stencil_test_enable: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { front: Some(_), .. })) |
                    Some((_, pso::DepthStencilInfo { back: Some(_), .. })) => vk::VK_TRUE,
                    _ => vk::VK_FALSE,
                },
                front: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { front: Some(ref s), .. })) => conv::map_stencil_side(s),
                    _ => unsafe { mem::zeroed() }, // TODO
                },
                back: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { back: Some(ref s), .. })) => conv::map_stencil_side(s),
                    _ => unsafe { mem::zeroed() }, // TODO
                },
                min_depth_bounds: 0.0,
                max_depth_bounds: 1.0,
            });

            // Build blend states for color attachments
            {
                let mut blend_states = Vec::with_capacity(desc.blender.targets.len());
                for info in &desc.blender.targets {
                    let mut blend = vk::PipelineColorBlendAttachmentState {
                        blend_enable: vk::VK_FALSE,
                        src_color_blend_factor: vk::BlendFactor::Zero,
                        dst_color_blend_factor: vk::BlendFactor::Zero,
                        color_blend_op: vk::BlendOp::Add,
                        src_alpha_blend_factor: vk::BlendFactor::Zero,
                        dst_alpha_blend_factor: vk::BlendFactor::Zero,
                        alpha_blend_op: vk::BlendOp::Add,
                        color_write_mask: vk::ColorComponentFlags::from_flags(info.mask.bits() as u32).unwrap(),
                    };

                    if let Some(ref b) = info.color {
                        blend.blend_enable = vk::VK_TRUE;
                        blend.src_color_blend_factor = conv::map_blend_factor(b.source, false);
                        blend.dst_color_blend_factor = conv::map_blend_factor(b.destination, false);
                        blend.color_blend_op = conv::map_blend_op(b.equation);
                    }
                    if let Some(ref b) = info.alpha {
                        blend.blend_enable = vk::VK_TRUE;
                        blend.src_alpha_blend_factor = conv::map_blend_factor(b.source, true);
                        blend.dst_alpha_blend_factor = conv::map_blend_factor(b.destination, true);
                        blend.alpha_blend_op = conv::map_blend_op(b.equation);
                    }

                    blend_states.push(blend);
                }
                color_attachments.push(blend_states);
            }

            info_color_blend_states.push(vk::PipelineColorBlendStateCreateInfo {
                s_type: vk::StructureType::PipelineColorBlendStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineColorBlendStateCreateFlags::empty(),
                logic_op_enable: vk::VK_FALSE, // TODO
                logic_op: vk::LogicOp::Clear,
                attachment_count: color_attachments.last().unwrap().len() as u32,
                p_attachments: color_attachments.last().unwrap().as_ptr(), // TODO:
                blend_constants: [0.0; 4], // TODO:
            });

            info_dynamic_states.push(vk::PipelineDynamicStateCreateInfo {
                s_type: vk::StructureType::PipelineDynamicStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineDynamicStateCreateFlags::empty(),
                dynamic_state_count: dynamic_states.len() as u32,
                p_dynamic_states: dynamic_states.as_ptr(),
            });

            Ok(vk::GraphicsPipelineCreateInfo {
                s_type: vk::StructureType::GraphicsPipelineCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineCreateFlags::empty(),
                stage_count: info_stages.last().unwrap().len() as u32,
                p_stages: info_stages.last().unwrap().as_ptr(),
                p_vertex_input_state: info_vertex_input_states.last().unwrap(),
                p_input_assembly_state: info_input_assembly_states.last().unwrap(),
                p_rasterization_state: info_rasterization_states.last().unwrap(),
                p_tessellation_state: if desc.shader_entries.hull_shader.is_some() &&
                                         desc.shader_entries.domain_shader.is_some()
                    { info_tessellation_states.last().unwrap() } else { ptr::null() },
                p_viewport_state: info_viewport_states.last().unwrap(),
                p_multisample_state: info_multisample_states.last().unwrap(),
                p_depth_stencil_state: info_depth_stencil_states.last().unwrap(),
                p_color_blend_state: info_color_blend_states.last().unwrap(),
                p_dynamic_state: info_dynamic_states.last().unwrap(),
                layout: signature.raw,
                render_pass: subpass.main_pass.raw,
                subpass: subpass.index as u32,
                base_pipeline_handle: vk::Pipeline::null(),
                base_pipeline_index: -1,
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
            Ok(pipelines) => {
                let mut pipelines = pipelines.iter();
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => {
                            let pipeline = *pipelines.next().unwrap();
                            Ok(n::GraphicsPipeline(pipeline))
                        }
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            }
            Err((pipelines, _err)) => {
                let mut pipelines = pipelines.iter();
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => {
                            let pipeline = *pipelines.next().unwrap();

                            // Check if pipeline compiled correctly
                            if pipeline == vk::Pipeline::null() {
                                Err(pso::CreationError) // TODO
                            } else {
                                Ok(n::GraphicsPipeline(pipeline))
                            }
                        }
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            }
        }
    }

    fn create_compute_pipelines(&mut self,
        descs: &[(&n::ShaderLib, pso::EntryPoint, &n::PipelineLayout)],
    ) -> Vec<Result<n::ComputePipeline, pso::CreationError>>
    {
        let infos = descs.iter().map(|&(shader_lib, entry_point, layout)| {
            let cs_module = if let Some(module) = shader_lib.shaders.get(&entry_point)
                    { module } else { return Err(pso::CreationError) };
            let stage = vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                stage: vk::SHADER_STAGE_COMPUTE_BIT,
                module: *cs_module,
                p_name: b"main\0".as_ptr() as *const i8, // TODO: GLSL source language
                p_specialization_info: ptr::null(),
            };

            Ok(vk::ComputePipelineCreateInfo {
                s_type: vk::StructureType::ComputePipelineCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineCreateFlags::empty(),
                stage: stage,
                layout: layout.raw,
                base_pipeline_handle: vk::Pipeline::null(),
                base_pipeline_index: -1,
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
            Ok(pipelines) => {
                let mut pipelines = pipelines.iter();
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => {
                            let pipeline = *pipelines.next().unwrap();
                            Ok(n::ComputePipeline(pipeline))
                        }
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            }
            Err((pipelines, _err)) => {
                let mut pipelines = pipelines.iter();
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => {
                            let pipeline = *pipelines.next().unwrap();

                            // Check if pipeline compiled correctly
                            if pipeline == vk::Pipeline::null() {
                                Err(pso::CreationError) // TODO
                            } else {
                                Ok(n::ComputePipeline(pipeline))
                            }
                        }
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            }
        }
    }

    fn create_framebuffer(&mut self, renderpass: &n::RenderPass,
        color_attachments: &[&n::RenderTargetView], depth_stencil_attachments: &[&n::DepthStencilView],
        width: u32, height: u32, layers: u32) -> n::FrameBuffer
    {
        let attachments = {
            let mut views = color_attachments.iter()
                .map(|attachment| attachment.view)
                .collect::<Vec<_>>();

            views.extend(depth_stencil_attachments.iter()
                .map(|attachment| attachment.view)
                .collect::<Vec<_>>());

            views
        };

        let info = vk::FramebufferCreateInfo {
            s_type: vk::StructureType::FramebufferCreateInfo,
            p_next: ptr::null(),
            flags: vk::FramebufferCreateFlags::empty(),
            render_pass: renderpass.raw,
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            width: width,
            height: height,
            layers: layers,
        };

        let framebuffer = unsafe {
            self.raw.0.create_framebuffer(&info, None)
                        .expect("error on framebuffer creation")
        };

        n::FrameBuffer { raw: framebuffer }
    }

    fn create_sampler(&mut self, sampler_info: image::SamplerInfo) -> n::Sampler {
        use core::state::Comparison;

        let (min, mag, mip, aniso) = conv::map_filter(sampler_info.filter);
        let info = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SamplerCreateInfo,
            p_next: ptr::null(),
            flags: vk::SamplerCreateFlags::empty(),
            mag_filter: mag,
            min_filter: min,
            mipmap_mode: mip,
            address_mode_u: conv::map_wrap(sampler_info.wrap_mode.0),
            address_mode_v: conv::map_wrap(sampler_info.wrap_mode.1),
            address_mode_w: conv::map_wrap(sampler_info.wrap_mode.2),
            mip_lod_bias: sampler_info.lod_bias.into(),
            anisotropy_enable: if aniso > 0.0 { vk::VK_TRUE } else { vk::VK_FALSE },
            max_anisotropy: aniso,
            compare_enable: if sampler_info.comparison.is_some() { vk::VK_TRUE } else { vk::VK_FALSE },
            compare_op: conv::map_comparison(sampler_info.comparison.unwrap_or(Comparison::Never)),
            min_lod: sampler_info.lod_range.0.into(),
            max_lod: sampler_info.lod_range.1.into(),
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
    fn create_buffer(&mut self, size: u64, _stride: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
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

        Ok(UnboundBuffer(n::Buffer {
            raw: buffer,
            memory: vk::DeviceMemory::null(),
        }))
    }

    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> Requirements {
        let req = self.raw.0.get_buffer_memory_requirements((buffer.0).raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
        }
    }

    fn bind_buffer_memory(&mut self, heap: &n::Heap, offset: u64, buffer: UnboundBuffer) -> Result<n::Buffer, buffer::CreationError> {
        assert_eq!(Ok(()), unsafe {
            self.raw.0.bind_buffer_memory((buffer.0).raw, heap.0, offset)
        });

        let buffer = n::Buffer {
            raw: buffer.0.raw,
            memory: heap.0,
        };

        Ok(buffer)
    }

    ///
    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<UnboundImage, image::CreationError>
    {
        use core::image::Kind::*;

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

        let bytes_per_texel = format.0.get_total_bits() / 8;
        let samples = match aa_mode {
            image::AaMode::Single => vk::SAMPLE_COUNT_1_BIT,
            _ => unimplemented!(),
        };

        let info = vk::ImageCreateInfo {
            s_type: vk::StructureType::ImageCreateInfo,
            p_next: ptr::null(),
            flags,
            image_type,
            format: conv::map_format(format.0, format.1).unwrap(), // TODO
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

        Ok(UnboundImage(n::Image{ raw, bytes_per_texel, extent }))
    }

    ///
    fn get_image_requirements(&mut self, image: &UnboundImage) -> Requirements {
        let req = self.raw.0.get_image_memory_requirements(image.0.raw);

        Requirements {
            size: req.size,
            alignment: req.alignment,
        }
    }

    ///
    fn bind_image_memory(&mut self, heap: &n::Heap, offset: u64, image: UnboundImage) -> Result<n::Image, image::CreationError> {
        // TODO: error handling
        assert_eq!(Ok(()), unsafe {
            self.raw.0.bind_image_memory(image.0.raw, heap.0, offset)
        });

        Ok(image.0)
    }

    fn view_buffer_as_constant(&mut self, buffer: &n::Buffer, offset: usize, size: usize) -> Result<n::ConstantBufferView, d::TargetViewError> {
        Ok(n::ConstantBufferView {
            buffer: buffer.raw,
            offset: offset,
            size: size,
        })
    }

    fn view_image_as_render_target(&mut self,
        image: &n::Image,
        format: format::Format,
        range: image::SubresourceRange,
    ) -> Result<n::RenderTargetView, d::TargetViewError>
    {
        let rtv = n::RenderTargetView {
            image: image.raw,
            view: self.create_image_view(image, format),
            range,
        };

        Ok(rtv)
    }

    fn view_image_as_shader_resource(&mut self, image: &n::Image, format: format::Format) -> Result<n::ShaderResourceView, d::TargetViewError> {
        // TODO: check format compatibility? Allow different formats?
        let view = self.create_image_view(image, format);
        let srv = n::ShaderResourceView::Image(view);
        Ok(srv)
    }

    fn view_image_as_unordered_access(&mut self, image: &n::Image, format: format::Format) -> Result<n::UnorderedAccessView, d::TargetViewError> {
        // TODO: check format compatibility? Allow different formats?
        let view = self.create_image_view(image, format);
        let uav = n::UnorderedAccessView::Image(view);
        Ok(uav)
    }

    fn create_descriptor_pool(&mut self,
        max_sets: usize,
        descriptor_pools: &[pso::DescriptorRangeDesc],
    ) -> n::DescriptorPool
    {
        let pools = descriptor_pools.iter().map(|pool| {
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

    fn create_descriptor_set_layout(&mut self, bindings: &[pso::DescriptorSetLayoutBinding])-> n::DescriptorSetLayout {
        let bindings = bindings.iter().map(|binding| {
            vk::DescriptorSetLayoutBinding {
                binding: binding.binding as u32,
                descriptor_type: conv::map_descriptor_type(binding.ty),
                descriptor_count: binding.count as u32,
                stage_flags: conv::map_stage_flags(binding.stage_flags),
                p_immutable_samplers: ptr::null(), // TODO
            }
        }).collect::<Vec<_>>();

        let info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DescriptorSetLayoutCreateInfo,
            p_next: ptr::null(),
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            binding_count: bindings.len() as u32,
            p_bindings: bindings.as_ptr(),
        };

        let layout = unsafe {
            self.raw.0.create_descriptor_set_layout(&info, None)
                        .expect("Error on descriptor set layout creation") // TODO
        };

        n::DescriptorSetLayout {
            raw: layout,
        }
    }

    fn update_descriptor_sets(&mut self, writes: &[pso::DescriptorSetWrite<B>]) {
        let mut image_infos = Vec::new();
        let mut buffer_infos = Vec::new();
        // let mut texel_buffer_views = Vec::new();

        for write in writes {
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
                    for &(srv, layout) in images {
                        let view = if let n::ShaderResourceView::Image(view) = *srv { view }
                                    else { panic!("Wrong shader resource view (expected image)") }; // TODO

                        image_infos.push(vk::DescriptorImageInfo {
                            sampler: vk::Sampler::null(),
                            image_view: view,
                            image_layout: conv::map_image_layout(layout),
                        });
                    }
                }

                pso::DescriptorWrite::ConstantBuffer(ref cbvs) => {
                    for cbv in cbvs {
                        buffer_infos.push(vk::DescriptorBufferInfo {
                            buffer: cbv.buffer,
                            offset: cbv.offset as u64,
                            range: cbv.size as u64,
                        });
                    }
                }

                _ => unimplemented!(), // TODO
            };
        }

        // Track current subslice for each write
        let mut cur_image_index = 0;
        let mut cur_buffer_index = 0;

        let writes = writes.iter().map(|write| {
            let (ty, count, image_info, buffer_info, texel_buffer_view) = match write.write {
                pso::DescriptorWrite::Sampler(ref samplers) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += samplers.len();

                    (vk::DescriptorType::Sampler, samplers.len(),
                        info_ptr, ptr::null(), ptr::null())
                }
                pso::DescriptorWrite::SampledImage(ref images) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += images.len();

                    (vk::DescriptorType::SampledImage, images.len(),
                        info_ptr, ptr::null(), ptr::null())
                }
                pso::DescriptorWrite::StorageImage(ref images) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += images.len();

                    (vk::DescriptorType::StorageImage, images.len(),
                        info_ptr, ptr::null(), ptr::null())
                }
                pso::DescriptorWrite::ConstantBuffer(ref cbvs) => {
                    let info_ptr = &buffer_infos[cur_buffer_index] as *const _;
                    cur_buffer_index += cbvs.len();

                    (vk::DescriptorType::UniformBuffer, cbvs.len(),
                        ptr::null(), info_ptr, ptr::null())
                }
                pso::DescriptorWrite::InputAttachment(ref images) => {
                    let info_ptr = &image_infos[cur_image_index] as *const _;
                    cur_image_index += images.len();

                    (vk::DescriptorType::InputAttachment, images.len(),
                        info_ptr, ptr::null(), ptr::null())
                }
                _ => unimplemented!(), // TODO
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

    /// Acquire a mapping Reader.
    fn read_mapping<'a, T>(&self, buf: &'a n::Buffer, offset: u64, size: u64)
                           -> Result<mapping::Reader<'a, B, T>, mapping::Error>
        where T: Copy
    {
        self.map_buffer(buf, offset, size)
            .map(|(ptr, mapping)| {
                let slice = unsafe {
                    slice::from_raw_parts(ptr as *const T, size as usize / mem::size_of::<T>())
                };
                unsafe {
                    mapping::Reader::new(slice, mapping)
                }
            })
    }

    /// Acquire a mapping Writer
    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a n::Buffer, offset: u64, size: u64)
                                -> Result<mapping::Writer<'a, B, T>, mapping::Error>
        where T: Copy
    {
        self.map_buffer(buf, offset, size)
            .map(|(ptr, mapping)| {
                let slice = unsafe {
                    slice::from_raw_parts_mut(ptr as *mut T, size as usize / mem::size_of::<T>())
                };
                unsafe {
                    mapping::Writer::new(slice, mapping)
                }
            })
    }

    fn create_semaphore(&mut self) -> n::Semaphore {
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

    fn create_fence(&mut self, signaled: bool) -> n::Fence {
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

    fn reset_fences(&mut self, fences: &[&n::Fence]) {
        let fences = fences.iter().map(|fence| fence.0).collect::<Vec<_>>();
        assert_eq!(Ok(()), unsafe {
            self.raw.0.reset_fences(&fences)
        });
    }

    fn wait_for_fences(&mut self, fences: &[&n::Fence], wait: d::WaitFor, timeout_ms: u32) -> bool {
        let fences = fences.iter().map(|fence| fence.0).collect::<Vec<_>>();
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

    fn destroy_heap(&mut self, heap: n::Heap) {
        unsafe { self.raw.0.free_memory(heap.0, None); }
    }

    fn destroy_shader_lib(&mut self, shader_lib: n::ShaderLib) {
        for shader in shader_lib.shaders.into_iter() {
            unsafe { self.raw.0.destroy_shader_module(shader.1, None); }
        }
    }

    fn destroy_renderpass(&mut self, rp: n::RenderPass) {
        unsafe { self.raw.0.destroy_render_pass(rp.raw, None); }
    }

    fn destroy_pipeline_layout(&mut self, pl: n::PipelineLayout) {
        unsafe { self.raw.0.destroy_pipeline_layout(pl.raw, None); }
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: n::GraphicsPipeline) {
        unsafe { self.raw.0.destroy_pipeline(pipeline.0, None); }
    }

    fn destroy_compute_pipeline(&mut self, pipeline: n::ComputePipeline) {
        unsafe { self.raw.0.destroy_pipeline(pipeline.0, None); }
    }

    fn destroy_framebuffer(&mut self, fb: n::FrameBuffer) {
        unsafe { self.raw.0.destroy_framebuffer(fb.raw, None); }
    }

    fn destroy_buffer(&mut self, buffer: n::Buffer) {
        unsafe { self.raw.0.destroy_buffer(buffer.raw, None); }
    }

    fn destroy_image(&mut self, image: n::Image) {
        unsafe { self.raw.0.destroy_image(image.raw, None); }
    }

    fn destroy_render_target_view(&mut self, rtv: n::RenderTargetView) {
        unsafe { self.raw.0.destroy_image_view(rtv.view, None); }
    }

    fn destroy_depth_stencil_view(&mut self, dsv: n::DepthStencilView) {
        unsafe { self.raw.0.destroy_image_view(dsv.view, None); }
    }

    fn destroy_constant_buffer_view(&mut self, _: n::ConstantBufferView) { }

    fn destroy_shader_resource_view(&mut self, srv: n::ShaderResourceView) {
        match srv {
            n::ShaderResourceView::Buffer => (),
            n::ShaderResourceView::Image(view) => unsafe {
                self.raw.0.destroy_image_view(view, None);
            }
        }
    }

    fn destroy_unordered_access_view(&mut self, uav: n::UnorderedAccessView) {
        match uav {
            n::UnorderedAccessView::Buffer => (),
            n::UnorderedAccessView::Image(view) => unsafe {
                self.raw.0.destroy_image_view(view, None);
            }
        }
    }

    fn destroy_sampler(&mut self, sampler: n::Sampler) {
        unsafe { self.raw.0.destroy_sampler(sampler.0, None); }
    }

    fn destroy_descriptor_pool(&mut self, pool: n::DescriptorPool) {
        unsafe { self.raw.0.destroy_descriptor_pool(pool.raw, None); }
    }

    fn destroy_descriptor_set_layout(&mut self, layout: n::DescriptorSetLayout) {
        unsafe { self.raw.0.destroy_descriptor_set_layout(layout.raw, None); }
    }

    fn destroy_fence(&mut self, fence: n::Fence) {
        unsafe { self.raw.0.destroy_fence(fence.0, None); }
    }

    fn destroy_semaphore(&mut self, semaphore: n::Semaphore) {
        unsafe { self.raw.0.destroy_semaphore(semaphore.0, None); }
    }
}
