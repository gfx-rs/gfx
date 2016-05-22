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
use gfx_core::{self as core, handle as h, factory as f, state};
use vk;
use {command, data, native};
use {Resources as R, SharePointer};


#[derive(Copy, Clone)]
pub struct RawMapping {
    pointer: *mut c_void,
}

impl core::mapping::Raw for RawMapping {
    unsafe fn set<T>(&self, index: usize, val: T) {
        *(self.pointer as *mut T).offset(index as isize) = val;
    }

    unsafe fn to_slice<T>(&self, len: usize) -> &[T] {
        slice::from_raw_parts(self.pointer as *const T, len)
    }

    unsafe fn to_mut_slice<T>(&self, len: usize) -> &mut [T] {
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
        let alloc_info = vk::CommandBufferAllocateInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            pNext: ptr::null(),
            commandPool: self.command_pool,
            level: vk::COMMAND_BUFFER_LEVEL_PRIMARY,
            commandBufferCount: 1,
        };
        let begin_info = vk::CommandBufferBeginInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: ptr::null(),
            flags: 0,
            pInheritanceInfo: ptr::null(),
        };
        let (dev, vk) = self.share.get_device();
        let mut buf = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.AllocateCommandBuffers(dev, &alloc_info, &mut buf)
        });
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BeginCommandBuffer(buf, &begin_info)
        });
        command::Buffer::new(buf, self.queue_family_index, self.share.clone())
    }

    fn view_texture(&mut self, htex: &h::RawTexture<R>, desc: core::tex::ResourceDesc, is_target: bool)
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


    #[doc(hidden)]
    pub fn view_swapchain_image(&mut self, image: vk::Image, format: core::format::Format, size: (u32, u32))
                                -> Result<h::RawRenderTargetView<R>, f::TargetViewError> {
        use gfx_core::Factory;
        use gfx_core::handle::Producer;
        use gfx_core::tex as t;

        let raw_tex = native::Texture {
            image: image,
            layout: cell::Cell::new(vk::IMAGE_LAYOUT_GENERAL),
            memory: 0,
        };
        let tex_desc = t::Descriptor {
            kind: t::Kind::D2(size.0 as t::Size, size.1 as t::Size, t::AaMode::Single),
            levels: 1,
            format: format.0,
            bind: f::RENDER_TARGET,
            usage: f::Usage::GpuOnly,
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

    fn alloc(&self, usage: f::Usage, reqs: vk::MemoryRequirements) -> vk::DeviceMemory {
        let info = vk::MemoryAllocateInfo {
            sType: vk::STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: ptr::null(),
            allocationSize: reqs.size,
            memoryTypeIndex: if let f::Usage::CpuOnly(_) = usage {
                self.mem_system_id
            }else {
                self.mem_video_id
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
        let mut stages = Vec::with_capacity(3);
        if true {
            stages.push(vk::PipelineShaderStageCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stage: vk::SHADER_STAGE_VERTEX_BIT,
                module: *prog.vertex.reference(&mut self.frame_handles),
                pName: entry_name.as_ptr() as *const i8,
                pSpecializationInfo: ptr::null(),
            });
        }
        if let Some(ref geom) = prog.geometry {
            stages.push(vk::PipelineShaderStageCreateInfo {
                sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stage: vk::SHADER_STAGE_GEOMETRY_BIT,
                module: *geom.reference(&mut self.frame_handles),
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
                module: *prog.pixel.reference(&mut self.frame_handles),
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
    type Mapper = RawMapping;

    fn get_capabilities(&self) -> &core::Capabilities {
        unimplemented!()
    }

    fn create_buffer_raw(&mut self, info: f::BufferInfo) -> Result<h::RawBuffer<R>, f::BufferError> {
        use gfx_core::handle::Producer;
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
        let buffer = native::Buffer {
            buffer: buf,
            memory: self.alloc(info.usage, reqs),
        };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BindBufferMemory(dev, buf, buffer.memory, 0)
        });
        Ok(self.share.handles.borrow_mut().make_buffer(buffer, info))
    }

    fn create_buffer_const_raw(&mut self, _data: &[u8], _stride: usize, _role: f::BufferRole, _bind: f::Bind)
                               -> Result<h::RawBuffer<R>, f::BufferError> {
        unimplemented!()
    }

    fn create_shader(&mut self, _stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use gfx_core::handle::Producer;
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
        Ok(self.share.handles.borrow_mut().make_shader(shader))
    }

    fn create_program(&mut self, shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        use gfx_core::handle::Producer;
        use gfx_core::shade as s;

        let prog = match shader_set.clone() {
            core::ShaderSet::Simple(vs, ps) => native::Program {
                vertex: vs,
                geometry: None,
                pixel: ps,
            },
            core::ShaderSet::Geometry(vs, gs, ps) => native::Program {
                vertex: vs,
                geometry: Some(gs),
                pixel: ps,
            },
        };
        let info = s::ProgramInfo {
            vertex_attributes: Vec::new(),
            globals: Vec::new(),
            constant_buffers: Vec::new(),
            textures: Vec::new(),
            unordereds: Vec::new(),
            samplers: Vec::new(),
            outputs: Vec::new(),
            knows_outputs: false,
        };
        Ok(self.share.handles.borrow_mut().make_program(prog, info))
    }

    fn create_pipeline_state_raw(&mut self, program: &h::Program<R>, _desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        use gfx_core::handle::Producer;
        let stages = self.get_shader_stages(program);
        let (dev, vk) = self.share.get_device();

        let set_layout = {
            let info = vk::DescriptorSetLayoutCreateInfo {
                sType: vk::STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                bindingCount: 0,
                pBindings: ptr::null(), //TODO
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
        let pipeline = {
            let info = vk::GraphicsPipelineCreateInfo {
                sType: vk::STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                stageCount: stages.len() as u32,
                pStages: stages.as_ptr(),
                layout: pipe_layout,
                renderPass: 0, //TODO
                subpass: 0,
                basePipelineHandle: 0,
                basePipelineIndex: 0,
                .. unsafe { mem::zeroed() } //TODO
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
            program: program.clone(),
        };
        Ok(self.share.handles.borrow_mut().make_pso(pso, program))
    }

    fn create_texture_raw(&mut self, desc: core::tex::Descriptor, hint: Option<core::format::ChannelType>,
                          _data_opt: Option<&[&[u8]]>) -> Result<h::RawTexture<R>, core::tex::Error> {
        use gfx_core::handle::Producer;

        let (w, h, d, aa) = desc.kind.get_dimensions();
        let slices = desc.kind.get_num_slices();
        let (usage, tiling) = data::map_usage_tiling(desc.usage, desc.bind);
        let chan_type = hint.unwrap_or(core::format::ChannelType::Uint);
        let image_info = vk::ImageCreateInfo {
            sType: vk::STRUCTURE_TYPE_IMAGE_CREATE_INFO,
            pNext: ptr::null(),
            flags: vk::IMAGE_CREATE_MUTABLE_FORMAT_BIT |
                (if desc.kind.is_cube() {vk::IMAGE_CREATE_CUBE_COMPATIBLE_BIT} else {0}),
            imageType: data::map_image_type(desc.kind),
            format: match data::map_format(desc.format, chan_type) {
                Some(f) => f,
                None => return Err(core::tex::Error::Format(desc.format, hint)),
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
            vk.CreateImage(dev, &image_info, ptr::null(), &mut image)
        });
        let reqs = unsafe {
            let mut out = mem::zeroed();
            vk.GetImageMemoryRequirements(dev, image, &mut out);
            out
        };
        let tex = native::Texture {
            image: image,
            layout: cell::Cell::new(image_info.initialLayout),
            memory: self.alloc(desc.usage, reqs),
        };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BindImageMemory(dev, image, tex.memory, 0)
        });
        Ok(self.share.handles.borrow_mut().make_texture(tex, desc))
    }

    fn view_buffer_as_shader_resource_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                      -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_buffer_as_unordered_access_raw(&mut self, _hbuf: &h::RawBuffer<R>)
                                       -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_shader_resource_raw(&mut self, htex: &h::RawTexture<R>, desc: core::tex::ResourceDesc)
                                       -> Result<h::RawShaderResourceView<R>, f::ResourceViewError> {
        use gfx_core::handle::Producer;
        self.view_texture(htex, desc, false).map(|view|
            self.share.handles.borrow_mut().make_texture_srv(view, htex))
    }

    fn view_texture_as_unordered_access_raw(&mut self, _htex: &h::RawTexture<R>)
                                        -> Result<h::RawUnorderedAccessView<R>, f::ResourceViewError> {
        Err(f::ResourceViewError::Unsupported) //TODO
    }

    fn view_texture_as_render_target_raw(&mut self, htex: &h::RawTexture<R>, desc: core::tex::RenderDesc)
                                         -> Result<h::RawRenderTargetView<R>, f::TargetViewError>
    {
        use gfx_core::handle::Producer;
        let rdesc = core::tex::ResourceDesc {
            channel: desc.channel,
            layer: desc.layer,
            min: 0,
            max: 0,
            swizzle: core::format::Swizzle::new(),
        };
        let mut dim = htex.get_info().kind.get_dimensions();
        if rdesc.layer.is_some() {
            dim.2 = 1; // slice of the depth/array
        }
        match self.view_texture(htex, rdesc, true) {
            Ok(view) => Ok(self.share.handles.borrow_mut().make_rtv(view, htex, dim)),
            Err(f::ResourceViewError::NoBindFlag) => Err(f::TargetViewError::NoBindFlag),
            Err(f::ResourceViewError::Channel(ct)) => Err(f::TargetViewError::Channel(ct)),
            Err(f::ResourceViewError::Layer(le))   => Err(f::TargetViewError::Layer(le)),
            Err(f::ResourceViewError::Unsupported) => Err(f::TargetViewError::Unsupported),
        }
    }

    fn view_texture_as_depth_stencil_raw(&mut self, _htex: &h::RawTexture<R>, _desc: core::tex::DepthStencilDesc)
                                         -> Result<h::RawDepthStencilView<R>, f::TargetViewError>
    {
        unimplemented!()
    }

    fn create_sampler(&mut self, info: core::tex::SamplerInfo) -> h::Sampler<R> {
        use gfx_core::handle::Producer;

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
        self.share.handles.borrow_mut().make_sampler(sampler, info)
    }

    fn map_buffer_raw(&mut self, _buf: &h::RawBuffer<R>, _access: f::MapAccess) -> RawMapping {
        unimplemented!()
    }

    fn unmap_buffer_raw(&mut self, _map: RawMapping) {
        unimplemented!()
    }

    fn map_buffer_readable<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                           -> core::mapping::Readable<T, R, Factory> {
        unimplemented!()
    }

    fn map_buffer_writable<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                                    -> core::mapping::Writable<T, R, Factory> {
        unimplemented!()
    }

    fn map_buffer_rw<T: Copy>(&mut self, _buf: &h::Buffer<R, T>)
                              -> core::mapping::RW<T, R, Factory> {
        unimplemented!()
    }
}