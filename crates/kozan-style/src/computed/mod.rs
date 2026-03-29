//! Computed CSS values — fully resolved except percentages.
//!
//! All relative units (em, rem, vw, vh, ch, etc.) are resolved to px.
//! Percentages survive until layout resolves them against the containing block.

mod length;
mod length_percentage;
mod percentage;

pub use length::Length;
pub use length_percentage::{CalcLeaf, LengthPercentage};
pub use percentage::Percentage;
