// Copyright 2015 The Gfx-rs Developers.
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

use syntax::{abi, ast, codemap, ext};
use syntax::ext::base::ItemDecorator;
use syntax::ext::build::AstBuilder;
use syntax::owned_slice::OwnedSlice;
use syntax::parse::token;
use syntax::ptr::P;

#[derive(Copy, PartialEq, Debug)]
enum Param {
    Uniform,
    Block,
    Texture,
    Special,
}

#[derive(Copy, Debug)]
enum ParamError {
    DeprecatedTexture,
}

/// Classify variable types (`i32`, `TextureParam`, etc) into the `Param`
fn classify(node: &ast::Ty_) -> Result<Param, ParamError> {
    match *node {
        ast::TyPath(_,ref path) => match path.segments.last() {
            Some(segment) => match segment.identifier.name.as_str() {
                "RawBufferHandle" => Ok(Param::Block),
                "TextureParam" => Ok(Param::Texture),
                "TextureHandle" => Err(ParamError::DeprecatedTexture),
                "PhantomData"   => Ok(Param::Special),
                _ => Ok(Param::Uniform),
            },
            None => Ok(Param::Uniform),
        },
        _ => Ok(Param::Uniform),
    }
}

/// Generates the the method body for `gfx::shade::ParamValues::create_link`
fn method_create(cx: &mut ext::base::ExtCtxt,
                 span: codemap::Span,
                 definition: &ast::StructDef,
                 input: ast::Ident,
                 link_ident: ast::Ident,
                 path_root: ast::Ident)
                 -> P<ast::Block> {
    let init_expr = cx.expr_struct_ident(
        span, link_ident,
        definition.fields.iter().scan((), |_, field| {
            field.node.ident().map(|name|
                cx.field_imm(field.span, name, cx.expr_none(field.span))
            )
        }).collect()
    );
    let class_info: Vec<(Param, P<ast::Expr>)> = definition.fields.iter().scan((), |_, field|
        match (field.node.ident(), classify(&field.node.ty.node)) {
            (None, _) => {
                cx.span_err(field.span, "Named fields are required for `ShaderParam`");
                None
            },
            (Some(fname), Ok(c)) => {
                let name = match super::find_name(cx, field.span, &field.node.attrs) {
                    Some(name) => name,
                    None => token::get_ident(fname),
                };
                Some((c, cx.expr_str(field.span, name)))
            },
            (Some(fname), Err(e)) => {
                cx.span_err(field.span, &format!(
                    "Unrecognized parameter ({:?}) type {:?}",
                    fname.as_str(), e
                ));
                None
            },
        }
    ).collect();
    let gen_arms = |ptype: Param, var: ast::Ident| -> Vec<ast::Arm> {
        class_info.iter().zip(definition.fields.iter())
                  .filter(|&(&(ref class, _), _)| *class == ptype)
                  .scan((), |_, (&(_, ref name_expr), field)|
            field.node.ident().map(|name| quote_arm!(cx,
                $name_expr => {out.$name = Some(i as $path_root::gfx::shade::$var)}
            ))
        ).collect()
    };
    let uniform_arms = gen_arms(Param::Uniform, cx.ident_of("VarUniform"));
    let block_arms = gen_arms(Param::Block, cx.ident_of("VarBlock"));
    let texture_arms = gen_arms(Param::Texture, cx.ident_of("VarTexture"));
    let expr = quote_expr!(cx, {
        let mut out = $init_expr;
        for (i, u) in $input.uniforms.iter().enumerate() {
            let _ = i; // suppress warning about unused i
            match &u.name[..] {
                $uniform_arms
                _ => return Err($path_root::gfx::shade::
                    ParameterError::MissingUniform(u.name.clone())),
            }
        }
        for (i, b) in $input.blocks.iter().enumerate() {
            let _ = i; // suppress warning about unused i
            match &b.name[..] {
                $block_arms
                _ => return Err($path_root::gfx::shade::
                    ParameterError::MissingBlock(b.name.clone())),
            }
        }
        for (i, t) in $input.textures.iter().enumerate() {
            let _ = i; // suppress warning about unused i
            match &t.name[..] {
                $texture_arms
                _ => return Err($path_root::gfx::shade::
                    ParameterError::MissingTexture(t.name.clone())),
            }
        }
        Ok(out)
    });
    cx.block_expr(expr)
}

