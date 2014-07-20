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


static INTERNAL_ERROR: &'static str = "Only free-standing named structs allowed to derive ShaderParam";
static LINK_RESULT_ERROR: &'static str = "ParameterLinkError";

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
    let error = cx.expr_path(cx.path_global(span, vec![
        cx.ident_of("gfx"),
        cx.ident_of(LINK_RESULT_ERROR),
    ]));
    match *substr.fields {
        generic::StaticStruct(definition, ref summary) => {
            match *summary {
                generic::Named(ref fields) => {
                    let out = definition.fields.iter().zip(fields.iter()).map(|(field, &(ident, s))| {
                        let fun = match classify(&field.node.ty.node) {
                            ParamBlock => "find_block",
                            ParamUniform => "find_uniform",
                            ParamTexture => "find_texture",
                        };
                        let value = cx.expr_method_call(
                            span, substr.nonself_args[0], cx.ident_of(fun),
                            vec![cx.expr_str(span, token::get_ident(ident))]
                        );
                        let unwrap = cx.expr_method_call(span, value,
                            cx.ident_of("unwrap"), Vec::new());
                        cx.field_imm(s, ident, unwrap)
                    }).collect();
                    cx.expr_ok(span, cx.expr_struct_ident(span, link_ident, out))
                },
                generic::Unnamed(_) => {
                    cx.span_err(span, INTERNAL_ERROR);
                    cx.expr_err(span, error)
                },
            }
        },
        _ => {
            cx.span_err(span, INTERNAL_ERROR);
            cx.expr_err(span, error)
        },
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
            context.span_warn(span, INTERNAL_ERROR);
            return;
        }
    };
    let link_name = format!("_{}Link", item.ident.as_str());
    push(context.item_struct(span, ast::Ident::new(
        token::intern(link_name.as_slice())
        ), link_def));
    // deriving ShaderParam
    let method_arg = generic::ty::Ptr(
        box generic::ty::Literal(generic::ty::Path::new_local("S")),
        generic::ty::Borrowed(None, ast::MutImmutable)
    );
    let error_path = generic::ty::Path::new(vec!["gfx", LINK_RESULT_ERROR]);
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec!["gfx", "ShaderParam"],
            lifetime: None,
            params: vec![box generic::ty::Literal(
                generic::ty::Path::new_local(link_name.as_slice())
            )],
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
                        generic::ty::Path::new(vec!["gfx", "ParameterSink"]),
                    ])],
                },
                explicit_self: None,
                args: vec![method_arg],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Result"],
                        lifetime: None,
                        params: vec![
                            box generic::ty::Literal(
                                generic::ty::Path::new_local(link_name.as_slice())
                            ),
                            box generic::ty::Literal(error_path)
                        ],
                        global: false,
                    },
                ),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_create(cx, span, sub, link_name.as_slice())
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
