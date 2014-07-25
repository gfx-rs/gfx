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
use std::from_str::FromStr;
use std::gc::Gc;
use syntax::{ast, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::{attr, codemap};
use syntax::parse::token;

fn make_path_vec(cx: &mut ext::base::ExtCtxt, to: &str) -> Vec<ast::Ident> {
    vec![cx.ident_of("gfx"), cx.ident_of("attrib"), cx.ident_of(to)]
}

fn make_path_expr(cx: &mut ext::base::ExtCtxt, span: codemap::Span, to: &str) -> Gc<ast::Expr> {
    let vec = make_path_vec(cx, to);
    cx.expr_path(cx.path(span, vec))
}

#[deriving(PartialEq)]
enum Modifier {
    Normalized,
    AsFloat,
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

impl FromStr for Modifier {
    fn from_str(src: &str) -> Option<Modifier> {
        match src {
            "normalized" => Some(Normalized),
            "as_float" => Some(AsFloat),
            "as_double" => Some(AsDouble),
            _ => None,
        }
    }
}

/// Scan through the field's attributes and extract a relevant modifier
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

/// Find a gfx::attrib::Type that describes a given type
fn decode_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               ty_ident: &ast::Ident, modifier: Option<Modifier>) -> Gc<ast::Expr> {
    let ty_str = ty_ident.name.as_str();
    let (type_vec, params) = match ty_str {
        "f32" | "f64" => {
            let kind = match modifier {
                None | Some(AsFloat) => "FloatDefault",
                Some(AsDouble) => "FloatPrecision",
                Some(Normalized) => {
                    cx.span_warn(span, format!(
                        "Incompatible float modifier attribute: `#[{}]`", modifier
                    ).as_slice());
                    ""
                }
            };
            let sub_type = format!("F{}", ty_str.slice_from(1));
            (make_path_vec(cx, "Float"), vec![
                make_path_expr(cx, span, kind),
                make_path_expr(cx, span, sub_type.as_slice())
            ])
        },
        "u8" | "u16" | "u32" | "u64" | "uint" |
        "i8" | "i16" | "i32" | "i64" | "int" => {
            let sign = if ty_str.starts_with("i") { "Signed" } else { "Unsigned" };
            let kind = match modifier {
                None => "IntRaw",
                Some(Normalized) => "IntNormalized",
                Some(AsFloat) => "IntAsFloat",
                Some(AsDouble) => {
                    cx.span_warn(span, format!(
                        "Incompatible int modifier attribute: `#[{}]`", modifier
                    ).as_slice());
                    ""
                }
            };
            let sub_type = format!("U{}", ty_str.slice_from(1));
            (make_path_vec(cx, "Int"), vec![
                make_path_expr(cx, span, kind),
                make_path_expr(cx, span, sub_type.as_slice()),
                make_path_expr(cx, span, sign)
            ])
        },
        ty_str => {
            cx.span_err(span, format!("Unrecognized component type: `{}`", ty_str).as_slice());
            (Vec::new(), Vec::new())
        },
    };
    cx.expr_call_global(span, type_vec, params)
}

fn decode_count_and_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                         field: &ast::StructField) -> (Gc<ast::Expr>, Gc<ast::Expr>) {
    let modifier = find_modifier(cx, span, field.node.attrs.as_slice());
    match field.node.ty.node {
        ast::TyPath(ref p, _, _) => (
            cx.expr_lit(span, ast::LitIntUnsuffixed(1)),
            decode_type(cx, span, &p.segments[0].identifier, modifier),
        ),
        ast::TyFixedLengthVec(pty, expr) => (expr, match pty.node {
            ast::TyPath(ref p, _, _) => {
                decode_type(cx, span, &p.segments[0].identifier, modifier)
            },
            _ => {
                cx.span_err(span, format!(
                    "Unsupported fixed vector sub-type: `{}`",
                    pty.node).as_slice()
                );
                cx.expr_lit(span, ast::LitNil)
            },
        }),
        _ => {
            cx.span_err(span, format!(
                    "Unsupported attribute type: `{}`",
                    field.node.ty.node
                ).as_slice()
            );
            (cx.expr_lit(span, ast::LitNil), cx.expr_lit(span, ast::LitNil))
        },
    }
}

