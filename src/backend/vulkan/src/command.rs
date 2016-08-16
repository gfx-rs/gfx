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

use std::{mem, ptr};
use std::collections::hash_map::{HashMap, Entry};
use vk;
use gfx_core::{self as core, draw, pso, shade, target, tex};
use gfx_core::state::RefValues;
use gfx_core::{IndexType, VertexCount};
use native;
use {Resources, Share, SharePointer};


pub struct Buffer {
    inner: vk::CommandBuffer,
    parent_pool: vk::CommandPool,
    family: u32,
    share: SharePointer,
    last_render_pass: vk::RenderPass,
    fbo_cache: HashMap<pso::PixelTargetSet<Resources>, vk::Framebuffer>,
    temp_attachments: Vec<vk::ImageView>,
}

impl Buffer {
    #[doc(hidden)]
    pub fn new(pool: vk::CommandPool, family: u32, share: SharePointer) -> Buffer {
        let alloc_info = vk::CommandBufferAllocateInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            pNext: ptr::null(),
            commandPool: pool,
            level: vk::COMMAND_BUFFER_LEVEL_PRIMARY,
            commandBufferCount: 1,
        };
        let begin_info = vk::CommandBufferBeginInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: ptr::null(),
            flags: 0,
            pInheritanceInfo: ptr::null(),
        };
        Buffer {
            inner: {
                let (dev, vk) = share.get_device();
                let mut buf = 0;
                assert_eq!(vk::SUCCESS, unsafe {
                    vk.AllocateCommandBuffers(dev, &alloc_info, &mut buf)
                });
                assert_eq!(vk::SUCCESS, unsafe {
                    vk.BeginCommandBuffer(buf, &begin_info)
                });
                buf
            },
            parent_pool: pool,
            family: family,
            share: share,
            last_render_pass: 0,
            fbo_cache: HashMap::new(),
            temp_attachments: Vec::new(),
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let (dev, vk) = self.share.get_device();
        unsafe {
            vk.FreeCommandBuffers(dev, self.parent_pool, 1, &self.inner);
        }
        for &fbo in self.fbo_cache.values() {
            unsafe {
                vk.DestroyFramebuffer(dev, fbo, ptr::null());
            }
        }
    }
}

impl Buffer {
    pub fn image_barrier(&mut self, image: vk::Image, aspect: vk::ImageAspectFlags,
                         old_layout: vk::ImageLayout, new_layout: vk::ImageLayout) {
        let barrier = vk::ImageMemoryBarrier {
            sType: vk::STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
            pNext: ptr::null(),
            srcAccessMask: if old_layout == vk::IMAGE_LAYOUT_PREINITIALIZED || new_layout == vk::IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL {
                vk::ACCESS_HOST_WRITE_BIT | vk::ACCESS_TRANSFER_WRITE_BIT
            } else {0},
            dstAccessMask: match new_layout {
                vk::IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL | vk::IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL =>
                    vk::ACCESS_TRANSFER_READ_BIT | vk::ACCESS_HOST_WRITE_BIT | vk::ACCESS_TRANSFER_WRITE_BIT,
                vk::IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL => vk::ACCESS_SHADER_READ_BIT,
                _ => 0,
            },
            oldLayout: old_layout,
            newLayout: new_layout,
            srcQueueFamilyIndex: self.family,
            dstQueueFamilyIndex: self.family,
            image: image,
            subresourceRange: vk::ImageSubresourceRange {
                aspectMask: aspect,
                baseMipLevel: 0,
                levelCount: 1,
                baseArrayLayer: 0,
                layerCount: 1,
            },
        };
        let (_dev, vk) = self.share.get_device();
        unsafe {
            vk.CmdPipelineBarrier(self.inner,
                vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT, vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT, 0,
                0, ptr::null(), 0, ptr::null(), 1, &barrier);
        }
    }
}

impl draw::CommandBuffer<Resources> for Buffer {
    fn reset(&mut self) {
        let (_, vk) = self.share.get_device();
        assert_eq!(vk::SUCCESS, unsafe {
            vk.ResetCommandBuffer(self.inner, 0)
        });
    }