/// Generates the the method body for `gfx::shade::ParamValues::fill_params`
fn method_fill(cx: &mut ext::base::ExtCtxt,
               span: codemap::Span,
               definition: &ast::StructDef,
               path_root: ast::Ident)
               -> P<ast::Block> {
    let max_num = cx.expr_usize(span, definition.fields.len());
    let mut calls = vec![
        quote_stmt!(cx, use self::$path_root::gfx::shade::ToUniform;),
        quote_stmt!(cx, out.uniforms.reserve($max_num);),
        quote_stmt!(cx, out.blocks.reserve($max_num);),
        quote_stmt!(cx, out.textures.reserve($max_num);),
    ];
    calls.extend(definition.fields.iter().scan((), |_, field| {
        let name = match field.node.ident() {
            Some(n) => n,
            None => {
                cx.span_err(span, "Named fields are required for `ShaderParam`");
                return None
            }
        };
        classify(&field.node.ty.node).ok().map(|param| match param {
            Param::Uniform => quote_stmt!(cx,
                link.$name.map_or((), |id| {
                    if out.uniforms.len() <= id as usize {
                        unsafe { out.uniforms.set_len(id as usize + 1) }
                    }
                    *out.uniforms.get_mut(id as usize).unwrap() = self.$name.to_uniform()
                })
            ),
            Param::Block   => quote_stmt!(cx,
                link.$name.map_or((), |id| {
                    if out.blocks.len() <= id as usize {
                        unsafe { out.blocks.set_len(id as usize + 1) }
                    }
                    *out.blocks.get_mut(id as usize).unwrap() = {self.$name}
                })
            ),
            Param::Texture => quote_stmt!(cx,
                link.$name.map_or((), |id| {
                    if out.textures.len() <= id as usize {
                        unsafe { out.textures.set_len(id as usize + 1) }
                    }
                    *out.textures.get_mut(id as usize).unwrap() = {self.$name}
                })
            ),
            Param::Special => quote_stmt!(cx, ()),
        })
    }));
    cx.block_all(span, calls, None)
}

/// A helper function that translates variable type (`i32`, `TextureHandle`, etc)
/// into the corresponding shader var id type (`VarUniform`, `VarBlock`, or `VarTexture`)
fn node_to_var_type(cx: &mut ext::base::ExtCtxt,
                    span: codemap::Span, node: &ast::Ty_,
                    path_root: ast::Ident) -> P<ast::Ty> {
    let id = cx.ident_of(match classify(node) {
        Ok(Param::Uniform) => "VarUniform",
        Ok(Param::Block)   => "VarBlock",
        Ok(Param::Texture) => "VarTexture",
        Ok(Param::Special) => return quote_ty!(cx, Option<()>),
        Err(ParamError::DeprecatedTexture) => {
            cx.span_err(span, "Use gfx::shade::TextureParam for texture vars instead of gfx::shade::TextureHandle");
            ""
        },
    });
    quote_ty!(cx, Option<$path_root::gfx::shade::$id>)
}

fn impl_type(cx: &mut ext::base::ExtCtxt, span: codemap::Span, name: &str, type_ident: ast::Ident) -> ast::ImplItem {
    ast::TypeImplItem(P(ast::Typedef {
        id: ast::DUMMY_NODE_ID,
        span: span,
        ident: cx.ident_of(name),
        vis: ast::Visibility::Inherited,
        attrs: Vec::new(),
        typ: cx.ty_ident(span, type_ident),
    }))
}

fn impl_method(cx: &mut ext::base::ExtCtxt, span: codemap::Span, name: &str,
               with_self: bool, declaration: P<ast::FnDecl>, body: P<ast::Block>)
               -> ast::ImplItem {
    ast::MethodImplItem(P(ast::Method {
        attrs: Vec::new(),
        id: ast::DUMMY_NODE_ID,
        span: span,
        node: ast::MethDecl(
            cx.ident_of(name),
            ast::Generics {
                lifetimes: Vec::new(),
                ty_params: OwnedSlice::empty(),
                where_clause: ast::WhereClause {
                    id: ast::DUMMY_NODE_ID,
                    predicates: Vec::new(),
                },
            },
            abi::Abi::Rust,
            codemap::Spanned {
                node: if with_self {
                        ast::SelfRegion(None, ast::MutImmutable, cx.ident_of("self"))
                    }else {
                        ast::SelfStatic
                },
                span: span,
            },
            ast::Unsafety::Normal,
            declaration,
            body,
            ast::Visibility::Inherited
        )
    }))
}


