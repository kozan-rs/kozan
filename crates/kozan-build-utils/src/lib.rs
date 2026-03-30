//! Shared build-script utilities for Kozan codegen.
//!
//! Contains the TOML schema parser and code writer used by
//! both `kozan-style` and `kozan-css` build scripts.

mod schema;
mod writer;
pub mod match_algo;

pub use schema::*;
pub use writer::CodeWriter;

/// Convert a CSS or Rust name to PascalCase.
///
/// Splits on `-` (CSS) and `_` (Rust), capitalizes each segment.
/// `"border-top-width"` → `"BorderTopWidth"`, `"font_size"` → `"FontSize"`.
pub fn to_pascal(name: &str) -> String {
    name.split(|c| c == '-' || c == '_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
