//! Resolves the path to `kozan_core` at compile time and shared codegen helpers.
//!
//! Uses `proc-macro-crate` to handle all scenarios:
//! - Inside `kozan-core` itself → `crate`
//! - Normal dependency → `::kozan_core`
//! - Renamed dependency (e.g., `kozan = { package = "kozan-core" }`) → `::kozan`

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

/// Returns a token stream representing the path to `kozan_core`.
pub fn kozan_core_path() -> TokenStream {
    match crate_name("kozan-core") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident }
        }
        Err(_) => {
            // Fallback for edge cases (e.g., doc tests).
            quote! { ::kozan_core }
        }
    }
}

/// Generate a `Debug` impl that prints `TypeName(handle)`.
pub fn debug_impl(input: &DeriveInput, krate: &TokenStream) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::core::fmt::Debug for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}({:?})", stringify!(#name), #krate::HasHandle::handle(self))
            }
        }
    }
}
