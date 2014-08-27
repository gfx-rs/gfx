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

use std::{fmt, from_str};
use std::gc::Gc;
use syntax::{ast, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::{attr, codemap};
use syntax::parse::token;

/// A component modifier.
#[deriving(PartialEq)]
enum Modifier {
    /// Corresponds to the `#[normalized]` attribute.
    ///
    /// Normalizes the component at runtime. Unsigned integers are normalized to
    /// `[0, 1]`. Signed integers are normalized to `[-1, 1]`.
    Normalized,
    /// Corresponds to the `#[as_float]` attribute.
    ///
    /// Casts the component to a float precision floating-point number at runtime.
    AsFloat,
    /// Corresponds to the `#[as_double]` attribute.
    ///
    /// Specifies a high-precision float.
    AsDouble,
}

impl fmt::Show for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Normalized => write!(f, "normalized"),
            AsFloat => write!(f, "as_float"),
            AsDouble => write!(f, "as_double"),
        }
    }
}

impl from_str::FromStr for Modifier {
    fn from_str(src: &str) -> Option<Modifier> {
        match src {
            "normalized" => Some(Normalized),
            "as_float" => Some(AsFloat),
            "as_double" => Some(AsDouble),
            _ => None,
        }
    }
}

/// Scan through the field's attributes and extract a relevant modifier. If
/// multiple modifier attributes are found, use the first modifier and emit a
/// warning.
fn find_modifier(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                 attributes: &[ast::Attribute]) -> Option<Modifier> {
    attributes.iter().fold(None, |modifier, attribute| {
        match attribute.node.value.node {
            ast::MetaWord(ref word) => {
                from_str(word.get()).and_then(|new_modifier| {
                    attr::mark_used(attribute);
                    modifier.map_or(Some(new_modifier), |modifier| {
                        cx.span_warn(span, format!(
                            "Extra attribute modifier detected: `#[{}]` - \
                            ignoring in favour of `#[{}]`.", new_modifier, modifier
                        ).as_slice());
                        None
                    })
                }).or(modifier)
            },
            _ => modifier,
        }
    })
}

/// Scan through attributes to find the instance rate, if specified
fn get_instance_rate(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                     attributes: &[ast::Attribute]) -> Gc<ast::Expr> {
    for attribute in attributes.iter() {
        match attribute.node.value.node {
            ast::MetaNameValue(ref name, ref value) => match (name.get(), &value.node) {
                ("instance_rate", &ast::LitStr(ref value_str, _)) => {
                    match from_str::from_str(value_str.get()) {
                        Some(value) => {
                            attr::mark_used(attribute);
                            return cx.expr_u8(span, value);
                        },
                        None => {
                            cx.span_err(span, "Invalid argument for #[instance_rate = ...]");
                        },
                    }
                },
                _ => (),
            },
            _ => ()
        }
    }
    cx.expr_u8(span, 0)
}

/// Find a `gfx::attrib::Type` that describes the given type identifier.
fn decode_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               ty_ident: &ast::Ident, modifier: Option<Modifier>,
               path_root: ast::Ident) -> Gc<ast::Expr> {
    let ty_str = ty_ident.name.as_str();
    match ty_str {
        "f32" | "f64" => {
            let kind = cx.ident_of(match modifier {
                None | Some(AsFloat) => "FloatDefault",
                Some(AsDouble) => "FloatPrecision",
                Some(Normalized) => {
                    cx.span_warn(span, format!(
                        "Incompatible float modifier attribute: `#[{}]`", modifier
                    ).as_slice());
                    ""
                }
            });
            let sub_type = cx.ident_of(format!("F{}", ty_str.slice_from(1)).as_slice());
            quote_expr!(cx, $path_root::gfx::attrib::Float($path_root::gfx::attrib::$kind,
                                                           $path_root::gfx::attrib::$sub_type))
        },
        "u8" | "u16" | "u32" | "u64" |
        "i8" | "i16" | "i32" | "i64" => {
            let sign = cx.ident_of({
                if ty_str.starts_with("i") { "Signed" } else { "Unsigned" }
            });
            let kind = cx.ident_of(match modifier {
                None => "IntRaw",
                Some(Normalized) => "IntNormalized",
                Some(AsFloat) => "IntAsFloat",
                Some(AsDouble) => {
                    cx.span_warn(span, format!(
                        "Incompatible int modifier attribute: `#[{}]`", modifier
                    ).as_slice());
                    ""
                }
            });
            let sub_type = cx.ident_of(format!("U{}", ty_str.slice_from(1)).as_slice());
            quote_expr!(cx, $path_root::gfx::attrib::Int($path_root::gfx::attrib::$kind,
                                                         $path_root::gfx::attrib::$sub_type,
                                                         $path_root::gfx::attrib::$sign))
        },
        "uint" | "int" => {
            cx.span_err(span, format!("Pointer-sized integer components are \
                                      not supported, but found: `{}`. Use an \
                                      integer component with an explicit size \
                                      instead.", ty_str).as_slice());
            cx.expr_lit(span, ast::LitNil)
        },
        ty_str => {
            cx.span_err(span, format!("Unrecognized component type: `{}`",
                                      ty_str).as_slice());
            cx.expr_lit(span, ast::LitNil)
        },
    }
}

