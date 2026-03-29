//! CSS image and gradient values.

use crate::{Atom, Color};
use kozan_style_macros::ToComputedValue;

/// A single CSS image value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum Image {
    None,
    Url(Atom),
    LinearGradient(Box<LinearGradient>),
    RadialGradient(Box<RadialGradient>),
    ConicGradient(Box<ConicGradient>),
}

impl Default for Image {
    fn default() -> Self { Self::None }
}

/// CSS `background-image` / `mask-image`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ImageList {
    None,
    Images(Box<[Image]>),
}

impl Default for ImageList {
    fn default() -> Self { Self::None }
}

/// CSS `linear-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct LinearGradient {
    pub angle: f32,
    pub stops: Box<[ColorStop]>,
}

/// CSS `radial-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct RadialGradient {
    pub shape: RadialShape,
    pub stops: Box<[ColorStop]>,
}

/// CSS `conic-gradient()`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ConicGradient {
    pub from_angle: f32,
    pub stops: Box<[ColorStop]>,
}

/// CSS radial gradient shape keyword.
#[derive(Clone, Copy, Debug, PartialEq, ToComputedValue)]
pub enum RadialShape { Circle, Ellipse }

/// A color stop in a CSS gradient.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ColorStop {
    pub color: Color,
    pub position: Option<f32>,
}
