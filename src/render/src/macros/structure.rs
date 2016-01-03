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

//! Macro for implementing Structure for vertex and constant buffers.

#[macro_export]
macro_rules! gfx_vertex_struct {
    ($root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $root {
            $( pub $field: $ty, )*
        }

        impl $crate::pso::Structure<$crate::format::Format> for $root {
            fn query(name: &str) -> Option<$crate::pso::Element<$crate::format::Format>> {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Stride};
                let stride = size_of::<$root>() as Stride;
                let tmp: &$root = unsafe{ ::std::mem::uninitialized() };
                let base = tmp as *const _ as usize;
                match name {
                $(
                    $name => Some($crate::pso::Element {
                        format: <$ty as $crate::format::Formatted>::get_format(),
                        offset: ((&tmp.$field as *const _ as usize) - base) as Offset,
                        stride: stride,
                    }),
                )*
                    _ => None,
                }
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_constant_struct {
    ($root:ident {
        $( $field:ident: $ty:ty, )*
    }) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $root {
            $( pub $field: $ty, )*
        }

        impl $crate::pso::Structure<$crate::shade::ConstFormat> for $root {
            fn query(name: &str) -> Option<$crate::pso::Element<$crate::shade::ConstFormat>> {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Stride};
                let stride = size_of::<$root>() as Stride;
                let tmp: &$root = unsafe{ ::std::mem::uninitialized() };
                let base = tmp as *const _ as usize;
                match name {
                $(
                    stringify!($field) => Some($crate::pso::Element {
                        format: <$ty as $crate::shade::Formatted>::get_format(),
                        offset: ((&tmp.$field as *const _ as usize) - base) as Offset,
                        stride: stride,
                    }),
                )*
                    _ => None,
                }
            }
        }
    }
}