/// Encode the following expression:
/// `unsafe { &(*(0u as *const $id_struct)).$id_field as *const _ as uint }`
fn make_offset_expr(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                    id_struct: ast::Ident, id_field: ast::Ident) -> Gc<ast::Expr> {
    let ex = cx.expr_cast(span,
        cx.expr_uint(span, 0),
        cx.ty(span, ast::TyPtr(ast::MutTy {
            ty: cx.ty_ident(span, id_struct),
            mutbl: ast::MutImmutable,
        }))
    );
    let ex = cx.expr(span, ast::ExprParen(
        cx.expr_deref(span, ex)
    ));
    let ex = cx.expr_addr_of(span,
        cx.expr_field_access(span, ex, id_field)
    );
    let ex = cx.expr_cast(span, ex,
        cx.ty(span, ast::TyPtr(ast::MutTy {
            ty: cx.ty_infer(span),
            mutbl: ast::MutImmutable
        }))
    );
    let offset = make_path_vec(cx, "Offset");
    let ex = cx.expr_cast(span, ex,
        cx.ty_path(cx.path(span, offset), None)
    );
    cx.expr_block(ast::P(ast::Block {
        view_items: Vec::new(),
        stmts: Vec::new(),
        expr: Some(ex),
        id: ast::DUMMY_NODE_ID,
        rules: ast::UnsafeBlock(ast::UserProvided),
        span: span,
    }))
}

/// `generate()` method generating code
fn method_generate(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                   substr: &generic::Substructure) -> Gc<ast::Expr> {
    match *substr.fields {
        generic::StaticStruct(ref definition, generic::Named(ref fields)) => {
            let mut statements = Vec::new();
            let id_at = cx.ident_of("at");
            let ex_new = cx.expr_call(span, cx.expr_path(cx.path(span,
                    vec![cx.ident_of("Vec"), cx.ident_of("with_capacity")]
                )), vec![cx.expr_uint(span, fields.len())]
            );
            statements.push(cx.stmt_let(span, true, id_at, ex_new));
            let path_stride = make_path_vec(cx, "Stride");
            let ex_stride = cx.expr_cast(span,
                cx.expr_call(span,
                    cx.expr_path(cx.path_all(
                        span,
                        true,
                        vec![
                            cx.ident_of("std"),
                            cx.ident_of("mem"),
                            cx.ident_of("size_of")
                        ],
                        Vec::new(),
                        vec![cx.ty_ident(span, substr.type_ident)]
                    )),
                    Vec::new()
                ),
                cx.ty_path(cx.path_global(span, path_stride), None)
            );
            for (def, &(ident, _)) in definition.fields.iter().zip(fields.iter()) {
                let (ex_count, ex_type) = decode_count_and_type(cx, span, def);
                let ex_offset = make_offset_expr(cx, span, substr.type_ident, ident);
                let ex_struct = cx.expr_struct(span,
                    cx.path(span, vec![
                        cx.ident_of("gfx"),
                        cx.ident_of("Attribute")
                        ]),
                    vec![
                        cx.field_imm(span, cx.ident_of("buffer"), substr.nonself_args[1]),
                        cx.field_imm(span, cx.ident_of("elem_count"), ex_count),
                        cx.field_imm(span, cx.ident_of("elem_type"), ex_type),
                        cx.field_imm(span, cx.ident_of("offset"), ex_offset),
                        cx.field_imm(span, cx.ident_of("stride"), ex_stride),
                        cx.field_imm(span, cx.ident_of("name"), cx.expr_method_call(span,
                            cx.expr_str(span, token::get_ident(ident)),
                            cx.ident_of("to_string"), Vec::new()))
                    ]
                );
                statements.push(cx.stmt_expr(cx.expr_method_call(
                    span,
                    cx.expr_ident(span, id_at),
                    cx.ident_of("push"),
                    vec![ex_struct]
                    )));
            }
            cx.expr_block(cx.block_all(
                span,
                Vec::new(),
                statements,
                Some(cx.expr_ident(span, id_at))
                ))
        },
        _ => {
            cx.span_err(span, "Unable to implement `generate()` on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        }
    }
}


/// Decorator for `vertex_format` attribute
pub fn expand_vertex_format(context: &mut ext::base::ExtCtxt, span: codemap::Span,
                            meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>,
                            push: |Gc<ast::Item>|) {
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec!["gfx", "VertexFormat"],
            lifetime: None,
            params: Vec::new(),
            global: true,
        },
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
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
                        vec!["gfx", "BufferHandle"]
                    )),
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Vec"],
                        lifetime: None,
                        params: vec![
                            box generic::ty::Literal(generic::ty::Path::new(
                                vec!["gfx", "Attribute"])),
                        ],
                        global: false,
                    },
                ),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(method_generate),
            },
        ],
    };
    trait_def.expand(context, meta_item, item, push);
}
