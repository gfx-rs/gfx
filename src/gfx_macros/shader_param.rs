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

use std::gc::{Gc, GC};
use syntax::{ast, ast_util, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::{attr, codemap};
use syntax::parse::token;


enum ParamType {
    ParamUniform,
    ParamBlock,
    ParamTexture,
}

#[deriving(Show)]
enum ParamError {
    ErrorDeprecatedTexture,
    ErrorUnknown,
}

/// Classify variable types (`i32`, `TextureParam`, etc) into the `ParamType`
fn classify(node: &ast::Ty_) -> Result<ParamType, ParamError> {
    match *node {
        ast::TyPath(ref path, _, _) => match path.segments.last() {
            Some(segment) => match segment.identifier.name.as_str() {
                "BufferHandle" => Ok(ParamBlock),
                "TextureParam" => Ok(ParamTexture),
                "TextureHandle" => Err(ErrorDeprecatedTexture),
                _ => Ok(ParamUniform),
            },
            None => Ok(ParamUniform),
        },
        _ => Ok(ParamUniform),
    }
}

/// Scan through the field's attributes and find out if it needs to be linked
/// with the shader.
/// #[unused] attribute prevents that
fn is_field_used(field: &ast::StructField) -> bool {
    field.node.attrs.iter().find(|attribute| {
        match attribute.node.value.node {
            ast::MetaWord(ref word) if word.get() == "unused" => {
                attr::mark_used(*attribute);
                true
            },
            _ => false,
        }
    }).is_none()
}

/// Generates the the method body for `gfx::shade::ParamValues::create_link`
fn method_create(cx: &mut ext::base::ExtCtxt, span: codemap::Span, substr: &generic::Substructure,
                 definition: Gc<ast::StructDef>, link_name: &str) -> Gc<ast::Expr> {
    let link_ident = cx.ident_of(link_name);
    match *substr.fields {
        //generic::StaticStruct(definition, generic::Named(ref fields)) => {
        generic::Struct(ref fields) => {
            let out = definition.fields.iter().zip(fields.iter())
                .filter(|&(def, _)| is_field_used(def)).map(|(def, f)| {
                let name = cx.expr_str(span, token::get_ident(f.name.unwrap()));
                let input = substr.nonself_args[0];
                let expr = match classify(&def.node.ty.node) {
                    //TODO: verify the type match
                    Ok(ParamUniform) => super::ugh(cx, |cx| quote_expr!(cx,
                        match $input.val0().iter().position(|u| u.name.as_slice() == $name) {
                            Some(p) => p as gfx::shade::VarUniform,
                            None => return Err(gfx::shade::ErrorUniform($name)),
                        }
                    )),
                    Ok(ParamBlock)   => super::ugh(cx, |cx| quote_expr!(cx,
                        match $input.val1().iter().position(|b| b.name.as_slice() == $name) {
                            Some(p) => p as gfx::shade::VarBlock,
                            None => return Err(gfx::shade::ErrorBlock($name)),
                        }
                    )),
                    Ok(ParamTexture) => super::ugh(cx, |cx| quote_expr!(cx,
                        match $input.val2().iter().position(|t| t.name.as_slice() == $name) {
                            Some(p) => p as gfx::shade::VarTexture,
                            None => return Err(gfx::shade::ErrorTexture($name)),
                        }
                    )),
                    Err(_) => {
                        cx.span_err(span, format!(
                            "Invalid uniform: {}",
                            f.name.unwrap().as_str(),
                            ).as_slice()
                        );
                        return cx.field_imm(span,
                            cx.ident_of("invalid"),
                            cx.expr_uint(span, 0)
                            );
                    },
                };
                cx.field_imm(f.span, f.name.unwrap(), expr)
            }).collect();
            cx.expr_ok(span, cx.expr_struct_ident(span, link_ident, out))
        },
        _ => {
            cx.span_err(span, "Unable to implement `ShaderParam::create_link()` on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        },
    }
}

/// Generates the the method body for `gfx::shade::ParamValues::fill_params`
fn method_fill(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               substr: &generic::Substructure, definition: Gc<ast::StructDef>)
               -> Gc<ast::Expr> {
    match *substr.fields {
        generic::Struct(ref fields) => {
            let calls = definition.fields.iter().zip(fields.iter())
            .filter(|&(def, _)| is_field_used(def)).map(|(def, f)| {
                let out = substr.nonself_args[1];
                let value_id = f.self_;
                let var_id = cx.expr_field_access(
                    span,
                    substr.nonself_args[0],
                    f.name.unwrap()
                    );
                match classify(&def.node.ty.node) {
                    Ok(ParamUniform) => super::ugh(cx, |cx| quote_stmt!(cx,
                        $out.uniforms[$var_id as uint] = Some($value_id.to_uniform());
                    )),
                    Ok(ParamBlock)   => super::ugh(cx, |cx| quote_stmt!(cx,
                        $out.blocks[$var_id as uint] = Some $value_id;
                    )),
                    Ok(ParamTexture) => super::ugh(cx, |cx| quote_stmt!(cx,
                        $out.textures[$var_id as uint] = Some $value_id;
                    )),
                    Err(_) => {
                        cx.span_err(span, format!(
                            "Invalid uniform: {}",
                            f.name.unwrap().as_str(),
                            ).as_slice()
                        );
                        cx.stmt_expr(cx.expr_uint(span, 0))
                    },
                }
            }).collect();
            let view = cx.view_use_simple(
                span,
                ast::Inherited,
                cx.path(span, vec![
                    cx.ident_of("gfx"),
                    cx.ident_of("shade"),
                    cx.ident_of("ToUniform"),
                ])
            );
            cx.expr_block(cx.block_all(span, vec![view], calls, None))
        },
        _ => {
            cx.span_err(span, "Unable to implement `ShaderParam::bind()` on a non-structure");
            cx.expr_lit(span, ast::LitNil)
        }
    }
}

/// A helper function that translates variable type (`i32`, `TextureHandle`, etc)
/// into the corresponding shader var id type (`VarUniform`, `VarBlock`, or `VarTexture`)
fn node_to_var_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span, node: &ast::Ty_) -> Gc<ast::Ty> {
    let id = match classify(node) {
        Ok(ParamUniform) => "VarUniform",
        Ok(ParamBlock)   => "VarBlock",
        Ok(ParamTexture) => "VarTexture",
        Err(ErrorDeprecatedTexture) => {
            cx.span_err(span, "Use gfx::shade::TextureParam for texture vars instead of gfx::shade::TextureHandle");
            ""
        },
        Err(ErrorUnknown) => {
            cx.span_err(span, format!("Unknown node: {}", node).as_slice());
            ""
        },
    };
    cx.ty_path(cx.path_global(span, vec![
        cx.ident_of("gfx"),
        cx.ident_of("shade"),
        cx.ident_of(id),
    ]), None)
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
                fields: definition.fields.iter()
                    .filter(|&f| is_field_used(f))
                    .map(|f| codemap::Spanned {
                        node: ast::StructField_ {
                            kind: f.node.kind,
                            id: f.node.id,
                            ty: node_to_var_type(context, f.span, &f.node.ty.node),
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
            context.span_err(span, "Only free-standing named structs allowed to derive ShaderParam");
            return;
        }
    };
    let link_name = format!("_{}Link", item.ident.as_str());
    let link_ident = context.ident_of(link_name.as_slice());
    let link_ty = box generic::ty::Literal(
        generic::ty::Path::new_local(link_name.as_slice())
    );
    // Almost `context.item_struct(span, link_ident, link_def)` but with visibility
    push(box (GC) ast::Item {
        ident: link_ident,
        attrs: Vec::new(),
        id: ast::DUMMY_NODE_ID,
        node: ast::ItemStruct(
            box(GC) link_def,
            ast_util::empty_generics()
        ),
        vis: item.vis,
        span: span,
    });
    // constructing the `CustomShell` typedef
    match meta_item.node {
        ast::MetaWord(_) => (),
        ast::MetaList(_, ref items) if items.len() == 1 => match items[0].deref().node {
            ast::MetaWord(ref shell_name) => {
                // pub type $shell_ident = gfx::shade::CustomShell<$link_ident, $self_ident>
                let path = context.ty_path(
                    context.path_all(span, true,
                        vec![
                            context.ident_of("gfx"),
                            context.ident_of("shade"),
                            context.ident_of("CustomShell"),
                        ],
                        Vec::new(),
                        vec![
                            context.ty_ident(span, link_ident),
                            context.ty_ident(span, item.ident)
                        ]
                    ), None
                );
                push(box(GC) ast::Item {
                    ident: context.ident_of(shell_name.get()),
                    attrs: Vec::new(),
                    id: ast::DUMMY_NODE_ID,
                    node: ast::ItemTy(path, ast_util::empty_generics()),
                    vis: item.vis,
                    span: span,
                })
            },
            _ => {
                context.span_err(meta_item.span,
                    "Invalid arguments for `#[shader_param]`")
            }
        },
        _ => {
            context.span_err(meta_item.span,
                "Invalid arguments for `#[shader_param]`")
        }
    }
    // deriving ShaderParam
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec!["gfx", "shade", "ShaderParam"],
            lifetime: None,
            params: vec![link_ty.clone()],
            global: true,
        },
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
            generic::MethodDef {
                name: "create_link",
                generics: generic::ty::LifetimeBounds::empty(),
                explicit_self: Some(Some(generic::ty::Borrowed(
                    None, ast::MutImmutable
                ))),
                args: vec![
                    generic::ty::Literal(generic::ty::Path::new(
                        vec!["gfx", "shade", "ParamLinkInput"])),
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Result"],
                        lifetime: None,
                        params: vec![
                            link_ty.clone(),
                            box generic::ty::Literal(generic::ty::Path {
                                path: vec!["gfx", "shade", "ParameterError"],
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
                name: "fill_params",
                generics: generic::ty::LifetimeBounds::empty(),
                explicit_self: Some(Some(generic::ty::Borrowed(
                    None, ast::MutImmutable
                ))),
                args: vec![
                    generic::ty::Ptr(
                        link_ty.clone(),
                        generic::ty::Borrowed(None, ast::MutImmutable)
                    ),
                    generic::ty::Literal(
                        generic::ty::Path::new(vec!["gfx", "shade", "ParamValues"]),
                    ),
                ],
                ret_ty: generic::ty::Tuple(Vec::new()),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_fill(cx, span, sub, base_def)
                ),
            },
        ],
    };
    trait_def.expand(context, meta_item, item, push);
}
