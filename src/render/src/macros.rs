//! Various helper macros.

#[macro_export]
macro_rules! gfx_format {
    ($name:ident : $surface:ident = $container:ident<$channel:ident>) => {
        impl $crate::format::Formatted for $name {
            type Surface = $crate::format::$surface;
            type Channel = $crate::format::$channel;
            type View = $crate::format::$container<
                <$crate::format::$channel as $crate::format::ChannelTyped>::ShaderType
                >;
        }
    }
}

#[macro_export]
macro_rules! gfx_buffer_struct {
    ($name:ident { $( $field:ident: $ty:ty, )* }) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        #[allow(non_snake_case)]
        pub struct $name {
            $( $field: $ty, )*
        }

        unsafe impl $crate::memory::Pod for $name {}

        impl $crate::pso::Structure for $name
            where $( $ty: $crate::format::BufferFormat, )*
        {
            fn elements() -> Vec<$crate::hal::pso::Element<$crate::format::Format>> {
                let mut elements = Vec::new();
                let mut offset = 0;
                $(
                    elements.push($crate::hal::pso::Element {
                        format: <$ty as $crate::format::Formatted>::SELF,
                        offset: offset as u32,
                    });
                    offset += ::std::mem::size_of::<$ty>();
                )*
                let _ = offset;
                elements
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_descriptors {
    ($name:ident { $( $field:ident: $bind:ty, )* }) => {
        #[allow(missing_docs)]
        pub mod $name {
            #[allow(unused_imports)]
            use super::*;
            use $crate::{hal, pso, handle, image};
            use $crate::Backend;

            pub struct Set<B: Backend> {
                $( $field: usize, )*
                layout: handle::raw::DescriptorSetLayout<B>,
                raw: pso::RawDescriptorSet<B>,
            }

            pub struct Data<B: Backend> {
                $( $field: [Option<<$bind as pso::Bind<B>>::Handle>; <$bind as pso::BindDesc>::COUNT], )*
            }

            pub struct Component;

            impl<B: Backend> pso::Descriptors<B> for Set<B> {
                type Data = Data<B>;

                fn from_raw(
                    layout: handle::raw::DescriptorSetLayout<B>,
                    raw: pso::RawDescriptorSet<B>
                ) -> (Self, Self::Data) {
                    let mut binding = 0;
                    let mut next_binding = || {let b = binding; binding += 1; b };
                    (Set {
                        $( $field: next_binding(), )*
                        layout,
                        raw
                    }, Data {
                        $( $field: [None; <$bind as pso::BindDesc>::COUNT], )*
                    })
                }

                fn layout_bindings() -> Vec<hal::pso::DescriptorSetLayoutBinding> {
                    let mut bindings = Vec::new();
                    $({
                        let binding = bindings.len();
                        bindings.push(hal::pso::DescriptorSetLayoutBinding {
                            binding,
                            ty: <$bind as pso::BindDesc>::TYPE,
                            count: <$bind as pso::BindDesc>::COUNT,
                            // TODO: specify stage
                            stage_flags: hal::pso::ShaderStageFlags::all(),
                        });
                    })*
                    bindings
                }

                fn layout(&self) -> &B::DescriptorSetLayout { self.layout.resource() }
                fn set(&self) -> &B::DescriptorSet { self.raw.resource() }
            }

            impl<B: $crate::Backend> Data<B> {
                $(
                    pub fn $field<'a, 'b>(&'a mut self, set: &'b Set<B>)
                        -> pso::DescriptorSetBindRef<'b, 'a, B, $bind>
                    {
                        pso::DescriptorSetBindRef {
                            set: set.raw.resource(),
                            binding: set.$field,
                            handles: &mut self.$field,
                        }
                    }
                )*
            }

            impl<'a, B: Backend> pso::Component<'a, B> for Component {
                type Init = &'a Set<B>;
                type Data = (&'a Set<B>, &'a Data<B>);

                fn descriptor_layout<'b>(init: &'b Self::Init) -> Option<&'b B::DescriptorSetLayout>
                    where 'a: 'b
                {
                    Some(init.layout.resource())
                }

                fn descriptor_set<'b>(data: &'b Self::Data) -> Option<&'b B::DescriptorSet>
                    where 'a: 'b
                {
                    Some(data.0.raw.resource())
                }

                fn require<'b>(
                    data: &'b Self::Data,
                    buffers: &mut Vec<(&'b handle::raw::Buffer<B>, hal::buffer::State)>,
                    images: &mut Vec<(&'b handle::raw::Image<B>, image::Subresource, hal::image::State)>,
                    others: &mut handle::Bag<B>,
                )
                    where 'a: 'b
                {
                    $(
                        for handle_opt in &(data.1).$field {
                            handle_opt.as_ref().map(|h| {
                                <$bind as pso::Bind<B>>::require(h, buffers, images, others);
                            });
                        }
                    )*
                }
            }
        }
    }
}

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
            use $crate::hal::command::RenderPassInlineEncoder;

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
                pub viewports: &'a [$crate::hal::Viewport],
                pub scissors: &'a [$crate::hal::target::Rect],
                pub framebuffer: &'a handle::raw::Framebuffer<B>,
                $( pub $cmp_name: <$cmp as pso::Component<'a, B>>::Data, )*
            }

            impl<'a, B: Backend> pso::GraphicsPipelineInit<B> for Init<'a, B> {
                type Pipeline = Meta<B>;

                fn create(
                    self,
                    device: &mut Device<B>,
                    shader_entries: $crate::hal::pso::GraphicsShaderSet<B>,
                    primitive: Primitive,
                    rasterizer: pso::Rasterizer
                ) -> Result<Self::Pipeline, pso::CreationError> {
                    let mut desc_layouts = Vec::new();
                    $( desc_layouts.extend(<$cmp as pso::Component<'a, B>>::descriptor_layout(&self.$cmp_name)); )*
                    let layout = device.create_pipeline_layout_raw(&desc_layouts[..]);
                    let render_pass = {
                        let mut attachments = Vec::new();
                        let mut color_attachments = Vec::new();
                        $(
                            if let Some(attach) = <$cmp as pso::Component<'a, B>>::attachment(&self.$cmp_name) {
                                let attach_id = attachments.len();
                                attachments.push(cpass::Attachment {
                                    format: attach.format,
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

                    let mut pipeline_desc = cpso::GraphicsPipelineDesc::new(
                        primitive, rasterizer
                    );
                    $(
                        <$cmp as pso::Component<'a, B>>::append_desc(self.$cmp_name, &mut pipeline_desc);
                    )*

                    let pipeline = {
                        let subpass = cpass::Subpass {
                            index: 0,
                            main_pass: render_pass.resource()
                        };
                        device.create_graphics_pipeline_raw(
                            shader_entries, layout.resource(), subpass, &pipeline_desc
                        )?
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
                ) -> RenderPassInlineEncoder<'b, B>
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
                        cpso::VERTEX_INPUT,
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
                    cmd_buffer.bind_graphics_descriptor_sets(meta.layout.resource(), 0, &descs[..]);
                    // TODO: difference with viewport ?
                    let extent = self.framebuffer.info().extent;
                    let render_rect = $crate::hal::target::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as u16,
                        h: extent.height as u16
                    };
                    cmd_buffer.begin_renderpass_inline(
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
