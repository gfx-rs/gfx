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
        #[derive(Clone, Copy, Debug, PartialEq)]
        $(#[$attr])*
        pub struct $root {
            $( pub $field: $ty, )*
        }

        unsafe impl $crate::traits::Pod for $root {}

        impl $crate::pso::buffer::Structure<$runtime_format> for $root {
            fn query(name: &str) -> ::std::option::Option<$crate::pso::buffer::Element<$runtime_format>> {
                use std::mem::{size_of, transmute};
                use $crate::pso::buffer::{Element, ElemOffset};
                // using "1" here as a simple non-zero pointer addres
                let tmp: &$root = unsafe{ transmute(1usize) };
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
