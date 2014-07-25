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

use std::gc::Gc;
use syntax::{ast, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::codemap;
use syntax::parse::token;


pub static ATTRIB_NORMALIZED: &'static str = "normalized";
pub static ATTRIB_AS_FLOAT  : &'static str = "as_float";
pub static ATTRIB_AS_DOUBLE : &'static str = "as_double";

pub fn attribute_modifier(_cx: &mut ext::base::ExtCtxt, _span: codemap::Span,
                          _meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>) -> Gc<ast::Item> {
    item
}

fn make_path_vec(cx: &mut ext::base::ExtCtxt, to: &str) -> Vec<ast::Ident> {
    vec![cx.ident_of("gfx"), cx.ident_of("attrib"), cx.ident_of(to)]
}

fn make_path_expr(cx: &mut ext::base::ExtCtxt, span: codemap::Span, to: &str) -> Gc<ast::Expr> {
    let vec = make_path_vec(cx, to);
    cx.expr_path(cx.path(span, vec))
}

#[deriving(PartialEq, Show)]
enum Modifier {
    ModNone,
    ModNormalized,
    ModAsFloat,
    ModAsDouble,
}

/// Scan through the field's attributes and extract a relevant modifier
fn find_modifier(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                 attributes: &[ast::Attribute]) -> Modifier {
    attributes.iter().fold(ModNone, |md, at| {
        match at.node.value.node {
            ast::MetaWord(ref s) => match (md, s.get()) {
                (ModNone, ATTRIB_NORMALIZED) => ModNormalized,
                (ModNone, ATTRIB_AS_FLOAT)   => ModAsFloat,
                (ModNone, ATTRIB_AS_DOUBLE)  => ModAsDouble,
                (_, ATTRIB_NORMALIZED) | (_, ATTRIB_AS_FLOAT) | (_, ATTRIB_AS_DOUBLE) => {
                    cx.span_warn(span, format!(
                        "Extra attribute modifier detected: {}",
                        s.get()).as_slice()
                        );
                    md
                },
                _ => md,
            },
            _ => md,
        }
    })
}

/// Find a gfx::attrib::Type that describes a given type
fn decode_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               stype: &str, modifier: Modifier) -> Gc<ast::Expr> {
    let c0 = stype.char_at(0);
    let (type_vec, params) = match c0 {
        'f' => {
            let kind = match modifier {
                ModNone => "FloatDefault",
                ModAsDouble => "FloatPrecision",
                _ => {
                    cx.span_warn(span, format!(
                        "Incompatible float modifier: {}",
                        modifier).as_slice()
                        );
                    ""
                }
            };
            let sub_type = format!("F{}", stype.slice_from(1));
            (make_path_vec(cx, "Float"), vec![
                make_path_expr(cx, span, kind),
                make_path_expr(cx, span, sub_type.as_slice())
            ])
        },
        'u' | 'i' => {
            let sign = if c0=='i' {"Signed"} else {"Unsigned"};
            let kind = match modifier {
                ModNone => "IntRaw",
                ModNormalized => "IntNormalized",
                ModAsFloat => "IntAsFloat",
                _ => {
                    cx.span_warn(span, format!(
                        "Incompatible int modifier: {}",
                        modifier).as_slice()
                        );
                    ""
                }
            };
            let sub_type = format!("U{}", stype.slice_from(1));
            (make_path_vec(cx, "Int"), vec![
                make_path_expr(cx, span, kind),
                make_path_expr(cx, span, sub_type.as_slice()),
                make_path_expr(cx, span, sign)
            ])
        },
        _ => {
            cx.span_err(span, format!(
                "Unrecognized element type: {}",
                stype).as_slice()
                );
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
            decode_type(cx, span,
                p.segments[0].identifier.name.as_str(),
                modifier)
        ),
        ast::TyFixedLengthVec(pty, expr) => (expr, match pty.node {
            ast::TyPath(ref p, _, _) => decode_type(cx, span,
                p.segments[0].identifier.name.as_str(),
                modifier),
            _ => {
                cx.span_err(span, format!(
                    "Unsupported fixed vector sub-type: {}",
                    pty.node).as_slice()
                );
                cx.expr_lit(span, ast::LitNil)
            },
        }),
        _ => {
            cx.span_err(span, format!(
                    "Unsupported attribute type: {}",
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
            statements.push(cx.stmt_let(span, true, id_at, cx.expr_vec_ng(span)));
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
                        cx.ident_of("mesh"),
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
            path: vec!["gfx", "mesh", "VertexFormat"],
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
                                vec!["gfx", "mesh", "Attribute"])),
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
