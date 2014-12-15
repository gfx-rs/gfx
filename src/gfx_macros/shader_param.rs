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

use syntax::{ast, ast_util, ext};
use syntax::ext::build::AstBuilder;
use syntax::ext::deriving::generic;
use syntax::codemap;
use syntax::parse::token;
use syntax::ptr::P;

#[deriving(Copy, PartialEq, Show)]
enum Param {
    Uniform,
    Block,
    Texture,
}

#[deriving(Copy, Show)]
enum ParamError {
    DeprecatedTexture,
}

/// Classify variable types (`i32`, `TextureParam`, etc) into the `Param`
fn classify(node: &ast::Ty_) -> Result<Param, ParamError> {
    match *node {
        ast::TyPath(ref path, _) => match path.segments.last() {
            Some(segment) => match segment.identifier.name.as_str() {
                "RawBufferHandle" => Ok(Param::Block),
                "TextureParam" => Ok(Param::Texture),
                "TextureHandle" => Err(ParamError::DeprecatedTexture),
                _ => Ok(Param::Uniform),
            },
            None => Ok(Param::Uniform),
        },
        _ => Ok(Param::Uniform),
    }
}

/// Generates the the method body for `gfx::shade::ParamValues::create_link`
fn method_create(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
                 substr: &generic::Substructure,
                 link_name: &str,
                 path_root: ast::Ident) -> P<ast::Expr> {
    let link_ident = cx.ident_of(link_name);
    match *substr.fields {
        generic::StaticStruct(definition, generic::Named(ref fields)) => {
            let init_expr = cx.expr_struct_ident(
                span, link_ident,
                fields.iter().map(|&(fname, fspan)| {
                    cx.field_imm(fspan, fname, cx.expr_none(fspan))
                }).collect()
            );
            let class_info: Vec<(Param, P<ast::Expr>)> = definition.fields.iter()
                    .zip(fields.iter()).scan((), |_, (def, &(fname, fspan))|
                match classify(&def.node.ty.node) {
                    Ok(c) => {
                        let name = match super::find_name(cx, span, def.node.attrs.as_slice()) {
                            Some(name) => name,
                            None => token::get_ident(fname),
                        };
                        Some((c, cx.expr_str(fspan, name)))
                    },
                    Err(e) => {
                        cx.span_err(fspan, format!(
                            "Unrecognized parameter ({}) type {}",
                            fname.as_str(), e
                            ).as_slice()
                        );
                        None
                    },
                }
            ).collect();
            let gen_arms = |ptype: Param, var: ast::Ident| -> Vec<ast::Arm> {
                class_info.iter().zip(fields.iter())
                          .filter(|&(&(class, _), _)| class == ptype)
                          .map(|(&(_, ref name_expr), &(fname, fspan))|
                    cx.arm(fspan,
                        vec![cx.pat_lit(fspan, name_expr.clone())],
                        quote_expr!(cx,
                            out.$fname = Some(i as $path_root::gfx::shade::$var)
                        )
                    )
                ).collect()
            };
            let uniform_arms = gen_arms(Param::Uniform, cx.ident_of("VarUniform"));
            let block_arms = gen_arms(Param::Block, cx.ident_of("VarBlock"));
            let texture_arms = gen_arms(Param::Texture, cx.ident_of("VarTexture"));
            let input = &substr.nonself_args[1];
            quote_expr!(cx, {
                let mut out = $init_expr;
                for (i, u) in $input.uniforms.iter().enumerate() {
                    let _ = i; // suppress warning about unused i
                    match u.name.as_slice() {
                        $uniform_arms
                        _ => return Err($path_root::gfx::shade::
                            ParameterError::MissingUniform(u.name.clone())),
                    }
                }
                for (i, b) in $input.blocks.iter().enumerate() {
                    let _ = i; // suppress warning about unused i
                    match b.name.as_slice() {
                        $block_arms
                        _ => return Err($path_root::gfx::shade::
                            ParameterError::MissingBlock(b.name.clone())),
                    }
                }
                for (i, t) in $input.textures.iter().enumerate() {
                    let _ = i; // suppress warning about unused i
                    match t.name.as_slice() {
                        $texture_arms
                        _ => return Err($path_root::gfx::shade::
                            ParameterError::MissingTexture(t.name.clone())),
                    }
                }
                Ok(out)
            })
        },
        _ => {
            cx.span_err(span, "Unable to implement `ShaderParam::create_link()` on a non-structure");
            cx.expr_tuple(span, vec![])
        },
    }
}

