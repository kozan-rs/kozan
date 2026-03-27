//! Implementation of `#[derive(Event)]`.
//!
//! Generates `impl Event for T` from struct attributes.
//!
//! # Usage
//!
//! ```ignore
//! #[derive(Event)]
//! #[event(bubbles, cancelable)]
//! pub struct ClickEvent {
//!     pub x: f32,
//!     pub y: f32,
//! }
//! ```
//!
//! Generates:
//! ```ignore
//! impl Event for ClickEvent {
//!     fn bubbles(&self) -> Bubbles { Bubbles::Yes }
//!     fn cancelable(&self) -> Cancelable { Cancelable::Yes }
//!     fn as_any(&self) -> &dyn Any { self }
//! }
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result};

use crate::crate_path::kozan_core_path;

struct EventAttrs {
    bubbles: bool,
    cancelable: bool,
}

fn parse_attrs(input: &DeriveInput) -> Result<EventAttrs> {
    let mut bubbles = false;
    let mut cancelable = false;

    for attr in &input.attrs {
        if !attr.path().is_ident("event") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("bubbles") {
                bubbles = true;
            } else if meta.path.is_ident("cancelable") {
                cancelable = true;
            } else {
                // Reject unknown attributes at compile time — catches typos
                // like `#[event(canceleable)]` immediately.
                return Err(
                    meta.error("unknown event attribute (expected `bubbles` or `cancelable`)")
                );
            }
            Ok(())
        })?;
    }

    Ok(EventAttrs {
        bubbles,
        cancelable,
    })
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let krate = kozan_core_path();

    let attrs = parse_attrs(input)?;

    let bubbles = if attrs.bubbles {
        quote! { #krate::events::Bubbles::Yes }
    } else {
        quote! { #krate::events::Bubbles::No }
    };

    let cancelable = if attrs.cancelable {
        quote! { #krate::events::Cancelable::Yes }
    } else {
        quote! { #krate::events::Cancelable::No }
    };

    Ok(quote! {
        impl #impl_generics #krate::events::Event for #name #ty_generics #where_clause {
            #[inline]
            fn bubbles(&self) -> #krate::events::Bubbles {
                #bubbles
            }

            #[inline]
            fn cancelable(&self) -> #krate::events::Cancelable {
                #cancelable
            }

            #[inline]
            fn as_any(&self) -> &dyn ::core::any::Any {
                self
            }
        }
    })
}
