#![recursion_limit = "192"]

extern crate proc_macro;
extern crate syn;
#[macro_use] extern crate quote;

use proc_macro::TokenStream;


#[proc_macro_derive(VertexData)]
pub fn vertex(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_macro_input(&s).unwrap();
    let gen = structure(ast, quote!(gfx::format::Formatted), quote!(gfx::format::Format));
    gen.parse().unwrap()
}

#[proc_macro_derive(ConstantBuffer)]
pub fn constant(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_macro_input(&s).unwrap();
    let gen = structure(ast, quote!(gfx::shade::Formatted), quote!(gfx::shade::ConstFormat));
    gen.parse().unwrap()
}

fn structure(ast: syn::DeriveInput, ty_compile: quote::Tokens, ty_run: quote::Tokens)
             -> quote::Tokens {
    let name = &ast.ident;
    let fields = match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(ref fields)) => fields,
        _ => panic!("gfx-rs custom derives can only be casted on structs"),
    };
    let match_name = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            stringify!(#ident) => Some(Element {
                format: <#ty as #ty_compile>::get_format(),
                offset: ((&tmp.#ident as *const _ as usize) - base) as ElemOffset + big_offset,
            }),
        }
    });
    quote! {
        impl gfx::pso::buffer::Structure<#ty_run> for #name {
            fn query(field_name: &str) -> Option<gfx::pso::buffer::Element<#ty_run>> {
                use std::mem::{size_of, transmute};
                use gfx::pso::buffer::{Element, ElemOffset};
                // using an address of 1 as a simplest non-zero pointer to avoid UB
                let tmp: &#name = unsafe { transmute(1usize) };
                let base = tmp as *const _ as usize;
                let (sub_name, big_offset) = {
                    let mut split = field_name.split(|c| c == '[' || c == ']');
                    let _ = split.next().unwrap();
                    match split.next() {
                        Some(s) => {
                            let array_id: ElemOffset = s.parse().unwrap();
                            let sub_name = match split.next() {
                                Some(s) if s.starts_with('.') => &s[1..],
                                _ => field_name,
                            };
                            (sub_name, array_id * (size_of::<#name>() as ElemOffset))
                        },
                        None => (field_name, 0),
                    }
                };
                match sub_name {
                    #(#match_name)*
                    _ => None,
                }
            }
        }
    }
}
