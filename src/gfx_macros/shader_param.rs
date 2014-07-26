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
use syntax::{codemap, owned_slice};
use syntax::parse::token;


enum ParamType {
    ParamUniform,
    ParamBlock,
    ParamTexture,
}

/// Classify variable types (`i32`, `TextureHandle`, etc) into the `ParamType`
fn classify(node: &ast::Ty_) -> ParamType {
    match *node {
        ast::TyPath(ref path, _, _) => match path.segments.last() {
            Some(segment) => match segment.identifier.name.as_str() {
                "BufferHandle" => ParamBlock,
                "TextureParam" => ParamTexture,
                _ => ParamUniform,
            },
            None => ParamUniform,
        },
        _ => ParamUniform,
    }
}

/// `create_link()` method generating code
fn method_create(cx: &mut ext::base::ExtCtxt, span: codemap::Span, substr: &generic::Substructure,
                 definition: Gc<ast::StructDef>, link_name: &str) -> Gc<ast::Expr> {
    let link_ident = cx.ident_of(link_name);
    match *substr.fields {
        //generic::StaticStruct(definition, generic::Named(ref fields)) => {
        generic::Struct(ref fields) => {
            let out = definition.fields.iter().zip(fields.iter()).map(|(def, f)| {
                let (fun, ret) = match classify(&def.node.ty.node) {
                    ParamUniform => ("find_uniform", "ErrorUniform"),
                    ParamBlock   => ("find_block",   "ErrorBlock"),
                    ParamTexture => ("find_texture", "ErrorTexture"),
                };
                let id_ret = cx.ident_of(ret);
                let expr_field = cx.expr_str(span, token::get_ident(f.name.unwrap()));
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
                cx.field_imm(f.span, f.name.unwrap(), unwrap)
            }).collect();
            cx.expr_ok(span, cx.expr_struct_ident(span, link_ident, out))
        },
        _ => {
            cx.span_err(span, "Unable to implement `create_link()` on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        },
    }
}

/// `upload()` method generating code
fn method_upload(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                 substr: &generic::Substructure, definition: Gc<ast::StructDef>)
                 -> Gc<ast::Expr> {
    match *substr.fields {
        generic::Struct(ref fields) => {
            let calls = definition.fields.iter().zip(fields.iter()).map(|(def, f)| {
                let (arg_id, value) = match classify(&def.node.ty.node) {
                    ParamUniform => {
                        let value = cx.expr_method_call(
                            span,
                            f.self_,
                            cx.ident_of("to_uniform"),
                            Vec::new()
                        );
                        (1u, value)
                    },
                    ParamBlock   => {
                        (2u, f.self_)
                    },
                    ParamTexture => {
                        (3u, f.self_)
                    },
                };
                let expr_id = cx.expr_field_access(span, substr.nonself_args[0], f.name.unwrap());
                cx.stmt_expr(cx.expr_call(span, substr.nonself_args[arg_id], vec![expr_id, value]))
            }).collect();
            let view = cx.view_use_simple(
                span,
                ast::Inherited,
                cx.path(span, vec![cx.ident_of("gfx"), cx.ident_of("ToUniform")])
            );
            cx.expr_block(cx.block_all(span, vec![view], calls, None))
        },
        _ => {
            cx.span_err(span, "Unable to implement `upload()` on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        }
    }
}

/// A helper function that translates variable type (`i32`, `TextureHandle`, etc)
/// into the corresponding shader var id type (`VarUniform`, `VarTexture`, etc)
fn node_to_var_path(span: codemap::Span, node: &ast::Ty_) -> ast::Path {
    let id = match classify(node) {
        ParamUniform => "VarUniform",
        ParamBlock   => "VarBlock",
        ParamTexture => "VarTexture",
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

/// Decorator for `shader_param` attribute
pub fn expand(context: &mut ext::base::ExtCtxt, span: codemap::Span,
              meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>,
              push: |Gc<ast::Item>|) {
    // constructing the Link struct
    let (base_def, link_def) = match item.node {
        ast::ItemStruct(definition, ref generics) => {
            if generics.lifetimes.len() > 0 {
                context.bug("Generics are not allowed in ShaderParam struct");
            }
            (definition, ast::StructDef {
                fields: definition.fields.
                    iter().map(|f| codemap::Spanned {
                        node: ast::StructField_ {
                            kind: f.node.kind,
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
            })
        },
        _ => {
            context.span_warn(span, "Only free-standing named structs allowed to derive ShaderParam");
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
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
            generic::MethodDef {
                name: "create_link",
                generics: generic::ty::LifetimeBounds {
                    lifetimes: Vec::new(),
                    bounds: vec![("S", None, vec![
                        generic::ty::Path::new(vec!["gfx", "ParameterSink"])
                    ])],
                },
                explicit_self: Some(Some(generic::ty::Borrowed(
                    None, ast::MutImmutable
                ))),
                args: vec![
                    generic::ty::Ptr(
                        box generic::ty::Literal(generic::ty::Path::new_local("S")),
                        generic::ty::Borrowed(None, ast::MutMutable)
                    )
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Result"],
                        lifetime: None,
                        params: vec![
                            link_ty.clone(),
                            box generic::ty::Literal(generic::ty::Path {
                                path: vec!["gfx", "ParameterError"],
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
                    method_create(cx, span, sub, base_def, link_name.as_slice())
                ),
            },
            generic::MethodDef {
                name: "upload",
                generics: generic::ty::LifetimeBounds {
                    lifetimes: vec!["'a"],
                    bounds: Vec::new(),
                },
                explicit_self: Some(Some(generic::ty::Borrowed(
                    None, ast::MutImmutable
                ))),
                args: vec![
                    generic::ty::Ptr(
                        link_ty.clone(),
                        generic::ty::Borrowed(None, ast::MutImmutable)
                    ),
                    generic::ty::Literal(
                        generic::ty::Path::new_(vec!["gfx", "FnUniform"], Some("'a"), Vec::new(), true),
                    ),
                    generic::ty::Literal(
                        generic::ty::Path::new_(vec!["gfx", "FnBlock"],   Some("'a"), Vec::new(), true),
                    ),
                    generic::ty::Literal(
                        generic::ty::Path::new_(vec!["gfx", "FnTexture"], Some("'a"), Vec::new(), true),
                    ),
                ],
                ret_ty: generic::ty::Tuple(Vec::new()),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_upload(cx, span, sub, base_def)
                ),
            },
        ],
    };
    trait_def.expand(context, meta_item, item, push);
}
