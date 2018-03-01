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
                        let binding = bindings.len() as _;
                        bindings.push(hal::pso::DescriptorSetLayoutBinding {
                            binding,
                            ty: <$bind as pso::BindDesc>::TYPE,
                            count: <$bind as pso::BindDesc>::COUNT as _,
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
                            binding: set.$field as _,
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
