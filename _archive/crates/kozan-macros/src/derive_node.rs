//! Implementation of `#[derive(Node)]`.
//!
//! Generates: `HasHandle` + `EventTarget` + `Node` + `Debug`.
//! For non-element nodes (Text, Comment) that cannot have children.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::crate_path::{debug_impl, kozan_core_path};

pub fn expand(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let krate = kozan_core_path();
    let debug = debug_impl(input, &krate);

    quote! {
        impl #impl_generics #krate::HasHandle for #name #ty_generics #where_clause {
            #[inline]
            fn handle(&self) -> #krate::Handle { self.0 }
        }

        impl #impl_generics #krate::EventTarget for #name #ty_generics #where_clause {}
        impl #impl_generics #krate::Node for #name #ty_generics #where_clause {}

        #debug
    }
}
