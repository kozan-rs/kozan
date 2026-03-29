//! `css_match!` proc macro — readable syntax, optimal integer-comparison codegen.
//!
//! # What you write:
//! ```ignore
//! css_match! { ident,
//!     "srgb" => ColorSpace::Srgb,
//!     "xyz-d50" | "xyz" => ColorSpace::XyzD50,
//!     _ => return Err(...)
//! }
//! ```
//!
//! # What the compiler sees (4-byte keyword "srgb"):
//! ```ignore
//! match ident.len() {
//!     4 => {
//!         let __b = ident.as_bytes();
//!         if u32::from_ne_bytes([__b[0], __b[1], __b[2], __b[3]])
//!             | 0x20202020 == u32::from_ne_bytes([b's', b'r', b'g', b'b'])
//!         { ColorSpace::Srgb }
//!         else { return Err(...) }
//!     },
//!     ...
//! }
//! ```
//!
//! # Algorithm: Length-First Explicit Integer Comparison
//!
//! 1. `ident.len()` — free (stored in fat pointer)
//! 2. `ident.as_bytes()` — pointer cast (zero copy)
//! 3. Load bytes as u16/u32/u64 chunks, OR with `0x2020`/`0x20202020`/etc,
//!    compare against pre-computed lowercase constant — 3 CPU instructions per chunk.
//!
//! CSS identifiers contain only letters, digits, and hyphens. For all of these,
//! `| 0x20` is safe: letters become lowercase, digits and hyphens are unchanged
//! (bit 5 is already set for 0x2D `-`, 0x30-0x39 `0-9`).
//!
//! Chunk strategy: decompose keyword length into fewest integer comparisons:
//! - 1 byte: single u8 check
//! - 2 bytes: 1 u16 check
//! - 3 bytes: u16 + u8
//! - 4 bytes: 1 u32 check
//! - 5-7 bytes: u32 + remainder
//! - 8 bytes: 1 u64 check
//! - 9+ bytes: u64 chunks + remainder
//!
//! Result: 25-byte keyword = 4 comparisons (3×u64 + 1×u8) vs 25 byte-by-byte checks.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, Token};

/// Parsed input for `css_match!`.
pub struct CssMatchInput {
    /// The expression to match (e.g. `ident`, `&name`).
    pub expr: Expr,
    /// Keyword arms: each has one or more CSS strings and a body expression.
    pub arms: Vec<CssMatchArm>,
    /// The wildcard/default arm body.
    pub default: Expr,
}

pub struct CssMatchArm {
    pub keywords: Vec<String>,
    pub body: Expr,
}

impl Parse for CssMatchInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse the match expression.
        let expr: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut arms = Vec::new();
        let mut default = None;

        while !input.is_empty() {
            // Check for wildcard `_`.
            if input.peek(Token![_]) {
                input.parse::<Token![_]>()?;
                input.parse::<Token![=>]>()?;
                default = Some(input.parse::<Expr>()?);
                // Consume optional trailing comma.
                let _ = input.parse::<Token![,]>();
                break;
            }

            // Parse one or more string literals separated by `|`.
            let mut keywords = Vec::new();
            let lit: LitStr = input.parse()?;
            keywords.push(lit.value());
            while input.peek(Token![|]) {
                input.parse::<Token![|]>()?;
                let lit: LitStr = input.parse()?;
                keywords.push(lit.value());
            }

            input.parse::<Token![=>]>()?;
            let body: Expr = input.parse()?;

            // Consume optional trailing comma.
            let _ = input.parse::<Token![,]>();

            arms.push(CssMatchArm { keywords, body });
        }

        let default = default.ok_or_else(|| {
            syn::Error::new(input.span(), "css_match! requires a `_ => ...` default arm")
        })?;

        Ok(CssMatchInput { expr, arms, default })
    }
}

