use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericParam, Ident};

use crate::common::{add_trait_bounds, has_attr};

/// Derives `ToComputedValue` by recursing on each field.
///
/// For each field of type `F`, calls `F::to_computed_value(field, ctx)`.
/// The `ComputedValue` associated type mirrors the struct/enum but with each
/// generic `T` replaced by `<T as ToComputedValue>::ComputedValue`.
///
/// If a field is `#[computed(no_field_bound)]`, it is cloned as-is.
pub fn derive(input: DeriveInput) -> TokenStream {
    let name = &input.ident;

    // If there are no generic type parameters, the type IS its own computed value.
    // Just generate a trivial identity impl.
    let has_type_params = input.generics.type_params().next().is_some();

    if !has_type_params {
        return derive_identity(name, &input);
    }

    let generics = add_trait_bounds(&input.generics, &quote!(kozan_style::ToComputedValue));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Build the computed output type: replace each T with <T as ToComputedValue>::ComputedValue
    let computed_ty_args: Vec<TokenStream> = input.generics.params.iter().map(|p| {
        match p {
            GenericParam::Type(t) => {
                let ident = &t.ident;
                quote! { <#ident as kozan_style::ToComputedValue>::ComputedValue }
            }
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote! { #lt }
            }
            GenericParam::Const(c) => {
                let ident = &c.ident;
                quote! { #ident }
            }
        }
    }).collect();

    let (to_body, from_body) = match &input.data {
        Data::Enum(data) => derive_enum_bodies(data),
        Data::Struct(data) => derive_struct_bodies(&data.fields),
        Data::Union(_) => panic!("ToComputedValue cannot be derived for unions"),
    };

    quote! {
        impl #impl_generics kozan_style::ToComputedValue for #name #ty_generics #where_clause {
            type ComputedValue = #name<#(#computed_ty_args),*>;

            fn to_computed_value(&self, ctx: &kozan_style::ComputeContext) -> Self::ComputedValue {
                #to_body
            }

            fn from_computed_value(computed: &Self::ComputedValue) -> Self {
                #from_body
            }
        }
    }
}

/// For types with no generics: ToComputedValue is identity (clone).
fn derive_identity(name: &Ident, input: &DeriveInput) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics kozan_style::ToComputedValue for #name #ty_generics #where_clause {
            type ComputedValue = Self;

            #[inline]
            fn to_computed_value(&self, _ctx: &kozan_style::ComputeContext) -> Self {
                self.clone()
            }

            #[inline]
            fn from_computed_value(computed: &Self) -> Self {
                computed.clone()
            }
        }
    }
}

