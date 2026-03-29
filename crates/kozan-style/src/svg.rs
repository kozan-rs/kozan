//! CSS/SVG paint and stroke values.

use crate::{Atom, Color};
use crate::specified::LengthPercentage;
use kozan_style_macros::ToComputedValue;

/// SVG `fill` / `stroke` paint value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum SvgPaint {
    None,
    Color(Color),
    Url(Atom, Option<Color>),
    ContextFill,
    ContextStroke,
}

impl SvgPaint {
    /// Opaque black paint.
    pub const BLACK: SvgPaint = SvgPaint::Color(Color::BLACK);
}

impl Default for SvgPaint {
    fn default() -> Self { Self::None }
}

/// SVG `stroke-dasharray`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum StrokeDasharray {
    None,
    Values(Box<[LengthPercentage]>),
}

impl Default for StrokeDasharray {
    fn default() -> Self { Self::None }
}

/// SVG `paint-order`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum PaintOrder {
    Normal,
    Custom(Box<[PaintTarget]>),
}

impl Default for PaintOrder {
    fn default() -> Self { Self::Normal }
}

/// SVG paint-order target component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum PaintTarget { Fill, Stroke, Markers }
