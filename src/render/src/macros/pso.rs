// Copyright 2015 The Gfx-rs Developers.
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

//! Macros for implementing PipelineInit and PipelineData.

#[macro_export]
macro_rules! gfx_pipeline {
    ($data:ident $meta:ident $init:ident {
        $( $field:ident: $ty:ty, )*
    }) => {
        use $crate::pso::{DataLink, DataBind, Descriptor, InitError, RawDataSet};

        #[derive(Clone, Debug)]
        pub struct $data<R: $crate::Resources> {
            $( pub $field: <$ty as DataBind<R>>::Data, )*
        }

        pub struct $meta {
            $( $field: $ty, )*
        }

        pub struct $init<'a> {
            $( pub $field: <$ty as DataLink<'a>>::Init, )*
        }

        impl<'a> $crate::pso::PipelineInit for $init<'a> {
            type Meta = $meta;
            fn link_to(&self, desc: &mut Descriptor, info: &$crate::ProgramInfo) -> Result<Self::Meta, InitError> {
                let mut meta = $meta {
                    $( $field: <$ty as DataLink<'a>>::new(), )*
                };
                // v#
                for at in &info.vertex_attributes {
                    $(
                        match meta.$field.link_input(at, &self.$field) {
                            Some(Ok(d)) => {
                                assert!(meta.$field.is_active());
                                desc.attributes[at.slot as usize] = Some(d);
                                continue;
                            },
                            Some(Err(fm)) => return Err(
                                InitError::VertexImport(at.slot, Some(fm))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::VertexImport(at.slot, None));
                }
                // c#
                for cb in &info.constant_buffers {
                    $(
                        match meta.$field.link_constant_buffer(cb, &self.$field) {
                            Some(Ok(())) => {
                                assert!(meta.$field.is_active());
                                continue;
                            },
                            Some(Err(_)) => return Err(
                                InitError::ConstantBuffer(cb.slot, Some(()))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::ConstantBuffer(cb.slot, None));
                }
                // global constants
                for gc in &info.globals {
                    $(
                        match meta.$field.link_global_constant(gc, &self.$field) {
                            Some(Ok(())) => {
                                assert!(meta.$field.is_active());
                                continue;
                            },
                            Some(Err(_)) => return Err(
                                InitError::GlobalConstant(gc.location, Some(()))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::GlobalConstant(gc.location, None));
                }
                // t#
                for srv in &info.textures {
                    $(
                        match meta.$field.link_resource_view(srv, &self.$field) {
                            Some(Ok(())) => {
                                assert!(meta.$field.is_active());
                                continue;
                            },
                            Some(Err(_)) => return Err(
                                InitError::ResourceView(srv.slot, Some(()))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::ResourceView(srv.slot, None));
                }
                // u#
                for uav in &info.unordereds {
                    $(
                        match meta.$field.link_unordered_view(uav, &self.$field) {
                            Some(Ok(())) => {
                                assert!(meta.$field.is_active());
                                continue;
                            },
                            Some(Err(_)) => return Err(
                                InitError::UnorderedView(uav.slot, Some(()))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::UnorderedView(uav.slot, None));
                }
                // s#
                for sm in &info.samplers {
                    $(
                        match meta.$field.link_sampler(sm, &self.$field) {
                            Some(()) => {
                                assert!(meta.$field.is_active());
                                continue;
                            },
                            None => (),
                        }
                    )*
                    return Err(InitError::Sampler(sm.slot, None));
                }
                // color targets
                for out in &info.outputs {
                    $(
                        match meta.$field.link_output(out, &self.$field) {
                            Some(Ok(d)) => {
                                assert!(meta.$field.is_active());
                                desc.color_targets[out.slot as usize] = Some(d);
                                continue;
                            },
                            Some(Err(fm)) => return Err(
                                InitError::PixelExport(out.slot, Some(fm))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::PixelExport(out.slot, None));
                }
                // depth-stencil
                for _ in 0 .. 1 {
                    $(
                      match meta.$field.link_depth_stencil(&self.$field) {
                            Some(d) => {
                                assert!(meta.$field.is_active());
                                desc.depth_stencil = Some(d);
                                continue;
                            },
                            None => (),
                        }
                    )*
                }
                // done
                Ok(meta)
            }
        }

        impl<R: $crate::Resources> $crate::pso::PipelineData<R> for $data<R> {
            type Meta = $meta;
            fn bake(&self, meta: &Self::Meta, man: &mut $crate::handle::Manager<R>) -> RawDataSet<R> {
                let mut out = RawDataSet::new();
                $(
                    meta.$field.bind_to(&mut out, &self.$field, man);
                )*
                out
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_pipeline_init {
    ($data:ident $meta:ident $init:ident {
        $( $field:ident: $ty:ty = $value:expr, )*
    }) => {
        gfx_pipeline!( $data $meta $init {
            $( $field: $ty, )*
        });
        impl $init<'static> {
            pub fn new() -> $init<'static> {
                $init {
                    $( $field: $value, )*
                }
            }
        }
    }
}
