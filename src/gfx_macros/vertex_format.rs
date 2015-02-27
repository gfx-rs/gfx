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

use std::fmt;
use std::str::FromStr;
use syntax::{ast, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::{attr, codemap};
use syntax::parse::token;
use syntax::ptr::P;
use syntax::ext::base::ItemDecorator;

/// A component modifier.
#[derive(Copy, PartialEq)]
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

impl fmt::Debug for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Modifier::Normalized => write!(f, "normalized"),
            Modifier::AsFloat => write!(f, "as_float"),
            Modifier::AsDouble => write!(f, "as_double"),
        }
    }
}

impl FromStr for Modifier {
    type Err = ();

    fn from_str(src: &str) -> Result<Modifier, ()> {
        match src {
            "normalized" => Ok(Modifier::Normalized),
            "as_float" => Ok(Modifier::AsFloat),
            "as_double" => Ok(Modifier::AsDouble),
            _ => Err(()),
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
                word.parse().ok().and_then(|new_modifier| {
                    attr::mark_used(attribute);
                    modifier.map_or(Some(new_modifier), |modifier| {
                        cx.span_warn(span, &format!(
                            "Extra attribute modifier detected: `#[{:?}]` - \
                            ignoring in favour of `#[{:?}]`.", new_modifier, modifier
                        ));
                        None
                    })
                }).or(modifier)
            },
            _ => modifier,
        }
    })
}

/// Find a `gfx::attrib::Type` that describes the given type identifier.
fn decode_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               ty_ident: &ast::Ident, modifier: Option<Modifier>,
               path_root: ast::Ident) -> P<ast::Expr> {
    let ty_str = ty_ident.name.as_str();
    match ty_str {
        "f32" | "f64" => {
            let kind = cx.ident_of(match modifier {
                None | Some(Modifier::AsFloat) => "Default",
                Some(Modifier::AsDouble) => "Precision",
                Some(Modifier::Normalized) => {
                    cx.span_warn(span, &format!(
                        "Incompatible float modifier attribute: `#[{:?}]`", modifier
                    ));
                    ""
                }
            });
            let size = cx.ident_of(&format!("F{}", ty_str.slice_from(1)));
            quote_expr!(cx, $path_root::gfx::attrib::Type::Float($path_root::gfx::attrib::FloatSubType::$kind,
                                                                 $path_root::gfx::attrib::FloatSize::$size))
        },
        "u8" | "u16" | "u32" | "u64" |
        "i8" | "i16" | "i32" | "i64" => {
            let sign = cx.ident_of({
                if ty_str.starts_with("i") { "Signed" } else { "Unsigned" }
            });
            let kind = cx.ident_of(match modifier {
                None => "Raw",
                Some(Modifier::Normalized) => "Normalized",
                Some(Modifier::AsFloat) => "AsFloat",
                Some(Modifier::AsDouble) => {
                    cx.span_warn(span, &format!(
                        "Incompatible int modifier attribute: `#[{:?}]`", modifier
                    ));
                    ""
                }
            });
            let size = cx.ident_of(&format!("U{}", ty_str.slice_from(1)));
            quote_expr!(cx, $path_root::gfx::attrib::Type::Int($path_root::gfx::attrib::IntSubType::$kind,
                                                               $path_root::gfx::attrib::IntSize::$size,
                                                               $path_root::gfx::attrib::SignFlag::$sign))
        },
        ty_str => {
            cx.span_err(span, &format!("Unrecognized component type: `{:?}`",
                                      ty_str));
            cx.expr_tuple(span, vec![])
        },
    }
}

fn decode_count_and_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                         field: &ast::StructField,
                         path_root: ast::Ident) -> (P<ast::Expr>, P<ast::Expr>) {
    let modifier = find_modifier(cx, span, &field.node.attrs);
    match field.node.ty.node {
        ast::TyPath(_,ref p) => (
            cx.expr_lit(span, ast::LitInt(1, ast::UnsuffixedIntLit(ast::Plus))),
            decode_type(cx, span, &p.segments[0].identifier, modifier, path_root),
        ),
        ast::TyFixedLengthVec(ref pty, ref expr) => (expr.clone(), match pty.node {
            ast::TyPath(_,ref p) => {
                decode_type(cx, span, &p.segments[0].identifier, modifier, path_root)
            },
            _ => {
                cx.span_err(span, &format!("Unsupported fixed vector sub-type: \
                                          `{:?}`",pty.node));
                cx.expr_tuple(span, vec![])
            },
        }),
        _ => {
            cx.span_err(span, &format!("Unsupported attribute type: `{:?}`",
                                      field.node.ty.node));
            (cx.expr_tuple(span, vec![]), cx.expr_tuple(span, vec![]))
        },
    }
}

