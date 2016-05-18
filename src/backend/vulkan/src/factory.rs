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

use std::{mem, ptr, slice};
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
    qf_index: u32,
    command_pool: vk::CommandPool,
    frame_handles: h::Manager<R>,
}

impl Factory {
    pub fn new(share: SharePointer, qf_index: u32) -> Factory {
        let com_info = vk::CommandPoolCreateInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: ptr::null(),
            flags: vk::COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,
            queueFamilyIndex: qf_index,
        };
        let com_pool = unsafe {
            let (dev, vk) = share.get_device();
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateCommandPool(dev, &com_info, ptr::null(), &mut out));
            out
        };
        Factory {
            share: share,
            qf_index: qf_index,
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
        let buf = unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.AllocateCommandBuffers(dev, &alloc_info, &mut out));
            assert_eq!(vk::SUCCESS, vk.BeginCommandBuffer(out, &begin_info));
            out
        };
        command::Buffer::new(buf, self.qf_index, self.share.clone())
    }

    fn view_texture(&mut self, htex: &h::RawTexture<R>, desc: core::tex::ResourceDesc, is_target: bool)
                    -> Result<vk::ImageView, f::ResourceViewError> {
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
        Ok(unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateImageView(dev, &info, ptr::null(), &mut out));
            out
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
        unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateFence(dev, &info, ptr::null(), &mut out));
            out
        }
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

    fn create_buffer_raw(&mut self, _info: f::BufferInfo) -> Result<h::RawBuffer<R>, f::BufferError> {
        unimplemented!()
    }

    fn create_buffer_const_raw(&mut self, _data: &[u8], _stride: usize, _role: f::BufferRole, _bind: f::Bind)
                                -> Result<h::RawBuffer<R>, f::BufferError> {
        unimplemented!()
    }

    fn create_shader(&mut self, _stage: core::shade::Stage, code: &[u8])
                     -> Result<h::Shader<R>, core::shade::CreateShaderError> {
        use gfx_core::handle::Producer;
        let (dev, vk) = self.share.get_device();
        let info = vk::ShaderModuleCreateInfo {
            sType: vk::STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            codeSize: code.len(),
            pCode: code.as_ptr() as *const _,
        };
        let shader = unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateShaderModule(dev, &info, ptr::null(), &mut out));
            out
        };
        Ok(self.share.handles.borrow_mut().make_shader(shader))
    }

    fn create_program(&mut self, _shader_set: &core::ShaderSet<R>)
                      -> Result<h::Program<R>, core::shade::CreateProgramError> {
        unimplemented!()
    }

    fn create_pipeline_state_raw(&mut self, _program: &h::Program<R>, _desc: &core::pso::Descriptor)
                                 -> Result<h::RawPipelineState<R>, core::pso::CreationError> {
        /*let shader = native::Shader(vk::PipelineShaderStageCreateInfo {
            sType: vk::STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            stage: 0, //TODO
            module: !0, //TODO
            pName: ptr::null(),
            pSpecializationInfo: ptr::null(),
        });*/
        unimplemented!()
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
        let mut alloc_info = vk::MemoryAllocateInfo {
            sType: vk::STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: ptr::null(),
            allocationSize: 0,
            memoryTypeIndex: 0, //TODO
        };
        let (dev, vk) = self.share.get_device();
        let tex = unsafe {
            let mut image = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateImage(dev, &image_info, ptr::null(), &mut image));
            let mut reqs = mem::zeroed();
            vk.GetImageMemoryRequirements(dev, image, &mut reqs);
            alloc_info.allocationSize = reqs.size;
            let mut mem = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.AllocateMemory(dev, &alloc_info, ptr::null(), &mut mem));
            assert_eq!(vk::SUCCESS, vk.BindImageMemory(dev, image, mem, 0));
            native::Texture { image: image, memory: mem }
        };
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
        let sampler = unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateSampler(dev, &native_info, ptr::null(), &mut out));
            out
        };
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