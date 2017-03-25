// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ash::vk;
use ash::version::DeviceV1_0;
use std::{mem, ptr};
use std::sync::Arc;
use std::collections::BTreeMap;

use core::{self, buffer, format, factory as f, image, memory, pass, shade, state as s};
use core::{HeapType, SubPass};
use core::pso::{self, EntryPoint};
use {data, native, state};
use {Factory, Resources as R};

#[derive(Debug)]
pub struct UnboundBuffer(native::Buffer);

#[derive(Debug)]
pub struct UnboundImage(native::Image);

impl Factory {
    pub fn create_shader_library(&mut self, shaders: &[(EntryPoint, &[u8])]) -> Result<native::ShaderLib, shade::CreateShaderError> {
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
                self.inner.0.create_shader_module(&info, None)
                            .expect("Error on shader module creation") // TODO: error handling
            };

            shader_map.insert(entry_point, module);
        }
        Ok(native::ShaderLib { shaders: shader_map })
    }
}

impl core::Factory<R> for Factory {
    fn create_heap(&mut self, heap_type: &HeapType, size: u64) -> native::Heap {
        let info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MemoryAllocateInfo,
            p_next: ptr::null(),
            allocation_size: size,
            memory_type_index: heap_type.id as u32,
        };

        let memory = unsafe {
            self.inner.0.allocate_memory(&info, None)
                        .expect("Error on heap creation") // TODO: error handling
        };