    fn bind_pipeline_state(&mut self, pso: native::Pipeline) {
        let (_, vk) = self.share.get_device();
        self.last_render_pass = pso.render_pass;
        unsafe {
            vk.CmdBindPipeline(self.inner, vk::PIPELINE_BIND_POINT_GRAPHICS, pso.pipeline);
        }
    }

    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Resources>) {}
    fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<Resources>]) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: &[pso::ResourceViewParam<Resources>]) {}
    fn bind_unordered_views(&mut self, _: &[pso::UnorderedViewParam<Resources>]) {}
    fn bind_samplers(&mut self, _: &[pso::SamplerParam<Resources>]) {}

    fn bind_pixel_targets(&mut self, pts: pso::PixelTargetSet<Resources>) {
        let (dev, vk) = self.share.get_device();
        let vp = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: pts.size.0 as f32,
            height: pts.size.1 as f32,
            minDepth: 0.0,
            maxDepth: 1.0,
        };
        let fbo = match self.fbo_cache.entry(pts) {
            Entry::Occupied(oe) => *oe.get(),
            Entry::Vacant(ve) => {
                let mut ats = &mut self.temp_attachments;
                ats.clear();
                for color in pts.colors.iter() {
                    if let &Some(ref tv) = color {
                        ats.push(tv.view);
                    }
                }
                match (pts.depth, pts.stencil) {
                    (None, None) => (),
                    (Some(vd), Some(vs)) => {
                        if vd != vs {
                            error!("Different depth and stencil are not supported")
                        }
                        ats.push(vd.view);
                    },
                    (Some(vd), None) => ats.push(vd.view),
                    (None, Some(vs)) => ats.push(vs.view),
                }
                let info = vk::FramebufferCreateInfo {
                    sType: vk::STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    renderPass: self.last_render_pass,
                    attachmentCount: ats.len() as u32,
                    pAttachments: ats.as_ptr(),
                    width: pts.size.0 as u32,
                    height: pts.size.1 as u32,
                    layers: pts.size.2 as u32,
                };
                let mut out = 0;
                assert_eq!(vk::SUCCESS, unsafe {
                    vk.CreateFramebuffer(dev, &info, ptr::null(), &mut out)
                });
                *ve.insert(out)
            },
        };
        let rp_info = vk::RenderPassBeginInfo {
            sType: vk::STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
            pNext: ptr::null(),
            renderPass: self.last_render_pass,
            framebuffer: fbo,
            renderArea: vk::Rect2D {
                offset: vk::Offset2D {
                    x: 0,
                    y: 0,
                },
                extent: vk::Extent2D {
                    width: pts.size.0 as u32,
                    height: pts.size.1 as u32,
                },
            },
            clearValueCount: 0,
            pClearValues: ptr::null(),
        };
        unsafe {
            vk.CmdSetViewport(self.inner, 0, 1, &vp);
            vk.CmdBeginRenderPass(self.inner, &rp_info, vk::SUBPASS_CONTENTS_INLINE);
        }
        //TODO: EndRenderPass
    }

    fn bind_index(&mut self, _: native::Buffer, _: IndexType) {}
    fn set_scissor(&mut self, _: target::Rect) {}
    fn set_ref_values(&mut self, _: RefValues) {}
    fn update_buffer(&mut self, _: native::Buffer, _: &[u8], _: usize) {}
    fn update_texture(&mut self, _: native::Texture, _: tex::Kind, _: Option<tex::CubeFace>,
                      _: &[u8], _: tex::RawImageInfo) {}
    fn generate_mipmap(&mut self, _: native::TextureView) {}

    fn clear_color(&mut self, tv: native::TextureView, color: draw::ClearColor) {
        let (_, vk) = self.share.get_device();
        let value = match color {
            draw::ClearColor::Float(v) => vk::ClearColorValue::float32(v),
            draw::ClearColor::Int(v)   => vk::ClearColorValue::int32(v),
            draw::ClearColor::Uint(v)  => vk::ClearColorValue::uint32(v),
        };
        unsafe {
            vk.CmdClearColorImage(self.inner, tv.image, tv.layout, &value, 1, &tv.sub_range);
        }
    }

    fn clear_depth_stencil(&mut self, tv: native::TextureView, depth: Option<target::Depth>,
                           stencil: Option<target::Stencil>) {
        let (_, vk) = self.share.get_device();
        let value = vk::ClearDepthStencilValue {
            depth: depth.unwrap_or(1.0), //TODO
            stencil: stencil.unwrap_or(0) as u32, //TODO
        };
        unsafe {
            vk.CmdClearDepthStencilImage(self.inner, tv.image, tv.layout, &value, 1, &tv.sub_range);
        }
    }

    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: VertexCount, _: VertexCount,
                         _: VertexCount, _: draw::InstanceOption) {}
}