/// Generates the the method body for `gfx::VertexFormat::generate`.
fn method_body(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                   substr: &generic::Substructure,
                   path_root: ast::Ident) -> P<ast::Expr> {
    match *substr.fields {
        generic::StaticStruct(ref definition, generic::Named(ref fields)) => {
            let attribute_pushes = definition.fields.iter().zip(fields.iter())
                .map(|(def, &(ident, _))| {
                    let struct_ident = substr.type_ident;
                    let buffer_expr = &substr.nonself_args[1];
                    let (count_expr, type_expr) = decode_count_and_type(cx, span, def, path_root);
                    let ident_str = match super::find_name(cx, span, &def.node.attrs) {
                        Some(name) => name,
                        None => token::get_ident(ident),
                    };
                    let ident_str = &ident_str[..];
                    let instance_expr = cx.expr_u8(span, 0); // not supposed to be set by the macro
                    quote_expr!(cx, {
                        attributes.push($path_root::gfx::Attribute {
                            name: $ident_str.to_string(),
                            buffer: $buffer_expr,
                            format: $path_root::gfx::attrib::Format {
                                elem_count: $count_expr,
                                elem_type: $type_expr,
                                offset: unsafe {
                                    let x: $struct_ident = ::std::mem::uninitialized();
                                    let offset = (&x.$ident as *const _ as usize) -
                                        (&x as *const _ as usize);
                                    ::std::mem::forget(x);
                                    offset as $path_root::gfx::attrib::Offset
                                },
                                stride: { use std::mem;
                                    mem::size_of::<$struct_ident>() as
                                        $path_root::gfx::attrib::Stride
                                },
                                instance_rate: $instance_expr,
                            }
                        });
                    })
                }).collect::<Vec<P<ast::Expr>>>();
            let capacity = fields.len();
            quote_expr!(cx, {
                let mut attributes = Vec::with_capacity($capacity);
                $attribute_pushes;
                attributes
            })
        },
        _ => {
            cx.span_err(span, "Unable to implement `gfx::VertexFormat::generate` \
                              on a non-structure");
            cx.expr_tuple(span, vec![])
        }
    }
}

#[derive(Copy)]
pub struct VertexFormat;

impl ItemDecorator for VertexFormat {
    /// Derive a `gfx::VertexFormat` implementation for the `struct`
    fn expand(&self, context: &mut ext::base::ExtCtxt, span: codemap::Span,
              meta_item: &ast::MetaItem, item: &ast::Item,
              push: &mut FnMut(P<ast::Item>)) {
        // Insert the `gfx` reexport module
        let path_root = super::extern_crate_hack(context, span, |i| (*push)(i));
        let fixup = |item| {
            (*push)(super::fixup_extern_crate_paths(item, path_root))
        };

        // `impl<R: gfx::Resources> gfx::VertexFormat for $item`
        generic::TraitDef {
            span: span,
            attributes: Vec::new(),
            path: generic::ty::Path::new(
                vec![super::EXTERN_CRATE_HACK, "gfx", "VertexFormat"],
            ),
            additional_bounds: Vec::new(),
            generics: generic::ty::LifetimeBounds::empty(),
            methods: vec![
                // `fn generate(Option<Self>, gfx::RawBufferHandle) -> Vec<gfx::Attribute>`
                generic::MethodDef {
                    name: "generate",
                    generics: generic::ty::LifetimeBounds {
                        lifetimes: Vec::new(),
                        bounds: vec![
                            ("R", vec![
                                generic::ty::Path::new(vec![
                                    super::EXTERN_CRATE_HACK, "gfx", "Resources"
                                ]),
                            ]),
                        ],
                    },
                    explicit_self: None,
                    args: vec![
                        generic::ty::Literal(generic::ty::Path {
                            path: vec!["Option"],
                            lifetime: None,
                            params: vec![box generic::ty::Ptr(
                                box generic::ty::Self_,
                                generic::ty::PtrTy::Borrowed(None, ast::Mutability::MutImmutable)
                            )],
                            global: false,
                        }),
                        generic::ty::Literal(generic::ty::Path {
                            path: vec![super::EXTERN_CRATE_HACK, "gfx", "RawBufferHandle"],
                            lifetime: None,
                            params: vec![box generic::ty::Literal(generic::ty::Path::new_local("R"))],
                            global: false,
                        }),
                    ],
                    ret_ty: generic::ty::Literal(
                        generic::ty::Path {
                            path: vec!["Vec"],
                            lifetime: None,
                            params: vec![
                                box generic::ty::Literal(generic::ty::Path {
                                    path: vec![super::EXTERN_CRATE_HACK, "gfx", "Attribute"],
                                    lifetime: None,
                                    params: vec![box generic::ty::Literal(
                                        generic::ty::Path::new_local("R")
                                    )],
                                    global: false,
                                }),
                            ],
                            global: false,
                        },
                    ),
                    attributes: Vec::new(),
                    // generate the method body
                    combine_substructure: generic::combine_substructure(
                        box |c, s, ss| method_body(c, s, ss, path_root)),
                },
            ],
            associated_types: Vec::new(),
        }.expand(context, meta_item, item, fixup);
    }
}
