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

extern crate rustc;
extern crate syntax;

use std::gc::Gc;
use self::syntax::{ast, ext};
use self::syntax::ext::build::AstBuilder;
use self::syntax::ext::deriving::generic;
use self::syntax::{codemap, owned_slice};
use self::syntax::parse::token;
use self::rustc::plugin::Registry;


static FATAL_ERROR: &'static str = "Only free-standing named structs allowed to derive ShaderParam";

enum ParamType {
    ParamBlock,
    ParamUniform,
    ParamTexture,
}

fn classify(node: &ast::Ty_) -> ParamType {
    match *node {
        ast::TyPath(ref path, _, _) => match path.segments.last() {
            Some(segment) => match segment.identifier.name.as_str() {
                "BufferHandle" => ParamBlock,
                "TextureHandle" => ParamTexture,
                _ => ParamUniform,
            },
            None => ParamUniform,
        },
        _ => ParamUniform,
    }
}

fn method_create(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
        substr: &generic::Substructure, link_name: &str) -> Gc<ast::Expr> {
    let link_ident = cx.ident_of(link_name);
    match *substr.fields {
        generic::StaticStruct(definition, generic::Named(ref fields)) => {
            let out = definition.fields.iter().zip(fields.iter()).map(|(field, &(ident, s))| {
                let (fun, ret) = match classify(&field.node.ty.node) {
                    ParamBlock   => ("find_block",   "LinkMissingBlock"),
                    ParamUniform => ("find_uniform", "LinkMissingUniform"),
                    ParamTexture => ("find_texture", "LinkMissingTexture"),
                };
                let id_ret = cx.ident_of(ret);
                let expr_field = cx.expr_str(span, token::get_ident(ident));
                let value = cx.expr_method_call(
                    span, substr.nonself_args[0], cx.ident_of(fun),
                    vec![expr_field.clone()]
                );
                let error = cx.expr_call(span,
                    cx.expr_path(cx.path_global(span,
                        vec![cx.ident_of("gfx"), id_ret])
                    ),
                    vec![expr_field]
                );
                let some_ident = cx.ident_of("_p");
                let unwrap = cx.expr_match(span, value, vec![
                    cx.arm(span,
                        vec![cx.pat_enum(span, cx.path_ident(span, cx.ident_of("Some")),
                            vec![cx.pat_ident(span, some_ident)])
                        ],
                        cx.expr_ident(span, some_ident)
                    ),
                    cx.arm(span,
                        vec![cx.pat_enum(span,
                            cx.path_ident(span, cx.ident_of("None")),
                            Vec::new())
                        ],
                        cx.expr(span, ast::ExprRet(
                            Some(cx.expr_err(span, error))
                        ))
                    )
                ]);
                cx.field_imm(s, ident, unwrap)
            }).collect();
            cx.expr_ok(span, cx.expr_struct_ident(span, link_ident, out))
        },
        _ => {
            cx.span_err(span, FATAL_ERROR);
            cx.expr_err(span, cx.expr_path(
                cx.path_global(span, vec![
                    cx.ident_of("gfx"),
                    cx.ident_of("LinkInternalError"),
                ])
            ))
        },
    }
}

fn method_upload(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
        substr: &generic::Substructure) -> Gc<ast::Expr> {
    match *substr.fields {
        generic::Struct(ref fields) => {
            let calls = fields.iter().map(|f| {
                cx.stmt_expr(cx.expr_lit(span, ast::LitNil))
            }).collect();
            cx.expr_block(cx.block(span, calls, None))
        },
        _ => {
            cx.span_err(span, FATAL_ERROR);
            cx.expr_lit(span, ast::LitNil)
        }
    }
}

