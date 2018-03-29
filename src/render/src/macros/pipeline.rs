#[macro_export]
macro_rules! gfx_graphics_pipeline {
    ($name:ident {
        $( $cmp_name:ident: $cmp:ty, )*
    }) => {
        #[allow(missing_docs)]
        pub mod $name {
            #[allow(unused_imports)]
            use super::*;
            use $crate::{pso, handle};
            use $crate::{
                Backend, Supports, Transfer, Graphics, Encoder,
                Device, Primitive
            };
            use $crate::hal::{pass as cpass, pso as cpso};
            use $crate::hal::command::{RenderPassInlineEncoder, Primary};

            pub struct Meta<B: Backend> {
                layout: handle::raw::PipelineLayout<B>,
                render_pass: handle::raw::RenderPass<B>,
                pipeline: handle::raw::GraphicsPipeline<B>,
            }

            pub struct Init<'a, B: Backend> {
                $( pub $cmp_name: <$cmp as pso::Component<'a, B>>::Init, )*
            }

            pub struct Data<'a, B: Backend> {
                // TODO:
                pub viewports: &'a [pso::Viewport],
                pub scissors: &'a [pso::Rect],
                pub framebuffer: &'a handle::raw::Framebuffer<B>,
                $( pub $cmp_name: <$cmp as pso::Component<'a, B>>::Data, )*
            }

            impl<'a, B: Backend> pso::GraphicsPipelineInit<B> for Init<'a, B> {
                type Pipeline = Meta<B>;

                fn create<'b>(
                    self,
                    device: &mut Device<B>,
                    shader_entries: $crate::hal::pso::GraphicsShaderSet<'b, B>,
                    primitive: Primitive,
                    rasterizer: pso::Rasterizer
                ) -> Result<Self::Pipeline, pso::CreationError> {
                    let mut desc_layouts = Vec::new();
                    $( desc_layouts.extend(<$cmp as pso::Component<'a, B>>::descriptor_layout(&self.$cmp_name)); )*
                    let layout = device.create_pipeline_layout_raw(&desc_layouts[..], &[]);
                    let render_pass = {
                        let mut attachments = Vec::new();
                        let mut color_attachments = Vec::new();
                        $(
                            if let Some(attach) = <$cmp as pso::Component<'a, B>>::attachment(&self.$cmp_name) {
                                let attach_id = attachments.len();
                                attachments.push(cpass::Attachment {
                                    format: Some(attach.format),
                                    ops: attach.ops,
                                    stencil_ops: attach.stencil_ops,
                                    layouts: attach.required_layout .. attach.required_layout,
                                });
                                color_attachments.push((attach_id, attach.required_layout));
                            }
                        )*
                        let subpass = cpass::SubpassDesc {
                            colors: &color_attachments[..],
                            depth_stencil: None, //TODO
                            inputs: &[],
                            preserves: &[],
                        };

                        device.create_render_pass_raw(&attachments[..], &[subpass], &[])
                    };


                    let pipeline = {
                        let subpass = cpass::Subpass {
                            index: 0,
                            main_pass: render_pass.resource(),
                        };

                        let mut pipeline_desc = cpso::GraphicsPipelineDesc::new(
                            shader_entries,
                            primitive,
                            rasterizer,
                            layout.resource(),
                            subpass,
                        );
                        $(
                            <$cmp as pso::Component<'a, B>>::append_desc(self.$cmp_name, &mut pipeline_desc);
                        )*


                        device.create_graphics_pipeline_raw(pipeline_desc)?
                    };
                    Ok(Meta { layout, render_pass, pipeline })
                }
            }

            impl<B: Backend> pso::GraphicsPipelineMeta<B> for Meta<B> {
                fn layout(&self) -> &B::PipelineLayout { self.layout.resource() }
                fn render_pass(&self) -> &B::RenderPass { self.render_pass.resource() }
            }

            impl<'a, B: Backend> pso::GraphicsPipelineData<B> for Data<'a, B> {
                type Pipeline = Meta<B>;

                fn begin_renderpass<'b, 'c, C>(
                    self,
                    encoder: &'b mut Encoder<'c, B, C>,
                    meta: &'b Self::Pipeline
                ) -> RenderPassInlineEncoder<'b, B, Primary>
                    where Self: 'a, 'c: 'b, C: Supports<Transfer> + Supports<Graphics>
                {
                    let mut buffer_states = Vec::new();
                    let mut image_states = Vec::new();
                    $(
                        <$cmp as pso::Component<'a, B>>::require(
                            &self.$cmp_name,
                            &mut buffer_states,
                            &mut image_states,
                            encoder.handles());
                    )*
                    encoder.require_state(
                        cpso::PipelineStage::VERTEX_INPUT,
                        &buffer_states[..],
                        &image_states[..]
                    );

                    let cmd_buffer = encoder.mut_buffer();
                    cmd_buffer.set_viewports(self.viewports);
                    cmd_buffer.set_scissors(self.scissors);
                    cmd_buffer.bind_graphics_pipeline(meta.pipeline.resource());
                    let mut vbs = Vec::new();
                    $(
                        vbs.extend(<$cmp as pso::Component<'a, B>>::vertex_buffer(&self.$cmp_name));
                    )*
                    cmd_buffer.bind_vertex_buffers(cpso::VertexBufferSet(vbs));
                    let mut descs = Vec::new();
                    $(
                        descs.extend(<$cmp as pso::Component<'a, B>>::descriptor_set(&self.$cmp_name));
                    )*
                    cmd_buffer.bind_graphics_descriptor_sets(meta.layout.resource(), 0, descs);
                    // TODO: difference with viewport ?
                    let extent = self.framebuffer.info().extent;
                    let render_rect = pso::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as u16,
                        h: extent.height as u16
                    };
                    cmd_buffer.begin_render_pass_inline(
                        meta.render_pass.resource(),
                        self.framebuffer.resource(),
                        render_rect,
                        &[], // TODO
                    )
                }
            }
        }
    }
}
