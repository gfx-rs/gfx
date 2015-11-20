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

//! Macro for implementing ShaderLink

#[macro_export]
macro_rules! gfx_shader_link {
	($name:ident: $meta:ident {
		$( $semantic:ident@ $field:ident: $ty:ty, )*
	}) => {
		#[derive(Clone, Debug)]
		pub struct $name<R: $crate::Resources> {
			$( pub $field: $ty, )*
		}

		pub struct $meta<R: $crate::Resources> {
			$( $field: Option<<$ty as $crate::render::pso::DataLink<'static, R>>::Link>, )*
			_res: ::std::marker::PhantomData<R>,
		}

		impl<R: $crate::Resources> $crate::render::pso::LinkBuilder<'static, $meta<R>> for $name<R> {
			fn declare() -> $crate::device::pso::LinkMap<'static> {
				use std::collections::HashMap;
				use $crate::render::pso::DataLink;
				let mut map = HashMap::new();
				$( <$ty as DataLink<'static, R>>::declare_to(&mut map, stringify!($semantic)); )*
				map
			}
			fn register(map: &$crate::device::pso::RegisterMap<'static>) -> $meta<R> {
				use $crate::render::pso::DataLink;
				$meta {
					$( $field: <$ty as DataLink<'static, R>>::link(map, stringify!($semantic)), )*
					_res: ::std::marker::PhantomData,
				}
			}
		}

		impl<R: $crate::Resources> $crate::render::pso::ShaderLink<R> for $name<R> {
			type Meta = $meta<R>;

			fn define(&self, meta: &$meta<R>, man: &mut $crate::handle::Manager<R>)
						-> $crate::render::pso::ShaderDataSet<R>
			{
				use $crate::render::pso::DataLink;
				let mut out = $crate::render::pso::ShaderDataSet::new();
				$(
					if let Some(ref link) = meta.$field {
						self.$field.bind_to(&mut out, link, man);
					}
				)*
				out
			}
		}
	}
}
