use std::borrow::Borrow;
use std::{mem, ptr};
use std::ops::Range;
use std::sync::Arc;
use smallvec::SmallVec;
use ash::vk;
use ash::version::DeviceV1_0;

use hal::{buffer, command as com, memory, pso, query};
use hal::{DrawCount, IndexCount, InstanceCount, VertexCount, VertexOffset, WorkGroupCount};
use hal::format::Aspects;
use hal::image::{Filter, Layout, SubresourceRange};
use hal::range::RangeArg;
use {conv, native as n};
use {Backend, RawDevice};

#[derive(Clone)]
pub struct CommandBuffer {
    pub raw: vk::CommandBuffer,
    pub device: Arc<RawDevice>,
}

fn map_subpass_contents(contents: com::SubpassContents) -> vk::SubpassContents {
    match contents {
        com::SubpassContents::Inline => vk::SubpassContents::Inline,
        com::SubpassContents::SecondaryBuffers => vk::SubpassContents::SecondaryCommandBuffers,
    }
}

fn map_buffer_image_regions<T>(
    _image: &n::Image,
    regions: T,
) -> SmallVec<[vk::BufferImageCopy; 16]>
where
    T: IntoIterator,
    T::Item: Borrow<com::BufferImageCopy>,
{
    regions
        .into_iter()
        .map(|region| {
            let r = region.borrow();
            let image_subresource = conv::map_subresource_layers(&r.image_layers);
            vk::BufferImageCopy {
                buffer_offset: r.buffer_offset,
                buffer_row_length: r.buffer_width,
                buffer_image_height: r.buffer_height,
                image_subresource,
                image_offset: conv::map_offset(r.image_offset),
                image_extent: conv::map_extent(r.image_extent),
            }
        })
        .collect()
}

impl CommandBuffer {
    fn bind_descriptor_sets<I, J>(
        &mut self,
        bind_point: vk::PipelineBindPoint,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<n::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        let sets: SmallVec<[_; 16]> = sets.into_iter().map(|set| set.borrow().raw).collect();
        let dynamic_offsets: SmallVec<[_; 16]> = offsets.into_iter().map(|offset| *offset.borrow()).collect();

        unsafe {
            self.device.0.cmd_bind_descriptor_sets(
                self.raw,
                bind_point,
                layout.raw,
                first_set as u32,
                &sets,
                &dynamic_offsets,
            );
        }
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags, info: com::CommandBufferInheritanceInfo<Backend>) {
        let inheritance_info = vk::CommandBufferInheritanceInfo {
            s_type: vk::StructureType::CommandBufferInheritanceInfo,
            p_next: ptr::null(),
            render_pass: info.subpass.map_or(vk::types::RenderPass::null(), |subpass| subpass.main_pass.raw),
            subpass: info.subpass.map_or(0, |subpass| subpass.index as u32),
            framebuffer: info.framebuffer.map_or(vk::types::Framebuffer::null(), |buffer| buffer.raw),
            occlusion_query_enable: if info.occlusion_query_enable { vk::VK_TRUE } else { vk::VK_FALSE },
            query_flags: conv::map_query_control_flags(info.occlusion_query_flags),
            pipeline_statistics: conv::map_pipeline_statistics(info.pipeline_statistics),
        };

        let info = vk::CommandBufferBeginInfo {
            s_type: vk::StructureType::CommandBufferBeginInfo,
            p_next: ptr::null(),
            flags: conv::map_command_buffer_flags(flags),
            p_inheritance_info: &inheritance_info,
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
            vk::CommandBufferResetFlags::empty()
        };

        assert_eq!(Ok(()),
            unsafe { self.device.0.reset_command_buffer(self.raw, flags) }
        );
    }

