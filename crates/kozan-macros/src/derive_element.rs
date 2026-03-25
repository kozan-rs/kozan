//! Implementation of `#[derive(Element)]`.
//!
//! Generates the FULL trait chain:
//! `HasHandle` + `EventTarget` + `Node` + `ContainerNode` + `Element` + `HtmlElement` + `Debug`.
//!
//! User only writes:
//! ```ignore
//! #[derive(Copy, Clone, Element)]
//! #[element(tag = "button", focusable, data = ButtonData)]
//! pub struct HtmlButtonElement(Handle);
//! ```
//! Gets everything for free. Zero manual impl.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Lit, Result};

use crate::crate_path::{debug_impl, kozan_core_path};

struct ElementAttrs {
    tag: String,
    focusable: bool,
    data: Option<syn::Path>,
    /// If true, skip auto-generating `impl HtmlElement` (user provides their own).
    manual_html: bool,
    /// Path to a `DefaultEventHandlerFn` for this element type.
    default_handler: Option<syn::Path>,
}

fn parse_attrs(input: &DeriveInput) -> Result<ElementAttrs> {
    let mut tag = None;
    let mut focusable = false;
    let mut data = None;
    let mut manual_html = false;
    let mut default_handler = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("element") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    tag = Some(s.value());
                }
            } else if meta.path.is_ident("focusable") {
                focusable = true;
            } else if meta.path.is_ident("manual_html") {
                manual_html = true;
            } else if meta.path.is_ident("data") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                data = Some(path);
            } else if meta.path.is_ident("default_handler") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                default_handler = Some(path);
            }
            Ok(())
        })?;
    }

    // tag is optional — elements with no fixed tag (e.g., HtmlHeadingElement
    // for h1-h6) omit it and MUST be created via Document::create_with_tag().
    // Chrome: tag is always runtime data on Element, not hardcoded on the class.
    let tag = tag.unwrap_or_default();

    Ok(ElementAttrs {
        tag,
        focusable,
        data,
        manual_html,
        default_handler,
    })
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let attrs = parse_attrs(input)?;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let krate = kozan_core_path();

    let tag = &attrs.tag;
    let focusable = attrs.focusable;
    let data_type = attrs.data.map_or_else(|| quote!(()), |p| quote!(#p));
    let debug = debug_impl(input, &krate);

    let html_element_impl = if attrs.manual_html {
        quote! {} // User provides their own impl HtmlElement.
    } else {
        quote! {
            impl #impl_generics #krate::HtmlElement for #name #ty_generics #where_clause {}
        }
    };

    let handler_const = attrs.default_handler.map(|path| {
        quote! {
            const DEFAULT_EVENT_HANDLER: Option<#krate::dom::node::DefaultEventHandlerFn> = Some(#path);
        }
    });

    Ok(quote! {
        // Full trait chain — ONE derive generates everything.
        impl #impl_generics #krate::HasHandle for #name #ty_generics #where_clause {
            #[inline]
            fn handle(&self) -> #krate::Handle { self.0 }
        }

        impl #impl_generics #krate::EventTarget for #name #ty_generics #where_clause {}
        impl #impl_generics #krate::Node for #name #ty_generics #where_clause {}
        impl #impl_generics #krate::ContainerNode for #name #ty_generics #where_clause {}

        impl #impl_generics #krate::Element for #name #ty_generics #where_clause {
            type Data = #data_type;
            const TAG_NAME: &'static str = #tag;
            const IS_FOCUSABLE: bool = #focusable;
            #handler_const

            #[inline]
            fn from_handle(handle: #krate::Handle) -> Self { Self(handle) }
        }

        #html_element_impl

        #debug
    })
}
