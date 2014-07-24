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

fn decode_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span, ty: &str) -> Gc<ast::Expr> {
    cx.expr_lit(span, ast::LitIntUnsuffixed(0))
}

fn decode_count_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span, ty: &ast::Ty_) -> (Gc<ast::Expr>, Gc<ast::Expr>) {
    let (count, etype) = match *ty {
        ast::TyPath(ref p, _, _) => (1, decode_type(cx, span, p.segments[0].identifier.name.as_str())),
        ast::TyFixedLengthVec(pty, expr) => {
            let t = match pty.node {
                ast::TyPath(ref p, _, _) => decode_type(cx, span, p.segments[0].identifier.name.as_str()),
                _ => {
                    cx.span_err(span, format!(
                        "Unsupported fixed vector sub-type: {}",
                        pty.node).as_slice()
                    );
                    cx.expr_lit(span, ast::LitNil)
                },
            };
            let c = match expr.node {
                ast::ExprLit(plit) => match plit.node {
                    ast::LitIntUnsuffixed(value) => value,
                    _ => {
                        cx.span_err(span, format!(
                            "Unsupported vector size literal: {}",
                            plit.node).as_slice()
                        );
                        0
                    },
                },
                _ => {
                    cx.span_err(span, format!(
                        "Unsupported vector size: {}",
                        expr.node).as_slice()
                    );
                    0
                },
            };
            (c, t)
        },
        _ => {
            cx.span_err(span, format!(
                "Unsupported attribute type: {}",
                ty).as_slice()
            );
            (0, cx.expr_lit(span, ast::LitNil))
        },
    };
    (cx.expr_lit(span, ast::LitIntUnsuffixed(count)), etype)
}

/// `generate()` method generating code
fn method_generate(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                   substr: &generic::Substructure) -> Gc<ast::Expr> {
    match *substr.fields {
        generic::StaticStruct(ref definition, generic::Named(ref fields)) => {
            let mut statements = Vec::new();
            let id_at = cx.ident_of("at");
            statements.push(cx.stmt_let(span, true, id_at, cx.expr_vec_ng(span)));
            let ex_offset = cx.expr_lit(span, ast::LitIntUnsuffixed(0));    //TODO
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
                cx.ty_path(cx.path_global(span, vec![
                    cx.ident_of("gfx"),
                    cx.ident_of("attrib"),
                    cx.ident_of("Stride")
                    ]), None)
            );
            for (def, &(ident, _)) in definition.fields.iter().zip(fields.iter()) {
                let (ex_count, ex_type) = decode_count_type(cx, span, &def.node.ty.node);
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
