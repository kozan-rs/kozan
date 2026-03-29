use proc_macro2::TokenStream;

/// Generate a where clause adding `T: Trait` for each type parameter.
pub fn add_trait_bounds(generics: &syn::Generics, trait_path: &TokenStream) -> syn::Generics {
    let mut generics = generics.clone();
    for param in &mut generics.params {
        if let syn::GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(syn::parse_quote!(#trait_path));
        }
    }
    generics
}

/// Add multiple trait bounds to all type parameters.
pub fn add_trait_bounds_multi(generics: &syn::Generics, traits: &[TokenStream]) -> syn::Generics {
    let mut generics = generics.clone();
    for param in &mut generics.params {
        if let syn::GenericParam::Type(ref mut type_param) = *param {
            for tr in traits {
                type_param.bounds.push(syn::parse_quote!(#tr));
            }
        }
    }
    generics
}

/// Check if a field/variant has a specific attribute like `#[css(skip)]`.
pub fn has_attr(attrs: &[syn::Attribute], outer: &str, inner: &str) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident(outer) {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(inner) {
                found = true;
            }
            Ok(())
        });
        found
    })
}

/// Get a string value from an attribute like `#[css(keyword = "flex-start")]`.
pub fn get_attr_str(attrs: &[syn::Attribute], outer: &str, key: &str) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident(outer) {
            continue;
        }
        let mut value = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(key) {
                let v: syn::LitStr = meta.value()?.parse()?;
                value = Some(v.value());
            }
            Ok(())
        });
        if value.is_some() {
            return value;
        }
    }
    None
}
