// Copyright 2016 The Gfx-rs Developers.
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

use std::{cell, mem, ptr, slice};
use std::os::raw::c_void;
use core::{self, handle as h, pso, state, texture, buffer, mapping};
use core::memory::{self, Bind};
use core::factory::{self as f};
use core::format::ChannelType;
use core::target::Layer;
use vk;
use {command, data, native};
use {Resources as R, SharePointer};


#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MappingGate {
    pub pointer: *mut c_void,
    pub status: mapping::Status<R>,
}

unsafe impl Send for MappingGate {}
unsafe impl Sync for MappingGate {}

impl mapping::Gate<R> for MappingGate {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        slice::from_raw_parts_mut(self.pointer as *mut T, len)
    }
}

pub struct Factory {
    share: SharePointer,
    queue_family_index: u32,
    mem_video_id: u32,
    mem_system_id: u32,
    command_pool: vk::CommandPool,
    frame_handles: h::Manager<R>,
}

impl Factory {
    pub fn new(share: SharePointer, qf_index: u32, mvid: u32, msys: u32) -> Factory {
        let com_info = vk::CommandPoolCreateInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: ptr::null(),
            flags: vk::COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,
            queueFamilyIndex: qf_index,
        };
        let mut com_pool = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            let (dev, vk) = share.get_device();
            vk.CreateCommandPool(dev, &com_info, ptr::null(), &mut com_pool)
        });
        Factory {
            share: share,
            queue_family_index: qf_index,
            mem_video_id: mvid,
            mem_system_id: msys,
            command_pool: com_pool,
            frame_handles: h::Manager::new(),
        }
    }

    pub fn create_command_buffer(&mut self) -> command::Buffer {
        command::Buffer::new(self.command_pool, self.queue_family_index, self.share.clone())
    }

    fn view_texture(&mut self, htex: &h::RawTexture<R>, desc: texture::ResourceDesc, is_target: bool)
                    -> Result<native::TextureView, f::ResourceViewError> {
        let raw_tex = self.frame_handles.ref_texture(htex);
        let td = htex.get_info();
        let info = vk::ImageViewCreateInfo {
            sType: vk::STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            image: raw_tex.image,
            viewType: match data::map_image_view_type(td.kind, desc.layer) {
                Ok(vt) => vt,
                Err(e) => return Err(f::ResourceViewError::Layer(e)),
            },
            format: match data::map_format(td.format, desc.channel) {
                Some(f) => f,
                None => return Err(f::ResourceViewError::Channel(desc.channel)),
            },
            components: data::map_swizzle(desc.swizzle),
            subresourceRange: vk::ImageSubresourceRange {
                aspectMask: data::map_image_aspect(td.format, desc.channel, is_target),
                baseMipLevel: desc.min as u32,
                levelCount: (desc.max + 1 - desc.min) as u32,
                baseArrayLayer: desc.layer.unwrap_or(0) as u32,
                layerCount: match desc.layer {
                    Some(_) => 1,
                    None => td.kind.get_num_slices().unwrap_or(1) as u32,
                },
            },
        };

        let (dev, vk) = self.share.get_device();
        let mut view = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateImageView(dev, &info, ptr::null(), &mut view)
        });
        Ok(native::TextureView {
            image: raw_tex.image,
            view: view,
            layout: raw_tex.layout.get(), //care!
            sub_range: info.subresourceRange,
        })
    }

    fn view_target(&mut self, htex: &h::RawTexture<R>, channel: ChannelType, layer: Option<Layer>)
                   -> Result<native::TextureView, f::TargetViewError>
    {
        let rdesc = texture::ResourceDesc {
            channel: channel,
            layer: layer,
            min: 0,
            max: 0,
            swizzle: core::format::Swizzle::new(),
        };
        self.view_texture(htex, rdesc, true).map_err(|err| match err {
            f::ResourceViewError::NoBindFlag  => f::TargetViewError::NoBindFlag,
            f::ResourceViewError::Channel(ct) => f::TargetViewError::Channel(ct),
            f::ResourceViewError::Layer(le)   => f::TargetViewError::Layer(le),
            f::ResourceViewError::Unsupported => f::TargetViewError::Unsupported,
        })
    }


    #[doc(hidden)]
    pub fn view_swapchain_image(&mut self, image: vk::Image, format: core::format::Format, size: (u32, u32))
                                -> Result<h::RawRenderTargetView<R>, f::TargetViewError> {
        use core::Factory;
        use core::handle::Producer;
        use core::texture as t;

        let raw_tex = native::Texture {
            image: image,
            layout: cell::Cell::new(vk::IMAGE_LAYOUT_GENERAL),
            memory: 0,
        };
        let tex_desc = t::Info {
            kind: t::Kind::D2(size.0 as t::Size, size.1 as t::Size, t::AaMode::Single),
            levels: 1,
            format: format.0,
            bind: memory::RENDER_TARGET,
            usage: memory::Usage::Data,
        };
        let tex = self.frame_handles.make_texture(raw_tex, tex_desc);
        let view_desc = t::RenderDesc {
            channel: format.1,
            level: 0,
            layer: None,
        };

        self.view_texture_as_render_target_raw(&tex, view_desc)
    }

    pub fn create_fence(&mut self, signalled: bool) -> vk::Fence {
        let info = vk::FenceCreateInfo {
            sType: vk::STRUCTURE_TYPE_FENCE_CREATE_INFO,
            pNext: ptr::null(),
            flags: if signalled { vk::FENCE_CREATE_SIGNALED_BIT } else { 0 },
        };
        let (dev, vk) = self.share.get_device();
        let mut fence = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateFence(dev, &info, ptr::null(), &mut fence)
        });
        fence
    }

    fn create_buffer_impl(&mut self, info: &buffer::Info) -> (native::Buffer, Option<MappingGate>) {
        let (usage, _) = data::map_usage_tiling(info.usage, info.bind);
        let native_info = vk::BufferCreateInfo {
            sType: vk::STRUCTURE_TYPE_BUFFER_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            size: info.size as vk::DeviceSize,
            usage: usage,
            sharingMode: vk::SHARING_MODE_EXCLUSIVE,
            queueFamilyIndexCount: 1,
            pQueueFamilyIndices: &self.queue_family_index,
        };
        let (dev, vk) = self.share.get_device();
        let mut buf = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateBuffer(dev, &native_info, ptr::null(), &mut buf)
        });
        let reqs = unsafe {
            let mut out = mem::zeroed();
            vk.GetBufferMemoryRequirements(dev, buf, &mut out);
            out
        };
        let mem = self.alloc(info.usage, reqs);
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BindBufferMemory(dev, buf, mem, 0)
        });

        use core::memory::Usage::*;
        let mapping = match info.usage {
            Data | Dynamic => None,
            Upload | Download => Some({
                let mut m = MappingGate {
                    pointer: ptr::null_mut(),
                    status: mapping::Status::clean(),
                };

                let offset = 0;
                let flags = 0;
                assert_eq!(vk::SUCCESS, unsafe {
                    vk.MapMemory(dev, mem, offset, vk::WHOLE_SIZE, flags, &mut m.pointer)
                });

                m
            }),
        };

        (native::Buffer {
            buffer: buf,
            memory: mem,
        }, mapping)
    }

    fn alloc(&self, usage: memory::Usage, reqs: vk::MemoryRequirements) -> vk::DeviceMemory {
        use core::memory::Usage::*;
        let info = vk::MemoryAllocateInfo {
            sType: vk::STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: ptr::null(),
            allocationSize: reqs.size,
            memoryTypeIndex: match usage {
                // TODO: more fine-grained memory selection
                // HOST_CACHED if possible for Download
                Upload | Download => self.mem_system_id,
                Data | Dynamic => self.mem_video_id,
            },
        };
        let (dev, vk) = self.share.get_device();
        let mut mem = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.AllocateMemory(dev, &info, ptr::null(), &mut mem)
        });
        mem
    }

    fn get_shader_stages(&mut self, program: &h::Program<R>) -> Vec<vk::PipelineShaderStageCreateInfo> {
        let prog = self.frame_handles.ref_program(program);
        let entry_name = b"main\0"; //TODO
        let mut stages = Vec::new();
        if true {
            stages.push(vk::PipelineShaderStageCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stage: vk::SHADER_STAGE_VERTEX_BIT,
                module: prog.vertex,
                pName: entry_name.as_ptr() as *const i8,
                pSpecializationInfo: ptr::null(),
            });
        }
        if let Some(geom) = prog.geometry {
            stages.push(vk::PipelineShaderStageCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stage: vk::SHADER_STAGE_GEOMETRY_BIT,
                module: geom,
                pName: entry_name.as_ptr() as *const i8,
                pSpecializationInfo: ptr::null(),
            });
        }
        if true {
            stages.push(vk::PipelineShaderStageCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stage: vk::SHADER_STAGE_FRAGMENT_BIT,
                module: prog.pixel,
                pName: entry_name.as_ptr() as *const i8,
                pSpecializationInfo: ptr::null(),
            });
        }
        stages
    }
}

