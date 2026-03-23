//! Implementation of `#[derive(Props)]`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Result, Type};

use crate::crate_path::kozan_core_path;

struct PropsAttrs {
    element: syn::Path,
}

fn parse_attrs(input: &DeriveInput) -> Result<PropsAttrs> {
    let mut element = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("props") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("element") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                element = Some(path);
            }
            Ok(())
        })?;
    }

    let element = element.ok_or_else(|| {
        syn::Error::new_spanned(&input.ident, "#[props(element = ...)] is required")
    })?;

    Ok(PropsAttrs { element })
}

struct PropField {
    ident: syn::Ident,
    ty: Type,
}

fn extract_fields(data: &Data) -> Result<Vec<PropField>> {
    let fields = match data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "#[derive(Props)] requires named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[derive(Props)] only works on structs",
            ));
        }
    };

    Ok(fields
        .iter()
        .filter(|f| f.attrs.iter().any(|a| a.path().is_ident("prop")))
        .map(|f| PropField {
            ident: f.ident.clone().expect("named field must have an ident"),
            ty: f.ty.clone(),
        })
        .collect())
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let data_name = &input.ident;
    let attrs = parse_attrs(input)?;
    let element_type = &attrs.element;
    let krate = kozan_core_path();

    let fields = extract_fields(&input.data)?;

    let methods: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let field_name = &f.ident;
            let field_type = &f.ty;
            let setter_name = format_ident!("set_{}", field_name);

            quote! {
                #[inline]
                pub fn #field_name(&self) -> #field_type {
                    #krate::HasHandle::handle(self)
                        .read_data::<#data_name, _>(|d| d.#field_name.clone())
                        .unwrap_or_default()
                }

                #[inline]
                pub fn #setter_name(&self, value: impl Into<#field_type>) {
                    let value: #field_type = value.into();
                    #krate::HasHandle::handle(self)
                        .write_data::<#data_name, _>(|d| d.#field_name = value);
                }
            }
        })
        .collect();

    Ok(quote! {
        impl #element_type {
            #(#methods)*
        }
    })
}