#[derive(Copy)]
pub struct ShaderParam;
impl ItemDecorator for ShaderParam {
    /// Decorator for `shader_param` attribute
    fn expand(&self, context: &mut ext::base::ExtCtxt, span: codemap::Span,
              meta_item: &ast::MetaItem, item: &ast::Item,
              push: &mut FnMut(P<ast::Item>)) {
        // Insert the `gfx` reexport module
        let extern_hack = context.ident_of(super::EXTERN_CRATE_HACK);
        let path_root = super::extern_crate_hack(context, span, |i| (*push)(i));

        // constructing the Link struct
        let (base_def, generics, link_def) = match item.node {
            ast::ItemStruct(ref definition, ref generics) => {
                (definition, generics.clone(), ast::StructDef {
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
        
        // derive and push
        let link_name = format!("_{}Link", item.ident.as_str());
        let link_ident = context.ident_of(&link_name);
        let link_item = context.item_struct(span, link_ident, link_def)
                               .map(|mut item| {
            item.attrs.push(context.attribute(span,
                context.meta_list(span, token::InternedString::new("derive"), vec![
                        context.meta_word(span, token::InternedString::new("Copy")),
                        context.meta_word(span, token::InternedString::new("Debug")),
                ])
            ));
            item
        });
        (*push)(link_item);

        // process meta parameters (expecting none)
        match meta_item.node {
            ast::MetaWord(_) => (), //expected
            _ => {
                context.span_err(meta_item.span, "#[shader_param] needs no param");
            }
        }

        // find the generic `Resources` bound
        let resource_ident = match generics.ty_params.iter().find(|typ|
            typ.bounds.iter().find(|b| match **b {
                ast::TraitTyParamBound(ref poly_trait, _) =>
                    poly_trait.trait_ref.path.segments.last().unwrap().identifier.as_str() == "Resources",
                ast::RegionTyParamBound(_) => false,
            }).is_some()
        ){
            Some(typ) => typ.ident,
            None => {
                context.span_err(meta_item.span, "#[shader_param] unable to find generic `gfx::Resources` bound");
                context.ident_of("R")
            }
        };

        // construct `create_link()`
        let lifetimes = generics.lifetimes.iter().map(|ld| ld.lifetime).collect();
        let generic_parameters = generics.ty_params.iter().map(|ty|
            context.ty_ident(span, ty.ident)
        ).collect();
        let struct_ty = context.ty_path(context.path_all(
            span, false,
            vec![item.ident],
            lifetimes,
            generic_parameters,
            Vec::new(),
        ));
        let create_param = context.ident_of("params");
        let body_create = method_create(context, span, base_def, create_param, link_ident, path_root);
        let decl_create = context.fn_decl(
            vec![
                ast::Arg {
                    ty: quote_ty!(context, Option<&$struct_ty>),
                    pat: context.pat_wild(span),
                    id: ast::DUMMY_NODE_ID,
                },
                ast::Arg {
                    ty: quote_ty!(context, &$extern_hack::gfx::ProgramInfo),
                    pat: context.pat_ident(span, create_param),
                    id: ast::DUMMY_NODE_ID,
                },
            ],
            context.ty_path(context.path_all(
                span, false, vec![context.ident_of("Result")],
                Vec::new(), vec![
                    context.ty_ident(span, link_ident),
                    quote_ty!(context, $extern_hack::gfx::shade::ParameterError),
                ], Vec::new()
            )),
        );

        // construct `fill_params()`
        let body_fill = method_fill(context, span, base_def, path_root);
        let decl_fill = context.fn_decl(
             vec![
                ast::Arg::new_self(span, ast::MutImmutable, context.ident_of("self")),
                ast::Arg {
                    ty: quote_ty!(context, &$link_ident),
                    pat: context.pat_ident(span, context.ident_of("link")),
                    id: ast::DUMMY_NODE_ID,
                },
                ast::Arg {
                    ty: quote_ty!(context, $extern_hack::gfx::shade::ParamValues<$resource_ident>),
                    pat: context.pat_ident(span, context.ident_of("out")),
                    id: ast::DUMMY_NODE_ID,
                },
            ],
            quote_ty!(context, ())
        );

        // construct implementations for types and methods
        let impls = vec![
            impl_type(context, span, "Resources", resource_ident),
            impl_type(context, span, "Link", link_ident),
            impl_method(context, span, "create_link", false, decl_create, body_create),
            impl_method(context, span, "fill_params", true, decl_fill, body_fill),
        ];

        // final implementation item
        let item = context.item(span, item.ident, Vec::new(), ast::Item_::ItemImpl(
                ast::Unsafety::Normal,
                ast::ImplPolarity::Positive,
                generics,
                Some(context.trait_ref(context.path(span, vec![
                    extern_hack,
                    context.ident_of("gfx"),
                    context.ident_of("shade"),
                    context.ident_of("ShaderParam"),
                ]))),
                struct_ty,
                impls
        ));
        (*push)(super::fixup_extern_crate_paths(item, path_root));
    }
}
