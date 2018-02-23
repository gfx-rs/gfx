use std::borrow::Borrow;
use std::{mem, ptr};
use std::ops::Range;
use std::sync::Arc;
use smallvec::SmallVec;
use ash::vk;
use ash::version::DeviceV1_0;

use hal::{command as com, memory, pso, query};
use hal::{IndexCount, InstanceCount, VertexCount, VertexOffset};
use hal::buffer::IndexBufferView;
use hal::format::AspectFlags;
use hal::image::{ImageLayout, SubresourceRange};
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
    fn bind_descriptor_sets<T>(
        &mut self,
        bind_point: vk::PipelineBindPoint,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<n::DescriptorSet>,
    {
        let sets: SmallVec<[vk::DescriptorSet; 16]> = sets.into_iter().map(|set| set.borrow().raw).collect();
        let dynamic_offsets = &[]; // TODO

        unsafe {
            self.device.0.cmd_bind_descriptor_sets(
                self.raw,
                bind_point,
                layout.raw,
                first_set as u32,
                &sets,
                dynamic_offsets,
            );
        }
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags) {
        let info = vk::CommandBufferBeginInfo {
            s_type: vk::StructureType::CommandBufferBeginInfo,
            p_next: ptr::null(),
            flags: conv::map_command_buffer_flags(flags),
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

    fn begin_render_pass_raw<T>(
        &mut self,
        render_pass: &n::RenderPass,
        frame_buffer: &n::Framebuffer,
        render_area: com::Rect,
        clear_values: T,
        first_subpass: com::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ClearValueRaw>,
    {
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
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        let mut buffer_bars: SmallVec<[vk::BufferMemoryBarrier; 4]> = SmallVec::new();
        let mut image_bars: SmallVec<[vk::ImageMemoryBarrier; 4]> = SmallVec::new();

        for barrier in barriers {
            match *barrier.borrow() {
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
                vk::DependencyFlags::empty(), // dependencyFlags // TODO
                &[],
                &buffer_bars,
                &image_bars,
            );
        }
    }

    fn fill_buffer(
        &mut self,
        buffer: &n::Buffer,
        range: Range<u64>,
        data: u32,
    ) {
        unsafe {
            self.device.0.cmd_fill_buffer(
                self.raw,
                buffer.raw,
                range.start,
                range.end - range.start,
                data,
            );
        }
    }

    fn update_buffer(
        &mut self,
        buffer: &n::Buffer,
        offset: u64,
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

    fn clear_color_image_raw(
        &mut self,
        image: &n::Image,
        layout: ImageLayout,
        range: SubresourceRange,
        value: com::ClearColorRaw,
    ) {
        assert!(AspectFlags::COLOR.contains(range.aspects));
        let range = conv::map_subresource_range(&range);
        // Vulkan and HAL share same memory layout
        let clear_value = unsafe { mem::transmute(value) };

        unsafe {
            self.device.0.cmd_clear_color_image(
                self.raw,
                image.raw,
                conv::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn clear_depth_stencil_image_raw(
        &mut self,
        image: &n::Image,
        layout: ImageLayout,
        range: SubresourceRange,
        value: com::ClearDepthStencilRaw,
    ) {
        assert!((AspectFlags::DEPTH | AspectFlags::STENCIL).contains(range.aspects));
        let range = conv::map_subresource_range(&range);
        let clear_value = vk::ClearDepthStencilValue {
            depth: value.depth,
            stencil: value.stencil,
        };

        unsafe {
            self.device.0.cmd_clear_depth_stencil_image(
                self.raw,
                image.raw,
                conv::map_image_layout(layout),
                &clear_value,
                &[range],
            )
        };
    }

    fn clear_attachments<T, U>(&mut self, clears: T, rects: U)
    where
        T: IntoIterator,
        T::Item: Borrow<com::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<com::Rect>,
    {
        let clears: SmallVec<[vk::ClearAttachment; 16]> = clears
            .into_iter()
            .map(|clear| {
                match *clear.borrow() {
                    com::AttachmentClear::Color(index, cv) => {
                        vk::ClearAttachment {
                            aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
                            color_attachment: index as _,
                            clear_value: vk::ClearValue::new_color(conv::map_clear_color(cv)),
                        }
                    }
                    com::AttachmentClear::Depth(v) => {
                        vk::ClearAttachment {
                            aspect_mask: vk::IMAGE_ASPECT_DEPTH_BIT,
                            color_attachment: 0,
                            clear_value: vk::ClearValue::new_depth_stencil(conv::map_clear_depth(v)),
                        }
                    }
                    com::AttachmentClear::Stencil(v) => {
                        vk::ClearAttachment {
                            aspect_mask: vk::IMAGE_ASPECT_STENCIL_BIT,
                            color_attachment: 0,
                            clear_value: vk::ClearValue::new_depth_stencil(conv::map_clear_stencil(v)),
                        }
                    }
                    com::AttachmentClear::DepthStencil(cv) => {
                        vk::ClearAttachment {
                            aspect_mask: vk::IMAGE_ASPECT_DEPTH_BIT | vk::IMAGE_ASPECT_STENCIL_BIT,
                            color_attachment: 0,
                            clear_value: vk::ClearValue::new_depth_stencil(conv::map_clear_depth_stencil(cv)),
                        }
                    }
                }

            })
            .collect();

        let rects: SmallVec<[vk::ClearRect; 16]> = rects
            .into_iter()
            .map(|rect| {
                let rect = rect.borrow();
                vk::ClearRect {
                    base_array_layer: 0,
                    layer_count: vk::VK_REMAINING_ARRAY_LAYERS,
                    rect: vk::Rect2D {
                        offset: vk::Offset2D {
                            x: rect.x as _,
                            y: rect.y as _,
                        },
                        extent: vk::Extent2D {
                            width: rect.w as _,
                            height: rect.h as _,
                        },
                    },
                }
            })
            .collect();

        unsafe { self.device.0.cmd_clear_attachments(self.raw, &clears, &rects) };
    }

    fn resolve_image<T>(
        &mut self,
        src: &n::Image,
        src_layout: ImageLayout,
        dst: &n::Image,
        dst_layout: ImageLayout,
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
        src_layout: ImageLayout,
        dst: &n::Image,
        dst_layout: ImageLayout,
        filter: com::BlitFilter,
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
                // Vulkan and HAL share same filter
                mem::transmute(filter),
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

    fn set_viewports<T>(&mut self, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::Viewport>,
    {
        let viewports: SmallVec<[vk::Viewport; 16]> = viewports
            .into_iter()
            .map(|viewport| {
                let viewport = viewport.borrow();
                vk::Viewport {
                    x: viewport.rect.x as f32,
                    y: viewport.rect.y as f32,
                    width: viewport.rect.w as f32,
                    height: viewport.rect.h as f32,
                    min_depth: viewport.depth.start,
                    max_depth: viewport.depth.end,
                }
            })
            .collect();

        unsafe {
            self.device.0.cmd_set_viewport(self.raw, 0, &viewports);
        }
    }

    fn set_scissors<T>(&mut self, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::Rect>,
    {
        let scissors: SmallVec<[vk::Rect2D; 16]> = scissors
            .into_iter()
            .map(|scissor| {
                let scissor = scissor.borrow();
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

    fn set_stencil_reference(
        &mut self, front: com::StencilValue, back: com::StencilValue
    ) {
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

    fn set_blend_constants(&mut self, color: com::ColorValue) {
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

    fn bind_graphics_descriptor_sets<T>(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<n::DescriptorSet>,
    {
        self.bind_descriptor_sets(
            vk::PipelineBindPoint::Graphics,
            layout,
            first_set,
            sets,
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

    fn bind_compute_descriptor_sets<T>(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<n::DescriptorSet>,
    {
        self.bind_descriptor_sets(
            vk::PipelineBindPoint::Compute,
            layout,
            first_set,
            sets,
        );
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
        src_layout: ImageLayout,
        dst: &n::Image,
        dst_layout: ImageLayout,
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
                    src_subresource: conv::map_subresource_with_layers(r.aspect_mask, r.src_subresource, r.num_layers),
                    src_offset: conv::map_offset(r.src_offset),
                    dst_subresource: conv::map_subresource_with_layers(r.aspect_mask, r.dst_subresource, r.num_layers),
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
        dst_layout: ImageLayout,
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
        src_layout: ImageLayout,
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
        offset: u64,
        draw_count: u32,
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
