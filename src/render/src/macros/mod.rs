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


/// Defines vertex, constant and pipeline formats in one block.
///
/// # Example
///
/// ```{.rust}
/// #[macro_use]
/// extern crate gfx;
///
/// gfx_defines! {
///     vertex Vertex {
///         pos: [f32; 4] = "a_Pos",
///         tex_coord: [f32; 2] = "a_TexCoord",
///     }
///     
///     constant Locals {
///         transform: [[f32; 4]; 4] = "u_Transform",
///     }
///
///     pipeline pipe {
///         vbuf: gfx::VertexBuffer<Vertex> = (),
///         // Global buffers are added for compatibility when
///         // constant buffers are not supported.
///         transform: gfx::Global<[[f32; 4]; 4]> = "u_Transform",
///         locals: gfx::ConstantBuffer<Locals> = "Locals",
///         color: gfx::TextureSampler<[f32; 4]> = "t_Color",
///         out_color: gfx::RenderTarget<gfx::format::Rgba8> = "Target0",
///         out_depth: gfx::DepthTarget<gfx::format::DepthStencil> = 
///             gfx::preset::depth::LESS_EQUAL_WRITE,
///     }
/// }
///
/// impl Vertex {
///     fn new(p: [i8; 3], tex: [i8; 2]) -> Vertex {
///         Vertex {
///             pos: [p[0] as f32, p[1] as f32, p[2] as f32, 1.0f32],
///             tex_coord: [tex[0] as f32, tex[1] as f32],
///         }
///     }
/// }
///
/// fn main() {
///     let vertex_data = [
///         Vertex::new([-1, -1, 1], [0, 0]),
///         Vertex::new([ 1, -1, 1], [1, 0]),
///         // Add more vertices..
///     ];
/// }
/// ```
/// `vertex` and `constant` structures defined with `gfx_defines!`
/// can be extended with attributes:
///
/// ```{.rust}
/// #[macro_use]
/// extern crate gfx;
///
/// gfx_defines! {
///     #[derive(Default)]
///     vertex Vertex {
///         pos: [f32; 4] = "a_Pos",
///         tex_coord: [f32; 2] = "a_TexCoord",
///     }
/// }
///
/// fn main() {
///     let vertex = Vertex::default();
///     assert_eq!(vertex.pos[0], 0f32);
///     assert_eq!(vertex.tex_coord[0], 0f32);
/// }
/// ```
///
/// # `pipe`
///
/// The `pipeline state object` or `pso` can consist of the following
/// `pso` components:
///
/// - A [vertex buffer](pso/buffer/type.VertexBuffer.html) component to hold the vertices.
/// - An [instance buffer](pso/buffer/type.InstanceBuffer.html) component.
/// - Single or multiple [constant buffer](pso/buffer/struct.ConstantBuffer.html) components. (DX11 and OpenGL3)
/// - Single or multiple [global buffer](pso/buffer/struct.Global.html) components.
/// - Single or multiple [samplers](pso/resource/struct.Sampler.html).
/// - [Render](pso/target/struct.RenderTarget.html), [blend](pso/target/struct.BlendTarget.html), 
///   [depth](pso/target/struct.DepthTarget.html), [stencil](pso/target/struct.StencilTarget.html) targets.
/// - A [shader resource view](pso/resource/struct.ShaderResource.html) (SRV, DX11)
/// - An [unordered access view](pso/resource/struct.UnorderedAccess.html) (UAV, DX11, not yet implemented in the OpenGL backend)
/// - A [scissor](pso/target/struct.Scissor.html) rectangle value (DX11)
///
/// Structure of a `pipeline state object` can be defined freely.
///
/// It should be noted however, that you can have multiple objects of everything but
/// depth/stencil and scissor objects in a `pipeline state object`, which is the only
/// restriction in the freedom of defining a `pipeline state object`.
///
/// # `vertex`
///
/// Defines a vertex format to be passed onto a vertex buffer. Similar
/// to `pipeline state objects` multiple vertex formats can be set.
///
/// # `constant`
///
/// Defines a structure for shader constant data. This constant data
/// is then appended into a constant buffer in the `pso`. Constant buffers
/// are supported by DirectX 11 and OpenGL3 backend, but in OpenGL they
/// are called `Uniform Buffer Object`s or `UBO`s.
#[macro_export]
macro_rules! gfx_defines {
    ($(#[$attr:meta])* vertex $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_vertex_struct_meta!($(#[$attr])* vertex_struct_meta $name {$($field:$ty = $e,)+});
    };

    ($(#[$attr:meta])* constant $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_constant_struct_meta!($(#[$attr])* constant_struct_meta $name {$($field:$ty = $e,)+});
    };

    (pipeline $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    }) => {
        gfx_pipeline!($name {$($field:$ty = $e,)+});
    };

    // The recursive case for vertex structs
    ($(#[$attr:meta])* vertex $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    } $($tail:tt)+) => {
        gfx_defines! {
            $(#[$attr])*
            vertex $name { $($field : $ty = $e,)+ }
        }
        gfx_defines!($($tail)+);
    };

    // The recursive case for constant structs
    ($(#[$attr:meta])* constant $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    } $($tail:tt)+) => {
        gfx_defines! {
            $(#[$attr])*
            constant $name { $($field : $ty = $e,)+ }
        }
        gfx_defines!($($tail)+);
    };

    // The recursive case for the other keywords
    ($keyword:ident $name:ident {
            $( $field:ident : $ty:ty = $e:expr, )+
    } $($tail:tt)+) => {
        gfx_defines! {
            $keyword $name { $($field : $ty = $e,)+ }
        }
        gfx_defines!($($tail)+);
    };
}
