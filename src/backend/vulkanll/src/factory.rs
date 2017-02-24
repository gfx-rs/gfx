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

use core::{self, shade, state as s};
use core::SubPass;
use core::pso::{self, EntryPoint};
use {native, state};
use {Device, Resources as R};

impl Device {
    pub fn create_shader_library(&mut self, shaders: &[(EntryPoint, &[u8])]) -> Result<native::ShaderLib, shade::CreateShaderError> {
        let mut shader_map = BTreeMap::new();
        // TODO: handle entry points with the same name
        for &(entry_point, byte_code) in shaders {
            // TODO: unimplemented!()
            // shader_map.insert(entry_point, blob);
        }
        Ok(native::ShaderLib { shaders: shader_map })
    }
}

impl core::Factory<R> for Device {
    fn create_renderpass(&mut self) -> native::RenderPass {
        // TODO:
        // Dummy renderpass only
        let subpass_desc = vk::SubpassDescription {
            flags: vk::SubpassDescriptionFlags::empty(),
            pipeline_bind_point: vk::PipelineBindPoint::Graphics,
            input_attachment_count: 0, // TODO
            p_input_attachments: ptr::null(), // TODO
            color_attachment_count: 0, // TODO
            p_color_attachments: ptr::null(), // TODO
            p_resolve_attachments: ptr::null(), // TODO
            p_depth_stencil_attachment: ptr::null(), // TODO
            preserve_attachment_count: 0, // TODO
            p_preserve_attachments: ptr::null(), // TODO
        };

        let info = vk::RenderPassCreateInfo {
            s_type: vk::StructureType::RenderPassCreateInfo,
            p_next: ptr::null(),
            flags: vk::RenderPassCreateFlags::empty(),
            attachment_count: 0, // TODO
            p_attachments: ptr::null(), // TODO
            subpass_count: 1, // TODO
            p_subpasses: &subpass_desc, // TODO
            dependency_count: 0, // TODO
            p_dependencies: ptr::null(), // TODO
        };

        let renderpass = unsafe {
            self.inner.0.create_render_pass(&info, None)
                .expect("Error on render pass creation") // TODO: handle this better
        };

        native::RenderPass { inner: renderpass }
    }

    fn create_pipeline_signature(&mut self) -> native::PipelineSignature {
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

        native::PipelineSignature { layout: layout }
    }

    fn create_graphics_pipelines<'a>(&mut self, descs: &[(&native::ShaderLib, &native::PipelineSignature, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
        -> Vec<Result<(), pso::CreationError>>
    {
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
                    p_name: desc.shader_entries.vertex_shader.as_bytes().as_ptr() as *const i8,
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
                        p_name: pixel_shader.as_bytes().as_ptr() as *const i8,
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
                        p_name: geometry_shader.as_bytes().as_ptr() as *const i8,
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
                        p_name: domain_shader.as_bytes().as_ptr() as *const i8,
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
                        p_name: hull_shader.as_bytes().as_ptr() as *const i8,
                        p_specialization_info: ptr::null(),
                    });
                }

                stages
            };

            let (polygon_mode, line_width) = state::map_polygon_mode(desc.rasterizer.method);
            let dynamic_states = [];

            Ok(vk::GraphicsPipelineCreateInfo {
                s_type: vk::StructureType::GraphicsPipelineCreateInfo,
                p_next: ptr::null(),
                flags: vk::PipelineCreateFlags::empty(),
                stage_count: stages.len() as u32,
                p_stages: stages.as_ptr(),
                p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo {
                    s_type: vk::StructureType::PipelineVertexInputStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineVertexInputStateCreateFlags::empty(),
                    vertex_binding_description_count: 0, // TODO
                    p_vertex_binding_descriptions: ptr::null(), // TODO
                    vertex_attribute_description_count: 0, // TODO
                    p_vertex_attribute_descriptions: ptr::null(), // TODO
                },
                p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                    s_type: vk::StructureType::PipelineInputAssemblyStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
                    topology: state::map_topology(desc.primitive),
                    primitive_restart_enable: vk::VK_FALSE,
                },
                p_tessellation_state: if desc.shader_entries.hull_shader.is_some() &&
                                         desc.shader_entries.domain_shader.is_some() {
                    &vk::PipelineTessellationStateCreateInfo {
                        s_type: vk::StructureType::PipelineTessellationStateCreateInfo,
                        p_next: ptr::null(),
                        flags: vk::PipelineTessellationStateCreateFlags::empty(),
                        patch_control_points: 1 // TODO: 0 < control_points <= VkPhysicalDeviceLimits::maxTessellationPatchSize
                    }
                }  else {
                    // tessellation stage not enabled
                    ptr::null()
                },
                p_viewport_state: ptr::null(), // TODO
                p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
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
                },
                p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                    s_type: vk::StructureType::PipelineMultisampleStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineMultisampleStateCreateFlags::empty(),
                    rasterization_samples: vk::SAMPLE_COUNT_1_BIT, // TODO
                    sample_shading_enable: vk::VK_FALSE, // TODO
                    min_sample_shading: 0.0,  // TODO
                    p_sample_mask: ptr::null(), // TODO
                    alpha_to_coverage_enable: vk::VK_FALSE, // TODO
                    alpha_to_one_enable: vk::VK_FALSE, // TODO
                },
                p_depth_stencil_state: &vk::PipelineDepthStencilStateCreateInfo {
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
                }, // TODO
                p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                    s_type: vk::StructureType::PipelineColorBlendStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineColorBlendStateCreateFlags::empty(),
                    logic_op_enable: vk::VK_FALSE, // TODO
                    logic_op: vk::LogicOp::Clear,
                    attachment_count: 0, // TODO:
                    p_attachments: ptr::null(), // TODO:
                    blend_constants: [0.0; 4], // TODO:
                }, // TODO
                p_dynamic_state: &vk::PipelineDynamicStateCreateInfo {
                    s_type: vk::StructureType::PipelineDynamicStateCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::PipelineDynamicStateCreateFlags::empty(),
                    dynamic_state_count: dynamic_states.len() as u32,
                    p_dynamic_states: dynamic_states.as_ptr(),
                }, // TODO
                // TODO:
                layout: signature.layout,
                render_pass: subpass.main_pass.inner,
                subpass: subpass.index as u32,
                base_pipeline_handle: vk::Pipeline::null(),
                base_pipeline_index: -1,
            })
        }).collect::<Vec<_>>();
        
        // TODO: create the pipelines!

        Vec::new()
    }

    fn create_compute_pipelines(&mut self) -> Vec<Result<(), pso::CreationError>> {
        unimplemented!()
    }
}
