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
            fn elements() -> Vec<$crate::core::pso::Element<$crate::format::Format>> {
                let mut elements = Vec::new();
                let mut offset = 0;
                $(
                    elements.push($crate::core::pso::Element {
                        format: <$ty as $crate::format::Formatted>::get_format(),
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
macro_rules! gfx_descriptor_struct {
    ($name:ident { $( $field:ident: $bind:ty, )* }) => {
        #[allow(missing_docs)]
        #[derive(Debug)]
        pub struct $name<B: $crate::Backend> {
            $( $field: usize, )*
            layout: $crate::handle::raw::DescriptorSetLayout<B>,
            set: $crate::pso::RawDescriptorSet<B>,
        }

        impl<B: $crate::Backend> $crate::pso::Descriptors<B> for $name<B> {
            fn from_raw(
                layout: $crate::handle::raw::DescriptorSetLayout<B>,
                set: $crate::pso::RawDescriptorSet<B>
            ) -> Self {
                let mut binding = 0;
                let mut next_binding = || {let b = binding; binding += 1; b };
                $name {
                    $( $field: next_binding(), )*
                    layout,
                    set
                }
            }

            fn layout_bindings() -> Vec<$crate::core::pso::DescriptorSetLayoutBinding> {
                let mut bindings = Vec::new();
                $({
                    let binding = bindings.len();
                    bindings.push($crate::core::pso::DescriptorSetLayoutBinding {
                        binding,
                        ty: <$bind as $crate::pso::Bind>::desc_type(),
                        count: <$bind as $crate::pso::Bind>::desc_count(),
                        // TODO: specify stage
                        stage_flags: $crate::core::pso::ShaderStageFlags::all(),
                    });
                })*
                bindings
            }
            
            fn layout(&self) -> &B::DescriptorSetLayout { self.layout.resource() }
            fn set(&self) -> &B::DescriptorSet { self.set.resource() }
        }

        impl<B: $crate::Backend> $name<B> {
            $(
                fn $field(&self) -> $crate::pso::DescriptorSetBindRef<B, $bind> {
                    $crate::pso::DescriptorSetBindRef {
                        set: self.set(),
                        binding: self.$field,
                        phantom: ::std::marker::PhantomData,
                    }
                }
            )*
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
            use $crate::{pso, handle, Backend, Device, Encoder, Primitive};
            use $crate::core::pass::{self as cpass, SubpassRef};
            use $crate::core::{pso as cpso, image as cimg};
    
            pub struct Meta<B: Backend> {
                layout: handle::raw::PipelineLayout<B>,
                render_pass: handle::raw::RenderPass<B>,
                pipeline: handle::raw::GraphicsPipeline<B>,
            }

            pub struct Init<'a, B: Backend> {
                $( pub $cmp_name: <$cmp as pso::Component<'a, B>>::Init, )*
            }

            pub struct Data<'a, B: Backend> {
                $( pub $cmp_name: <$cmp as pso::Component<'a, B>>::Data, )*
            }

            impl<'a, B: Backend> pso::GraphicsPipelineInit<B> for Init<'a, B> {
                type Pipeline = Meta<B>;

                fn create(
                    self,
                    device: &mut Device<B>,
                    shader_entries: $crate::core::pso::GraphicsShaderSet<B>,
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
                                    layouts: attach.required_layout..attach.required_layout,
                                });
                                color_attachments.push((attach_id, attach.required_layout));
                            }
                        )*
                        let subpass = cpass::SubpassDesc {
                            color_attachments: &color_attachments[..],
                        };

                        // TODO:
                        let dependency = cpass::SubpassDependency {
                            passes: SubpassRef::External..SubpassRef::Pass(0),
                            stages: cpso::COLOR_ATTACHMENT_OUTPUT..cpso::COLOR_ATTACHMENT_OUTPUT,
                            accesses: cimg::Access::empty()..(cimg::COLOR_ATTACHMENT_READ | cimg::COLOR_ATTACHMENT_WRITE),
                        };

                        device.create_renderpass_raw(&attachments[..], &[subpass], &[dependency])
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
                fn pipeline(&self) -> &B::GraphicsPipeline { self.pipeline.resource() }
            }

            impl<'a, B: Backend> pso::GraphicsPipelineData<B> for Data<'a, B> {
                type Pipeline = Meta<B>;

                fn bind(
                    self,
                    _viewport: $crate::core::Viewport,
                    _scissor: $crate::core::target::Rect,
                    _pipeline: &Self::Pipeline)
                {

                }
            }
        }
    }
}