        native::Heap(memory)
    }

    fn create_renderpass(&mut self, attachments: &[pass::Attachment],
        subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> native::RenderPass
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
                format: data::map_format(attachment.format.0, attachment.format.1).unwrap(), // TODO: error handling
                samples: vk::SAMPLE_COUNT_1_BIT, // TODO: multisampling
                load_op: data::map_attachment_load_op(attachment.load_op),
                store_op: data::map_attachment_store_op(attachment.store_op),
                stencil_load_op: data::map_attachment_load_op(attachment.stencil_load_op),
                stencil_store_op: data::map_attachment_store_op(attachment.stencil_store_op),
                initial_layout: data::map_image_layout(attachment.src_layout),
                final_layout: data::map_image_layout(attachment.dst_layout),
            }
        }).collect::<Vec<_>>();

        let mut attachment_refs = Vec::new();

        let subpasses = subpasses.iter().map(|subpass| {
            {
                let color_attachments = subpass.color_attachments.iter()
                .map(|&(id, layout)| vk::AttachmentReference { attachment: id as u32, layout: data::map_image_layout(layout) })
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
                src_stage_mask: data::map_pipeline_stage(dependency.src_stage),
                dst_stage_mask: data::map_pipeline_stage(dependency.dst_stage),
                src_access_mask: data::map_image_access(dependency.src_access),
                dst_access_mask: data::map_image_access(dependency.dst_access),
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
            self.inner.0.create_render_pass(&info, None)
                .expect("Error on render pass creation") // TODO: handle this better
        };

        native::RenderPass { inner: renderpass }
    }

    fn create_pipeline_layout(&mut self) -> native::PipelineLayout {
        // TODO:
        // Dummy signature only
        let info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PipelineLayoutCreateInfo,
            p_next: ptr::null(),
            flags: vk::PipelineLayoutCreateFlags::empty(),
            set_layout_count: 0, // TODO
            p_set_layouts: ptr::null(), // TODO
            push_constant_range_count: 0, // TODO
            p_push_constant_ranges: ptr::null(), // TODO
        };

        let layout = unsafe {
            self.inner.0.create_pipeline_layout(&info, None)
                .expect("Error on pipeline signature creation") // TODO: handle this better
        };

        native::PipelineLayout { layout: layout }
    }

    fn create_graphics_pipelines<'a>(&mut self, descs: &[(&native::ShaderLib, &native::PipelineLayout, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
        -> Vec<Result<native::GraphicsPipeline, pso::CreationError>>
    {
        // Store pipeline parameters to avoid stack usage
        let mut info_stages                = Vec::with_capacity(descs.len());
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

            let (polygon_mode, line_width) = state::map_polygon_mode(desc.rasterizer.method);

            info_stages.push(stages);

            info_vertex_input_states.push(vk::PipelineVertexInputStateCreateInfo {
                s_type: vk::StructureType::PipelineVertexInputStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineVertexInputStateCreateFlags::empty(),
                vertex_binding_description_count: 0, // TODO
                p_vertex_binding_descriptions: ptr::null(), // TODO
                vertex_attribute_description_count: 0, // TODO
                p_vertex_attribute_descriptions: ptr::null(), // TODO
            });

            info_input_assembly_states.push(vk::PipelineInputAssemblyStateCreateInfo {
                s_type: vk::StructureType::PipelineInputAssemblyStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
                topology: state::map_topology(desc.primitive),
                primitive_restart_enable: vk::VK_FALSE,
            });

            info_rasterization_states.push(vk::PipelineRasterizationStateCreateInfo {
                s_type: vk::StructureType::PipelineRasterizationStateCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineRasterizationStateCreateFlags::empty(),
                depth_clamp_enable: vk::VK_TRUE, // TODO
                rasterizer_discard_enable: vk::VK_FALSE, // TODO
                polygon_mode: polygon_mode,
                cull_mode: state::map_cull_mode(desc.rasterizer.cull_face),
                front_face: state::map_front_face(desc.rasterizer.front_face),
                depth_bias_enable: if desc.rasterizer.offset.is_some() { vk::VK_TRUE } else { vk::VK_FALSE },
                depth_bias_constant_factor: desc.rasterizer.offset.map_or(0.0, |off| off.1 as f32),
                depth_bias_clamp: 16.0, // TODO: magic value?
                depth_bias_slope_factor: desc.rasterizer.offset.map_or(0.0, |off| off.0 as f32),
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
                    desc.depth_stencil { state::map_comparison(fun) } else { vk::CompareOp::Never },
                depth_bounds_test_enable: vk::VK_FALSE, // TODO
                stencil_test_enable: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { front: Some(_), .. })) |
                    Some((_, pso::DepthStencilInfo { back: Some(_), .. })) => vk::VK_TRUE,
                    _ => vk::VK_FALSE,
                },
                front: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { front: Some(ref s), .. })) => state::map_stencil_side(s),
                    _ => unsafe { mem::zeroed() }, // TODO
                },
                back: match desc.depth_stencil {
                    Some((_, pso::DepthStencilInfo { back: Some(ref s), .. })) => state::map_stencil_side(s),
                    _ => unsafe { mem::zeroed() }, // TODO
                },
                min_depth_bounds: 0.0,
                max_depth_bounds: 1.0,
            });

            // Build blend states for color attachments
            {
                let mut blend_states = Vec::with_capacity(desc.color_targets.len());
                for color_target in desc.color_targets.iter() {
                    let info = if let &Some(ref desc) = color_target { &desc.1 } else { break };
                    
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
                        blend.src_color_blend_factor = state::map_blend_factor(b.source, false);
                        blend.dst_color_blend_factor = state::map_blend_factor(b.destination, false);
                        blend.color_blend_op = state::map_blend_op(b.equation);
                    }
                    if let Some(ref b) = info.alpha {
                        blend.blend_enable = vk::VK_TRUE;
                        blend.src_alpha_blend_factor = state::map_blend_factor(b.source, true);
                        blend.dst_alpha_blend_factor = state::map_blend_factor(b.destination, true);
                        blend.alpha_blend_op = state::map_blend_op(b.equation);
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
                layout: signature.layout,
                render_pass: subpass.main_pass.inner,
                subpass: subpass.index as u32,
                base_pipeline_handle: vk::Pipeline::null(),
                base_pipeline_index: -1,
            })
        }).collect::<Vec<_>>();

        let valid_infos = infos.iter().filter_map(|info| info.clone().ok()).collect::<Vec<_>>();
        
        // TODO: create the pipelines!
        let pipelines = if valid_infos.is_empty() {
            Ok(Vec::new())
        } else {
            unsafe {
                self.inner.0.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &valid_infos,
                    None,
                )
            }
        };

        match pipelines {
            Ok(pipelines) => {
                let mut pipelines = pipelines.iter();
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => {
                            let pipeline = native::GraphicsPipeline {
                                pipeline: *pipelines.next().unwrap(),
                            };
                            Ok(pipeline)
                        }
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            },
            Err(err) => {
                infos.iter().map(|ref info| {
                    match **info {
                        Ok(_) => Err(pso::CreationError),
                        Err(ref err) => Err(err.clone()),
                    }
                }).collect::<Vec<_>>()
            }
        }
    }

    fn create_compute_pipelines(&mut self) -> Vec<Result<native::ComputePipeline, pso::CreationError>> {
        unimplemented!()
    }

    fn create_framebuffer(&mut self, renderpass: &native::RenderPass,
        color_attachments: &[&native::RenderTargetView], depth_stencil_attachments: &[&native::DepthStencilView],
        width: u32, height: u32, layers: u32) -> native::FrameBuffer
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
            render_pass: renderpass.inner,
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            width: width,
            height: height,
            layers: layers,
        };

        let framebuffer = unsafe {
            self.inner.0.create_framebuffer(&info, None)
                        .expect("error on framebuffer creation")
        };

        native::FrameBuffer { inner: framebuffer }
    }

    ///
    fn create_buffer(&mut self, size: u64, usage: buffer::Usage) -> Result<UnboundBuffer, buffer::CreationError> {
        let info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BufferCreateInfo,
            p_next: ptr::null(),
            flags: vk::BufferCreateFlags::empty(), // TODO:
            size: size,
            usage: data::map_buffer_usage(usage),
            sharing_mode: vk::SharingMode::Exclusive, // TODO:
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
        };

        let buffer = unsafe {
            self.inner.0.create_buffer(&info, None)
                        .expect("Error on buffer creation") // TODO: error handling
        };
        
        Ok(UnboundBuffer(native::Buffer(buffer)))
    }

    fn get_buffer_requirements(&mut self, buffer: &UnboundBuffer) -> memory::MemoryRequirements {
        let req = self.inner.0.get_buffer_memory_requirements((buffer.0).0);

        memory::MemoryRequirements {
            size: req.size,
            alignment: req.alignment,
        }
    }

    fn bind_buffer_memory(&mut self, heap: &native::Heap, offset: u64, buffer: UnboundBuffer) -> Result<native::Buffer, buffer::CreationError> {
        // TODO: error handling
        unsafe { self.inner.0.bind_buffer_memory((buffer.0).0, heap.0, offset); }

        Ok(buffer.0)
    }

    ///
    fn create_image(&mut self, heap: &native::Heap, offset: u64) -> Result<native::Image, image::CreationError> {
        unimplemented!()
    }

    fn view_image_as_render_target(&mut self, image: &native::Image, format: format::Format) -> Result<native::RenderTargetView, f::TargetViewError> {
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
            image: image.0,
            view_type: vk::ImageViewType::Type2d, // TODO
            format: data::map_format(format.0, format.1).unwrap(), // TODO
            components: components,
            subresource_range: subresource_range,
        };

        let view = unsafe {
            self.inner.0.create_image_view(&info, None)
                        .expect("Error on image view creation")
        };

        let rtv = native::RenderTargetView {
            image: image.0,
            view: view,
        };

        Ok(rtv)
    }
}