impl Drop for Factory {
    fn drop(&mut self) {
        let (dev, vk) = self.share.get_device();
        unsafe {
            vk.DestroyCommandPool(dev, self.command_pool, ptr::null())
        };
    }
}

impl core::Factory<R> for Factory {
    fn get_capabilities(&self) -> &core::Capabilities {
        unimplemented!()
    }

    fn create_buffer_raw(&mut self, info: buffer::Info) -> Result<h::RawBuffer<R>, buffer::CreationError> {
        use core::handle::Producer;
        let (buffer, mapping) = self.create_buffer_impl(&info);
        Ok(self.share.handles.lock().unwrap().make_buffer(buffer, info, mapping))
    }

    fn create_buffer_immutable_raw(&mut self, data: &[u8], stride: usize, role: buffer::Role, bind: Bind)
                               -> Result<h::RawBuffer<R>, buffer::CreationError> {
        use core::handle::Producer;
        let info = buffer::Info {
            role: role,
            usage: memory::Usage::Data,
            bind: bind,
            size: data.len(),
            stride: stride,
        };
        let (buffer, mapping) = self.create_buffer_impl(&info);
        let (dev, vk) = self.share.get_device();
        unsafe {
            // FIXME
            let mut ptr = ptr::null_mut();
            assert_eq!(vk::SUCCESS, vk.MapMemory(dev, buffer.memory, 0, data.len() as u64, 0, &mut ptr));
            ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
            vk.UnmapMemory(dev, buffer.memory);
        }
        Ok(self.share.handles.lock().unwrap().make_buffer(buffer, info, mapping))
    }

