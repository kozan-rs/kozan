use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::common::{add_trait_bounds, get_attr_str, has_attr};

pub fn derive(input: DeriveInput) -> TokenStream {
    let name = &input.ident;

    // Check for #[css(comma)] on the struct/enum itself
    let comma_separated = has_attr(&input.attrs, "css", "comma");
    // Check for #[css(function = "name")] on the struct/enum itself
    let function_name = get_attr_str(&input.attrs, "css", "function");

    let generics = add_trait_bounds(&input.generics, &quote!(kozan_style::ToCss));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Enum(data) => derive_enum(data, comma_separated),
        Data::Struct(data) => derive_struct(&data.fields, comma_separated),
        Data::Union(_) => panic!("ToCss cannot be derived for unions"),
    };

    let inner = if let Some(ref func) = function_name {
        quote! {
            dest.write_str(#func)?;
            dest.write_char('(')?;
            #body
            dest.write_char(')')
        }
    } else {
        body
    };

    quote! {
        impl #impl_generics kozan_style::ToCss for #name #ty_generics #where_clause {
            fn to_css<W: ::core::fmt::Write>(&self, dest: &mut W) -> ::core::fmt::Result {
                #inner
            }
        }
    }
}

fn derive_enum(data: &syn::DataEnum, _comma: bool) -> TokenStream {
    let arms: Vec<TokenStream> = data.variants.iter().map(|variant| {
        let ident = &variant.ident;
        let skip = has_attr(&variant.attrs, "css", "skip");
        if skip {
            return quote! { Self::#ident => Ok(()), };
        }

        // #[css(keyword = "...")]
        if let Some(kw) = get_attr_str(&variant.attrs, "css", "keyword") {
            return quote! { Self::#ident => dest.write_str(#kw), };
        }

        // #[css(function = "...")]
        if let Some(func) = get_attr_str(&variant.attrs, "css", "function") {
            match &variant.fields {
                Fields::Unnamed(fields) => {
                    let bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| quote::format_ident!("f{}", i))
                        .collect();
                    let writes: Vec<TokenStream> = bindings.iter().enumerate().map(|(i, b)| {
                        if i > 0 {
                            quote! { dest.write_str(", ")?; kozan_style::ToCss::to_css(#b, dest)?; }
                        } else {
                            quote! { kozan_style::ToCss::to_css(#b, dest)?; }
                        }
                    }).collect();
                    return quote! {
                        Self::#ident(#(#bindings),*) => {
                            dest.write_str(#func)?;
                            dest.write_char('(')?;
                            #(#writes)*
                            dest.write_char(')')
                        },
                    };
                }
                _ => panic!("#[css(function)] requires unnamed fields"),
            }
        }

        // Variant with fields: delegate to ToCss of inner value(s)
        match &variant.fields {
            Fields::Unit => {
                // Convert PascalCase to kebab-case
                let css_name = pascal_to_kebab(&ident.to_string());
                quote! { Self::#ident => dest.write_str(#css_name), }
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                quote! { Self::#ident(v) => kozan_style::ToCss::to_css(v, dest), }
            }
            Fields::Unnamed(fields) => {
                let bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("f{}", i))
                    .collect();
                let writes: Vec<TokenStream> = bindings.iter().enumerate().map(|(i, b)| {
                    if i > 0 {
                        quote! { dest.write_str(" ")?; kozan_style::ToCss::to_css(#b, dest)?; }
                    } else {
                        quote! { kozan_style::ToCss::to_css(#b, dest)?; }
                    }
                }).collect();
                quote! {
                    Self::#ident(#(#bindings),*) => {
                        #(#writes)*
                        Ok(())
                    },
                }
            }
            Fields::Named(_) => {
                panic!("ToCss derive does not support named fields in enum variants (use #[css(skip)] or a wrapper)");
            }
        }
    }).collect();

    quote! {
        match self {
            #(#arms)*
        }
    }
}

fn derive_struct(fields: &Fields, comma: bool) -> TokenStream {
    let separator = if comma { ", " } else { " " };

    match fields {
        Fields::Named(named) => {
            let writes: Vec<TokenStream> = named.named.iter().enumerate().filter_map(|(i, f)| {
                if has_attr(&f.attrs, "css", "skip") {
                    return None;
                }
                let field_name = f.ident.as_ref().unwrap();
                let write = if i > 0 {
                    quote! {
                        dest.write_str(#separator)?;
                        kozan_style::ToCss::to_css(&self.#field_name, dest)?;
                    }
                } else {
                    quote! { kozan_style::ToCss::to_css(&self.#field_name, dest)?; }
                };
                Some(write)
            }).collect();
            quote! { #(#writes)* Ok(()) }
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            quote! { kozan_style::ToCss::to_css(&self.0, dest) }
        }
        Fields::Unnamed(unnamed) => {
            let writes: Vec<TokenStream> = (0..unnamed.unnamed.len()).enumerate().map(|(i, _)| {
                let idx = syn::Index::from(i);
                if i > 0 {
                    quote! { dest.write_str(#separator)?; kozan_style::ToCss::to_css(&self.#idx, dest)?; }
                } else {
                    quote! { kozan_style::ToCss::to_css(&self.#idx, dest)?; }
                }
            }).collect();
            quote! { #(#writes)* Ok(()) }
        }
        Fields::Unit => quote! { Ok(()) },
    }
}

/// Convert PascalCase to kebab-case: "MinContent" → "min-content"
fn pascal_to_kebab(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('-');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}
