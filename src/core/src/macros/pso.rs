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

//! Macro for implementing PipelineData

#[macro_export]
macro_rules! gfx_pipeline {
    ($data:ident $meta:ident $init:ident {
        $( $field:ident: $ty:ty, )*
    }) => {
        use $crate::render::pso::{DataLink, DataBind};

        #[derive(Clone, Debug)]
        pub struct $data<R: $crate::Resources> {
            $( pub $field: <$ty as DataBind<R>>::Data, )*
        }

        pub struct $meta {
            $( $field: Option<$ty>, )*
        }

        pub struct $init<'a> {
            $( pub $field: <$ty as DataLink<'a>>::Init, )*
        }

        impl<'a> $crate::render::pso::PipelineInit<'a> for $init<'a> {
            type Meta = $meta;
            fn declare(&self) -> $crate::device::pso::LinkMap<'a> {
                use std::collections::HashMap;
                let mut map = HashMap::new();
                $( <$ty as DataLink<'a>>::declare_to(&mut map, &self.$field); )*
                map
            }
            fn register(&self, map: &$crate::device::pso::RegisterMap<'a>) -> $meta {
                $meta {
                    $( $field: <$ty as DataLink<'a>>::link(map, &self.$field), )*
                }
            }
        }

        impl<R: $crate::Resources> $crate::render::pso::PipelineData<R> for $data<R> {
            type Meta = $meta;
            fn define(&self, meta: &$meta, man: &mut $crate::handle::Manager<R>)
                        -> $crate::render::pso::RawDataSet<R> {
                let mut out = $crate::render::pso::RawDataSet::new();
                $(
                    if let Some(ref link) = meta.$field {
                        link.bind_to(&mut out, &self.$field, man);
                    }
                )*
                out
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_pipeline_init {
    ($data:ident $meta:ident $init:ident = $fun:ident {
        $( $field:ident: $ty:ty = $value:expr, )*
    }) => {
        gfx_pipeline!( $data $meta $init {
            $( $field: $ty, )*
        });
        pub fn $fun() -> $init<'static> {
            $init {
                $( $field: $value, )*
            }
        }
    }
}
