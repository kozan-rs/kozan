//! CSS text emphasis and background position types.

use crate::{Atom, TextEmphasisFill, TextEmphasisShape};
use crate::specified::LengthPercentage;
use crate::computed::Percentage;
use kozan_style_macros::ToComputedValue;

/// CSS `text-emphasis-style` — `none`, shape with fill, or custom string.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TextEmphasisStyleValue {
    None,
    Shape(TextEmphasisFill, TextEmphasisShape),
    Custom(Atom),
}

impl Default for TextEmphasisStyleValue {
    fn default() -> Self {
        Self::None
    }
}

/// CSS `background-position-x/y` component — keyword or length.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum PositionComponent {
    Left,
    Center,
    Right,
    Top,
    Bottom,
    Length(LengthPercentage),
}

impl Default for PositionComponent {
    fn default() -> Self {
        Self::Length(LengthPercentage::Percentage(Percentage::new(0.0)))
    }
}