    fn create_shader(&mut self, _stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use core::handle::Producer;
        use mirror::reflect_spirv_module;
        use native::Shader;
        let info = vk::ShaderModuleCreateInfo {
            sType: vk::STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            codeSize: code.len(),
            pCode: code.as_ptr() as *const _,
        };
        let (dev, vk) = self.share.get_device();
        let mut shader = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateShaderModule(dev, &info, ptr::null(), &mut shader)
        });
        let reflection = reflect_spirv_module(code);
        let shader = Shader {
            shader: shader,
            reflection: reflection,
        };
        Ok(self.share.handles.lock().unwrap().make_shader(shader))
    }

    fn create_program(&mut self, shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        use core::handle::Producer;
        use core::shade as s;
        use mirror::populate_info;

        let mut info = s::ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            output_depth: false,
            knows_outputs: false,
        };

        let fh = &mut self.frame_handles;
        let prog = match shader_set.clone() {
            core::ShaderSet::Simple(vs, ps) => {
                let (vs, ps) = (vs.reference(fh), ps.reference(fh));
                populate_info(&mut info, s::Stage::Vertex, &vs.reflection);
                populate_info(&mut info, s::Stage::Pixel, &ps.reflection);
                native::Program {
                    vertex: vs.shader,
                    geometry: None,
                    pixel: ps.shader,
                }
            }
            core::ShaderSet::Geometry(vs, gs, ps) => {
                let (vs, gs, ps) = (vs.reference(fh), gs.reference(fh), ps.reference(fh));
                populate_info(&mut info, s::Stage::Vertex, &vs.reflection);
                populate_info(&mut info, s::Stage::Geometry, &gs.reflection);
                populate_info(&mut info, s::Stage::Pixel, &ps.reflection);
                native::Program {
                    vertex: vs.shader,
                    geometry: Some(gs.shader),
                    pixel: ps.shader,
                }
            },
            core::ShaderSet::Tessellated(..) => unimplemented!(),
        };

        Ok(self.share.handles.lock().unwrap().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, desc: &pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, pso::CreationError> {
        use core::handle::Producer;
        let stages = self.get_shader_stages(program);
        let (dev, vk) = self.share.get_device();

        let set_layout = {
            let mut bindings = Vec::new();
            for (i, cb) in desc.constant_buffers.iter().enumerate() {
                if let &Some(usage) = cb {
                    bindings.push(vk::DescriptorSetLayoutBinding {
                        binding: i as u32,
                        descriptorType: vk::DESCRIPTOR_TYPE_UNIFORM_BUFFER,
                        descriptorCount: 1,
                        stageFlags: data::map_stage(usage),
                        pImmutableSamplers: ptr::null(),
                    });
                }
            }
            for (i, srv) in desc.resource_views.iter().enumerate() {
                if let &Some(usage) = srv {
                    bindings.push(vk::DescriptorSetLayoutBinding {
                        binding: i as u32,
                        descriptorType: vk::DESCRIPTOR_TYPE_SAMPLED_IMAGE,
                        descriptorCount: 1,
                        stageFlags: data::map_stage(usage),
                        pImmutableSamplers: ptr::null(),
                    });
                }
            }
            for (i, uav) in desc.unordered_views.iter().enumerate() {
                if let &Some(usage) = uav {
                    bindings.push(vk::DescriptorSetLayoutBinding {
                        binding: i as u32,
                        descriptorType: vk::DESCRIPTOR_TYPE_STORAGE_IMAGE, //TODO: buffer views
                        descriptorCount: 1,
                        stageFlags: data::map_stage(usage),
                        pImmutableSamplers: ptr::null(),
                    });
                }
            }
            for (i, sam) in desc.samplers.iter().enumerate() {
                if let &Some(usage) = sam {
                    bindings.push(vk::DescriptorSetLayoutBinding {
                        binding: i as u32,
                        descriptorType: vk::DESCRIPTOR_TYPE_SAMPLER,
                        descriptorCount: 1,
                        stageFlags: data::map_stage(usage),
                        pImmutableSamplers: ptr::null(),
                    });
                }
            }
            let info = vk::DescriptorSetLayoutCreateInfo {
                sType: vk::STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                bindingCount: bindings.len() as u32,
                pBindings: bindings.as_ptr(),
            };
            let mut out = 0;
            assert_eq!(vk::SUCCESS, unsafe {
                vk.CreateDescriptorSetLayout(dev, &info, ptr::null(), &mut out)
            });
            out
        };
        let pipe_layout = {
            let info = vk::PipelineLayoutCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                setLayoutCount: 1,
                pSetLayouts: &set_layout,
                pushConstantRangeCount: 0,
                pPushConstantRanges: ptr::null(),
            };
            let mut out = 0;
            assert_eq!(vk::SUCCESS, unsafe {
                vk.CreatePipelineLayout(dev, &info, ptr::null(), &mut out)
            });
            out
        };
        let pool = {
            let info = vk::DescriptorPoolCreateInfo {
                sType: vk::STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                maxSets: 100, //TODO
                poolSizeCount: 0,
                pPoolSizes: ptr::null(),
            };
            let mut out = 0;
            assert_eq!(vk::SUCCESS, unsafe {
                vk.CreateDescriptorPool(dev, &info, ptr::null(), &mut out)
            });
            out
        };
        let render_pass = {
            let mut attachments = Vec::new();
            let mut color_refs = Vec::new();
            for col in desc.color_targets.iter().filter_map(|c| c.as_ref()) {
                let layout = vk::IMAGE_LAYOUT_GENERAL; //TODO
                color_refs.push(vk::AttachmentReference {
                    attachment: attachments.len() as u32,
                    layout: layout,
                });
                attachments.push(vk::AttachmentDescription {
                    flags: 0,
                    format: match data::map_format((col.0).0, (col.0).1) {
                        Some(fm) => fm,
                        None => return Err(pso::CreationError),
                    },
                    samples: vk::SAMPLE_COUNT_1_BIT, //TODO
                    loadOp: vk::ATTACHMENT_LOAD_OP_LOAD,
                    storeOp: vk::ATTACHMENT_STORE_OP_STORE,
                    stencilLoadOp: vk::ATTACHMENT_LOAD_OP_DONT_CARE,
                    stencilStoreOp: vk::ATTACHMENT_STORE_OP_DONT_CARE,
                    initialLayout: layout,
                    finalLayout: layout,
                });
            }
            let ds_ref = vk::AttachmentReference {
                attachment: attachments.len() as u32,
                layout: vk::IMAGE_LAYOUT_GENERAL, //TODO
            };
            if let Some(ds) = desc.depth_stencil {
                attachments.push(vk::AttachmentDescription {
                    flags: 0,
                    format: match data::map_format((ds.0).0, (ds.0).1) {
                        Some(fm) => fm,
                        None => return Err(pso::CreationError),
                    },
                    samples: vk::SAMPLE_COUNT_1_BIT, //TODO
                    loadOp: vk::ATTACHMENT_LOAD_OP_LOAD,
                    storeOp: vk::ATTACHMENT_STORE_OP_STORE,
                    stencilLoadOp: vk::ATTACHMENT_LOAD_OP_LOAD,
                    stencilStoreOp: vk::ATTACHMENT_STORE_OP_STORE,
                    initialLayout: vk::IMAGE_LAYOUT_GENERAL, //TODO
                    finalLayout: vk::IMAGE_LAYOUT_GENERAL,
                });
            }
            let info = vk::RenderPassCreateInfo {
                sType: vk::STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                attachmentCount: attachments.len() as u32,
                pAttachments: attachments.as_ptr(),
                subpassCount: 1,
                pSubpasses: &vk::SubpassDescription {
                    flags: 0,
                    pipelineBindPoint: vk::PIPELINE_BIND_POINT_GRAPHICS,
                    inputAttachmentCount: 0,
                    pInputAttachments: ptr::null(),
                    colorAttachmentCount: color_refs.len() as u32,
                    pColorAttachments: color_refs.as_ptr(),
                    pResolveAttachments: ptr::null(),
                    pDepthStencilAttachment: if desc.depth_stencil.is_some() {&ds_ref} else {ptr::null()},
                    preserveAttachmentCount: 0,
                    pPreserveAttachments: ptr::null(),
                },
                dependencyCount: 0,
                pDependencies: ptr::null(),
            };
            let mut out = 0;
            assert_eq!(vk::SUCCESS, unsafe {
                vk.CreateRenderPass(dev, &info, ptr::null(), &mut out)
            });
            out
        };
        let pipeline = {
            let mut vertex_bindings = Vec::new();
            for (i, vbuf) in desc.vertex_buffers.iter().enumerate() {
                if let &Some(v) = vbuf {
                    vertex_bindings.push(vk::VertexInputBindingDescription {
                        binding: i as u32,
                        stride: v.stride as u32,
                        inputRate: v.rate as vk::VertexInputRate,
                    });
                }
            }
            let mut vertex_attributes = Vec::new();
            for (i, attr) in desc.attributes.iter().enumerate() {
                if let &Some(a) = attr {
                    vertex_attributes.push(vk::VertexInputAttributeDescription {
                        location: i as u32,
                        binding: a.0 as u32,
                        format: match data::map_format(a.1.format.0, a.1.format.1) {
                            Some(fm) => fm,
                            None => return Err(pso::CreationError),
                        },
                        offset: a.1.offset as u32,
                    });
                }
            }
            let mut attachments = Vec::new();
            for ocd in desc.color_targets.iter() {
                if let &Some(ref cd) = ocd {
                    attachments.push(data::map_blend(&cd.1));
                }
            }
            let (polygon, line_width) = data::map_polygon_mode(desc.rasterizer.method);
            let info = vk::GraphicsPipelineCreateInfo {
                sType: vk::STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stageCount: stages.len() as u32,
                pStages: stages.as_ptr(),
                pVertexInputState: &vk::PipelineVertexInputStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    vertexBindingDescriptionCount: vertex_bindings.len() as u32,
                    pVertexBindingDescriptions: vertex_bindings.as_ptr(),
                    vertexAttributeDescriptionCount: vertex_attributes.len() as u32,
                    pVertexAttributeDescriptions: vertex_attributes.as_ptr(),
                },
                pInputAssemblyState: &vk::PipelineInputAssemblyStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    topology: data::map_topology(desc.primitive),
                    primitiveRestartEnable: vk::FALSE,
                },
                pTessellationState: ptr::null(),
                pViewportState: &vk::PipelineViewportStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    viewportCount: 1,
                    pViewports: &vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                        minDepth: 0.0,
                        maxDepth: 1.0,
                    },
                    scissorCount: 1,
                    pScissors: &vk::Rect2D {
                        offset: vk::Offset2D {
                            x: 0, y: 0,
                        },
                        extent: vk::Extent2D {
                            width: 1, height: 1,
                        },
                    },
                },
                pRasterizationState: &vk::PipelineRasterizationStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    depthClampEnable: vk::TRUE,
                    rasterizerDiscardEnable: vk::FALSE,
                    polygonMode: polygon,
                    cullMode: data::map_cull_face(desc.rasterizer.cull_face),
                    frontFace: data::map_front_face(desc.rasterizer.front_face),
                    depthBiasEnable: if desc.rasterizer.offset.is_some() { vk::TRUE } else { vk::FALSE },
                    depthBiasConstantFactor: desc.rasterizer.offset.map_or(0.0, |off| off.1 as f32),
                    depthBiasClamp: 1.0,
                    depthBiasSlopeFactor: desc.rasterizer.offset.map_or(0.0, |off| off.0 as f32),
                    lineWidth: line_width,
                },
                pMultisampleState: &vk::PipelineMultisampleStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    rasterizationSamples: vk::SAMPLE_COUNT_1_BIT, //TODO
                    sampleShadingEnable: vk::FALSE,
                    minSampleShading: 0.0,
                    pSampleMask: ptr::null(),
                    alphaToCoverageEnable: vk::FALSE,
                    alphaToOneEnable: vk::FALSE,
                },
                pDepthStencilState: &vk::PipelineDepthStencilStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    depthTestEnable: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { depth: Some(_), ..} )) => vk::TRUE,
                        _ => vk::FALSE,
                    },
                    depthWriteEnable: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { depth: Some(state::Depth { write: true, ..}), ..} )) => vk::TRUE,
                        _ => vk::FALSE,
                    },
                    depthCompareOp: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { depth: Some(state::Depth { fun, ..}), ..} )) => data::map_comparison(fun),
                        _ => vk::COMPARE_OP_NEVER,
                    },
                    depthBoundsTestEnable: vk::FALSE,
                    stencilTestEnable: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { front: Some(_), ..} )) => vk::TRUE,
                        Some((_, pso::DepthStencilInfo { back: Some(_), ..} )) => vk::TRUE,
                        _ => vk::FALSE,
                    },
                    front: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { front: Some(ref s), ..} )) => data::map_stencil_side(s),
                        _ => unsafe { mem::zeroed() },
                    },
                    back: match desc.depth_stencil {
                        Some((_, pso::DepthStencilInfo { back: Some(ref s), ..} )) => data::map_stencil_side(s),
                        _ => unsafe { mem::zeroed() },
                    },
                    minDepthBounds: 0.0,
                    maxDepthBounds: 1.0,
                },
                pColorBlendState: &vk::PipelineColorBlendStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    logicOpEnable: vk::FALSE,
                    logicOp: vk::LOGIC_OP_CLEAR,
                    attachmentCount: attachments.len() as u32,
                    pAttachments: attachments.as_ptr(),
                    blendConstants: [0.0; 4],
                },
                pDynamicState: &vk::PipelineDynamicStateCreateInfo {
                    sType: vk::STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    dynamicStateCount: 1,
                    pDynamicStates: [
                        vk::DYNAMIC_STATE_VIEWPORT,
                        vk::DYNAMIC_STATE_SCISSOR,
                        vk::DYNAMIC_STATE_BLEND_CONSTANTS,
                        vk::DYNAMIC_STATE_STENCIL_REFERENCE,
                        ].as_ptr(),
                },
                layout: pipe_layout,
                renderPass: render_pass,
                subpass: 0,
                basePipelineHandle: 0,
                basePipelineIndex: 0,
            };
            let mut out = 0;
            assert_eq!(vk::SUCCESS, unsafe {
                vk.CreateGraphicsPipelines(dev, 0, 1, &info, ptr::null(), &mut out)
            });
            out
        };
        let pso = native::Pipeline {
            pipeline: pipeline,
            pipe_layout: pipe_layout,
            desc_layout: set_layout,
            desc_pool: pool,
            render_pass: render_pass,
            program: program.clone(),
        };
        Ok(self.share.handles.lock().unwrap().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: texture::Info, hint: Option<core::format::ChannelType>,
                          _data_opt: Option<&[&[u8]]>) -> Result<h::RawTexture<R>, texture::CreationError> {
        use core::handle::Producer;

        let (w, h, d, aa) = desc.kind.get_dimensions();
        let slices = desc.kind.get_num_slices();
        let (usage, tiling) = data::map_usage_tiling(desc.usage, desc.bind);
        let chan_type = hint.unwrap_or(core::format::ChannelType::Uint);
        let info = vk::ImageCreateInfo {
            sType: vk::STRUCTURE_TYPE_IMAGE_CREATE_INFO,
            pNext: ptr::null(),
            flags: vk::IMAGE_CREATE_MUTABLE_FORMAT_BIT |
                (if desc.kind.is_cube() {vk::IMAGE_CREATE_CUBE_COMPATIBLE_BIT} else {0}),
            imageType: data::map_image_type(desc.kind),
            format: match data::map_format(desc.format, chan_type) {
                Some(f) => f,
                None => return Err(texture::CreationError::Format(desc.format, hint)),
            },
            extent: vk::Extent3D {
                width: w as u32,
                height: h as u32,
                depth: if slices.is_none() {d as u32} else {1},
            },
            mipLevels: desc.levels as u32,
            arrayLayers: slices.unwrap_or(1) as u32,
            samples: aa.get_num_fragments() as vk::SampleCountFlagBits,
            tiling: tiling,
            usage: usage,
            sharingMode: vk::SHARING_MODE_EXCLUSIVE,
            queueFamilyIndexCount: 0,
            pQueueFamilyIndices: ptr::null(),
            initialLayout: data::map_image_layout(desc.bind),
        };
        let (dev, vk) = self.share.get_device();
        let mut image = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateImage(dev, &info, ptr::null(), &mut image)
        });
        let reqs = unsafe {
            let mut out = mem::zeroed();
            vk.GetImageMemoryRequirements(dev, image, &mut out);
            out
        };
        let tex = native::Texture {
            image: image,
            layout: cell::Cell::new(info.initialLayout),
            memory: self.alloc(desc.usage, reqs),
        };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BindImageMemory(dev, image, tex.memory, 0)
        });
        Ok(self.share.handles.lock().unwrap().make_texture(tex, desc))
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                      -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::ResourceDesc)
                                       -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        use core::handle::Producer;
        self.view_texture(htex, desc, false).map(|view|
            self.share.handles.lock().unwrap().make_texture_srv(view, htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::RenderDesc)
                                         -> Result<h::RawRenderTargetView<R>, f::TargetViewError>
    {
        use core::handle::Producer;
        let mut dim = htex.get_info().kind.get_dimensions();
        if desc.layer.is_some() {
            dim.2 = 1; // slice of the depth/array
        }
        self.view_target(htex, desc.channel, desc.layer).map(|view|
            self.share.handles.lock().unwrap().make_rtv(view, htex, dim))
    }

    fn view_texture_as_depth_stencil_raw(&mut self, htex: &h::RawTexture<R>, desc: texture::DepthStencilDesc)
                                         -> Result<h::RawDepthStencilView<R>, f::TargetViewError>
    {
        use core::handle::Producer;
        let mut dim = htex.get_info().kind.get_dimensions();
        if desc.layer.is_some() {
            dim.2 = 1; // slice of the depth/array
        }
        let channel = ChannelType::Unorm; //TODO
        self.view_target(htex, channel, desc.layer).map(|view|
            self.share.handles.lock().unwrap().make_dsv(view, htex, dim))
    }

    fn create_sampler(&mut self, info: texture::SamplerInfo) -> h::Sampler<R> {
        use core::handle::Producer;

        let (min, mag, mip, aniso) = data::map_filter(info.filter);
        let native_info = vk::SamplerCreateInfo {
            sType: vk::STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            magFilter: mag,
            minFilter: min,
            mipmapMode: mip,
            addressModeU: data::map_wrap(info.wrap_mode.0),
            addressModeV: data::map_wrap(info.wrap_mode.1),
            addressModeW: data::map_wrap(info.wrap_mode.2),
            mipLodBias: info.lod_bias.into(),
            anisotropyEnable: if aniso > 0.0 { vk::TRUE } else { vk::FALSE },
            maxAnisotropy: aniso,
            compareEnable: if info.comparison.is_some() { vk::TRUE } else { vk::FALSE },
            compareOp: data::map_comparison(info.comparison.unwrap_or(state::Comparison::Never)),
            minLod: info.lod_range.0.into(),
            maxLod: info.lod_range.1.into(),
            borderColor: match data::map_border_color(info.border) {
                Some(bc) => bc,
                None => {
                    error!("Unsupported border color {:x}", info.border.0);
                    vk::BORDER_COLOR_FLOAT_TRANSPARENT_BLACK
                }
            },
            unnormalizedCoordinates: vk::FALSE,
        };

        let (dev, vk) = self.share.get_device();
        let mut sampler = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateSampler(dev, &native_info, ptr::null(), &mut sampler)
        });
        self.share.handles.lock().unwrap().make_sampler(sampler, info)
    }

    fn read_mapping<'a, 'b, T>(&'a mut self, _: &'b h::Buffer<R, T>)
                               -> Result<mapping::Reader<'b, R, T>,
                                         mapping::Error>
        where T: Copy
    {
        unimplemented!()
    }

    fn write_mapping<'a, 'b, T>(&'a mut self, _: &'b h::Buffer<R, T>)
                                -> Result<mapping::Writer<'b, R, T>,
                                          mapping::Error>
        where T: Copy
    {
        unimplemented!()
    }
}