/// Generates the the method body for `gfx::shade::ParamValues::fill_params`
fn method_fill(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
               substr: &generic::Substructure,
               definition: P<ast::StructDef>,
               path_root: ast::Ident)
               -> P<ast::Expr> {
    match *substr.fields {
        generic::Struct(ref fields) => {
            let out = &substr.nonself_args[1];
            let max_num = cx.expr_uint(span, fields.len());
            let mut calls = vec![
                quote_stmt!(cx, $out.uniforms.reserve($max_num);),
                quote_stmt!(cx, $out.blocks.reserve($max_num);),
                quote_stmt!(cx, $out.textures.reserve($max_num);),
            ];
            calls.extend(definition.fields.iter().zip(fields.iter())
                                   .map(|(def, f)| {
                let value_id = &f.self_;
                let var_id = cx.expr_field_access(
                    span,
                    substr.nonself_args[0].clone(),
                    f.name.unwrap()
                    );
                match classify(&def.node.ty.node) {
                    Ok(Param::Uniform) => quote_stmt!(cx,
                        $var_id.map_or((), |id| {
                            if $out.uniforms.len() <= id as uint {
                                unsafe { $out.uniforms.set_len(id as uint + 1) }
                            }
                            *$out.uniforms.get_mut(id as uint).unwrap() = $value_id.to_uniform()
                        })
                    ),
                    Ok(Param::Block)   => quote_stmt!(cx,
                        $var_id.map_or((), |id| {
                            if $out.blocks.len() <= id as uint {
                                unsafe { $out.blocks.set_len(id as uint + 1) }
                            }
                            *$out.blocks.get_mut(id as uint).unwrap() = {$value_id}
                        })
                    ),
                    Ok(Param::Texture) => quote_stmt!(cx,
                        $var_id.map_or((), |id| {
                            if $out.textures.len() <= id as uint {
                                unsafe { $out.textures.set_len(id as uint + 1) }
                            }
                            *$out.textures.get_mut(id as uint).unwrap() = {$value_id}
                        })
                    ),
                    Err(_) => {
                        cx.span_err(span, format!(
                            "Invalid uniform: {}",
                            f.name.unwrap().as_str(),
                            ).as_slice()
                        );
                        cx.stmt_expr(cx.expr_uint(span, 0))
                    },
                }
            }));
            let view = cx.view_use_simple(
                span,
                ast::Inherited,
                cx.path(span, vec![
                    cx.ident_of("self"),
                    path_root,
                    cx.ident_of("gfx"),
                    cx.ident_of("shade"),
                    cx.ident_of("ToUniform"),
                ])
            );
            cx.expr_block(cx.block_all(span, vec![view], calls, None))
        },
        _ => {
            cx.span_err(span, "Unable to implement `ShaderParam::bind()` on a non-structure");
            cx.expr_tuple(span, vec![])
        }
    }
}

/// A helper function that translates variable type (`i32`, `TextureHandle`, etc)
/// into the corresponding shader var id type (`VarUniform`, `VarBlock`, or `VarTexture`)
fn node_to_var_type(cx: &mut ext::base::ExtCtxt,
                    span: codemap::Span, node: &ast::Ty_,
                    path_root: ast::Ident) -> P<ast::Ty> {
    let id = match classify(node) {
        Ok(Param::Uniform) => "VarUniform",
        Ok(Param::Block)   => "VarBlock",
        Ok(Param::Texture) => "VarTexture",
        Err(ParamError::DeprecatedTexture) => {
            cx.span_err(span, "Use gfx::shade::TextureParam for texture vars instead of gfx::shade::TextureHandle");
            ""
        },
    };
    cx.ty_option(cx.ty_path(
        cx.path(span, vec![
            path_root,
            cx.ident_of("gfx"),
            cx.ident_of("shade"),
            cx.ident_of(id),
        ]),
    ))
}

/// Extract all deriving() attributes into a separate array
fn copy_deriving(attribs: &[ast::Attribute]) -> Vec<ast::Attribute> {
    attribs.iter().filter(|at| {
        match at.node.value.node {
            ast::MetaList(ref s, _) => s.get() == "deriving",
            _ => false,
        }
    }).map(|at| at.clone()).collect()
}