fn node_to_var_path(span: codemap::Span, node: &ast::Ty_) -> ast::Path {
    let id = match classify(node) {
        ParamBlock => "BlockVarId",
        ParamUniform => "UniformVarId",
        ParamTexture => "TextureVarId",
    };
    ast::Path {
        span: span,
        global: true,
        segments: vec![
            ast::PathSegment {
                identifier: ast::Ident::new(token::intern("gfx")),
                lifetimes: Vec::new(),
                types: owned_slice::OwnedSlice::empty(),
            },
            ast::PathSegment {
                identifier: ast::Ident::new(token::intern(id)),
                lifetimes: Vec::new(),
                types: owned_slice::OwnedSlice::empty(),
            },
        ],
    }
}

fn expand_shader_param(context: &mut ext::base::ExtCtxt, span: codemap::Span,
        meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>, push: |Gc<ast::Item>|) {
    // constructing the Link struct
    let link_def = match item.node {
        ast::ItemStruct(ref definition, ref generics) => {
            if generics.lifetimes.len() > 0 {
                context.bug("Generics are not allowed in ShaderParam struct");
            }
            ast::StructDef {
                fields: definition.fields.
                    iter().map(|f| codemap::Spanned {
                        node: ast::StructField_ {
                            kind: f.node.kind,  //TODO
                            id: f.node.id,
                            ty: context.ty_path(
                                node_to_var_path(f.span, &f.node.ty.node),
                                None
                            ),
                            attrs: Vec::new(),
                        },
                        span: f.span,
                    }).collect(),
                ctor_id: None,
                super_struct: None,
                is_virtual: false,
            }
        },
        _ => {
            context.span_warn(span, FATAL_ERROR);
            return;
        }
    };
    let link_name = format!("_{}Link", item.ident.as_str());
    let link_ty = box generic::ty::Literal(
        generic::ty::Path::new_local(link_name.as_slice())
        );
    push(context.item_struct(span, ast::Ident::new(
        token::intern(link_name.as_slice())
        ), link_def));
    // deriving ShaderParam
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec!["gfx", "ShaderParam"],
            lifetime: None,
            params: vec![link_ty.clone()],
            global: true,
        },
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds {
            lifetimes: Vec::new(),
            bounds: Vec::new(),
        },
        methods: vec![
            generic::MethodDef {
                name: "create_link",
                generics: generic::ty::LifetimeBounds {
                    lifetimes: Vec::new(),
                    bounds: vec![("S", None, vec![
                        generic::ty::Path::new(vec!["gfx", "ParameterSink"])
                    ])],
                },
                explicit_self: None,
                args: vec![
                    generic::ty::Ptr(
                        box generic::ty::Literal(generic::ty::Path::new_local("S")),
                        generic::ty::Borrowed(None, ast::MutImmutable)
                    )
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Result"],
                        lifetime: None,
                        params: vec![
                            link_ty.clone(),
                            box generic::ty::Literal(generic::ty::Path {
                                path: vec!["gfx", "ParameterLinkError"],
                                lifetime: Some("'static"),
                                params: Vec::new(),
                                global: true,
                            })
                        ],
                        global: false,
                    },
                ),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_create(cx, span, sub, link_name.as_slice())
                ),
            },
            generic::MethodDef {
                name: "upload",
                generics: generic::ty::LifetimeBounds {
                    lifetimes: Vec::new(),
                    bounds: vec![("U", None, vec![
                        generic::ty::Path::new(vec!["gfx", "Uploader"])
                    ])],
                },
                explicit_self: Some(Some(generic::ty::Borrowed(
                    None, ast::MutImmutable
                ))),
                args: vec![
                    generic::ty::Ptr(
                        link_ty.clone(),
                        generic::ty::Borrowed(None, ast::MutImmutable)
                    ),
                    generic::ty::Ptr(
                        box generic::ty::Literal(generic::ty::Path::new_local("U")),
                        generic::ty::Borrowed(None, ast::MutMutable)
                    ),
                ],
                ret_ty: generic::ty::Tuple(Vec::new()),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_upload(cx, span, sub)
                ),
            },
        ],
    };
    trait_def.expand(context, meta_item, item, push);
}

#[plugin_registrar]
pub fn registrar(reg: &mut Registry) {
    reg.register_syntax_extension(token::intern("shader_param"),
        ext::base::ItemDecorator(expand_shader_param));
}
