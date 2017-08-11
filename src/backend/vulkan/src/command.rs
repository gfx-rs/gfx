use ash::vk;
use ash::version::DeviceV1_0;
use core::{command, memory, pso, shade, state, target, texture};
use core::{IndexType, VertexCount, VertexOffset};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearColor, ClearValue, ImageCopy, ImageResolve,
                    InstanceParams, SubpassContents};
use {data, native as n, Backend, RawDevice};
use std::ptr;
use std::sync::Arc;
use smallvec::SmallVec;

#[derive(Clone)]
pub struct SubmitInfo {
    pub command_buffer: vk::CommandBuffer,
}

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

impl command::RawCommandBuffer<Backend> for CommandBuffer {
    fn finish(&mut self) -> SubmitInfo {
        unsafe {
            self.device.0.end_command_buffer(self.raw); // TODO: error handling
        }

        SubmitInfo {
            command_buffer: self.raw,
        }
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
            clear_values.iter().map(data::map_clear_value).collect();

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
                self.raw, // commandBuffer
                &info,    // pRenderPassBegin
                contents, // contents
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

    fn pipeline_barrier(&mut self, _barriers: &[memory::Barrier]) {
        unimplemented!()
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        layout: texture::ImageLayout,
        color: ClearColor,
    ) {
        let clear_value = data::map_clear_color(color);

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
                data::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn clear_depth_stencil(
        &mut self,
        dsv: &n::DepthStencilView,
        layout: texture::ImageLayout,
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
                data::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        src_layout: texture::ImageLayout,
        dst: &n::Image,
        dst_layout: texture::ImageLayout,
        regions: &[ImageResolve],
    ) {
        let regions: SmallVec<[vk::ImageResolve; 16]> = regions
            .iter()
            .map(|region| {
                vk::ImageResolve {
                    src_subresource: data::map_subresource_layers(
                        vk::IMAGE_ASPECT_COLOR_BIT, // Specs [1.0.42] 18.6
                        &region.src_subresource,
                    ),
                    src_offset: data::map_offset(region.src_offset),
                    dst_subresource: data::map_subresource_layers(
                        vk::IMAGE_ASPECT_COLOR_BIT, // Specs [1.0.42] 18.6
                        &region.dst_subresource,
                    ),
                    dst_offset: data::map_offset(region.dst_offset),
                    extent: data::map_extent(region.extent),
                }
            })
            .collect();
        unsafe {
            self.device.0.cmd_resolve_image(
                self.raw,                           // commandBuffer
                src.raw,                            // srcImage
                data::map_image_layout(src_layout), // srcImageLayout
                dst.raw,                            // dstImage
                data::map_image_layout(dst_layout), // dstImageLayout
                &regions,                           // pRegions
            );
        }
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        unsafe {
            self.device.0.cmd_bind_index_buffer(
                self.raw,                             // commandBuffer
                ibv.buffer.raw,                       // buffer
                ibv.offset,                           // offset
                data::map_index_type(ibv.index_type), // indexType
            );
        }
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Backend>) {
        let buffers: SmallVec<[vk::Buffer; 16]> =
            vbs.0.iter().map(|&(ref buffer, _)| buffer.raw).collect();
        let offsets: SmallVec<[vk::DeviceSize; 16]> =
            vbs.0.iter().map(|&(_, offset)| offset as u64).collect();

        unsafe {
            self.device
                .0
                .cmd_bind_vertex_buffers(self.raw, 0, &buffers, &offsets);
        }
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        let viewports: SmallVec<[vk::Viewport; 16]> = viewports
            .iter()
            .map(|viewport| {
                vk::Viewport {
                    x: viewport.x as f32,
                    y: viewport.y as f32,
                    width: viewport.w as f32,
                    height: viewport.h as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
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

    fn set_ref_values(&mut self, rv: state::RefValues) {
        unsafe {
            self.device.0.cmd_set_blend_constants(self.raw, rv.blend);

            if rv.stencil.0 == rv.stencil.1 {
                // set front _and_ back
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FRONT_AND_BACK,
                    rv.stencil.0 as u32,
                );
            } else {
                // set both individually
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FACE_FRONT_BIT,
                    rv.stencil.0 as u32,
                );
                self.device.0.cmd_set_stencil_reference(
                    self.raw,
                    vk::STENCIL_FACE_BACK_BIT,
                    rv.stencil.1 as u32,
                );
            }
        }
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        unsafe {
            self.device
                .0
                .cmd_bind_pipeline(self.raw, vk::PipelineBindPoint::Graphics, pipeline.0)
        }
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        let sets: SmallVec<[vk::DescriptorSet; 16]> = sets.iter().map(|set| set.raw).collect();

        unsafe {
            self.device.0.cmd_bind_descriptor_sets(
                self.raw,                        // commandBuffer
                vk::PipelineBindPoint::Graphics, // pipelineBindPoint
                layout.raw,                      // layout
                first_set as u32,                // firstSet
                &sets,                           // pDescriptorSets
                &[],                             // pDynamicOffsets // TODO
            );
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unsafe {
            self.device
                .0
                .cmd_bind_pipeline(self.raw, vk::PipelineBindPoint::Compute, pipeline.0)
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.device.0.cmd_dispatch(
                self.raw, // commandBuffer
                x,        // groupCountX
                y,        // groupCountY
                z,        // groupCountZ
            )
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        unsafe {
            self.device
                .0
                .cmd_dispatch_indirect(self.raw, buffer.raw, offset)
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
            self.device
                .0
                .cmd_copy_buffer(self.raw, src.raw, dst.raw, &regions)
        }
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        src_layout: texture::ImageLayout,
        dst: &n::Image,
        dst_layout: texture::ImageLayout,
        regions: &[ImageCopy],
    ) {
        let regions: SmallVec<[vk::ImageCopy; 16]> = regions
            .iter()
            .map(|region| {
                let aspect_mask = data::map_image_aspects(region.aspect_mask);
                vk::ImageCopy {
                    src_subresource: data::map_subresource_layers(aspect_mask, &region.src_subresource),
                    src_offset: data::map_offset(region.src_offset),
                    dst_subresource: data::map_subresource_layers(aspect_mask, &region.dst_subresource),
                    dst_offset: data::map_offset(region.dst_offset),
                    extent: data::map_extent(region.extent),
                }
            })
            .collect();
    }

    fn copy_buffer_to_image(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        fn div(a: u32, b: u32) -> u32 {
            assert_eq!(a % b, 0);
            a / b
        };
        let regions: SmallVec<[vk::BufferImageCopy; 16]> = regions
            .iter()
            .map(|region| {
                let aspect_mask = data::map_image_aspects(region.image_aspect);
                let subresource_layers =
                    data::map_subresource_layers(aspect_mask, &region.image_subresource);
                let row_length = div(region.buffer_row_pitch, dst.bytes_per_texel as u32);
                vk::BufferImageCopy {
                    buffer_offset: region.buffer_offset,
                    buffer_row_length: row_length,
                    buffer_image_height: div(region.buffer_slice_pitch, row_length),
                    image_subresource: subresource_layers,
                    image_offset: vk::Offset3D {
                        x: region.image_offset.x,
                        y: region.image_offset.y,
                        z: region.image_offset.z,
                    },
                    image_extent: dst.extent.clone(),
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_copy_buffer_to_image(
                self.raw,                       // commandBuffer
                src.raw,                        // srcBuffer
                dst.raw,                        // dstImage
                data::map_image_layout(layout), // dstImageLayout
                &regions,                       // pRegions
            );
        }
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &n::Image,
        dst: &n::Buffer,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<InstanceParams>) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.device.0.cmd_draw(
                self.raw,       // commandBuffer
                count,          // vertexCount
                num_instances,  // instanceCount
                start,          // firstVertex
                start_instance, // firstInstance
            )
        }
    }

    fn draw_indexed(
        &mut self,
        start: VertexCount,
        count: VertexCount,
        base: VertexOffset,
        instances: Option<InstanceParams>,
    ) {
        let (num_instances, start_instance) = match instances {
            Some((num_instances, start_instance)) => (num_instances, start_instance),
            None => (1, 0),
        };

        unsafe {
            self.device.0.cmd_draw_indexed(
                self.raw,       // commandBuffer
                count,          // indexCount
                num_instances,  // instanceCount
                start,          // firstIndex
                base,           // vertexOffset
                start_instance, // firstInstance
            )
        }
    }

    fn draw_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unsafe {
            self.device.0.cmd_draw_indirect(
                self.raw,   // commandBuffer
                buffer.raw, // buffer
                offset,     // offset
                draw_count, // drawCount
                stride,     // stride
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
                self.raw,   // commandBuffer
                buffer.raw, // buffer
                offset,     // offset
                draw_count, // drawCount
                stride,     // stride
            )
        }
    }
}

pub struct SubpassCommandBuffer(pub CommandBuffer);