/// Decorator for `shader_param` attribute
pub fn expand(context: &mut ext::base::ExtCtxt, span: codemap::Span,
              meta_item: &ast::MetaItem, item: &ast::Item,
              push: |P<ast::Item>|) {
    // Insert the `gfx` reexport module
    let path_root = super::extern_crate_hack(context, span, |i| push(i));

    // constructing the Link struct
    let (base_def, link_def) = match item.node {
        ast::ItemStruct(ref definition, ref generics) => {
            if generics.lifetimes.len() > 0 {
                context.bug("Generics are not allowed in ShaderParam struct");
            }
            (definition, ast::StructDef {
                fields: definition.fields.iter()
                    .map(|f| codemap::Spanned {
                        node: ast::StructField_ {
                            kind: f.node.kind,
                            id: f.node.id,
                            ty: node_to_var_type(context, f.span, &f.node.ty.node, path_root),
                            attrs: Vec::new(),
                        },
                        span: f.span,
                    }).collect(),
                ctor_id: None,
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
    push(P(ast::Item {
        ident: link_ident,
        attrs: copy_deriving(item.attrs.as_slice()),
        id: ast::DUMMY_NODE_ID,
        node: ast::ItemStruct(
            P(link_def),
            ast_util::empty_generics()
        ),
        vis: item.vis,
        span: span,
    }));
    // constructing the `Batch` implementation typedef
    match meta_item.node {
        ast::MetaList(_, ref items) if items.len() <= 2 => {
            let batch_names = ["RefBatch", "OwnedBatch"];
            for (&batch, param) in batch_names.iter().zip(items.iter()) {
                match param.node {
                    ast::MetaWord(ref shell_name) => {
                        // pub type $shell_ident = hack::gfx::batch::RefBatch<$link_ident, $self_ident>
                        let path = context.ty_path(
                            context.path_all(span, false,
                                vec![
                                    path_root,
                                    context.ident_of("gfx"),
                                    context.ident_of("batch"),
                                    context.ident_of(batch),
                                ],
                                Vec::new(),
                                vec![
                                    context.ty_ident(span, link_ident),
                                    context.ty_ident(span, item.ident)
                                ],
                                Vec::new()
                            ),
                        );
                        push(P(ast::Item {
                            ident: context.ident_of(shell_name.get()),
                            attrs: Vec::new(),
                            id: ast::DUMMY_NODE_ID,
                            node: ast::ItemTy(path, ast_util::empty_generics()),
                            vis: item.vis,
                            span: span,
                        }))
                    },
                    _ => {
                        context.span_err(meta_item.span,
                            "The new batch name has to be a word")
                    }
                }
            }
        },
        _ => {
            context.span_err(meta_item.span,
                "Invalid argument. Please specify the typedef for your `Program`\n\
                as `#[shader_param(MyLightBatch, MyHeavyBatch)]`")
        }
    }
    // deriving ShaderParam
    let trait_def = generic::TraitDef {
        span: span,
        attributes: Vec::new(),
        path: generic::ty::Path {
            path: vec![super::EXTERN_CRATE_HACK, "gfx", "shade", "ShaderParam"],
            lifetime: None,
            params: vec![link_ty.clone()],
            global: false,
        },
        additional_bounds: Vec::new(),
        generics: generic::ty::LifetimeBounds::empty(),
        methods: vec![
            generic::MethodDef {
                name: "create_link",
                generics: generic::ty::LifetimeBounds::empty(),
                explicit_self: None,
                args: vec![
                    generic::ty::Literal(generic::ty::Path {
                        path: vec!["Option"],
                        lifetime: None,
                        params: vec![
                            box generic::ty::Ptr(
                                box generic::ty::Self,
                                generic::ty::Borrowed(None, ast::MutImmutable)
                            ),
                        ],
                        global: false,
                    }),
                    generic::ty::Ptr(
                        box generic::ty::Literal(generic::ty::Path::new(
                            vec![super::EXTERN_CRATE_HACK, "gfx", "ProgramInfo"])),
                        generic::ty::Borrowed(None, ast::MutImmutable)
                    ),
                ],
                ret_ty: generic::ty::Literal(
                    generic::ty::Path {
                        path: vec!["Result"],
                        lifetime: None,
                        params: vec![
                            link_ty.clone(),
                            box generic::ty::Literal(generic::ty::Path::new(
                                vec![super::EXTERN_CRATE_HACK, "gfx", "shade", "ParameterError"]
                            ))
                        ],
                        global: false,
                    },
                ),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_create(cx, span, sub, link_name.as_slice(), path_root)
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
                        generic::ty::Path::new(vec![super::EXTERN_CRATE_HACK, "gfx", "shade", "ParamValues"]),
                    ),
                ],
                ret_ty: generic::ty::Tuple(Vec::new()),
                attributes: Vec::new(),
                combine_substructure: generic::combine_substructure(|cx, span, sub|
                    method_fill(cx, span, sub, base_def.clone(), path_root)
                ),
            },
        ],
    };
    let fixup = |item| {
        push(super::fixup_extern_crate_paths(item, path_root))
    };
    trait_def.expand(context, meta_item, item, fixup);
}
