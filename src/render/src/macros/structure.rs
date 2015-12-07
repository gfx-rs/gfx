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

//! Macro for implementing Structure.

#[macro_export]
macro_rules! gfx_structure {
    ($root:ident {
        $( $name:ident@ $field:ident: $ty:ty, )*
    }) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $root {
            $( pub $field: $ty, )*
        }

        impl $crate::pso::Structure for $root {
            fn query(name: &str) -> Option<$crate::attrib::Format> {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Format, Stride};
                use $crate::attrib::format::ToFormat;
                let stride = size_of::<$root>() as Stride;
                let tmp: &$root = unsafe{ ::std::mem::uninitialized() };
                match name {
                $(
                    stringify!($name) => {
                        let (count, etype) = <$ty as ToFormat>::describe();
                        Some(Format {
                            elem_count: count,
                            elem_type: etype,
                            offset: ((&tmp.$field as *const _ as usize) - (&tmp as *const _ as usize)) as Offset,
                            stride: stride,
                            instance_rate: 0,
                        })
                    },
                )*
                    _ => None,
                }
            }
        }
    }
}