    fn begin_render_pass<T>(
        &mut self,
        render_pass: &n::RenderPass,
        frame_buffer: &n::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
        first_subpass: com::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ClearValueRaw>,
    {
        let render_area = conv::map_rect(&render_area);

        let clear_values: SmallVec<[vk::ClearValue; 16]> =
            clear_values
                .into_iter()
                .map(|clear| unsafe {
                    // Vulkan and HAL share same memory layout
                    mem::transmute(*clear.borrow())
                })
                .collect();

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

    fn next_subpass(&mut self, contents: com::SubpassContents) {
        let contents = map_subpass_contents(contents);
        unsafe {
            self.device.0.cmd_next_subpass(self.raw, contents);
        }
    }

    fn end_render_pass(&mut self) {
        unsafe {
            self.device.0.cmd_end_render_pass(self.raw);
        }
    }

    fn pipeline_barrier<'a, T>(
        &mut self,
        stages: Range<pso::PipelineStage>,
        dependencies: memory::Dependencies,
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        let mut global_bars: SmallVec<[vk::MemoryBarrier; 4]> = SmallVec::new();
        let mut buffer_bars: SmallVec<[vk::BufferMemoryBarrier; 4]> = SmallVec::new();
        let mut image_bars: SmallVec<[vk::ImageMemoryBarrier; 4]> = SmallVec::new();

        for barrier in barriers {
            match *barrier.borrow() {
                memory::Barrier::AllBuffers(ref access) => {
                    global_bars.push(vk::MemoryBarrier {
                        s_type: vk::StructureType::MemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_buffer_access(access.start),
                        dst_access_mask: conv::map_buffer_access(access.end),
                    });
                }
                memory::Barrier::AllImages(ref access) => {
                    global_bars.push(vk::MemoryBarrier {
                        s_type: vk::StructureType::MemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_image_access(access.start),
                        dst_access_mask: conv::map_image_access(access.end),
                    });
                }
                memory::Barrier::Buffer { ref states, target} => {
                    buffer_bars.push(vk::BufferMemoryBarrier {
                        s_type: vk::StructureType::BufferMemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_buffer_access(states.start),
                        dst_access_mask: conv::map_buffer_access(states.end),
                        src_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        dst_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED, // TODO
                        buffer: target.raw,
                        offset: 0,
                        size: vk::VK_WHOLE_SIZE,
                    });
                }
                memory::Barrier::Image { ref states, target, ref range } => {
                    let subresource_range = conv::map_subresource_range(range);
                    image_bars.push(vk::ImageMemoryBarrier {
                        s_type: vk::StructureType::ImageMemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: conv::map_image_access(states.start.0),
                        dst_access_mask: conv::map_image_access(states.end.0),
                        old_layout: conv::map_image_layout(states.start.1),
                        new_layout: conv::map_image_layout(states.end.1),
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
                conv::map_pipeline_stage(stages.start),
                conv::map_pipeline_stage(stages.end),
                mem::transmute(dependencies),
                &global_bars,
                &buffer_bars,
                &image_bars,
            );
        }
    }

    fn fill_buffer<R>(
        &mut self,
        buffer: &n::Buffer,
        range: R,
        data: u32,
    ) where
        R: RangeArg<buffer::Offset>,
    {
        let (offset, size) = conv::map_range_arg(&range);
        unsafe {
            self.device.0.cmd_fill_buffer(
                self.raw,
                buffer.raw,
                offset,
                size,
                data,
            );
        }
    }

    fn update_buffer(
        &mut self,
        buffer: &n::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        unsafe {
            self.device.0.cmd_update_buffer(
                self.raw,
                buffer.raw,
                offset,
                data,
            );
        }
    }

    fn clear_image<T>(
        &mut self,
        image: &n::Image,
        layout: Layout,
        color: com::ClearColorRaw,
        depth_stencil: com::ClearDepthStencilRaw,
        subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<SubresourceRange>,
    {
        let mut color_ranges = Vec::new();
        let mut ds_ranges = Vec::new();

        for subresource_range in subresource_ranges {
            let sub = subresource_range.borrow();
            let aspect_ds = sub.aspects & (Aspects::DEPTH | Aspects::STENCIL);
            let vk_range = conv::map_subresource_range(sub);
            if sub.aspects.contains(Aspects::COLOR) {
                color_ranges.push(vk::ImageSubresourceRange {
                    aspect_mask: conv::map_image_aspects(Aspects::COLOR),
                    .. vk_range
                });
            }
            if !aspect_ds.is_empty() {
                ds_ranges.push(vk::ImageSubresourceRange {
                    aspect_mask: conv::map_image_aspects(aspect_ds),
                    .. vk_range
                });
            }
        }

        // Vulkan and HAL share same memory layout
        let color_value = unsafe { mem::transmute(color) };
        let depth_stencil_value = vk::ClearDepthStencilValue {
            depth: depth_stencil.depth,
            stencil: depth_stencil.stencil,
        };

        unsafe {
            if !color_ranges.is_empty() {
                self.device.0.cmd_clear_color_image(
                    self.raw,
                    image.raw,
                    conv::map_image_layout(layout),
                    &color_value,
                    &color_ranges,
                )
            }
            if !ds_ranges.is_empty() {
                self.device.0.cmd_clear_depth_stencil_image(
                    self.raw,
                    image.raw,
                    conv::map_image_layout(layout),
                    &depth_stencil_value,
                    &ds_ranges,
                )
            }
        };
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<com::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        let clears: SmallVec<[vk::ClearAttachment; 16]> = clears
            .into_iter()
            .map(|clear| {
                match *clear.borrow() {
                    com::AttachmentClear::Color { index, value } => {
                        vk::ClearAttachment {
                            aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
                            color_attachment: index as _,
                            clear_value: vk::ClearValue { color: conv::map_clear_color(value) },
                        }
                    }
                    com::AttachmentClear::DepthStencil { depth, stencil } => {
                        vk::ClearAttachment {
                            aspect_mask: if depth.is_some() { vk::IMAGE_ASPECT_DEPTH_BIT } else { vk::ImageAspectFlags::empty() } |
                                if stencil.is_some() { vk::IMAGE_ASPECT_STENCIL_BIT } else { vk::ImageAspectFlags::empty() },
                            color_attachment: 0,
                            clear_value: vk::ClearValue {
                                depth: vk::ClearDepthStencilValue {
                                    depth: depth.unwrap_or_default(),
                                    stencil: stencil.unwrap_or_default(),
                                },
                            },
                        }
                    }
                }
            })
            .collect();

        let rects: SmallVec<[vk::ClearRect; 16]> = rects
            .into_iter()
            .map(|rect| {
                conv::map_clear_rect(rect.borrow())
            })
            .collect();

        unsafe { self.device.0.cmd_clear_attachments(self.raw, &clears, &rects) };
    }

    fn resolve_image<T>(
        &mut self,
        src: &n::Image,
        src_layout: Layout,
        dst: &n::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageResolve>,
    {
        let regions = regions
            .into_iter()
            .map(|region| {
                let r = region.borrow();
                vk::ImageResolve {
                    src_subresource: conv::map_subresource_layers(&r.src_subresource),
                    src_offset: conv::map_offset(r.src_offset),
                    dst_subresource: conv::map_subresource_layers(&r.dst_subresource),
                    dst_offset: conv::map_offset(r.dst_offset),
                    extent: conv::map_extent(r.extent),
                }
            })
            .collect::<SmallVec<[_; 4]>>();

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

    fn blit_image<T>(
        &mut self,
        src: &n::Image,
        src_layout: Layout,
        dst: &n::Image,
        dst_layout: Layout,
        filter: Filter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageBlit>
    {
        let regions = regions
            .into_iter()
            .map(|region| {
                let r = region.borrow();
                vk::ImageBlit {
                    src_subresource: conv::map_subresource_layers(&r.src_subresource),
                    src_offsets: [conv::map_offset(r.src_bounds.start), conv::map_offset(r.src_bounds.end)],
                    dst_subresource: conv::map_subresource_layers(&r.dst_subresource),
                    dst_offsets: [conv::map_offset(r.dst_bounds.start), conv::map_offset(r.dst_bounds.end)],
                }
            })
            .collect::<SmallVec<[_; 4]>>();

        unsafe {
            self.device.0.cmd_blit_image(
                self.raw,
                src.raw,
                conv::map_image_layout(src_layout),
                dst.raw,
                conv::map_image_layout(dst_layout),
                &regions,
                conv::map_filter(filter),
            );
        }
    }

    fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<Backend>) {
        unsafe {
            self.device.0.cmd_bind_index_buffer(
                self.raw,
                ibv.buffer.raw,
                ibv.offset,
                conv::map_index_type(ibv.index_type),
            );
        }
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<Backend>) {
        let buffers: SmallVec<[vk::Buffer; 16]> =
            vbs.0.iter().map(|&(ref buffer, _)| buffer.raw).collect();
        let offsets: SmallVec<[vk::DeviceSize; 16]> =
            vbs.0.iter().map(|&(_, offset)| offset as u64).collect();

        unsafe {
            self.device.0.cmd_bind_vertex_buffers(
                self.raw,
                first_binding,
                &buffers,
                &offsets,
            );
        }
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        let viewports: SmallVec<[vk::Viewport; 16]> = viewports
            .into_iter()
            .map(|viewport| {
                conv::map_viewport(viewport.borrow())
            })
            .collect();

        unsafe {
            self.device.0.cmd_set_viewport(self.raw, first_viewport, &viewports);
        }
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        let scissors: SmallVec<[vk::Rect2D; 16]> = scissors
            .into_iter()
            .map(|scissor| {
                conv::map_rect(scissor.borrow())
            })
            .collect();

        unsafe {
            self.device.0.cmd_set_scissor(self.raw, first_scissor, &scissors);
        }
    }

    fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        unsafe {
            // Vulkan and HAL share same faces bit flags
            self.device.0.cmd_set_stencil_reference(self.raw, mem::transmute(faces), value);
        }
    }

    fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        unsafe {
            // Vulkan and HAL share same faces bit flags
            self.device.0.cmd_set_stencil_compare_mask(self.raw, mem::transmute(faces), value);
        }
    }

    fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        unsafe {
            // Vulkan and HAL share same faces bit flags
            self.device.0.cmd_set_stencil_write_mask(self.raw, mem::transmute(faces), value);
        }
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        unsafe {
            self.device.0.cmd_set_blend_constants(self.raw, color);
        }
    }

    fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        unsafe {
            self.device.0.cmd_set_depth_bounds(self.raw, bounds.start, bounds.end);
        }
    }

    fn set_line_width(&mut self, width: f32) {
        unsafe {
            self.device.0.cmd_set_line_width(self.raw, width);
        }
    }

    fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        unsafe {
            self.device.0.cmd_set_depth_bias(
                self.raw,
                depth_bias.const_factor,
                depth_bias.clamp,
                depth_bias.slope_factor,
            );
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

    fn bind_graphics_descriptor_sets<I, J>(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<n::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        self.bind_descriptor_sets(
            vk::PipelineBindPoint::Graphics,
            layout,
            first_set,
            sets,
            offsets,
        );
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

    fn bind_compute_descriptor_sets<I, J>(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<n::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        self.bind_descriptor_sets(
            vk::PipelineBindPoint::Compute,
            layout,
            first_set,
            sets,
            offsets,
        );
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        unsafe {
            self.device.0.cmd_dispatch(
                self.raw,
                count[0],
                count[1],
                count[2],
            )
        }
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: buffer::Offset) {
        unsafe {
            self.device.0.cmd_dispatch_indirect(
                self.raw,
                buffer.raw,
                offset,
            )
        }
    }

    fn copy_buffer<T>(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::BufferCopy>,
    {
        let regions: SmallVec<[vk::BufferCopy; 16]> = regions
            .into_iter()
            .map(|region| {
                let region = region.borrow();
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

    fn copy_image<T>(
        &mut self,
        src: &n::Image,
        src_layout: Layout,
        dst: &n::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageCopy>,
    {
        let regions: SmallVec<[vk::ImageCopy; 16]> = regions
            .into_iter()
            .map(|region| {
                let r = region.borrow();
                vk::ImageCopy {
                    src_subresource: conv::map_subresource_layers(&r.src_subresource),
                    src_offset: conv::map_offset(r.src_offset),
                    dst_subresource: conv::map_subresource_layers(&r.dst_subresource),
                    dst_offset: conv::map_offset(r.dst_offset),
                    extent: conv::map_extent(r.extent),
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

    fn copy_buffer_to_image<T>(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
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

    fn copy_image_to_buffer<T>(
        &mut self,
        src: &n::Image,
        src_layout: Layout,
        dst: &n::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
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

    fn draw(&mut self, vertices: Range<VertexCount>, instances: Range<InstanceCount>) {
        unsafe {
            self.device.0.cmd_draw(
                self.raw,
                vertices.end - vertices.start,
                instances.end - instances.start,
                vertices.start,
                instances.start,
            )
        }
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        unsafe {
            self.device.0.cmd_draw_indexed(
                self.raw,
                indices.end - indices.start,
                instances.end - instances.start,
                indices.start,
                base_vertex,
                instances.start,
            )
        }
    }

    fn draw_indirect(
        &mut self,
        buffer: &n::Buffer,
        offset: buffer::Offset,
        draw_count: DrawCount,
        stride: u32,
    ) {
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
        offset: buffer::Offset,
        draw_count: DrawCount,
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

    fn begin_query(
        &mut self,
        query: query::Query<Backend>,
        control: query::QueryControl,
    ) {
        let mut flags = vk::QueryControlFlags::empty();
        if control.contains(query::QueryControl::PRECISE) {
            flags |= vk::QUERY_CONTROL_PRECISE_BIT;
        }

        unsafe {
            self.device.0.cmd_begin_query(
                self.raw,
                query.pool.0,
                query.id,
                flags
            )
        }
    }

    fn end_query(
        &mut self,
        query: query::Query<Backend>,
    ) {
        unsafe {
            self.device.0.cmd_end_query(
                self.raw,
                query.pool.0,
                query.id,
            )
        }
    }

    fn reset_query_pool(
        &mut self,
        pool: &n::QueryPool,
        queries: Range<query::QueryId>,
    ) {
        unsafe {
            self.device.0.cmd_reset_query_pool(
                self.raw,
                pool.0,
                queries.start,
                queries.end - queries.start,
            )
        }
    }

    fn write_timestamp(
        &mut self,
        stage: pso::PipelineStage,
        query: query::Query<Backend>,
    ) {
        unsafe {
            self.device.0.cmd_write_timestamp(
                self.raw,
                conv::map_pipeline_stage(stage),
                query.pool.0,
                query.id,
            )
        }
    }

    fn push_compute_constants(
        &mut self,
        layout: &n::PipelineLayout,
        offset: u32,
        constants: &[u32],
    ) {
        unsafe {
            self.device.0.cmd_push_constants(
                self.raw,
                layout.raw,
                vk::SHADER_STAGE_COMPUTE_BIT,
                offset * 4,
                memory::cast_slice(constants),
            );
        }
    }

    fn push_graphics_constants(
        &mut self,
        layout: &n::PipelineLayout,
        stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    ) {
        unsafe {
            self.device.0.cmd_push_constants(
                self.raw,
                layout.raw,
                conv::map_stage_flags(stages),
                offset * 4,
                memory::cast_slice(constants),
            );
        }
    }

    fn execute_commands<'a, I>(
        &mut self,
        buffers: I,
    ) where
        I: IntoIterator,
        I::Item: Borrow<CommandBuffer>,
    {
        let command_buffers = buffers.into_iter().map(|b| b.borrow().raw).collect::<Vec<_>>();
        unsafe { self.device.0.cmd_execute_commands(self.raw, &command_buffers); }
    }
}
