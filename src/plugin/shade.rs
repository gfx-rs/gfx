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
use self::syntax::codemap::Span;
use self::syntax::parse::token;
use self::rustc::plugin::Registry;


static NAME_MODIFIER : &'static str = "shader_param";
static NAME_DECORATOR: &'static str = "shader_param_impl";

fn expand_modifier(cx: &mut ext::base::ExtCtxt, span: Span,
        _meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>) -> Gc<ast::Item> {
    let at = cx.attribute(span, cx.meta_word(span, token::InternedString::new(NAME_DECORATOR)));
    cx.item(span, item.ident, item.attrs.clone().append_one(at), item.node.clone())
}

fn create_substructure(cx: &mut ext::base::ExtCtxt, span: Span, substr: &generic::Substructure) -> Gc<ast::Expr> {
    match *substr.fields {
        generic::StaticStruct(_definition, ref summary) => {
            match *summary {
                generic::Named(ref fields) => {
                    //quote_expr!(cx, "::std::default::Default::default()");
                    let default = cx.expr_call_global(span, vec![
                            cx.ident_of("std"),
                            cx.ident_of("default"),
                            cx.ident_of("Default"),
                            cx.ident_of("default")
                        ], Vec::new());
                    let tmp = fields.iter().map(|&(ident, s)|
                        cx.field_imm(s, ident, default)
                        ).collect();
                    cx.expr_struct_ident(span, substr.type_ident, tmp)
                },
                generic::Unnamed(_) => cx.bug("Unnamed structs are not allowed to derive ShaderParam"),
            }
        },
        _ => cx.bug("Only free-standing named structs allowed to derive ShaderParam"),
    }
}

fn expand_decorator(context: &mut ext::base::ExtCtxt, span: Span,
        meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>, push: |Gc<ast::Item>|) {
    let arg = generic::ty::Ptr(box generic::ty::Literal(
        generic::ty::Path::new(vec!["gfx", "ParameterSink"])),
        generic::ty::Borrowed(None, ast::MutImmutable));
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path::new(vec!("gfx", "ShaderParam")),
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
            generic::MethodDef {
                name: "create",
                generics: generic::ty::LifetimeBounds::empty(),
                explicit_self: None,
                args: vec![arg],
                ret_ty: generic::ty::Self,
                attributes: Vec::new(),
                const_nonmatching: false,
                combine_substructure: generic::combine_substructure(create_substructure),
            },
        ],
    };
    trait_def.expand(context, meta_item, item, push);
}

#[plugin_registrar]
pub fn registrar(reg: &mut Registry) {
    reg.register_syntax_extension(token::intern(NAME_MODIFIER),
        ext::base::ItemModifier(expand_modifier));
    reg.register_syntax_extension(token::intern(NAME_DECORATOR),
        ext::base::ItemDecorator(expand_decorator));
}
