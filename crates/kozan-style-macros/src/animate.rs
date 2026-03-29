use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident};

use crate::common::{add_trait_bounds, add_trait_bounds_multi, has_attr};

/// Derive `Animate`: interpolate between two values.
///
/// - Tuple variants with 1 field: delegate to field's Animate impl
/// - Unit variants: match only if both are same variant (discrete)
/// - `#[animation(error)]`: always returns Err(()) for that variant
/// - Mixed variants: discrete interpolation (swap at 50%)
pub fn derive_animate(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = add_trait_bounds_multi(&input.generics, &[
        quote!(kozan_style::Animate), quote!(Clone), quote!(PartialEq),
    ]);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Enum(data) => animate_enum(data),
        Data::Struct(data) => animate_struct(&data.fields),
        Data::Union(_) => panic!("Animate cannot be derived for unions"),
    };

    quote! {
        impl #impl_generics kozan_style::Animate for #name #ty_generics #where_clause {
            fn animate(&self, other: &Self, procedure: kozan_style::Procedure) -> Result<Self, ()> {
                #body
            }
        }
    }
}

/// Derive `ToAnimatedZero`.
///
/// - Tuple variants with 1 field: delegate to field
/// - Unit variants / `#[animation(error)]`: return Err(())
pub fn derive_to_animated_zero(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = add_trait_bounds(&input.generics, &quote!(kozan_style::ToAnimatedZero));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Enum(data) => animated_zero_enum(data),
        Data::Struct(data) => animated_zero_struct(&data.fields),
        Data::Union(_) => panic!("ToAnimatedZero cannot be derived for unions"),
    };

    quote! {
        impl #impl_generics kozan_style::ToAnimatedZero for #name #ty_generics #where_clause {
            fn to_animated_zero(&self) -> Result<Self, ()> {
                #body
            }
        }
    }
}

/// Derive `ComputeSquaredDistance`.
pub fn derive_compute_squared_distance(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = add_trait_bounds_multi(&input.generics, &[
        quote!(kozan_style::ComputeSquaredDistance), quote!(PartialEq),
    ]);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Enum(data) => squared_distance_enum(data),
        Data::Struct(data) => squared_distance_struct(&data.fields),
        Data::Union(_) => panic!("ComputeSquaredDistance cannot be derived for unions"),
    };

    quote! {
        impl #impl_generics kozan_style::ComputeSquaredDistance for #name #ty_generics #where_clause {
            fn compute_squared_distance(&self, other: &Self) -> Result<f64, ()> {
                #body
            }
        }
    }
}

// ── Animate enum ──

fn animate_enum(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<TokenStream> = data.variants.iter().map(|v| {
        let ident = &v.ident;
        let is_error = has_attr(&v.attrs, "animation", "error");

        if is_error {
            match &v.fields {
                Fields::Unit => quote! { (Self::#ident, Self::#ident) => Ok(Self::#ident), },
                _ => quote! { (Self::#ident(..), Self::#ident(..)) => Err(()), },
            }
        } else {
            match &v.fields {
                Fields::Unit => quote! { (Self::#ident, Self::#ident) => Ok(Self::#ident), },
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    quote! {
                        (Self::#ident(a), Self::#ident(b)) => {
                            Ok(Self::#ident(kozan_style::Animate::animate(a, b, procedure)?))
                        },
                    }
                }
                Fields::Unnamed(fields) => {
                    let a_binds: Vec<Ident> = (0..fields.unnamed.len()).map(|i| quote::format_ident!("a{}", i)).collect();
                    let b_binds: Vec<Ident> = (0..fields.unnamed.len()).map(|i| quote::format_ident!("b{}", i)).collect();
                    let animates: Vec<TokenStream> = a_binds.iter().zip(b_binds.iter()).map(|(a, b)| {
                        quote! { kozan_style::Animate::animate(#a, #b, procedure)? }
                    }).collect();
                    quote! {
                        (Self::#ident(#(#a_binds),*), Self::#ident(#(#b_binds),*)) => {
                            Ok(Self::#ident(#(#animates),*))
                        },
                    }
                }
                Fields::Named(_) => panic!("Animate derive doesn't support named fields in enum variants"),
            }
        }
    }).collect();

    quote! {
        match (self, other) {
            #(#arms)*
            _ => match procedure {
                kozan_style::Procedure::Interpolate { progress } => {
                    Ok(if progress < 0.5 { self.clone() } else { other.clone() })
                }
                _ => Err(()),
            },
        }
    }
}

fn animate_struct(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            quote! { Ok(Self(kozan_style::Animate::animate(&self.0, &other.0, procedure)?)) }
        }
        Fields::Named(named) => {
            let field_anims: Vec<TokenStream> = named.named.iter().map(|f| {
                let name = f.ident.as_ref().unwrap();
                quote! { #name: kozan_style::Animate::animate(&self.#name, &other.#name, procedure)? }
            }).collect();
            quote! { Ok(Self { #(#field_anims),* }) }
        }
        _ => quote! { Err(()) },
    }
}

// ── ToAnimatedZero enum ──

fn animated_zero_enum(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<TokenStream> = data.variants.iter().map(|v| {
        let ident = &v.ident;
        let is_error = has_attr(&v.attrs, "animation", "error");

        if is_error {
            return quote! { Self::#ident { .. } => Err(()), };
        }

        match &v.fields {
            Fields::Unit => quote! { Self::#ident => Err(()), },
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                quote! {
                    Self::#ident(v) => Ok(Self::#ident(kozan_style::ToAnimatedZero::to_animated_zero(v)?)),
                }
            }
            _ => quote! { Self::#ident { .. } => Err(()), },
        }
    }).collect();

    quote! { match self { #(#arms)* } }
}

fn animated_zero_struct(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            quote! { Ok(Self(kozan_style::ToAnimatedZero::to_animated_zero(&self.0)?)) }
        }
        _ => quote! { Err(()) },
    }
}

// ── ComputeSquaredDistance enum ──

fn squared_distance_enum(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<TokenStream> = data.variants.iter().map(|v| {
        let ident = &v.ident;
        match &v.fields {
            Fields::Unit => quote! { (Self::#ident, Self::#ident) => Ok(0.0), },
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                quote! {
                    (Self::#ident(a), Self::#ident(b)) => {
                        kozan_style::ComputeSquaredDistance::compute_squared_distance(a, b)
                    },
                }
            }
            _ => quote! { (Self::#ident { .. }, Self::#ident { .. }) => Ok(0.0), },
        }
    }).collect();

    quote! {
        match (self, other) {
            #(#arms)*
            _ => Ok(1.0),
        }
    }
}

fn squared_distance_struct(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            quote! { kozan_style::ComputeSquaredDistance::compute_squared_distance(&self.0, &other.0) }
        }
        _ => quote! { Ok(0.0) },
    }
}
