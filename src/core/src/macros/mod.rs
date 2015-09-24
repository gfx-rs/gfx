// Copyright 2014 The Gfx-rs Developers.
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

//! Macros for deriving `VertexFormat` and `ShaderParam`.

#[macro_export]
macro_rules! gfx_vertex {
    ($name:ident {
        $($gl_name:ident@ $field:ident: $ty:ty,)*
    }) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name {
            $(pub $field: $ty,)*
        }
        impl $crate::VertexFormat for $name {
            fn generate<R: $crate::Resources>(buffer: &$crate::handle::Buffer<R, $name>)
                        -> Vec<$crate::Attribute<R>> {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Stride};
                use $crate::attrib::format::ToFormat;
                let stride = size_of::<$name>() as Stride;
                let tmp: $name = unsafe{ ::std::mem::uninitialized() };
                let mut attributes = Vec::new();
                $(
                    let (count, etype) = <$ty as ToFormat>::describe();
                    let format = $crate::attrib::Format {
                        elem_count: count,
                        elem_type: etype,
                        offset: (&tmp.$field as *const _ as usize) - (&tmp as *const _ as usize),
                        stride: stride,
                        instance_rate: 0,
                    };
                    attributes.push($crate::Attribute {
                        name: stringify!($gl_name).to_string(),
                        format: format,
                        buffer: buffer.raw().clone(),
                    });
                )*
                attributes
            }
        }
    }
}

#[macro_export]
macro_rules! gfx_parameters {
    ($name:ident {
        $($gl_name:ident@ $field:ident: $ty:ty,)*
    }) => {
        #[derive(Clone, Debug)]
        pub struct $name<R: $crate::Resources> {
            $(pub $field: $ty,)*
            pub _r: ::std::marker::PhantomData<R>,
        }

        impl<R: $crate::Resources> $crate::shade::ShaderParam for $name<R> {
            type Resources = R;
            type Link = ($((Option<$crate::shade::ParameterId>, ::std::marker::PhantomData<$ty>),)*);

            fn create_link(_: Option<&$name<R>>, info: &$crate::ProgramInfo)
                           -> Result<Self::Link, $crate::shade::ParameterError>
            {
                use $crate::shade::Parameter;
                $(
                    let mut $field = None;
                )*
                // scan uniforms
                for (i, u) in info.uniforms.iter().enumerate() {
                    match &u.name[..] {
                        $(
                        stringify!($gl_name) => {
                            if !<$ty as Parameter<R>>::check_uniform(u) {
                                return Err($crate::shade::ParameterError::BadUniform(u.name.clone()))
                            }
                            $field = Some(i as $crate::shade::ParameterId);
                        },
                        )*
                        _ => return Err($crate::shade::ParameterError::MissingUniform(u.name.clone()))
                    }
                }
                // scan uniform blocks
                for (i, b) in info.blocks.iter().enumerate() {
                    match &b.name[..] {
                        $(
                        stringify!($gl_name) => {
                            if !<$ty as Parameter<R>>::check_block(b) {
                                return Err($crate::shade::ParameterError::BadBlock(b.name.clone()))
                            }
                            $field = Some(i as $crate::shade::ParameterId);
                        },
                        )*
                        _ => return Err($crate::shade::ParameterError::MissingBlock(b.name.clone()))
                    }
                }
                // scan textures
                for (i, t) in info.textures.iter().enumerate() {
                    match &t.name[..] {
                        $(
                        stringify!($gl_name) => {
                            if !<$ty as Parameter<R>>::check_texture(t) {
                                return Err($crate::shade::ParameterError::BadBlock(t.name.clone()))
                            }
                            $field = Some(i as $crate::shade::ParameterId);
                        },
                        )*
                        _ => return Err($crate::shade::ParameterError::MissingBlock(t.name.clone()))
                    }
                }
                Ok(( $(($field, ::std::marker::PhantomData),)* ))
            }

            fn fill_params(&self, link: &Self::Link, storage: &mut $crate::ParamStorage<R>) {
                use $crate::shade::Parameter;
                let &($(($field, _),)*) = link;
                $(
                    if let Some(id) = $field {
                        self.$field.put(id, storage);
                    }
                )*
            }
        }
    }
}
