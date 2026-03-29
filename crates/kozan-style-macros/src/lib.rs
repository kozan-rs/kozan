mod animate;
mod common;
mod css_match;
mod to_computed;
mod to_css;

/// Derive `ToCss` — auto-generates CSS serialization.
///
/// - Unit variants → kebab-case keywords
/// - Tuple variants → delegate to inner `ToCss`
/// - `#[css(keyword = "flex-start")]` — override keyword
/// - `#[css(function = "fit-content")]` — wrap in function notation
/// - `#[css(skip)]` — skip variant/field
/// - `#[css(comma)]` — comma-separate fields
#[proc_macro_derive(ToCss, attributes(css))]
pub fn derive_to_css(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    to_css::derive(input).into()
}

/// Derive `ToComputedValue` — specified → computed conversion.
///
/// - Generic types: recurses on fields, replaces `T` with `<T as ToComputedValue>::ComputedValue`
/// - Non-generic types: identity impl (clone)
/// - `#[computed(no_field_bound)]` — clone field as-is
#[proc_macro_derive(ToComputedValue, attributes(computed))]
pub fn derive_to_computed_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    to_computed::derive(input).into()
}

/// Derive `Animate` — interpolate between two computed values.
///
/// - Tuple variants: delegate to field's `Animate` impl
/// - Unit variants: discrete (match only if same)
/// - `#[animation(error)]` — always `Err(())` for that variant
/// - Mismatched variants: discrete swap at 50%
#[proc_macro_derive(Animate, attributes(animation))]
pub fn derive_animate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    animate::derive_animate(input).into()
}

/// Derive `ToAnimatedZero` — produce animation zero value.
///
/// - Tuple variants: delegate to field
/// - Unit / `#[animation(error)]`: `Err(())`
#[proc_macro_derive(ToAnimatedZero, attributes(animation))]
pub fn derive_to_animated_zero(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    animate::derive_to_animated_zero(input).into()
}

/// Derive `ComputeSquaredDistance` — paced animation distance.
///
/// - Tuple variants: delegate to field
/// - Same unit variant: 0.0
/// - Different variants: 1.0
#[proc_macro_derive(ComputeSquaredDistance, attributes(animation))]
pub fn derive_compute_squared_distance(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    animate::derive_compute_squared_distance(input).into()
}

/// Case-insensitive CSS keyword match with optimal byte-pattern codegen.
///
/// Readable syntax, maximum performance. Faster than cssparser's
/// `match_ignore_ascii_case!` — no stack copy, no lowercase loop,
/// just integer comparisons via length-first byte pattern dispatch.
///
/// ```ignore
/// css_match! { ident,
///     "srgb" => ColorSpace::Srgb,
///     "xyz-d50" | "xyz" => ColorSpace::XyzD50,
///     _ => return Err(...)
/// }
/// ```
#[proc_macro]
pub fn css_match(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as css_match::CssMatchInput);
    css_match::expand(input).into()
}