fn decode_count_and_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                         field: &ast::StructField,
                         path_root: ast::Ident) -> (Gc<ast::Expr>, Gc<ast::Expr>) {
    let modifier = find_modifier(cx, span, field.node.attrs.as_slice());
    match field.node.ty.node {
        ast::TyPath(ref p, _, _) => (
            cx.expr_lit(span, ast::LitInt(1, ast::UnsuffixedIntLit(ast::Plus))),
            decode_type(cx, span, &p.segments[0].identifier, modifier, path_root),
        ),
        ast::TyFixedLengthVec(pty, expr) => (expr, match pty.node {
            ast::TyPath(ref p, _, _) => {
                decode_type(cx, span, &p.segments[0].identifier, modifier, path_root)
            },
            _ => {
                cx.span_err(span, format!("Unsupported fixed vector sub-type: \
                                          `{}`",pty.node).as_slice());
                cx.expr_lit(span, ast::LitNil)
            },
        }),
        _ => {
            cx.span_err(span, format!("Unsupported attribute type: `{}`",
                                      field.node.ty.node).as_slice());
            (cx.expr_lit(span, ast::LitNil), cx.expr_lit(span, ast::LitNil))
        },
    }
}

/// Generates the the method body for `gfx::VertexFormat::generate`.
fn method_body(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                   substr: &generic::Substructure,
                   path_root: ast::Ident) -> Gc<ast::Expr> {
    match *substr.fields {
        generic::StaticStruct(ref definition, generic::Named(ref fields)) => {
            let attribute_pushes = definition.fields.iter().zip(fields.iter())
                .map(|(def, &(ident, _))| {
                    let struct_ident = substr.type_ident;
                    let buffer_expr = substr.nonself_args[1];
                    let (count_expr, type_expr) = decode_count_and_type(cx, span, def, path_root);
                    let ident_str = match super::find_name(cx, span, def.node.attrs.as_slice()) {
                        Some(name) => name,
                        None => token::get_ident(ident),
                    };
                    let ident_str = ident_str.get();
                    let instance_expr = get_instance_rate(cx, span, def.node.attrs.as_slice());
                    super::ugh(cx, |cx| quote_expr!(cx, {
                        attributes.push($path_root::gfx::Attribute {
                            buffer: $buffer_expr,
                            elem_count: $count_expr,
                            elem_type: $type_expr,
                            offset: unsafe {
                                &(*(0u as *const $struct_ident)).$ident as *const _ as $path_root::gfx::attrib::Offset
                            },
                            stride: { use std::mem; mem::size_of::<$struct_ident>() as $path_root::gfx::attrib::Stride },
                            name: $ident_str.to_string(),
                            instance_rate: $instance_expr,
                        });
                    }))
                }).collect::<Vec<Gc<ast::Expr>>>();
            let capacity = fields.len();
            super::ugh(cx, |cx| quote_expr!(cx, {
                let mut attributes = Vec::with_capacity($capacity);
                $attribute_pushes;
                attributes
            }))
        },
        _ => {
            cx.span_err(span, "Unable to implement `gfx::VertexFormat::generate` \
                              on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        }
    }
}


/// Derive a `gfx::VertexFormat` implementation for the `struct`
pub fn expand(context: &mut ext::base::ExtCtxt, span: codemap::Span,
              meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>,
              push: |Gc<ast::Item>|) {
    // Insert the `gfx` reexport module
    let path_root = super::extern_crate_hack(context, span, |i| push(i));
    let fixup = |item| {
        push(super::fixup_extern_crate_paths(item, path_root))
    };

    // `impl gfx::VertexFormat for $item`
    generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec![super::EXTERN_CRATE_HACK, "gfx", "VertexFormat"],
            lifetime: None,
            params: Vec::new(),
            global: true,
        },
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
            // `fn generate(Option<Self>, gfx::RawBufferHandle) -> Vec<gfx::Attribute>`
            generic::MethodDef {
                name: "generate",
                generics: generic::ty::LifetimeBounds::empty(),
                explicit_self: None,
                args: vec![
                    generic::ty::Literal(generic::ty::Path {
                        path: vec!["Option"],
                        lifetime: None,
                        params: vec![box generic::ty::Self],
                        global: false,
                    }),
                    generic::ty::Literal(generic::ty::Path::new(
                        vec![super::EXTERN_CRATE_HACK, "gfx", "RawBufferHandle"]
                    )),
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Vec"],
                        lifetime: None,
                        params: vec![
                            box generic::ty::Literal(generic::ty::Path::new(
                                vec![super::EXTERN_CRATE_HACK, "gfx", "Attribute"])),
                        ],
                        global: false,
                    },
                ),
                attributes: Vec::new(),
                // generate the method body
                combine_substructure: generic::combine_substructure(
                    |c, s, ss| method_body(c, s, ss, path_root)),
            },
        ],
    }.expand(context, meta_item, item, fixup);
}
