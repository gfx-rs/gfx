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

//! Various helper macros.

mod pso;
mod structure;

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


/// Defines vertex, constant and pipeline formats in one block
#[macro_export]
macro_rules! gfx_defines {
    (vertex $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_vertex_struct!($name {$($field:$ty = $e,)+});
    };

    (constant $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_constant_struct!($name {$($field:$ty = $e,)+});
    };

    (pipeline $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_pipeline!($name {$($field:$ty = $e,)+});
    };
	
    (pipeline $crt:ident : $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_pipeline!($crt:$name {$($field:$ty = $e,)+});
    };

    ($keyword:ident $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    } $($tail:tt)+) => {
        gfx_defines! {
            $keyword $name { $($field : $ty = $e,)+ }
        }
        gfx_defines!($($tail)+);
    };

    ($keyword:ident $name:ident {
            $( $field:ident : $ty:ty = $e:expr ),*
    }) => {
        gfx_defines! {
            $keyword $name {
                $($field : $ty = $e ,)*
            }
        }
    };
}
