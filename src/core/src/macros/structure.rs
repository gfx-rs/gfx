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

//! Macro for implementing Structure

#[macro_export]
macro_rules! gfx_structure {
    ($name:ident: $meta:ident {
        $( $semantic:ident@ $field:ident: $ty:ty, )*
    }) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name {
            $( pub $field: $ty, )*
        }

        pub struct $meta {
            $( $field: Option<$crate::device::pso::Register>, )*
        }

        impl $crate::render::pso::Structure for $name {
            type Meta = $meta;

            fn iter_fields<F: FnMut(&'static str, $crate::device::attrib::Format)>(mut fun: F) {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Format, Stride};
                use $crate::attrib::format::ToFormat;
                let stride = size_of::<$name>() as Stride;
                $(
                    let (count, etype) = <$ty as ToFormat>::describe();
                    let tmp: &$name = unsafe{ ::std::mem::uninitialized() };
                    let format = Format {
                        elem_count: count,
                        elem_type: etype,
                        offset: ((&tmp.$field as *const _ as usize) - (&tmp as *const _ as usize)) as Offset,
                        stride: stride,
                        instance_rate: 0,
                    };
                    fun(stringify!($semantic), format);
                )*
            }

            fn make_meta<F: Fn(&str) -> Option<$crate::device::pso::Register>>(fun: F) -> $meta {
                $meta {
                    $(
                        $field: fun(stringify!($semantic)),
                    )*
                }
            }

            fn iter_meta<F: FnMut($crate::device::pso::Register)>(meta: &Self::Meta, mut fun: F) {
                $(
                    if let Some(reg) = meta.$field {
                        fun(reg);
                    }
                )*
            }
        }
    }
}
