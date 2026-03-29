//! CSS filter, clipping, and shape values.

use crate::{Atom, Color};
use kozan_style_macros::ToComputedValue;

/// CSS `filter` / `backdrop-filter`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FilterList {
    None,
    Filters(Box<[FilterFunction]>),
}

impl Default for FilterList {
    fn default() -> Self { Self::None }
}

/// Individual CSS filter functions.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum FilterFunction {
    Blur(f32),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    HueRotate(f32),
    Invert(f32),
    Opacity(f32),
    Saturate(f32),
    Sepia(f32),
    DropShadow { x: f32, y: f32, blur: f32, color: Color },
    Url(Atom),
}

/// CSS `clip-path`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ClipPath {
    None,
    Url(Atom),
    Inset(Box<InsetRect>),
    Circle(Box<CircleShape>),
    Ellipse(Box<EllipseShape>),
    Polygon(Box<[(f32, f32)]>),
}

impl Default for ClipPath {
    fn default() -> Self { Self::None }
}

/// CSS `inset()` basic shape for clipping.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct InsetRect {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    pub round: [f32; 4],
}

/// CSS `circle()` basic shape.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct CircleShape { pub radius: f32, pub cx: f32, pub cy: f32 }

/// CSS `ellipse()` basic shape.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct EllipseShape { pub rx: f32, pub ry: f32, pub cx: f32, pub cy: f32 }

/// CSS `shape-outside`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ShapeOutside {
    None,
    Url(Atom),
    Inset(Box<InsetRect>),
    Circle(Box<CircleShape>),
    Ellipse(Box<EllipseShape>),
    Polygon(Box<[(f32, f32)]>),
}

impl Default for ShapeOutside {
    fn default() -> Self { Self::None }
}
