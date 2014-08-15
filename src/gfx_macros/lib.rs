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

#![crate_name = "gfx_macros"]
#![comment = "Helper macros for gfx-rs"]
#![license = "ASL2"]
#![crate_type = "dylib"]

#![feature(macro_rules, plugin_registrar, quote)]

//! Macro extensions crate.
//! Implements `shaders!` macro as well as `#[shader_param]` and
//! `#[vertex_format]` attributes.

extern crate rustc;
extern crate syntax;

use syntax::{ast, attr, ext, codemap};
use syntax::parse::token;

pub mod shader_param;
pub mod vertex_format;

/// Entry point for the plugin phase
#[plugin_registrar]
pub fn registrar(reg: &mut rustc::plugin::Registry) {
    use syntax::parse::token::intern;
    use syntax::ext::base;
    // Register the `#[shader_param]` attribute.
    reg.register_syntax_extension(intern("shader_param"),
        base::ItemDecorator(shader_param::expand));
    // Register the `#[vertex_format]` attribute.
    reg.register_syntax_extension(intern("vertex_format"),
        base::ItemDecorator(vertex_format::expand));
}

/// A hacky thing to get around 'moved value' errors when using `quote_expr!`
/// with `ext::base::ExtCtxt`s.
fn ugh<T, U>(x: &mut T, f: |&mut T| -> U) -> U { f(x) }

/// Scan through the field's attributes and extract the field vertex name. If
/// multiple names are found, use the first name and emit a warning.
fn find_name(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
             attributes: &[ast::Attribute]) -> Option<token::InternedString> {
    attributes.iter().fold(None, |name, attribute| {
        match attribute.node.value.node {
            ast::MetaNameValue(ref attr_name, ref attr_value) => {
                match (attr_name.get(), &attr_value.node) {
                    ("name", &ast::LitStr(ref new_name, _)) => {
                        attr::mark_used(attribute);
                        name.map_or(Some(new_name.clone()), |name| {
                            cx.span_warn(span, format!(
                                "Extra field name detected: {} - \
                                ignoring in favour of: {}", new_name, name
                            ).as_slice());
                            None
                        })
                    }
                    _ => None,
                }
            }
            _ => name,
        }
    })
}

#[macro_export]
macro_rules! shaders {
    (GLSL_120: $v:expr $($t:tt)*) => {
        ::gfx::ShaderSource {
            glsl_120: Some(::gfx::StaticBytes($v)),
            ..shaders!($($t)*)
        }
    };
    (GLSL_150: $v:expr $($t:tt)*) => {
        ::gfx::ShaderSource {
            glsl_150: Some(::gfx::StaticBytes($v)),
            ..shaders!($($t)*)
        }
    };
    () => {
        ::gfx::ShaderSource {
            glsl_120: None,
            glsl_150: None,
        }
    }
}
