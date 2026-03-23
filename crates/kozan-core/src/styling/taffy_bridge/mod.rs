// Based on stylo_taffy (https://github.com/nicoburniske/blitz)
// Licensed under MIT / Apache-2.0 / MPL-2.0
// Modified for Kozan — adapted to stylo 0.14 + taffy 0.9

//! Stylo ↔ Taffy bridge — converts `ComputedValues` to Taffy layout styles.
//!
//! Two approaches available:
//! - `to_taffy_style()` — eager conversion, produces `taffy::Style`
//! - `TaffyStyloStyle<T>` — zero-copy wrapper implementing Taffy traits directly

pub mod convert;
mod wrapper;

pub use wrapper::TaffyStyloStyle;
pub use convert::to_taffy_style;
