#![recursion_limit = "192"]

extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro2::TokenStream;

#[proc_macro_derive(VertexData)]
pub fn vertex(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    let gen = structure(
        ast,
        quote!(gfx::format::Formatted),
        quote!(gfx::format::Format),
    );
    gen.into()
}

#[proc_macro_derive(ConstantBuffer)]
pub fn constant(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    let gen = structure(
        ast,
        quote!(gfx::shade::Formatted),
        quote!(gfx::shade::ConstFormat),
    );
    gen.into()
}

fn structure(ast: syn::DeriveInput, ty_compile: TokenStream, ty_run: TokenStream) -> TokenStream {
    let name = &ast.ident;
    let fields = match ast.data {
        syn::Data::Struct(syn::DataStruct { ref fields, .. }) => fields,
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
        unsafe impl gfx::traits::Pod for #name {}

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