pub struct GraphicsQueue {
    share: SharePointer,
    family: u32,
    queue: vk::Queue,
    capabilities: core::Capabilities,
}

impl GraphicsQueue {
    #[doc(hidden)]
    pub fn new(share: SharePointer, q: vk::Queue, qf_id: u32) -> GraphicsQueue {
        let caps = core::Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: false,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
        };
        GraphicsQueue {
            share: share,
            family: qf_id,
            queue: q,
            capabilities: caps,
        }
    }
    #[doc(hidden)]
    pub fn get_share(&self) -> &Share {
        &self.share
    }
    #[doc(hidden)]
    pub fn get_queue(&self) -> vk::Queue {
        self.queue
    }
    #[doc(hidden)]
    pub fn get_family(&self) -> u32 {
        self.family
    }
}

impl core::Device for GraphicsQueue {
    type Resources = Resources;
    type CommandBuffer = Buffer;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.capabilities
    }

    fn pin_submitted_resources(&mut self, _: &core::handle::Manager<Resources>) {}

    fn submit(&mut self, com: &mut Buffer) {
        assert_eq!(self.family, com.family);
        let (_, vk) = self.share.get_device();
        assert_eq!(vk::SUCCESS, unsafe {
            vk.EndCommandBuffer(com.inner)
        });
        let submit_info = vk::SubmitInfo {
            sType: vk::STRUCTURE_TYPE_SUBMIT_INFO,
            commandBufferCount: 1,
            pCommandBuffers: &com.inner,
            .. unsafe { mem::zeroed() }
        };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.QueueSubmit(self.queue, 1, &submit_info, 0)
        });
        let begin_info = vk::CommandBufferBeginInfo {
            sType: vk::STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: ptr::null(),
            flags: 0,
            pInheritanceInfo: ptr::null(),
        };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.BeginCommandBuffer(com.inner, &begin_info)
        });
    }

    //note: this should really live elsewhere (Factory?)
    fn cleanup(&mut self) {
        let (dev, mut functions) = self.share.get_device();
        use gfx_core::handle::Producer;
        //self.frame_handles.clear();
        self.share.handles.borrow_mut().clean_with(&mut functions,
            |vk, b| unsafe { //buffer
                vk.DestroyBuffer(dev, b.buffer, ptr::null());
                vk.FreeMemory(dev, b.memory, ptr::null());
            },
            |vk, s| unsafe { //shader
                vk.DestroyShaderModule(dev, *s, ptr::null());
            },
            |_, _p| (), //program
            |vk, p| unsafe { //PSO
                vk.DestroyPipeline(dev, p.pipeline, ptr::null());
                vk.DestroyPipelineLayout(dev, p.pipe_layout, ptr::null());
                vk.DestroyDescriptorSetLayout(dev, p.desc_layout, ptr::null());
                vk.DestroyDescriptorPool(dev, p.desc_pool, ptr::null());
            },
            |vk, t| if t.memory != 0 {unsafe { //texture
                vk.DestroyImage(dev, t.image, ptr::null());
                vk.FreeMemory(dev, t.memory, ptr::null());
            }},
            |vk, v| unsafe { //SRV
                vk.DestroyImageView(dev, v.view, ptr::null());
            },
            |_, _| (), //UAV
            |vk, v| unsafe { //RTV
                vk.DestroyImageView(dev, v.view, ptr::null());
            },
            |vk, v| unsafe { //DSV
                vk.DestroyImageView(dev, v.view, ptr::null());
            },
            |_, _v| (), //sampler
            |_, _| (), //fence
        );
    }
}
