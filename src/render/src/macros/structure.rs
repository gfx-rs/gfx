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
macro_rules! gfx_impl_struct {
    ($runtime_format:ty : $compile_format:path = $root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => (gfx_impl_struct_meta! {
        impl_struct_meta $runtime_format : $compile_format = $root {
            $( $field : $ty = $name, )*
        }
    })
}

#[macro_export]
macro_rules! gfx_impl_struct_meta {
    ($(#[$attr:meta])* impl_struct_meta $runtime_format:ty : $compile_format:path = $root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => {
        #[allow(missing_docs)]
        #[derive(Clone, Copy, Debug)]
        $(#[$attr])*
        pub struct $root {
            $( pub $field: $ty, )*
        }

        unsafe impl $crate::traits::Pod for $root {}

        impl $crate::pso::buffer::Structure<$runtime_format> for $root {
            fn query(name: &str) -> Option<$crate::pso::buffer::Element<$runtime_format>> {
                use std::mem::size_of;
                use $crate::pso::buffer::{Element, ElemOffset};
                let tmp: &$root = unsafe{ ::std::mem::uninitialized() };
                let base = tmp as *const _ as usize;
                //HACK: special treatment of array queries
                let (sub_name, big_offset) = {
                    let mut split = name.split(|c| c == '[' || c == ']');
                    let _ = split.next().unwrap();
                    match split.next() {
                        Some(s) => {
                            let array_id: ElemOffset = s.parse().unwrap();
                            let sub_name = match split.next() {
                                Some(s) if s.starts_with('.') => &s[1..],
                                _ => name,
                            };
                            (sub_name, array_id * (size_of::<$root>() as ElemOffset))
                        },
                        None => (name, 0),
                    }
                };
                match sub_name {
                $(
                    $name => Some(Element {
                        format: <$ty as $compile_format>::get_format(),
                        offset: ((&tmp.$field as *const _ as usize) - base) as ElemOffset + big_offset,
                    }),
                )*
                    _ => None,
                }
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_vertex_struct {
    ($root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => (gfx_vertex_struct_meta! {
        vertex_struct_meta $root {
            $( $field : $ty = $name, )*
        }
    })
}

#[macro_export]
macro_rules! gfx_vertex_struct_meta {
    ($(#[$attr:meta])* vertex_struct_meta $root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => (gfx_impl_struct_meta!{
        $(#[$attr])* impl_struct_meta
        $crate::format::Format : $crate::format::Formatted =
        $root {
            $( $field: $ty = $name, )*
        }
    })
}

#[macro_export]
macro_rules! gfx_constant_struct {
    ($root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => (gfx_constant_struct_meta!{
        constant_struct_meta $root {
            $( $field : $ty = $name, )*
        }
    })
}

#[macro_export]
macro_rules! gfx_constant_struct_meta {
    ($(#[$attr:meta])* constant_struct_meta $root:ident {
        $( $field:ident: $ty:ty = $name:expr, )*
    }) => (gfx_impl_struct_meta!{
        $(#[$attr])* impl_struct_meta
        $crate::shade::ConstFormat : $crate::shade::Formatted =
        $root {
            $( $field: $ty = $name, )*
        }
    })
}
