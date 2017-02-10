#![recursion_limit = "192"]

extern crate proc_macro;
extern crate syn;
#[macro_use] extern crate quote;

use proc_macro::TokenStream;

#[proc_macro_derive(GfxStruct)]
pub fn structure(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_macro_input(&s).unwrap();
    let name = &ast.ident;
    let fields = match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(ref fields)) => fields,
        _ => panic!("`GfxStruct` can only be derived for structs"),
    };
    let match_name = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            stringify!(#ident) => Some(Element {
                format: <#ty as gfx::format::Formatted>::get_format(),
                offset: ((&tmp.#ident as *const _ as usize) - base) as ElemOffset + big_offset,
            }),
        }
    });
    let gen = quote! {
        impl gfx::pso::buffer::Structure<gfx::format::Format> for #name {
            fn query(field_name: &str) -> Option<gfx::pso::buffer::Element<gfx::format::Format>> {
                use std::mem::{size_of, transmute};
                use gfx::pso::buffer::{Element, ElemOffset};
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
    };
    gen.parse().unwrap()
}
