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

        pub struct $init {
            $( pub $field: <$ty as DataLink>::Init, )*
        }

        impl $crate::pso::PipelineInit for $init {
            type Meta = $meta;
            fn link_to(&self, desc: &mut Descriptor, info: &$crate::ProgramInfo) -> Result<Self::Meta, InitError> {
                let mut meta = $meta {
                    $( $field: <$ty as DataLink>::new(), )*
                };
                for a in &info.attributes {
                    $(
                        match meta.$field.link_input(a, &self.$field) {
                            Some(Ok(d)) => {
                                desc.attributes[a.slot as usize] = Some(d);
                                break;
                            },
                            Some(Err(fm)) => return Err(
                                InitError::VertexImport(a.slot, Some(fm))
                            ),
                            None => (),
                        }
                    )*
                    return Err(InitError::VertexImport(a.slot, None));
                }
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
        impl $init {
            pub fn new() -> $init {
                $init {
                    $( $field: $value, )*
                }
            }
        }
    }
}
