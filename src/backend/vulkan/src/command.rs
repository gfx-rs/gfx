use std::{cmp, ptr};
use std::sync::Arc;
use smallvec::SmallVec;
use ash::vk;
use ash::version::DeviceV1_0;

use core::{command, memory, pso, target};
use core::{IndexCount, VertexCount, VertexOffset, Viewport};
use core::buffer::IndexBufferView;
use core::command::{
    BufferCopy, BufferImageCopy, ClearColor, ClearValue, ImageCopy, ImageResolve,
    InstanceParams, SubpassContents,
};
use core::image::ImageLayout;
use {conv, native as n};
use {Backend, RawDevice};

#[derive(Clone)]
pub struct CommandBuffer {
    pub raw: vk::CommandBuffer,
    pub device: Arc<RawDevice>,
}

fn map_subpass_contents(contents: SubpassContents) -> vk::SubpassContents {
    match contents {
        SubpassContents::Inline => vk::SubpassContents::Inline,
        SubpassContents::SecondaryBuffers => vk::SubpassContents::SecondaryCommandBuffers,
    }
}

fn map_buffer_image_regions(
    image: &n::Image,
    regions: &[BufferImageCopy],
) -> SmallVec<[vk::BufferImageCopy; 16]> {
    fn div(a: u32, b: u32) -> u32 {
        assert_eq!(a % b, 0);
        a / b
    };
    regions
        .iter()
        .map(|region| {
            let aspect_mask = conv::map_image_aspects(region.image_aspect);
            let row_length = div(region.buffer_row_pitch, image.bytes_per_texel as u32);
            let image_subresource = conv::map_subresource_layers(aspect_mask, &region.image_subresource);
            vk::BufferImageCopy {
                buffer_offset: region.buffer_offset,
                buffer_row_length: row_length,
                buffer_image_height: div(region.buffer_slice_pitch, row_length),
                image_subresource,
                image_offset: vk::Offset3D {
                    x: region.image_offset.x,
                    y: region.image_offset.y,
                    z: region.image_offset.z,
                },
                image_extent: image.extent.clone(),
            }
        })
        .collect()
}

impl command::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self) {
        let info = vk::CommandBufferBeginInfo {
            s_type: vk::StructureType::CommandBufferBeginInfo,
            p_next: ptr::null(),
            flags: vk::COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            p_inheritance_info: ptr::null(),
        };