fn derive_enum_bodies(data: &syn::DataEnum) -> (TokenStream, TokenStream) {
    let to_arms: Vec<TokenStream> = data.variants.iter().map(|v| {
        let ident = &v.ident;
        match &v.fields {
            Fields::Unit => quote! {
                Self::#ident => Self::ComputedValue::#ident,
            },
            Fields::Unnamed(fields) => {
                let bindings: Vec<Ident> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("__f{}", i))
                    .collect();
                let no_bounds: Vec<bool> = fields.unnamed.iter()
                    .map(|f| has_attr(&f.attrs, "computed", "no_field_bound"))
                    .collect();
                let converts: Vec<TokenStream> = bindings.iter().zip(no_bounds.iter()).map(|(b, &no)| {
                    if no {
                        quote! { #b.clone() }
                    } else {
                        quote! { kozan_style::ToComputedValue::to_computed_value(#b, ctx) }
                    }
                }).collect();
                quote! {
                    Self::#ident(#(#bindings),*) => Self::ComputedValue::#ident(#(#converts),*),
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<&Ident> = fields.named.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let no_bounds: Vec<bool> = fields.named.iter()
                    .map(|f| has_attr(&f.attrs, "computed", "no_field_bound"))
                    .collect();
                let converts: Vec<TokenStream> = field_names.iter().zip(no_bounds.iter()).map(|(name, &no)| {
                    if no {
                        quote! { #name: #name.clone() }
                    } else {
                        quote! { #name: kozan_style::ToComputedValue::to_computed_value(#name, ctx) }
                    }
                }).collect();
                quote! {
                    Self::#ident { #(#field_names),* } => Self::ComputedValue::#ident { #(#converts),* },
                }
            }
        }
    }).collect();

    let from_arms: Vec<TokenStream> = data.variants.iter().map(|v| {
        let ident = &v.ident;
        match &v.fields {
            Fields::Unit => quote! {
                Self::ComputedValue::#ident => Self::#ident,
            },
            Fields::Unnamed(fields) => {
                let bindings: Vec<Ident> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("__f{}", i))
                    .collect();
                let no_bounds: Vec<bool> = fields.unnamed.iter()
                    .map(|f| has_attr(&f.attrs, "computed", "no_field_bound"))
                    .collect();
                let converts: Vec<TokenStream> = bindings.iter().zip(no_bounds.iter()).map(|(b, &no)| {
                    if no {
                        quote! { #b.clone() }
                    } else {
                        quote! { kozan_style::ToComputedValue::from_computed_value(#b) }
                    }
                }).collect();
                quote! {
                    Self::ComputedValue::#ident(#(#bindings),*) => Self::#ident(#(#converts),*),
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<&Ident> = fields.named.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let no_bounds: Vec<bool> = fields.named.iter()
                    .map(|f| has_attr(&f.attrs, "computed", "no_field_bound"))
                    .collect();
                let converts: Vec<TokenStream> = field_names.iter().zip(no_bounds.iter()).map(|(name, &no)| {
                    if no {
                        quote! { #name: #name.clone() }
                    } else {
                        quote! { #name: kozan_style::ToComputedValue::from_computed_value(#name) }
                    }
                }).collect();
                quote! {
                    Self::ComputedValue::#ident { #(#field_names),* } => Self::#ident { #(#converts),* },
                }
            }
        }
    }).collect();

    let to_body = quote! { match self { #(#to_arms)* } };
    let from_body = quote! { match computed { #(#from_arms)* } };

    (to_body, from_body)
}

fn derive_struct_bodies(fields: &Fields) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Named(named) => {
            let to_fields: Vec<TokenStream> = named.named.iter().map(|f| {
                let name = f.ident.as_ref().unwrap();
                if has_attr(&f.attrs, "computed", "no_field_bound") {
                    quote! { #name: self.#name.clone() }
                } else {
                    quote! { #name: kozan_style::ToComputedValue::to_computed_value(&self.#name, ctx) }
                }
            }).collect();
            let from_fields: Vec<TokenStream> = named.named.iter().map(|f| {
                let name = f.ident.as_ref().unwrap();
                if has_attr(&f.attrs, "computed", "no_field_bound") {
                    quote! { #name: computed.#name.clone() }
                } else {
                    quote! { #name: kozan_style::ToComputedValue::from_computed_value(&computed.#name) }
                }
            }).collect();

            (
                quote! { Self::ComputedValue { #(#to_fields),* } },
                quote! { Self { #(#from_fields),* } },
            )
        }
        Fields::Unnamed(unnamed) => {
            let to_fields: Vec<TokenStream> = (0..unnamed.unnamed.len()).map(|i| {
                let idx = syn::Index::from(i);
                let f = &unnamed.unnamed[i];
                if has_attr(&f.attrs, "computed", "no_field_bound") {
                    quote! { self.#idx.clone() }
                } else {
                    quote! { kozan_style::ToComputedValue::to_computed_value(&self.#idx, ctx) }
                }
            }).collect();
            let from_fields: Vec<TokenStream> = (0..unnamed.unnamed.len()).map(|i| {
                let idx = syn::Index::from(i);
                let f = &unnamed.unnamed[i];
                if has_attr(&f.attrs, "computed", "no_field_bound") {
                    quote! { computed.#idx.clone() }
                } else {
                    quote! { kozan_style::ToComputedValue::from_computed_value(&computed.#idx) }
                }
            }).collect();

            (
                quote! { Self::ComputedValue(#(#to_fields),*) },
                quote! { Self(#(#from_fields),*) },
            )
        }
        Fields::Unit => (quote! { Self::ComputedValue }, quote! { Self }),
    }
}
