//! Specified CSS values — what the user writes.
//!
//! These preserve the original CSS units (em, rem, vw, vh, ch, etc.)
//! and are converted to computed values via `ToComputedValue`.

pub mod length;
mod length_percentage;

pub use length::{
    AbsoluteLength, ContainerRelativeLength, FontRelativeLength, Length,
    ViewportPercentageLength,
};
pub use length_percentage::{LengthPercentage, SpecifiedLeaf};