        assert_eq!(Ok(()),
            unsafe { self.device.0.begin_command_buffer(self.raw, &info) }
        );
    }

    fn finish(&mut self) {
        assert_eq!(Ok(()), unsafe {
            self.device.0.end_command_buffer(self.raw)
        });
    }

    fn reset(&mut self, release_resources: bool) {
        let flags = if release_resources {
            vk::COMMAND_BUFFER_RESET_RELEASE_RESOURCES_BIT
        } else {
            vk::CommandBufferResetFlags ::empty()
        };

        assert_eq!(Ok(()),
            unsafe { self.device.0.reset_command_buffer(self.raw, flags) }
        );
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        frame_buffer: &n::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    ) {
        let render_area = vk::Rect2D {
            offset: vk::Offset2D {
                x: render_area.x as i32,
                y: render_area.y as i32,
            },
            extent: vk::Extent2D {
                width: render_area.w as u32,
                height: render_area.h as u32,
            },
        };

        let clear_values: SmallVec<[vk::ClearValue; 16]> =
            clear_values.iter().map(conv::map_clear_value).collect();

        let info = vk::RenderPassBeginInfo {
            s_type: vk::StructureType::RenderPassBeginInfo,
            p_next: ptr::null(),
            render_pass: render_pass.raw,
            framebuffer: frame_buffer.raw,
            render_area,
            clear_value_count: clear_values.len() as u32,
            p_clear_values: clear_values.as_ptr(),
        };

        let contents = map_subpass_contents(first_subpass);
        unsafe {
            self.device.0.cmd_begin_render_pass(
                self.raw,
                &info,
                contents,
            );
        }
    }

    fn next_subpass(&mut self, contents: SubpassContents) {
        let contents = map_subpass_contents(contents);
        unsafe {
            self.device.0.cmd_next_subpass(self.raw, contents);
        }
    }

    fn end_renderpass(&mut self) {
        unsafe {
            self.device.0.cmd_end_render_pass(self.raw);
        }
    }

    fn pipeline_barrier(
        &mut self,
        src_stages: pso::PipelineStage,
        dst_stages: pso::PipelineStage,
        barriers: &[memory::Barrier<Backend>],
    ) {
        let mut memory_bars: SmallVec<[vk::MemoryBarrier; 4]> = SmallVec::new();
        let mut buffer_bars: SmallVec<[vk::BufferMemoryBarrier; 4]> = SmallVec::new();
        let mut image_bars: SmallVec<[vk::ImageMemoryBarrier; 4]> = SmallVec::new();

        for barrier in barriers {
            match *barrier {
                memory::Barrier::AllBuffers { access_src, access_dst } => {
                    memory_bars.push(vk::MemoryBarrier {
                        s_type: vk::StructureType::MemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_buffer_access(access_src),
                        dst_access_mask: conv::map_buffer_access(access_dst),
                    });
                }
                memory::Barrier::AllImages { access_src, access_dst } => {
                    memory_bars.push(vk::MemoryBarrier {
                        s_type: vk::StructureType::MemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_image_access(access_src),
                        dst_access_mask: conv::map_image_access(access_dst),
                    });
                }
                memory::Barrier::Buffer { state_src, state_dst, target, ref range } => {
                    buffer_bars.push(vk::BufferMemoryBarrier {
                        s_type: vk::StructureType::BufferMemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_buffer_access(state_src),
                        dst_access_mask: conv::map_buffer_access(state_dst),
                        src_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        dst_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        buffer: target.raw,
                        offset: range.start,
                        size: range.end - range.start,
                    });
                }
                memory::Barrier::Image { state_src, state_dst, target, ref range } => {
                    let subresource_range = conv::map_subresource_range(vk::IMAGE_ASPECT_COLOR_BIT, range);
                    image_bars.push(vk::ImageMemoryBarrier {
                        s_type: vk::StructureType::ImageMemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_image_access(state_src.0),
                        dst_access_mask: conv::map_image_access(state_dst.0),
                        old_layout: conv::map_image_layout(state_src.1),
                        new_layout: conv::map_image_layout(state_dst.1),
                        src_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        dst_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        image: target.raw,
                        subresource_range,
                    });
                }
            }
        }

        unsafe {
            self.device.0.cmd_pipeline_barrier(
                self.raw, // commandBuffer
                conv::map_pipeline_stage(src_stages),
                conv::map_pipeline_stage(dst_stages),
                vk::DependencyFlags::empty(), // dependencyFlags // TODO
                &memory_bars,
                &buffer_bars,
                &image_bars,
            );
        }
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        layout: ImageLayout,
        color: ClearColor,
    ) {
        let clear_value = conv::map_clear_color(color);

        let range = {
            let (ref mip_levels, ref array_layers) = rtv.range;
            vk::ImageSubresourceRange {
                aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
                base_mip_level: mip_levels.start as u32,
                level_count: (mip_levels.end - mip_levels.start) as u32,
                base_array_layer: array_layers.start as u32,
                layer_count: (array_layers.end - array_layers.start) as u32,
            }
        };

        unsafe {
            self.device.0.cmd_clear_color_image(
                self.raw,
                rtv.image,
                conv::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn clear_depth_stencil(
        &mut self,
        dsv: &n::DepthStencilView,
        layout: ImageLayout,
        depth: Option<target::Depth>,
        stencil: Option<target::Stencil>,
    ) {
        let clear_value = vk::ClearDepthStencilValue {
            depth: depth.unwrap_or(0.0),
            stencil: stencil.unwrap_or(0) as u32,
        };

        let range = {
            let (ref mip_levels, ref array_layers) = dsv.range;
            let mut aspect_mask = vk::ImageAspectFlags::empty();
            if depth.is_some() {
                aspect_mask |= vk::IMAGE_ASPECT_DEPTH_BIT;
            }
            if stencil.is_some() {
                aspect_mask |= vk::IMAGE_ASPECT_STENCIL_BIT;
            }

            vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: mip_levels.start as u32,
                level_count: (mip_levels.end - mip_levels.start) as u32,
                base_array_layer: array_layers.start as u32,
                layer_count: (array_layers.end - array_layers.start) as u32,
            }
        };

        unsafe {
            self.device.0.cmd_clear_depth_stencil_image(
                self.raw,
                dsv.image,
                conv::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        src_layout: ImageLayout,
        dst: &n::Image,
        dst_layout: ImageLayout,
        regions: &[ImageResolve],
    ) {
        let regions: SmallVec<[vk::ImageResolve; 16]> = regions
            .iter()
            .map(|region| {
                let offset = vk::Offset3D {
                    x: 0,
                    y: 0,
                    z: 0,
                };

                vk::ImageResolve {
                    src_subresource: conv::map_subresource_with_layers(
                        vk::IMAGE_ASPECT_COLOR_BIT, // Specs [1.0.42] 18.6
                        region.src_subresource,
                        region.num_layers,
                    ),
                    src_offset: offset.clone(),
                    dst_subresource: conv::map_subresource_with_layers(
                        vk::IMAGE_ASPECT_COLOR_BIT, // Specs [1.0.42] 18.6
                        region.dst_subresource,
                        region.num_layers,
                    ),
                    dst_offset: offset,
                    extent: vk::Extent3D {
                        width:  cmp::max(1, src.extent.width  >> region.src_subresource.0),
                        height: cmp::max(1, src.extent.height >> region.src_subresource.0),
                        depth:  cmp::max(1, src.extent.depth  >> region.src_subresource.0),
                    },
                }
            })
            .collect();
        unsafe {
            self.device.0.cmd_resolve_image(
                self.raw,
                src.raw,
                conv::map_image_layout(src_layout),
                dst.raw,
                conv::map_image_layout(dst_layout),
                &regions,
            );
        }
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        unsafe {
            self.device.0.cmd_bind_index_buffer(
                self.raw,
                ibv.buffer.raw,
                ibv.offset,
                conv::map_index_type(ibv.index_type),
            );
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Backend>) {
        let buffers: SmallVec<[vk::Buffer; 16]> =
            vbs.0.iter().map(|&(ref buffer, _)| buffer.raw).collect();
        let offsets: SmallVec<[vk::DeviceSize; 16]> =
            vbs.0.iter().map(|&(_, offset)| offset as u64).collect();

        unsafe {
            self.device.0.cmd_bind_vertex_buffers(
                self.raw,
                0,
                &buffers,
                &offsets,
            );
        }
    }

    fn set_viewports(&mut self, viewports: &[Viewport]) {
        let viewports: SmallVec<[vk::Viewport; 16]> = viewports
            .iter()
            .map(|viewport| {
                vk::Viewport {
                    x: viewport.x as f32,
                    y: viewport.y as f32,
                    width: viewport.w as f32,
                    height: viewport.h as f32,
                    min_depth: viewport.near,
                    max_depth: viewport.far,
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_set_viewport(self.raw, &viewports);
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        let scissors: SmallVec<[vk::Rect2D; 16]> = scissors
            .iter()
            .map(|scissor| {
                vk::Rect2D {
                    offset: vk::Offset2D {
                        x: scissor.x as i32,
                        y: scissor.y as i32,
                    },
                    extent: vk::Extent2D {
                        width: scissor.w as u32,
                        height: scissor.h as u32,
                    },
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_set_scissor(self.raw, &scissors);
        }
    }

    fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil) {
        unsafe {
            if front == back {
                // set front _and_ back
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FRONT_AND_BACK,
                    front as u32,
                );
            } else {
                // set both individually
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FACE_FRONT_BIT,
                    front as u32,
                );
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FACE_BACK_BIT,
                    back as u32,
                );
            }
        }
    }

    fn set_blend_constants(&mut self, color: target::ColorValue) {
        unsafe {
            self.device.0.cmd_set_blend_constants(self.raw, color);
        }
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        unsafe {
            self.device.0.cmd_bind_pipeline(
                self.raw,
                vk::PipelineBindPoint::Graphics,
                pipeline.0,
            )
        }
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        let sets: SmallVec<[vk::DescriptorSet; 16]> = sets.iter().map(|set| set.raw).collect();
        let dynamic_offsets = &[]; // TODO

        unsafe {
            self.device.0.cmd_bind_descriptor_sets(
                self.raw,
                vk::PipelineBindPoint::Graphics,
                layout.raw,
                first_set as u32,
                &sets,
                dynamic_offsets,
            );
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unsafe {
            self.device.0.cmd_bind_pipeline(
                self.raw,
                vk::PipelineBindPoint::Compute,
                pipeline.0,
            )
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.device.0.cmd_dispatch(
                self.raw,
                x,
                y,
                z,
            )
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        unsafe {
            self.device.0.cmd_dispatch_indirect(
                self.raw,
                buffer.raw,
                offset,
            )
        }
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        let regions: SmallVec<[vk::BufferCopy; 16]> = regions
            .iter()
            .map(|region| {
                vk::BufferCopy {
                    src_offset: region.src,
                    dst_offset: region.dst,
                    size: region.size,
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_copy_buffer(
                self.raw,
                src.raw,
                dst.raw,
                &regions,
            )
        }
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        src_layout: ImageLayout,
        dst: &n::Image,
        dst_layout: ImageLayout,
        regions: &[ImageCopy],
    ) {
        let regions: SmallVec<[vk::ImageCopy; 16]> = regions
            .iter()
            .map(|region| {
                let aspect_mask = conv::map_image_aspects(region.aspect_mask);
                vk::ImageCopy {
                    src_subresource: conv::map_subresource_with_layers(aspect_mask, region.src_subresource, region.num_layers),
                    src_offset: conv::map_offset(region.src_offset),
                    dst_subresource: conv::map_subresource_with_layers(aspect_mask, region.dst_subresource, region.num_layers),
                    dst_offset: conv::map_offset(region.dst_offset),
                    extent: conv::map_extent(region.extent),
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_copy_image(
                self.raw,
                src.raw,
                conv::map_image_layout(src_layout),
                dst.raw,
                conv::map_image_layout(dst_layout),
                &regions,
            );
        }
    }

    fn copy_buffer_to_image(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        dst_layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        let regions = map_buffer_image_regions(dst, regions);

        unsafe {
            self.device.0.cmd_copy_buffer_to_image(
                self.raw,
                src.raw,
                dst.raw,
                conv::map_image_layout(dst_layout),
                &regions,
            );
        }
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &n::Image,
        src_layout: ImageLayout,
        dst: &n::Buffer,
        regions: &[BufferImageCopy],
    ) {
        let regions = map_buffer_image_regions(src, regions);

        unsafe {
            self.device.0.cmd_copy_image_to_buffer(
                self.raw,
                src.raw,
                conv::map_image_layout(src_layout),
                dst.raw,
                &regions,
            );
        }
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.device.0.cmd_draw(
                self.raw,
                count,
                num_instances,
                start,
                start_instance,
            )
        }
    }

    fn draw_indexed(
        &mut self,
        start: IndexCount,
        count: IndexCount,
        base: VertexOffset,
        instances: Option<InstanceParams>,
    ) {
        let (num_instances, start_instance) = instances.unwrap_or((1, 0));

        unsafe {
            self.device.0.cmd_draw_indexed(
                self.raw,
                count,
                num_instances,
                start,
                base,
                start_instance,
            )
        }
    }

    fn draw_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unsafe {
            self.device.0.cmd_draw_indirect(
                self.raw,
                buffer.raw,
                offset,
                draw_count,
                stride,
            )
        }
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &n::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unsafe {
            self.device.0.cmd_draw_indexed_indirect(
                self.raw,
                buffer.raw,
                offset,
                draw_count,
                stride,
            )
        }
    }
}

pub struct SubpassCommandBuffer(pub CommandBuffer);