/// Generates the length-first if-chain match.
pub fn expand(input: CssMatchInput) -> TokenStream {
    let expr = &input.expr;
    let default = &input.default;

    // Collect all (keyword_string, body_expr) pairs, flattening `|` arms.
    let mut entries: Vec<(&str, &Expr)> = Vec::new();
    for arm in &input.arms {
        for kw in &arm.keywords {
            entries.push((kw.as_str(), &arm.body));
        }
    }

    // Group by keyword length.
    let mut by_len: std::collections::BTreeMap<usize, Vec<(&str, &Expr)>> = std::collections::BTreeMap::new();
    for &(kw, body) in &entries {
        by_len.entry(kw.len()).or_default().push((kw, body));
    }

    // If only one length group, skip outer match.
    if by_len.len() == 1 {
        let (_, group) = by_len.iter().next().unwrap();
        return gen_if_chain(expr, group, default);
    }

    // Generate match on length.
    let len_arms: Vec<TokenStream> = by_len.iter().map(|(len, group)| {
        let if_chain = gen_if_chain(expr, group, default);
        quote! { #len => { #if_chain } }
    }).collect();

    quote! {
        match (#expr).len() {
            #(#len_arms,)*
            _ => #default,
        }
    }
}

/// Generates `if b[0]|0x20==b'x' && ... { body } else if ... { } else { default }`.
fn gen_if_chain(expr: &Expr, group: &[(&str, &Expr)], default: &Expr) -> TokenStream {
    let mut result = quote! { #default };

    // Build the chain in reverse so the last else is the default.
    for &(kw, body) in group.iter().rev() {
        let condition = gen_byte_condition(kw);
        result = quote! {
            if #condition { #body } else { #result }
        };
    }

    // Wrap with `let __b = expr.as_bytes();`
    quote! {
        {
            let __b = (#expr).as_bytes();
            #result
        }
    }
}

/// Decomposes a keyword length into (offset, chunk_size) pairs using the
/// largest possible integer types: u64 → u32 → u16 → u8.
fn int_chunks(len: usize) -> Vec<(usize, usize)> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < len {
        let remaining = len - offset;
        let chunk = if remaining >= 8 { 8 }
                    else if remaining >= 4 { 4 }
                    else if remaining >= 2 { 2 }
                    else { 1 };
        chunks.push((offset, chunk));
        offset += chunk;
    }
    chunks
}

/// Generates an explicit integer comparison condition for a CSS keyword.
///
/// For each chunk of bytes, generates:
/// ```ignore
/// u32::from_ne_bytes([__b[0], __b[1], __b[2], __b[3]]) | 0x20202020u32
///     == u32::from_ne_bytes([b'd', b'i', b's', b'c'])
/// ```
///
/// CSS identifiers contain only [a-zA-Z0-9\-]. For all of these, `| 0x20`
/// is safe: letters → lowercase, digits/hyphens unchanged (bit 5 already set).
/// This lets us blanket-OR entire integer chunks without per-byte branching.
///
/// LLVM compiles `u32::from_ne_bytes([__b[0], ..., __b[3]])` to a single
/// unaligned load instruction. Combined with OR + CMP, each chunk is 3 instructions.
fn gen_byte_condition(kw: &str) -> TokenStream {
    let bytes = kw.as_bytes();
    let chunks = int_chunks(bytes.len());

    let checks: Vec<TokenStream> = chunks.iter().map(|&(offset, size)| {
        // Emit target bytes as from_ne_bytes CODE, not pre-computed host integers.
        // The compiler const-folds this for the correct target endianness.
        let lower_bytes: Vec<u8> = (0..size)
            .map(|i| bytes[offset + i].to_ascii_lowercase())
            .collect();

        match size {
            1 => {
                let idx = syn::Index::from(offset);
                let b0 = lower_bytes[0];
                quote! { __b[#idx] | 0x20 == #b0 }
            }
            2 => {
                let i0 = syn::Index::from(offset);
                let i1 = syn::Index::from(offset + 1);
                let (b0, b1) = (lower_bytes[0], lower_bytes[1]);
                quote! {
                    u16::from_ne_bytes([__b[#i0], __b[#i1]])
                        | 0x2020u16 == u16::from_ne_bytes([#b0, #b1])
                }
            }
            4 => {
                let i0 = syn::Index::from(offset);
                let i1 = syn::Index::from(offset + 1);
                let i2 = syn::Index::from(offset + 2);
                let i3 = syn::Index::from(offset + 3);
                let (b0, b1, b2, b3) = (lower_bytes[0], lower_bytes[1], lower_bytes[2], lower_bytes[3]);
                quote! {
                    u32::from_ne_bytes([__b[#i0], __b[#i1], __b[#i2], __b[#i3]])
                        | 0x20202020u32 == u32::from_ne_bytes([#b0, #b1, #b2, #b3])
                }
            }
            8 => {
                let i0 = syn::Index::from(offset);
                let i1 = syn::Index::from(offset + 1);
                let i2 = syn::Index::from(offset + 2);
                let i3 = syn::Index::from(offset + 3);
                let i4 = syn::Index::from(offset + 4);
                let i5 = syn::Index::from(offset + 5);
                let i6 = syn::Index::from(offset + 6);
                let i7 = syn::Index::from(offset + 7);
                let (b0, b1, b2, b3) = (lower_bytes[0], lower_bytes[1], lower_bytes[2], lower_bytes[3]);
                let (b4, b5, b6, b7) = (lower_bytes[4], lower_bytes[5], lower_bytes[6], lower_bytes[7]);
                quote! {
                    u64::from_ne_bytes([__b[#i0], __b[#i1], __b[#i2], __b[#i3],
                                        __b[#i4], __b[#i5], __b[#i6], __b[#i7]])
                        | 0x2020202020202020u64
                        == u64::from_ne_bytes([#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7])
                }
            }
            _ => unreachable!(),
        }
    }).collect();

    if checks.len() == 1 {
        checks.into_iter().next().unwrap()
    } else {
        quote! { #(#checks)&&* }
    }
}
